use crate::config::{save_snapshot, ConfigState, EnvVar, ServerEntry};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStatus {
    pub id: String,
    pub state: String, // "running", "stopped", "crashed"
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerResources {
    pub pid: u32,
    pub uptime_secs: u64,
    pub memory_kb: u64,
    pub cpu_percent: f64,
}

pub struct RunningServer {
    pub(crate) child: Child,
    started_at: Instant,
    restart_count: u32,
}

/// How a server last exited: cleanly (stopped by the user or exit code 0)
/// or crashed (non-zero exit / signal / wait error).
pub struct LastExit {
    pub crashed: bool,
}

type LogBuffer = Arc<Mutex<Vec<String>>>;

pub struct ServerManager {
    pub running: Mutex<HashMap<String, RunningServer>>,
    /// Per-server log buffers. Kept separate from `running` so logs survive
    /// a stop or crash and can still be viewed/exported afterwards.
    pub logs: Mutex<HashMap<String, LogBuffer>>,
    pub last_exit: Mutex<HashMap<String, LastExit>>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
            last_exit: Mutex::new(HashMap::new()),
        }
    }

    fn log_buffer(&self, id: &str) -> LogBuffer {
        let mut logs = self.logs.lock().unwrap();
        logs.entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
            .clone()
    }
}

const MAX_OUTPUT_LINES: usize = 5000;
/// Hard cap on a single output line — a server that emits a megabyte of JSON
/// in one line shouldn't be able to balloon our memory.
const MAX_LINE_BYTES: usize = 8 * 1024;
/// Coalesce stdout/stderr emits to the UI to at most this often per server.
const EMIT_INTERVAL_MS: u64 = 100;
/// How long to wait for a SIGTERM'd process group before escalating to SIGKILL.
const TERM_GRACE_MS: u64 = 2000;
/// A run at least this long counts as stable: the auto-restart retry counter
/// resets, so a server that crashed a few times weeks ago isn't permanently
/// denied restarts.
const STABLE_RUN_SECS: u64 = 60;

fn timestamp() -> String {
    let now = chrono::Local::now();
    now.format("%H:%M:%S").to_string()
}

fn truncate_line(mut line: String) -> String {
    if line.len() > MAX_LINE_BYTES {
        // Walk back to a UTF-8 char boundary so we don't slice mid-codepoint.
        let mut cut = MAX_LINE_BYTES;
        while cut > 0 && !line.is_char_boundary(cut) {
            cut -= 1;
        }
        line.truncate(cut);
        line.push_str(" …[truncated]");
    }
    line
}

/// Strip ANSI escape sequences (CSI color/cursor codes, OSC titles) so
/// colored output from dev servers reads cleanly in the log viewer and
/// exported logs.
fn strip_ansi(s: &str) -> String {
    if !s.contains('\u{1b}') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ params/intermediates … final byte in @–~
            Some('[') => {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            }
            // OSC: ESC ] … terminated by BEL or ST (ESC \)
            Some(']') => {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == '\u{7}' {
                        break;
                    }
                    if n == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            // Two-character escapes (ESC c, ESC =, …)
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// Send SIGTERM to the child's whole process group (raw commands run under
/// `bash -c`, so the actual server is a grandchild — killing just the direct
/// child would orphan it). Escalates to SIGKILL after a grace period.
fn terminate_child(child: &mut Child) -> Result<(), String> {
    #[cfg(unix)]
    {
        let pgid = child.id() as i32;
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        for _ in 0..(TERM_GRACE_MS / 100) {
            match child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(format!("Failed to stop: {}", e)),
            }
        }
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        child.kill().map_err(|e| format!("Failed to stop: {}", e))?;
    }
    let _ = child.wait();
    Ok(())
}

/// Terminate every managed server. Called on quit so we don't orphan the
/// processes we spawned. SIGTERMs all groups first, then reaps with a short
/// grace period and SIGKILLs stragglers.
pub fn shutdown_all_servers(app_handle: &AppHandle) {
    let Some(manager) = app_handle.try_state::<ServerManager>() else {
        return;
    };
    let servers: Vec<(String, RunningServer)> = {
        let mut running = manager.running.lock().unwrap();
        running.drain().collect()
    };
    if servers.is_empty() {
        return;
    }
    #[cfg(unix)]
    for (_, server) in &servers {
        unsafe {
            libc::kill(-(server.child.id() as i32), libc::SIGTERM);
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    for (_, mut server) in servers {
        if let Ok(Some(_)) = server.child.try_wait() {
            continue;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(server.child.id() as i32), libc::SIGKILL);
        }
        let _ = server.child.kill();
        let _ = server.child.wait();
    }
}

/// Spawns a background thread that batches emits for one (server_id, stream).
/// Returns a closure-friendly sender: push lines into the Vec, the thread
/// drains and emits at most every EMIT_INTERVAL_MS. The thread exits on its
/// own once all producers have dropped their handle and the queue is empty.
fn start_emit_batcher(handle: AppHandle, server_id: String) -> Arc<Mutex<Vec<String>>> {
    let pending: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_clone = Arc::clone(&pending);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(EMIT_INTERVAL_MS));
            let batch: Vec<String> = {
                let mut q = pending_clone.lock().unwrap();
                if q.is_empty() {
                    // Sole owner + empty queue means the reader threads are
                    // done: the process exited. Don't leak this thread.
                    if Arc::strong_count(&pending_clone) == 1 {
                        break;
                    }
                    continue;
                }
                std::mem::take(&mut *q)
            };
            let _ = handle.emit("server-output-batch", (&server_id, &batch));
        }
    });
    pending
}

fn spawn_server(
    entry: &ServerEntry,
    app_handle: &AppHandle,
    output: LogBuffer,
) -> Result<RunningServer, String> {
    let mut cmd = if let Some(ref raw) = entry.raw_command {
        let mut c = Command::new("bash");
        c.arg("-c").arg(raw);
        c
    } else if let Some(ref executable) = entry.executable {
        let mut c = Command::new(executable);
        c.args(&entry.arguments);
        c
    } else {
        return Err("No command or executable configured".to_string());
    };

    if let Some(ref dir) = entry.working_directory {
        cmd.current_dir(dir);
    }
    for env in &entry.env_vars {
        cmd.env(&env.key, &env.value);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Own process group so stop/quit can kill the whole tree (bash -c children).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to start: {}", e))?;

    let server_id = entry.id.clone();
    let pending = start_emit_batcher(app_handle.clone(), server_id.clone());

    // Mark the start in the (possibly retained) log so runs are separable.
    let marker = format!("[{}] ── server started ──", timestamp());
    {
        let mut buf = output.lock().unwrap();
        buf.push(marker.clone());
    }
    pending.lock().unwrap().push(marker);

    let consume = |reader: Box<dyn BufRead + Send>, prefix_stderr: bool, output: LogBuffer, pending: Arc<Mutex<Vec<String>>>| {
        std::thread::spawn(move || {
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                let line = strip_ansi(&line);
                let stamped = if prefix_stderr {
                    format!("[{}] [stderr] {}", timestamp(), line)
                } else {
                    format!("[{}] {}", timestamp(), line)
                };
                let stamped = truncate_line(stamped);
                {
                    let mut buf = output.lock().unwrap();
                    buf.push(stamped.clone());
                    if buf.len() > MAX_OUTPUT_LINES {
                        let excess = buf.len() - MAX_OUTPUT_LINES;
                        buf.drain(0..excess);
                    }
                }
                pending.lock().unwrap().push(stamped);
            }
        });
    };

    if let Some(stdout) = child.stdout.take() {
        consume(Box::new(BufReader::new(stdout)), false, Arc::clone(&output), Arc::clone(&pending));
    }
    if let Some(stderr) = child.stderr.take() {
        consume(Box::new(BufReader::new(stderr)), true, Arc::clone(&output), Arc::clone(&pending));
    }

    Ok(RunningServer {
        child,
        started_at: Instant::now(),
        restart_count: 0,
    })
}

// ── Commands ───────────────────────────────────────────

#[tauri::command]
pub fn get_servers(state: State<ConfigState>) -> Vec<ServerEntry> {
    let config = state.0.lock().unwrap();
    config.servers.clone()
}

#[tauri::command]
pub fn add_server(
    state: State<ConfigState>,
    app_handle: AppHandle,
    name: String,
    raw_command: Option<String>,
    executable: Option<String>,
    arguments: Vec<String>,
    working_directory: Option<String>,
    env_vars: Vec<EnvVar>,
    auto_launch: bool,
    auto_restart: bool,
    max_retries: u32,
    restart_cooldown_secs: u64,
) -> Result<ServerEntry, String> {
    let entry = ServerEntry::new(
        name, raw_command, executable, arguments, working_directory,
        env_vars, auto_launch, auto_restart, max_retries, restart_cooldown_secs,
    );
    {
        let mut config = state.0.lock().unwrap();
        config.servers.push(entry.clone());
    }
    save_snapshot(&state)?;
    let _ = app_handle.emit("tray-update", ());
    Ok(entry)
}

#[tauri::command]
pub fn update_server(
    state: State<ConfigState>,
    app_handle: AppHandle,
    id: String,
    name: String,
    raw_command: Option<String>,
    executable: Option<String>,
    arguments: Vec<String>,
    working_directory: Option<String>,
    env_vars: Vec<EnvVar>,
    auto_launch: bool,
    auto_restart: bool,
    max_retries: u32,
    restart_cooldown_secs: u64,
) -> Result<ServerEntry, String> {
    let updated = {
        let mut config = state.0.lock().unwrap();
        let server = config.servers.iter_mut().find(|s| s.id == id).ok_or("Server not found")?;
        server.name = name;
        server.raw_command = raw_command;
        server.executable = executable;
        server.arguments = arguments;
        server.working_directory = working_directory;
        server.env_vars = env_vars;
        server.auto_launch = auto_launch;
        server.auto_restart = auto_restart;
        server.max_retries = max_retries;
        server.restart_cooldown_secs = restart_cooldown_secs;
        server.clone()
    };
    save_snapshot(&state)?;
    let _ = app_handle.emit("tray-update", ());
    Ok(updated)
}

#[tauri::command]
pub fn duplicate_server(
    state: State<ConfigState>,
    app_handle: AppHandle,
    id: String,
) -> Result<ServerEntry, String> {
    let entry = {
        let mut config = state.0.lock().unwrap();
        let idx = config.servers.iter().position(|s| s.id == id).ok_or("Server not found")?;
        let src = &config.servers[idx];
        let copy = ServerEntry::new(
            format!("{} (copy)", src.name),
            src.raw_command.clone(),
            src.executable.clone(),
            src.arguments.clone(),
            src.working_directory.clone(),
            src.env_vars.clone(),
            // Never duplicate auto-launch — two copies of the same server
            // racing for the same port at startup is a footgun.
            false,
            src.auto_restart,
            src.max_retries,
            src.restart_cooldown_secs,
        );
        config.servers.insert(idx + 1, copy.clone());
        copy
    };
    save_snapshot(&state)?;
    let _ = app_handle.emit("tray-update", ());
    Ok(entry)
}

#[tauri::command]
pub fn remove_server(
    state: State<ConfigState>,
    manager: State<ServerManager>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let server = {
        let mut running = manager.running.lock().unwrap();
        running.remove(&id)
    };
    if let Some(mut server) = server {
        let _ = terminate_child(&mut server.child);
    }
    manager.logs.lock().unwrap().remove(&id);
    manager.last_exit.lock().unwrap().remove(&id);
    {
        let mut config = state.0.lock().unwrap();
        config.servers.retain(|s| s.id != id);
    }
    save_snapshot(&state)?;
    let _ = app_handle.emit("tray-update", ());
    Ok(())
}

#[tauri::command]
pub fn start_server(
    state: State<ConfigState>,
    manager: State<ServerManager>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    {
        let running = manager.running.lock().unwrap();
        if running.contains_key(&id) {
            return Err("Server is already running".to_string());
        }
    }

    let entry = {
        let config = state.0.lock().unwrap();
        config.servers.iter().find(|s| s.id == id).ok_or("Server not found")?.clone()
    };

    let log_buf = manager.log_buffer(&id);
    let running_server = spawn_server(&entry, &app_handle, log_buf)?;

    {
        let mut running = manager.running.lock().unwrap();
        running.insert(id.clone(), running_server);
    }
    manager.last_exit.lock().unwrap().remove(&id);
    let _ = app_handle.emit("tray-update", ());
    Ok(())
}

#[tauri::command]
pub fn stop_server(manager: State<ServerManager>, app_handle: AppHandle, id: String) -> Result<(), String> {
    let server = {
        let mut running = manager.running.lock().unwrap();
        running.remove(&id)
    };
    let Some(mut server) = server else {
        return Err("Server is not running".to_string());
    };
    terminate_child(&mut server.child)?;
    manager.last_exit.lock().unwrap().insert(id, LastExit { crashed: false });
    let _ = app_handle.emit("tray-update", ());
    Ok(())
}

#[tauri::command]
pub fn restart_server(
    state: State<ConfigState>,
    manager: State<ServerManager>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let server = {
        let mut running = manager.running.lock().unwrap();
        running.remove(&id)
    };
    if let Some(mut server) = server {
        terminate_child(&mut server.child)?;
    }
    start_server(state, manager, app_handle, id)
}

#[tauri::command]
pub fn get_server_output(manager: State<ServerManager>, id: String) -> Vec<String> {
    let logs = manager.logs.lock().unwrap();
    logs.get(&id)
        .map(|buf| buf.lock().unwrap().clone())
        .unwrap_or_default()
}

#[tauri::command]
pub fn clear_server_log(manager: State<ServerManager>, id: String) {
    if let Some(buf) = manager.logs.lock().unwrap().get(&id) {
        buf.lock().unwrap().clear();
    }
}

#[tauri::command]
pub fn export_server_log(manager: State<ServerManager>, id: String, path: String) -> Result<(), String> {
    let content = {
        let logs = manager.logs.lock().unwrap();
        let buf = logs.get(&id).ok_or("No log output for this server")?;
        let output = buf.lock().unwrap();
        output.join("\n")
    };
    std::fs::write(&path, content).map_err(|e| format!("Failed to write log: {}", e))
}

#[tauri::command]
pub fn get_server_resources(manager: State<ServerManager>, id: String) -> Result<ServerResources, String> {
    let (pid, uptime_secs) = {
        let running = manager.running.lock().unwrap();
        let server = running.get(&id).ok_or("Server is not running")?;
        (server.child.id(), server.started_at.elapsed().as_secs())
    };

    // Read memory from /proc/{pid}/status
    let memory_kb = std::fs::read_to_string(format!("/proc/{}/status", pid))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0);

    // Read CPU from /proc/{pid}/stat (the sampling sleep happens outside
    // the manager lock so other commands aren't blocked).
    let cpu_percent = read_cpu_percent(pid).unwrap_or(0.0);

    Ok(ServerResources {
        pid,
        uptime_secs,
        memory_kb,
        cpu_percent,
    })
}

fn read_proc_jiffies(pid: u32) -> Option<u64> {
    // Reads utime+stime from /proc/{pid}/stat. The 2nd field is `(comm)` and
    // can contain spaces or parentheses, so split off everything up to the
    // last ')' before parsing the rest.
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let rparen = stat.rfind(')')?;
    let after = &stat[rparen + 1..];
    let fields: Vec<&str> = after.split_whitespace().collect();
    // After ')': field index 0 = state (3rd field overall), so utime is index 11, stime is index 12.
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

fn read_cpu_percent(pid: u32) -> Option<f64> {
    let clk_tck: f64 = 100.0;
    let sample_window = Duration::from_millis(100);

    let t0 = read_proc_jiffies(pid)?;
    std::thread::sleep(sample_window);
    let t1 = read_proc_jiffies(pid)?;

    let delta = t1.saturating_sub(t0) as f64 / clk_tck;
    let elapsed = sample_window.as_secs_f64();
    if elapsed > 0.0 {
        Some((delta / elapsed) * 100.0)
    } else {
        Some(0.0)
    }
}

#[tauri::command]
pub fn get_all_server_statuses(manager: State<ServerManager>) -> Vec<ServerStatus> {
    let running = manager.running.lock().unwrap();
    let last_exit = manager.last_exit.lock().unwrap();
    let mut statuses: Vec<ServerStatus> = running
        .keys()
        .map(|id| ServerStatus {
            id: id.clone(),
            state: "running".to_string(),
        })
        .collect();
    for (id, exit) in last_exit.iter() {
        if !running.contains_key(id) {
            statuses.push(ServerStatus {
                id: id.clone(),
                state: if exit.crashed { "crashed" } else { "stopped" }.to_string(),
            });
        }
    }
    statuses
}

/// Background watcher: reaps crashed servers and triggers auto-restart.
/// Runs once at startup; decouples restart timing from frontend polling.
pub fn start_server_watcher(app_handle: &AppHandle) {
    let handle = app_handle.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));

        let exited: Vec<(String, RunningServer, bool)> = {
            let manager = handle.try_state::<ServerManager>().unwrap();
            let mut running = manager.running.lock().unwrap();
            let mut to_remove: Vec<(String, bool)> = Vec::new();
            for (id, server) in running.iter_mut() {
                match server.child.try_wait() {
                    Ok(Some(status)) => to_remove.push((id.clone(), !status.success())),
                    Err(_) => to_remove.push((id.clone(), true)),
                    Ok(None) => {}
                }
            }
            to_remove
                .into_iter()
                .filter_map(|(id, crashed)| running.remove(&id).map(|s| (id, s, crashed)))
                .collect()
        };

        if exited.is_empty() {
            continue;
        }

        let _ = handle.emit("tray-update", ());

        for (id, old_server, crashed) in exited {
            {
                let manager = handle.try_state::<ServerManager>().unwrap();
                manager.last_exit.lock().unwrap().insert(id.clone(), LastExit { crashed });
            }

            let cs = handle.try_state::<ConfigState>().unwrap();
            let config = cs.0.lock().unwrap().clone();
            let Some(entry) = config.servers.iter().find(|s| s.id == id).cloned() else { continue };

            if crashed && config.settings.notifications_enabled {
                let _ = handle.emit(
                    "server-notification",
                    format!("Server \"{}\" crashed", entry.name),
                );
            }

            // A stable run wipes the retry history — only rapid crash loops
            // should exhaust max_retries.
            let prior_restarts = if old_server.started_at.elapsed() >= Duration::from_secs(STABLE_RUN_SECS) {
                0
            } else {
                old_server.restart_count
            };
            if !entry.auto_restart || prior_restarts >= entry.max_retries {
                continue;
            }
            let cooldown = Duration::from_secs(entry.restart_cooldown_secs);
            let handle_inner = handle.clone();
            let id_inner = id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(cooldown);
                let manager = handle_inner.try_state::<ServerManager>().unwrap();
                let log_buf = manager.log_buffer(&id_inner);
                if let Ok(mut new_server) = spawn_server(&entry, &handle_inner, log_buf) {
                    new_server.restart_count = prior_restarts + 1;
                    let mut running = manager.running.lock().unwrap();
                    running.insert(id_inner.clone(), new_server);
                    drop(running);
                    manager.last_exit.lock().unwrap().remove(&id_inner);
                    let _ = handle_inner.emit("tray-update", ());
                }
            });
        }
    });
}

#[tauri::command]
pub fn open_server_directory(state: State<ConfigState>, id: String) -> Result<(), String> {
    let config = state.0.lock().unwrap();
    let server = config.servers.iter().find(|s| s.id == id).ok_or("Server not found")?;

    let dir = if let Some(ref wd) = server.working_directory {
        std::path::PathBuf::from(wd)
    } else if let Some(ref exe) = server.executable {
        std::path::PathBuf::from(exe)
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or("Cannot determine directory")?
    } else {
        return Err("No directory or executable path configured".to_string());
    };

    open::that(&dir).map_err(|e| format!("Failed to open directory: {}", e))?;
    Ok(())
}

/// Auto-launch servers marked with auto_launch on startup
pub fn auto_launch_servers(config: &ConfigState, manager: &ServerManager, app_handle: &AppHandle) {
    let config = config.0.lock().unwrap();

    for entry in &config.servers {
        if entry.auto_launch {
            let log_buf = manager.log_buffer(&entry.id);
            if let Ok(server) = spawn_server(entry, app_handle, log_buf) {
                let mut running = manager.running.lock().unwrap();
                running.insert(entry.id.clone(), server);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_csi_color_codes() {
        assert_eq!(strip_ansi("\u{1b}[32mready\u{1b}[0m in 120ms"), "ready in 120ms");
        assert_eq!(strip_ansi("\u{1b}[1;31;40merror\u{1b}[m"), "error");
    }

    #[test]
    fn strips_osc_sequences() {
        assert_eq!(strip_ansi("\u{1b}]0;window title\u{7}text"), "text");
        assert_eq!(strip_ansi("\u{1b}]8;;https://x\u{1b}\\link"), "link");
    }

    #[test]
    fn leaves_plain_text_alone() {
        assert_eq!(strip_ansi("plain output"), "plain output");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn handles_truncated_escape() {
        assert_eq!(strip_ansi("dangling \u{1b}"), "dangling ");
        assert_eq!(strip_ansi("cut \u{1b}[31"), "cut ");
    }
}
