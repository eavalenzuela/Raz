use crate::config::{save_config, AppEntry, ConfigState, EnvVar, LinkEntry, Settings};
use std::collections::HashMap;
use std::process::Command;
use tauri::State;

#[tauri::command]
pub fn quit_app() {
    std::process::exit(0);
}

#[tauri::command]
pub fn get_apps(state: State<ConfigState>) -> Vec<AppEntry> {
    let config = state.0.lock().unwrap();
    config.apps.clone()
}

#[tauri::command]
pub fn add_app(
    state: State<ConfigState>,
    name: String,
    raw_command: Option<String>,
    executable: Option<String>,
    arguments: Vec<String>,
    working_directory: Option<String>,
    env_vars: Vec<EnvVar>,
    icon: Option<String>,
    type_label: Option<String>,
) -> Result<AppEntry, String> {
    let mut config = state.0.lock().unwrap();
    let entry = AppEntry::new(name, raw_command, executable, arguments, working_directory, env_vars, icon, type_label);
    config.apps.push(entry.clone());
    save_config(&config)?;
    Ok(entry)
}

#[tauri::command]
pub fn update_app(
    state: State<ConfigState>,
    id: String,
    name: String,
    raw_command: Option<String>,
    executable: Option<String>,
    arguments: Vec<String>,
    working_directory: Option<String>,
    env_vars: Vec<EnvVar>,
    icon: Option<String>,
    type_label: Option<String>,
) -> Result<AppEntry, String> {
    let mut config = state.0.lock().unwrap();
    let app = config.apps.iter_mut().find(|a| a.id == id).ok_or("App not found")?;
    app.name = name;
    app.raw_command = raw_command;
    app.executable = executable;
    app.arguments = arguments;
    app.working_directory = working_directory;
    app.env_vars = env_vars;
    app.icon = icon;
    app.type_label = type_label;
    let updated = app.clone();
    save_config(&config)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_app(state: State<ConfigState>, id: String) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    config.apps.retain(|a| a.id != id);
    save_config(&config)
}

#[tauri::command]
pub fn reorder_apps(state: State<ConfigState>, ids: Vec<String>) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    let mut reordered = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(app) = config.apps.iter().find(|a| &a.id == id) {
            reordered.push(app.clone());
        }
    }
    config.apps = reordered;
    save_config(&config)
}

#[tauri::command]
pub fn launch_app(state: State<ConfigState>, id: String) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    let app = config.apps.iter().find(|a| a.id == id).ok_or("App not found")?;

    if let Some(ref raw) = app.raw_command {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(raw);
        if let Some(ref dir) = app.working_directory {
            cmd.current_dir(dir);
        }
        cmd.spawn().map_err(|e| format!("Failed to launch: {}", e))?;
    } else if let Some(ref executable) = app.executable {
        let mut cmd = Command::new(executable);
        cmd.args(&app.arguments);
        if let Some(ref dir) = app.working_directory {
            cmd.current_dir(dir);
        }
        for env in &app.env_vars {
            cmd.env(&env.key, &env.value);
        }
        cmd.spawn().map_err(|e| format!("Failed to launch: {}", e))?;
    } else {
        return Err("No command or executable configured".to_string());
    }

    // Track launch stats
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Some(app) = config.apps.iter_mut().find(|a| a.id == id) {
        app.launch_count += 1;
        app.last_launched = Some(now);
    }
    let _ = save_config(&config);

    Ok(())
}

#[tauri::command]
pub fn open_app_directory(state: State<ConfigState>, id: String) -> Result<(), String> {
    let config = state.0.lock().unwrap();
    let app = config.apps.iter().find(|a| a.id == id).ok_or("App not found")?;

    let dir = if let Some(ref wd) = app.working_directory {
        std::path::PathBuf::from(wd)
    } else if let Some(ref exe) = app.executable {
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

#[tauri::command]
pub fn resolve_icon(icon: String) -> Option<String> {
    // If it's already an absolute path and exists, return it
    let path = std::path::PathBuf::from(&icon);
    if path.is_absolute() && path.exists() {
        return Some(icon);
    }

    // Try common icon directories for theme icon names
    let icon_dirs = [
        "/usr/share/icons/hicolor",
        "/usr/share/pixmaps",
        "/usr/share/icons",
    ];
    let sizes = ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24", "16x16", "scalable"];
    let categories = ["apps", "devices", "mimetypes", "places", "status"];
    let extensions = ["png", "svg", "xpm"];

    for dir in &icon_dirs {
        for size in &sizes {
            for cat in &categories {
                for ext in &extensions {
                    let candidate = format!("{}/{}/{}/{}.{}", dir, size, cat, icon, ext);
                    if std::path::Path::new(&candidate).exists() {
                        return Some(candidate);
                    }
                }
            }
        }
        // Also check flat structure (e.g. /usr/share/pixmaps/icon.png)
        for ext in &extensions {
            let candidate = format!("{}/{}.{}", dir, icon, ext);
            if std::path::Path::new(&candidate).exists() {
                return Some(candidate);
            }
        }
    }

    // Check ~/.local/share/icons as well
    if let Some(data_dir) = dirs::data_dir() {
        let local_icons = data_dir.join("icons");
        for size in &sizes {
            for cat in &categories {
                for ext in &extensions {
                    let candidate = local_icons.join("hicolor").join(size).join(cat).join(format!("{}.{}", icon, ext));
                    if candidate.exists() {
                        return Some(candidate.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    None
}

#[tauri::command]
pub fn read_icon_base64(path: String) -> Result<String, String> {
    use std::fs;

    let data = fs::read(&path).map_err(|e| format!("Failed to read icon: {}", e))?;
    let mime = if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".xpm") {
        "image/x-xpixmap"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        // Try to detect from magic bytes
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            "image/png"
        } else if data.starts_with(b"<svg") || data.starts_with(b"<?xml") {
            "image/svg+xml"
        } else {
            "image/png"
        }
    };

    let mut b64 = String::new();
    b64.push_str("data:");
    b64.push_str(mime);
    b64.push_str(";base64,");
    let engine = base64_encode(&data);
    b64.push_str(&engine);
    Ok(b64)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[derive(serde::Serialize)]
pub struct DesktopFileInfo {
    pub name: String,
    pub exec: String,
    pub path: Option<String>,
    pub icon: Option<String>,
}

/// Parse a .desktop file and unescape its Exec line into a real shell command.
fn parse_desktop_file(content: &str) -> Result<DesktopFileInfo, String> {
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let name = fields.get("Name").ok_or("Missing Name field")?.clone();
    let exec_raw = fields.get("Exec").ok_or("Missing Exec field")?;

    // Unescape .desktop Exec field: \\\\ -> \\, \\ (before space) -> \ (escape space)
    // The .desktop spec says: \s = space, \n = newline, \t = tab, \r = carriage return, \\ = backslash
    let mut exec = String::with_capacity(exec_raw.len());
    let mut chars = exec_raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('\\') => { chars.next(); exec.push('\\'); }
                Some('s') => { chars.next(); exec.push(' '); }
                Some('n') => { chars.next(); exec.push('\n'); }
                Some('t') => { chars.next(); exec.push('\t'); }
                Some('r') => { chars.next(); exec.push('\r'); }
                // desktop spec: \ before other chars like space is just escaping
                Some(' ') => { chars.next(); exec.push_str("\\ "); }
                _ => exec.push('\\'),
            }
        } else {
            exec.push(c);
        }
    }

    // Strip desktop field codes (%f, %F, %u, %U, etc.)
    let exec = exec
        .replace("%f", "").replace("%F", "")
        .replace("%u", "").replace("%U", "")
        .replace("%i", "").replace("%c", "")
        .replace("%k", "")
        .trim().to_string();

    Ok(DesktopFileInfo {
        name,
        exec,
        path: fields.get("Path").cloned(),
        icon: fields.get("Icon").cloned(),
    })
}

#[tauri::command]
pub fn import_desktop_file(path: String) -> Result<DesktopFileInfo, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    parse_desktop_file(&content)
}

#[tauri::command]
pub fn add_app_from_desktop(
    state: State<ConfigState>,
    path: String,
) -> Result<AppEntry, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let info = parse_desktop_file(&content)?;

    // Resolve icon theme name to a file path
    let resolved_icon = info.icon.and_then(|i| resolve_icon(i));

    let mut config = state.0.lock().unwrap();
    let entry = AppEntry::new(
        info.name,
        Some(info.exec),
        None,
        Vec::new(),
        info.path,
        Vec::new(),
        resolved_icon,
        None,
    );
    config.apps.push(entry.clone());
    save_config(&config)?;
    Ok(entry)
}

// ── Bulk Desktop Import ───────────────────────────────

#[derive(serde::Serialize)]
pub struct DesktopCandidate {
    pub path: String,
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

#[tauri::command]
pub fn scan_desktop_files(state: State<ConfigState>) -> Vec<DesktopCandidate> {
    let config = state.0.lock().unwrap();
    let existing_names: std::collections::HashSet<String> =
        config.apps.iter().map(|a| a.name.to_lowercase()).collect();

    let dirs = [
        "/usr/share/applications",
        "/usr/local/share/applications",
    ];
    let home_apps = dirs::data_dir().map(|d| d.join("applications"));

    let mut candidates = Vec::new();

    let scan_dir = |dir: &std::path::Path, candidates: &mut Vec<DesktopCandidate>, existing: &std::collections::HashSet<String>| {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // Skip NoDisplay=true entries
                    if content.lines().any(|l| l.trim() == "NoDisplay=true") {
                        continue;
                    }
                    if let Ok(info) = parse_desktop_file(&content) {
                        if !existing.contains(&info.name.to_lowercase()) {
                            candidates.push(DesktopCandidate {
                                path: path.to_string_lossy().to_string(),
                                name: info.name,
                                exec: info.exec,
                                icon: info.icon,
                            });
                        }
                    }
                }
            }
        }
    };

    for dir in &dirs {
        scan_dir(std::path::Path::new(dir), &mut candidates, &existing_names);
    }
    if let Some(ref home) = home_apps {
        scan_dir(home, &mut candidates, &existing_names);
    }

    candidates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    candidates
}

#[tauri::command]
pub fn bulk_import_desktop(
    state: State<ConfigState>,
    paths: Vec<String>,
) -> Result<Vec<AppEntry>, String> {
    let mut config = state.0.lock().unwrap();
    let mut imported = Vec::new();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path, e))?;
        let info = parse_desktop_file(&content)?;
        let resolved_icon = info.icon.and_then(|i| resolve_icon(i));
        let entry = AppEntry::new(
            info.name,
            Some(info.exec),
            None,
            Vec::new(),
            info.path,
            Vec::new(),
            resolved_icon,
            None,
        );
        config.apps.push(entry.clone());
        imported.push(entry);
    }

    save_config(&config)?;
    Ok(imported)
}

// ── Links ──────────────────────────────────────────────

#[tauri::command]
pub fn get_links(state: State<ConfigState>) -> Vec<LinkEntry> {
    let config = state.0.lock().unwrap();
    config.links.clone()
}

#[tauri::command]
pub fn add_link(
    state: State<ConfigState>,
    name: String,
    url: String,
    icon: Option<String>,
) -> Result<LinkEntry, String> {
    let mut config = state.0.lock().unwrap();
    let entry = LinkEntry::new(name, url, icon);
    config.links.push(entry.clone());
    save_config(&config)?;
    Ok(entry)
}

#[tauri::command]
pub fn update_link(
    state: State<ConfigState>,
    id: String,
    name: String,
    url: String,
    icon: Option<String>,
) -> Result<LinkEntry, String> {
    let mut config = state.0.lock().unwrap();
    let link = config.links.iter_mut().find(|l| l.id == id).ok_or("Link not found")?;
    link.name = name;
    link.url = url;
    link.icon = icon;
    let updated = link.clone();
    save_config(&config)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_link(state: State<ConfigState>, id: String) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    config.links.retain(|l| l.id != id);
    save_config(&config)
}

#[tauri::command]
pub fn reorder_links(state: State<ConfigState>, ids: Vec<String>) -> Result<(), String> {
    let mut config = state.0.lock().unwrap();
    let mut reordered = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(link) = config.links.iter().find(|l| &l.id == id) {
            reordered.push(link.clone());
        }
    }
    config.links = reordered;
    save_config(&config)
}

#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

// ── Desktop Entry ─────────────────────────────────────

#[tauri::command]
pub fn create_desktop_entry() -> Result<String, String> {
    // Prefer the installed path; fall back to current executable
    let installed = std::path::PathBuf::from("/usr/bin/raz");
    let exe_path = if installed.exists() {
        installed
    } else {
        std::env::current_exe()
            .map_err(|e| format!("Cannot determine executable path: {}", e))?
    };

    // Copy icon to ~/.local/share/icons/
    let data_dir = dirs::data_dir().ok_or("Cannot determine data directory")?;
    let icon_dir = data_dir.join("icons");
    std::fs::create_dir_all(&icon_dir).map_err(|e| format!("Failed to create icon dir: {}", e))?;
    let icon_dest = icon_dir.join("raz.png");
    let icon_bytes = include_bytes!("../icons/128x128.png");
    std::fs::write(&icon_dest, icon_bytes).map_err(|e| format!("Failed to write icon: {}", e))?;

    // Write .desktop file
    let apps_dir = data_dir.join("applications");
    std::fs::create_dir_all(&apps_dir).map_err(|e| format!("Failed to create applications dir: {}", e))?;
    let desktop_path = apps_dir.join("raz.desktop");

    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Raz\n\
         Comment=Personal launcher and homepage\n\
         Exec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupWMClass=raz\n",
        exe_path.display(),
        icon_dest.display(),
    );

    std::fs::write(&desktop_path, &content)
        .map_err(|e| format!("Failed to write .desktop file: {}", e))?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&desktop_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Also copy to ~/Desktop/ if it exists
    if let Some(home) = dirs::home_dir() {
        let desktop_dir = home.join("Desktop");
        if desktop_dir.is_dir() {
            let desktop_shortcut = desktop_dir.join("raz.desktop");
            std::fs::write(&desktop_shortcut, &content)
                .map_err(|e| format!("Failed to write desktop shortcut: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&desktop_shortcut, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }
        }
    }

    Ok("Desktop icon created".to_string())
}

// ── Settings ──────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<ConfigState>) -> Settings {
    let config = state.0.lock().unwrap();
    config.settings.clone()
}

#[tauri::command]
pub fn update_settings(
    state: State<ConfigState>,
    settings: Settings,
) -> Result<Settings, String> {
    let mut config = state.0.lock().unwrap();
    config.settings = settings;
    save_config(&config)?;
    Ok(config.settings.clone())
}
