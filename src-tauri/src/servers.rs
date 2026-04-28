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
    output: Arc<Mutex<Vec<String>>>,
    started_at: Instant,
    restart_count: u32,
}

pub struct ServerManager(pub Mutex<HashMap<String, RunningServer>>);

impl ServerManager {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

const MAX_OUTPUT_LINES: usize = 5000;
/// Hard cap on a single output line — a server that emits a megabyte of JSON
/// in one line shouldn't be able to balloon our memory.
const MAX_LINE_BYTES: usize = 8 * 1024;
/// Coalesce stdout/stderr emits to the UI to at most this often per server.
const EMIT_INTERVAL_MS: u64 = 100;

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

/// Spawns a background thread that batches emits for one (server_id, stream).
/// Returns a closure-friendly sender: push lines into the Vec, the thread
/// drains and emits at most every EMIT_INTERVAL_MS.
fn start_emit_batcher(handle: AppHandle, server_id: String) -> Arc<Mutex<Vec<String>>> {
    let pending: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_clone = Arc::clone(&pending);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(EMIT_INTERVAL_MS));
            let batch: Vec<String> = {
                let mut q = pending_clone.lock().unwrap();
                if q.is_empty() {
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
) -> Result<RunningServer, String> {
    let mut child = if let Some(ref raw) = entry.raw_command {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(raw);
        if let Some(ref dir) = entry.working_directory {
            cmd.current_dir(dir);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.spawn().map_err(|e| format!("Failed to start: {}", e))?
    } else if let Some(ref executable) = entry.executable {
        let mut cmd = Command::new(executable);
        cmd.args(&entry.arguments);
        if let Some(ref dir) = entry.working_directory {
            cmd.current_dir(dir);
        }
        for env in &entry.env_vars {
            cmd.env(&env.key, &env.value);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.spawn().map_err(|e| format!("Failed to start: {}", e))?
    } else {
        return Err("No command or executable configured".to_string());
    };

    let output = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_id = entry.id.clone();
    let pending = start_emit_batcher(app_handle.clone(), server_id.clone());

    let consume = |reader: Box<dyn BufRead + Send>, prefix_stderr: bool, output: Arc<Mutex<Vec<String>>>, pending: Arc<Mutex<Vec<String>>>| {
        std::thread::spawn(move || {
            for line in reader.lines() {
                let Ok(line) = line else { continue };
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
        output,
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
pub fn remove_server(
    state: State<ConfigState>,
    manager: State<ServerManager>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    {
        let mut running = manager.0.lock().unwrap();
        if let Some(mut server) = running.remove(&id) {
            let _ = server.child.kill();
        }
    }
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
    let entry = {
        let config = state.0.lock().unwrap();
        config.servers.iter().find(|s| s.id == id).ok_or("Server not found")?.clone()
    };

    let running_server = spawn_server(&entry, &app_handle)?;

    {
        let mut running = manager.0.lock().unwrap();
        running.insert(id, running_server);
    }
    let _ = app_handle.emit("tray-update", ());
    Ok(())
}

#[tauri::command]
pub fn stop_server(manager: State<ServerManager>, app_handle: AppHandle, id: String) -> Result<(), String> {
    {
        let mut running = manager.0.lock().unwrap();
        if let Some(mut server) = running.remove(&id) {
            server.child.kill().map_err(|e| format!("Failed to stop: {}", e))?;
            let _ = server.child.wait();
        } else {
            return Err("Server is not running".to_string());
        }
    }
    let _ = app_handle.emit("tray-update", ());
    Ok(())
}

#[tauri::command]
pub fn get_server_output(manager: State<ServerManager>, id: String) -> Vec<String> {
    let running = manager.0.lock().unwrap();
    if let Some(server) = running.get(&id) {
        let output = server.output.lock().unwrap();
        output.clone()
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub fn export_server_log(manager: State<ServerManager>, id: String, path: String) -> Result<(), String> {
    let running = manager.0.lock().unwrap();
    if let Some(server) = running.get(&id) {
        let output = server.output.lock().unwrap();
        let content = output.join("\n");
        std::fs::write(&path, content).map_err(|e| format!("Failed to write log: {}", e))?;
        Ok(())
    } else {
        Err("Server is not running".to_string())
    }
}

#[tauri::command]
pub fn get_server_resources(manager: State<ServerManager>, id: String) -> Result<ServerResources, String> {
    let running = manager.0.lock().unwrap();
    let server = running.get(&id).ok_or("Server is not running")?;

    let pid = server.child.id();
    let uptime_secs = server.started_at.elapsed().as_secs();

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

    // Read CPU from /proc/{pid}/stat
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
    let running = manager.0.lock().unwrap();
    running
        .keys()
        .map(|id| ServerStatus {
            id: id.clone(),
            state: "running".to_string(),
        })
        .collect()
}

/// Background watcher: reaps crashed servers and triggers auto-restart.
/// Runs once at startup; decouples restart timing from frontend polling.
pub fn start_server_watcher(app_handle: &AppHandle) {
    let handle = app_handle.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));

        let crashed: Vec<(String, RunningServer)> = {
            let manager = handle.try_state::<ServerManager>().unwrap();
            let mut running = manager.0.lock().unwrap();
            let mut to_remove = Vec::new();
            for (id, server) in running.iter_mut() {
                match server.child.try_wait() {
                    Ok(Some(_)) | Err(_) => to_remove.push(id.clone()),
                    Ok(None) => {}
                }
            }
            to_remove
                .into_iter()
                .filter_map(|id| running.remove(&id).map(|s| (id, s)))
                .collect()
        };

        if crashed.is_empty() {
            continue;
        }

        let _ = handle.emit("tray-update", ());

        for (id, old_server) in crashed {
            let cs = handle.try_state::<ConfigState>().unwrap();
            let config = cs.0.lock().unwrap().clone();
            let Some(entry) = config.servers.iter().find(|s| s.id == id).cloned() else { continue };
            if !entry.auto_restart || old_server.restart_count >= entry.max_retries {
                continue;
            }
            let cooldown = Duration::from_secs(entry.restart_cooldown_secs);
            let handle_inner = handle.clone();
            let id_inner = id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(cooldown);
                if let Ok(mut new_server) = spawn_server(&entry, &handle_inner) {
                    new_server.restart_count = old_server.restart_count + 1;
                    let manager = handle_inner.try_state::<ServerManager>().unwrap();
                    let mut running = manager.0.lock().unwrap();
                    running.insert(id_inner, new_server);
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
    let mut running = manager.0.lock().unwrap();

    for entry in &config.servers {
        if entry.auto_launch {
            if let Ok(server) = spawn_server(entry, app_handle) {
                running.insert(entry.id.clone(), server);
            }
        }
    }
}
