use crate::config::{save_config, ConfigState, PinnedItem, StatusMonitor};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

// ── Pinned Items ───────────────────────────────────────

#[tauri::command]
pub fn get_pinned(state: State<ConfigState>) -> Vec<PinnedItem> {
    let config = state.0.lock().unwrap();
    config.pinned.clone()
}

#[tauri::command]
pub fn pin_item(
    state: State<ConfigState>,
    app_handle: AppHandle,
    source_id: String,
    source_type: String,
    name: String,
) -> Result<PinnedItem, String> {
    let mut config = state.0.lock().unwrap();
    // Don't pin duplicates
    if config.pinned.iter().any(|p| p.source_id == source_id && p.source_type == source_type) {
        return Err("Item is already pinned".to_string());
    }
    let item = PinnedItem {
        id: Uuid::new_v4().to_string(),
        source_id,
        source_type,
        name,
    };
    config.pinned.push(item.clone());
    save_config(&config)?;
    let _ = app_handle.emit("pinned-changed", ());
    Ok(item)
}

#[tauri::command]
pub fn unpin_item(state: State<ConfigState>, app_handle: AppHandle, id: String) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    config.pinned.retain(|p| p.id != id);
    save_config(&config)?;
    let _ = app_handle.emit("pinned-changed", ());
    Ok(())
}

// ── Status Monitor ─────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MonitorStatus {
    pub id: String,
    pub state: String, // "up", "down", "unknown"
    pub last_check: Option<u64>, // seconds ago
}

pub struct MonitorState {
    pub statuses: Arc<Mutex<HashMap<String, MonitorStatusEntry>>>,
    pub stop_flag: Arc<Mutex<bool>>,
}

pub struct MonitorStatusEntry {
    state: String,
    last_check: Instant,
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(Mutex::new(HashMap::new())),
            stop_flag: Arc::new(Mutex::new(false)),
        }
    }
}

#[tauri::command]
pub fn get_status_monitors(state: State<ConfigState>) -> Vec<StatusMonitor> {
    let config = state.0.lock().unwrap();
    config.status_monitors.clone()
}

#[tauri::command]
pub fn add_status_monitor(
    state: State<ConfigState>,
    name: String,
    target: String,
    check_type: String,
    check_interval_secs: u64,
) -> Result<StatusMonitor, String> {
    let mut config = state.0.lock().unwrap();
    let monitor = StatusMonitor::new(name, target, check_type, check_interval_secs);
    config.status_monitors.push(monitor.clone());
    save_config(&config)?;
    Ok(monitor)
}

#[tauri::command]
pub fn update_status_monitor(
    state: State<ConfigState>,
    id: String,
    name: String,
    target: String,
    check_type: String,
    check_interval_secs: u64,
) -> Result<StatusMonitor, String> {
    let mut config = state.0.lock().unwrap();
    let monitor = config.status_monitors.iter_mut().find(|m| m.id == id).ok_or("Monitor not found")?;
    monitor.name = name;
    monitor.target = target;
    monitor.check_type = check_type;
    monitor.check_interval_secs = check_interval_secs;
    let updated = monitor.clone();
    save_config(&config)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_status_monitor(state: State<ConfigState>, id: String) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    config.status_monitors.retain(|m| m.id != id);
    save_config(&config)
}

#[tauri::command]
pub fn get_monitor_statuses(monitor_state: State<MonitorState>) -> Vec<MonitorStatus> {
    let statuses = monitor_state.statuses.lock().unwrap();
    statuses
        .iter()
        .map(|(id, entry)| MonitorStatus {
            id: id.clone(),
            state: entry.state.clone(),
            last_check: Some(entry.last_check.elapsed().as_secs()),
        })
        .collect()
}

fn check_target(target: &str, check_type: &str) -> bool {
    match check_type {
        "http" => {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .danger_accept_invalid_certs(true)
                .build();
            match client {
                Ok(c) => c.get(target).send().is_ok(),
                Err(_) => false,
            }
        }
        "ping" => {
            // Use system ping command
            std::process::Command::new("ping")
                .args(["-c", "1", "-W", "5", target])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Start the background monitoring loop
pub fn start_monitor_loop(
    _config_state: &ConfigState,
    monitor_state: &MonitorState,
    app_handle: &AppHandle,
) {
    let statuses = monitor_state.statuses.clone();
    let stop_flag = monitor_state.stop_flag.clone();
    let handle = app_handle.clone();

    std::thread::spawn(move || {
        let mut prev_states: HashMap<String, String> = HashMap::new();

        loop {
            if *stop_flag.lock().unwrap() {
                break;
            }

            // Re-read monitors from live config state each cycle
            let monitors = {
                let config_state = handle.try_state::<ConfigState>().unwrap();
                let config = config_state.0.lock().unwrap();
                config.status_monitors.clone()
            };

            for monitor in &monitors {
                let is_up = check_target(&monitor.target, &monitor.check_type);
                let new_state = if is_up { "up" } else { "down" };

                // Check for state change and notify (respecting settings)
                let prev = prev_states.get(&monitor.id);
                if let Some(prev) = prev {
                    if prev != new_state {
                        let cs = handle.try_state::<ConfigState>().unwrap();
                        let settings = cs.0.lock().unwrap().settings.clone();
                        let should_notify = settings.notifications_enabled
                            && ((new_state == "down" && settings.notify_on_down)
                                || (new_state == "up" && settings.notify_on_up));
                        if should_notify {
                            let title = if new_state == "down" {
                                format!("{} is down", monitor.name)
                            } else {
                                format!("{} is back up", monitor.name)
                            };
                            let _ = handle.emit("monitor-notification", &title);
                        }
                    }
                }
                prev_states.insert(monitor.id.clone(), new_state.to_string());

                {
                    let mut map = statuses.lock().unwrap();
                    map.insert(
                        monitor.id.clone(),
                        MonitorStatusEntry {
                            state: new_state.to_string(),
                            last_check: Instant::now(),
                        },
                    );
                }

                let _ = handle.emit("monitor-update", ());
            }

            // Sleep, checking stop flag periodically
            for _ in 0..30 {
                if *stop_flag.lock().unwrap() {
                    return;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    });
}
