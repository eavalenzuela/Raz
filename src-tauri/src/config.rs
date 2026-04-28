use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub raw_command: Option<String>,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub type_label: Option<String>,
    #[serde(default)]
    pub launch_count: u64,
    #[serde(default)]
    pub last_launched: Option<u64>, // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub raw_command: Option<String>,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    #[serde(default)]
    pub auto_launch: bool,
    #[serde(default)]
    pub auto_restart: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_cooldown")]
    pub restart_cooldown_secs: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_cooldown() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedItem {
    pub id: String,
    pub source_id: String,
    pub source_type: String, // "app" or "link"
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMonitor {
    pub id: String,
    pub name: String,
    pub target: String,
    pub check_type: String, // "http" or "ping"
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
}

fn default_check_interval() -> u64 {
    60
}

impl StatusMonitor {
    pub fn new(name: String, target: String, check_type: String, check_interval_secs: u64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            target,
            check_type,
            check_interval_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_check_interval")]
    pub default_check_interval_secs: u64,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_true")]
    pub notify_on_down: bool,
    #[serde(default = "default_true")]
    pub notify_on_up: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_check_interval_secs: 60,
            notifications_enabled: true,
            notify_on_down: true,
            notify_on_up: true,
            minimize_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RazConfig {
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub links: Vec<LinkEntry>,
    #[serde(default)]
    pub servers: Vec<ServerEntry>,
    #[serde(default)]
    pub pinned: Vec<PinnedItem>,
    #[serde(default)]
    pub status_monitors: Vec<StatusMonitor>,
    #[serde(default)]
    pub settings: Settings,
}

pub struct ConfigState(pub Mutex<RazConfig>);

fn config_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("raz");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("config.json")
}

pub fn load_config() -> RazConfig {
    let path = config_path();
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        RazConfig::default()
    }
}

pub fn save_config(config: &RazConfig) -> Result<(), String> {
    let path = config_path();
    let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Snapshot the config under the lock, drop the guard, then save. Use this
/// from command handlers so a slow disk write doesn't block other commands
/// and a panic during serialization doesn't poison the mutex.
pub fn save_snapshot(state: &ConfigState) -> Result<(), String> {
    let snapshot = { state.0.lock().unwrap().clone() };
    save_config(&snapshot)
}

impl AppEntry {
    pub fn new(
        name: String,
        raw_command: Option<String>,
        executable: Option<String>,
        arguments: Vec<String>,
        working_directory: Option<String>,
        env_vars: Vec<EnvVar>,
        icon: Option<String>,
        type_label: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            raw_command,
            executable,
            arguments,
            working_directory,
            env_vars,
            icon,
            type_label,
            launch_count: 0,
            last_launched: None,
        }
    }
}

impl ServerEntry {
    pub fn new(
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
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            raw_command,
            executable,
            arguments,
            working_directory,
            env_vars,
            auto_launch,
            auto_restart,
            max_retries,
            restart_cooldown_secs,
        }
    }
}

impl LinkEntry {
    pub fn new(name: String, url: String, icon: Option<String>, folder: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            url,
            icon,
            folder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip_preserves_data() {
        let mut cfg = RazConfig::default();
        cfg.apps.push(AppEntry::new(
            "Foo".to_string(),
            Some("foo --bar".to_string()),
            None,
            vec![],
            Some("/tmp".to_string()),
            vec![EnvVar { key: "K".into(), value: "V".into() }],
            None,
            Some("Editor".to_string()),
        ));
        cfg.links.push(LinkEntry::new(
            "Site".to_string(),
            "https://example.com".to_string(),
            None,
            Some("Home".to_string()),
        ));
        cfg.servers.push(ServerEntry::new(
            "Dev".to_string(),
            None,
            Some("/usr/bin/server".to_string()),
            vec!["--port".into(), "8080".into()],
            None,
            vec![],
            true, true, 5, 10,
        ));
        cfg.status_monitors.push(StatusMonitor::new(
            "API".to_string(),
            "https://api.example.com".to_string(),
            "http".to_string(),
            30,
        ));
        cfg.settings.notifications_enabled = false;

        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: RazConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.apps.len(), 1);
        assert_eq!(parsed.apps[0].name, "Foo");
        assert_eq!(parsed.apps[0].env_vars[0].key, "K");
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].folder.as_deref(), Some("Home"));
        assert_eq!(parsed.servers.len(), 1);
        assert!(parsed.servers[0].auto_restart);
        assert_eq!(parsed.servers[0].max_retries, 5);
        assert_eq!(parsed.status_monitors.len(), 1);
        assert_eq!(parsed.status_monitors[0].check_interval_secs, 30);
        assert!(!parsed.settings.notifications_enabled);
        assert!(parsed.settings.minimize_to_tray); // default still applies
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let json = r#"{"apps":[],"servers":[{"id":"x","name":"S"}]}"#;
        let parsed: RazConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.servers[0].max_retries, 3);
        assert_eq!(parsed.servers[0].restart_cooldown_secs, 5);
        assert!(!parsed.servers[0].auto_restart);
        assert!(parsed.settings.notifications_enabled);
    }
}
