use crate::config::{save_config, ConfigState, EnvVar, ServerEntry};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;
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

fn timestamp() -> String {
    let now = chrono::Local::now();
    now.format("%H:%M:%S").to_string()
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

    // Read stdout
    if let Some(stdout) = child.stdout.take() {
        let output_clone = Arc::clone(&output);
        let handle = app_handle.clone();
        let id = server_id.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let stamped = format!("[{}] {}", timestamp(), line);
                    {
                        let mut buf = output_clone.lock().unwrap();
                        buf.push(stamped.clone());
                        if buf.len() > MAX_OUTPUT_LINES {
                            let excess = buf.len() - MAX_OUTPUT_LINES;
                            buf.drain(0..excess);
                        }
                    }
                    let _ = handle.emit("server-output", (&id, &stamped));
                }
            }
        });
    }

    // Read stderr
    if let Some(stderr) = child.stderr.take() {
        let output_clone = Arc::clone(&output);
        let handle = app_handle.clone();
        let id = server_id.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let stamped = format!("[{}] [stderr] {}", timestamp(), line);
                    {
                        let mut buf = output_clone.lock().unwrap();
                        buf.push(stamped.clone());
                        if buf.len() > MAX_OUTPUT_LINES {
                            let excess = buf.len() - MAX_OUTPUT_LINES;
                            buf.drain(0..excess);
                        }
                    }
                    let _ = handle.emit("server-output", (&id, &stamped));
                }
            }
        });
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
    let entry = {
        let mut config = state.0.lock().unwrap();
        let entry = ServerEntry::new(
            name, raw_command, executable, arguments, working_directory,
            env_vars, auto_launch, auto_restart, max_retries, restart_cooldown_secs,
        );
        config.servers.push(entry.clone());
        save_config(&config)?;
        entry
    };
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
        let updated = server.clone();
        save_config(&config)?;
        updated
    };
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
        save_config(&config)?;
    }
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

fn read_cpu_percent(pid: u32) -> Option<f64> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let fields: Vec<&str> = stat.split_whitespace().collect();
    if fields.len() < 22 {
        return None;
    }
    let utime: u64 = fields[13].parse().ok()?;
    let stime: u64 = fields[14].parse().ok()?;
    let starttime: u64 = fields[21].parse().ok()?;

    let uptime_str = std::fs::read_to_string("/proc/uptime").ok()?;
    let system_uptime: f64 = uptime_str.split_whitespace().next()?.parse().ok()?;

    let clk_tck: f64 = 100.0; // sysconf(_SC_CLK_TCK), almost always 100 on Linux
    let total_time = (utime + stime) as f64 / clk_tck;
    let elapsed = system_uptime - (starttime as f64 / clk_tck);

    if elapsed > 0.0 {
        Some((total_time / elapsed) * 100.0)
    } else {
        Some(0.0)
    }
}

#[tauri::command]
pub fn get_all_server_statuses(
    manager: State<ServerManager>,
    app_handle: AppHandle,
) -> Vec<ServerStatus> {
    let mut running = manager.0.lock().unwrap();
    let mut statuses = Vec::new();

    let mut crashed = Vec::new();
    for (id, server) in running.iter_mut() {
        match server.child.try_wait() {
            Ok(Some(_exit)) => {
                crashed.push(id.clone());
                statuses.push(ServerStatus {
                    id: id.clone(),
                    state: "crashed".to_string(),
                });
            }
            Ok(None) => {
                statuses.push(ServerStatus {
                    id: id.clone(),
                    state: "running".to_string(),
                });
            }
            Err(_) => {
                crashed.push(id.clone());
                statuses.push(ServerStatus {
                    id: id.clone(),
                    state: "crashed".to_string(),
                });
            }
        }
    }

    // Handle crashed servers — auto-restart if configured
    for id in crashed {
        let removed = running.remove(&id);
        if let Some(old_server) = removed {
            let config = {
                let cs = app_handle.try_state::<ConfigState>().unwrap();
                let cfg = cs.0.lock().unwrap().clone();
                cfg
            };
            if let Some(entry) = config.servers.iter().find(|s| s.id == id) {
                if entry.auto_restart && old_server.restart_count < entry.max_retries {
                    let cooldown = std::time::Duration::from_secs(entry.restart_cooldown_secs);
                    let entry = entry.clone();
                    let id = id.clone();
                    let handle = app_handle.clone();

                    // Spawn restart in background after cooldown
                    std::thread::spawn(move || {
                        std::thread::sleep(cooldown);
                        if let Ok(mut new_server) = spawn_server(&entry, &handle) {
                            new_server.restart_count = old_server.restart_count + 1;
                            let manager = handle.try_state::<ServerManager>().unwrap();
                            let mut running = manager.0.lock().unwrap();
                            running.insert(id, new_server);
                            let _ = handle.emit("tray-update", ());
                        }
                    });
                }
            }
        }
    }

    statuses
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
