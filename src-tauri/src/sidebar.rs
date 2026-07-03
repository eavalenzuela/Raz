use crate::config::{save_snapshot, ConfigState, PinnedItem, StatusMonitor};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// How many past check results to retain per monitor for the uptime strip.
const HISTORY_LEN: usize = 30;

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
    let item = {
        let mut config = state.0.lock().unwrap();
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
        item
    };
    save_snapshot(&state)?;
    let _ = app_handle.emit("pinned-changed", ());
    Ok(item)
}

#[tauri::command]
pub fn unpin_item(state: State<ConfigState>, app_handle: AppHandle, id: String) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        config.pinned.retain(|p| p.id != id);
    }
    save_snapshot(&state)?;
    let _ = app_handle.emit("pinned-changed", ());
    Ok(())
}

// ── Status Monitor ─────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MonitorStatus {
    pub id: String,
    pub state: String, // "up", "down", "unknown"
    pub last_check: Option<u64>, // seconds ago
    /// Most recent check results, oldest first (true = up).
    pub history: Vec<bool>,
}

pub struct MonitorState {
    pub statuses: Arc<Mutex<HashMap<String, MonitorStatusEntry>>>,
    pub stop_flag: Arc<Mutex<bool>>,
}

pub struct MonitorStatusEntry {
    state: String,
    last_check: Instant,
    history: Vec<bool>,
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
    let monitor = StatusMonitor::new(name, target, check_type, check_interval_secs);
    {
        let mut config = state.0.lock().unwrap();
        config.status_monitors.push(monitor.clone());
    }
    save_snapshot(&state)?;
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
    let updated = {
        let mut config = state.0.lock().unwrap();
        let monitor = config.status_monitors.iter_mut().find(|m| m.id == id).ok_or("Monitor not found")?;
        monitor.name = name;
        monitor.target = target;
        monitor.check_type = check_type;
        monitor.check_interval_secs = check_interval_secs;
        monitor.clone()
    };
    save_snapshot(&state)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_status_monitor(
    state: State<ConfigState>,
    monitor_state: State<MonitorState>,
    id: String,
) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        config.status_monitors.retain(|m| m.id != id);
    }
    // Drop the cached status so the status bar doesn't keep counting it.
    monitor_state.statuses.lock().unwrap().remove(&id);
    save_snapshot(&state)
}

#[tauri::command]
pub fn set_monitor_enabled(
    state: State<ConfigState>,
    monitor_state: State<MonitorState>,
    app_handle: AppHandle,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        let monitor = config.status_monitors.iter_mut().find(|m| m.id == id).ok_or("Monitor not found")?;
        monitor.enabled = enabled;
    }
    if !enabled {
        // Drop the cached status immediately so the status bar and sidebar
        // stop counting a paused monitor.
        monitor_state.statuses.lock().unwrap().remove(&id);
    }
    save_snapshot(&state)?;
    let _ = app_handle.emit("monitor-update", ());
    Ok(())
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
            history: entry.history.clone(),
        })
        .collect()
}

/// The ping target is handed to the `ping` CLI, which has no `--` separator —
/// restrict it to hostname/IP characters so a target like "-f" can't inject
/// options.
fn valid_ping_target(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('-')
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
}

fn check_target(target: &str, check_type: &str) -> bool {
    match check_type {
        "http" => {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build();
            match client {
                // A reachable server returning 4xx/5xx is not "up" — only
                // count 2xx/3xx as healthy.
                Ok(c) => match c.get(target).send() {
                    Ok(resp) => {
                        let status = resp.status();
                        status.is_success() || status.is_redirection()
                    }
                    Err(_) => false,
                },
                Err(_) => false,
            }
        }
        "ping" => {
            if !valid_ping_target(target) {
                return false;
            }
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

    // Debounce: only fire a notification after this many consecutive checks
    // in the new state. Prevents flapping links from spamming notifications.
    const DEBOUNCE_CHECKS: u32 = 2;

    std::thread::spawn(move || {
        struct Tracker {
            announced_state: String,
            pending_state: String,
            pending_count: u32,
            next_check: Instant,
            history: VecDeque<bool>,
        }
        let mut trackers: HashMap<String, Tracker> = HashMap::new();

        loop {
            if *stop_flag.lock().unwrap() {
                break;
            }

            // Paused monitors are excluded here, so they're never checked and
            // their trackers/statuses are pruned like removed ones — a
            // re-enabled monitor starts fresh and announces immediately.
            let monitors: Vec<StatusMonitor> = {
                let config_state = handle.try_state::<ConfigState>().unwrap();
                let config = config_state.0.lock().unwrap();
                config.status_monitors.iter().filter(|m| m.enabled).cloned().collect()
            };

            let now = Instant::now();
            let monitor_ids: std::collections::HashSet<String> =
                monitors.iter().map(|m| m.id.clone()).collect();
            trackers.retain(|id, _| monitor_ids.contains(id));
            // Also prune cached statuses, or monitors removed by a config
            // import would keep counting in the status bar forever.
            statuses.lock().unwrap().retain(|id, _| monitor_ids.contains(id));

            // Run every due check on its own thread so one slow or down
            // target (up to a 10s timeout) can't delay the others.
            let due: Vec<StatusMonitor> = monitors
                .into_iter()
                .filter(|m| {
                    trackers
                        .get(&m.id)
                        .map(|t| now >= t.next_check)
                        .unwrap_or(true)
                })
                .collect();
            let results: Vec<(StatusMonitor, bool)> = due
                .into_iter()
                .map(|monitor| {
                    std::thread::spawn(move || {
                        let is_up = check_target(&monitor.target, &monitor.check_type);
                        (monitor, is_up)
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|h| h.join().ok())
                .collect();

            let checked_any = !results.is_empty();

            for (monitor, is_up) in results {
                let new_state = if is_up { "up" } else { "down" }.to_string();

                let interval = Duration::from_secs(monitor.check_interval_secs.max(5));
                let entry = trackers.entry(monitor.id.clone()).or_insert_with(|| Tracker {
                    announced_state: "unknown".to_string(),
                    pending_state: new_state.clone(),
                    pending_count: 0,
                    next_check: now,
                    history: VecDeque::new(),
                });

                if entry.pending_state == new_state {
                    entry.pending_count = entry.pending_count.saturating_add(1);
                } else {
                    entry.pending_state = new_state.clone();
                    entry.pending_count = 1;
                }
                entry.next_check = now + interval;
                entry.history.push_back(is_up);
                if entry.history.len() > HISTORY_LEN {
                    entry.history.pop_front();
                }

                // First observation announces immediately (no flap to debounce).
                // Subsequent transitions require DEBOUNCE_CHECKS confirmations.
                let is_first = entry.announced_state == "unknown";
                let should_announce = entry.announced_state != new_state
                    && (is_first || entry.pending_count >= DEBOUNCE_CHECKS);
                if should_announce {
                    let prev_known = entry.announced_state != "unknown";
                    entry.announced_state = new_state.clone();
                    if prev_known {
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

                {
                    let mut map = statuses.lock().unwrap();
                    map.insert(
                        monitor.id.clone(),
                        MonitorStatusEntry {
                            state: entry.announced_state.clone(),
                            last_check: Instant::now(),
                            history: entry.history.iter().copied().collect(),
                        },
                    );
                }
            }

            if checked_any {
                let _ = handle.emit("monitor-update", ());
            }

            // Tight outer loop — per-monitor due-time gating handles cadence.
            for _ in 0..5 {
                if *stop_flag.lock().unwrap() {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_hosts_and_ips() {
        assert!(valid_ping_target("example.com"));
        assert!(valid_ping_target("192.168.1.1"));
        assert!(valid_ping_target("fe80::1"));
        assert!(valid_ping_target("my-nas.local"));
    }

    #[test]
    fn rejects_option_injection_and_junk() {
        assert!(!valid_ping_target("-f"));
        assert!(!valid_ping_target("--help"));
        assert!(!valid_ping_target(""));
        assert!(!valid_ping_target("host; rm -rf /"));
        assert!(!valid_ping_target("a b"));
    }
}
