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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RazConfig {
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub links: Vec<LinkEntry>,
    #[serde(default)]
    pub servers: Vec<ServerEntry>,
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
    fs::write(&path, data).map_err(|e| e.to_string())
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
        }
    }
}

impl LinkEntry {
    pub fn new(name: String, url: String, icon: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            url,
            icon,
        }
    }
}
