use crate::config::{save_snapshot, AppEntry, ConfigState, EnvVar, LinkEntry, Settings};
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
    let entry = AppEntry::new(name, raw_command, executable, arguments, working_directory, env_vars, icon, type_label);
    {
        let mut config = state.0.lock().unwrap();
        config.apps.push(entry.clone());
    }
    save_snapshot(&state)?;
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
    let updated = {
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
        app.clone()
    };
    save_snapshot(&state)?;
    Ok(updated)
}

#[tauri::command]
pub fn remove_app(state: State<ConfigState>, id: String) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        config.apps.retain(|a| a.id != id);
    }
    save_snapshot(&state)
}

#[tauri::command]
pub fn reorder_apps(state: State<ConfigState>, ids: Vec<String>) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        let id_set: std::collections::HashSet<&String> = ids.iter().collect();
        let mut reordered = Vec::with_capacity(config.apps.len());
        for id in &ids {
            if let Some(app) = config.apps.iter().find(|a| &a.id == id) {
                reordered.push(app.clone());
            }
        }
        // Preserve any entries the caller didn't include (defensive against
        // a stale id list from the frontend racing with an add).
        for app in &config.apps {
            if !id_set.contains(&app.id) {
                reordered.push(app.clone());
            }
        }
        config.apps = reordered;
    }
    save_snapshot(&state)
}

#[tauri::command]
pub fn launch_app(state: State<ConfigState>, id: String) -> Result<(), String> {
    let app = {
        let config = state.0.lock().unwrap();
        config.apps.iter().find(|a| a.id == id).ok_or("App not found")?.clone()
    };

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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    {
        let mut config = state.0.lock().unwrap();
        if let Some(app) = config.apps.iter_mut().find(|a| a.id == id) {
            app.launch_count += 1;
            app.last_launched = Some(now);
        }
    }
    let _ = save_snapshot(&state);

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

/// Walk known icon directories once and build a `stem -> (priority, path)` map.
/// Priority orders results so larger / preferred sizes win for the same name.
fn build_icon_index() -> HashMap<String, (u32, String)> {
    let mut index: HashMap<String, (u32, String)> = HashMap::new();

    // Lower number = higher priority.
    let size_priority = |size: &str| -> u32 {
        match size {
            "scalable" => 5,
            "256x256" => 10,
            "128x128" => 20,
            "96x96" => 30,
            "64x64" => 40,
            "48x48" => 50,
            "32x32" => 60,
            "24x24" => 70,
            "16x16" => 80,
            _ => 100,
        }
    };
    let ext_priority = |ext: &str| -> u32 {
        match ext {
            "svg" => 0,
            "png" => 1,
            "xpm" => 2,
            "ico" => 3,
            _ => 9,
        }
    };

    let consider = |stem: &str, ext: &str, size: &str, full: String, index: &mut HashMap<String, (u32, String)>| {
        let prio = size_priority(size) * 10 + ext_priority(ext);
        match index.get(stem) {
            Some((existing_prio, _)) if *existing_prio <= prio => {}
            _ => { index.insert(stem.to_string(), (prio, full)); }
        }
    };

    let mut roots: Vec<std::path::PathBuf> = vec![
        std::path::PathBuf::from("/usr/share/icons"),
        std::path::PathBuf::from("/usr/share/pixmaps"),
        std::path::PathBuf::from("/usr/local/share/icons"),
    ];
    if let Some(d) = dirs::data_dir() {
        roots.push(d.join("icons"));
    }

    for root in &roots {
        let Ok(top) = std::fs::read_dir(root) else { continue };
        // For pixmaps-style flat dirs, files live directly under root.
        // For theme dirs, structure is theme/size/category/file.
        for entry in top.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let (Some(stem), Some(ext)) = (path.file_stem().and_then(|s| s.to_str()), path.file_name().and_then(|n| n.to_str()).and_then(|n| n.rsplit('.').next())) {
                    consider(stem, ext, "scalable", path.to_string_lossy().to_string(), &mut index);
                }
                continue;
            }
            // Theme dir — descend size/category.
            let Ok(sizes) = std::fs::read_dir(&path) else { continue };
            for size_entry in sizes.flatten() {
                let size_path = size_entry.path();
                let size = size_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let Ok(cats) = std::fs::read_dir(&size_path) else { continue };
                for cat_entry in cats.flatten() {
                    let cat_path = cat_entry.path();
                    let Ok(files) = std::fs::read_dir(&cat_path) else { continue };
                    for file_entry in files.flatten() {
                        let file_path = file_entry.path();
                        if !file_path.is_file() { continue; }
                        let stem = match file_path.file_stem().and_then(|s| s.to_str()) {
                            Some(s) => s.to_string(),
                            None => continue,
                        };
                        let ext = match file_path.extension().and_then(|s| s.to_str()) {
                            Some(s) => s.to_string(),
                            None => continue,
                        };
                        if !matches!(ext.as_str(), "png" | "svg" | "xpm" | "ico") { continue; }
                        consider(&stem, &ext, &size, file_path.to_string_lossy().to_string(), &mut index);
                    }
                }
            }
        }
    }

    index
}

#[tauri::command]
pub fn resolve_icon(icon: String) -> Option<String> {
    let path = std::path::PathBuf::from(&icon);
    if path.is_absolute() && path.exists() {
        return Some(icon);
    }

    use std::sync::OnceLock;
    static INDEX: OnceLock<HashMap<String, (u32, String)>> = OnceLock::new();
    let index = INDEX.get_or_init(build_icon_index);
    index.get(&icon).map(|(_, p)| p.clone())
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

    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{};base64,{}", mime, encoded))
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
    let resolved_icon = info.icon.and_then(resolve_icon);

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
    {
        let mut config = state.0.lock().unwrap();
        config.apps.push(entry.clone());
    }
    save_snapshot(&state)?;
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
    let mut imported = Vec::new();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path, e))?;
        let info = parse_desktop_file(&content)?;
        let resolved_icon = info.icon.and_then(resolve_icon);
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
        {
            let mut config = state.0.lock().unwrap();
            config.apps.push(entry.clone());
        }
        imported.push(entry);
    }

    save_snapshot(&state)?;
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
    folder: Option<String>,
) -> Result<LinkEntry, String> {
    let entry = LinkEntry::new(name, url, icon, folder);
    {
        let mut config = state.0.lock().unwrap();
        config.links.push(entry.clone());
    }
    save_snapshot(&state)?;
    Ok(entry)
}

#[tauri::command]
pub fn update_link(
    state: State<ConfigState>,
    id: String,
    name: String,
    url: String,
    icon: Option<String>,
    folder: Option<String>,
) -> Result<LinkEntry, String> {
    let updated = {
        let mut config = state.0.lock().unwrap();
        let link = config.links.iter_mut().find(|l| l.id == id).ok_or("Link not found")?;
        link.name = name;
        link.url = url;
        link.icon = icon;
        link.folder = folder;
        link.clone()
    };
    save_snapshot(&state)?;
    Ok(updated)
}

#[tauri::command]
pub fn fetch_favicon(url: String) -> Result<String, String> {
    let hostname = url::Url::parse(&url)
        .map_err(|e| format!("Invalid URL: {}", e))?
        .host_str()
        .ok_or("No host in URL")?
        .to_string();

    let cache_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("raz")
        .join("favicons");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let dest = cache_dir.join(format!("{}.png", hostname));

    // Return cached if exists
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }

    // Try Google favicon service
    let favicon_url = format!("https://www.google.com/s2/favicons?domain={}&sz=64", hostname);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&favicon_url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err("Failed to fetch favicon".to_string());
    }

    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn fetch_url_metadata(url: String) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err("Failed to fetch URL".to_string());
    }

    let body = resp.text().map_err(|e| e.to_string())?;

    // Extract <title>
    if let Some(start) = body.find("<title>").or_else(|| body.find("<Title>")).or_else(|| body.find("<TITLE>")) {
        let after = &body[start + 7..];
        if let Some(end) = after.find("</title>").or_else(|| after.find("</Title>")).or_else(|| after.find("</TITLE>")) {
            let title = after[..end].trim().to_string();
            if !title.is_empty() {
                return Ok(title);
            }
        }
    }

    Err("No title found".to_string())
}

#[tauri::command]
pub fn remove_link(state: State<ConfigState>, id: String) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        config.links.retain(|l| l.id != id);
    }
    save_snapshot(&state)
}

#[tauri::command]
pub fn reorder_links(state: State<ConfigState>, ids: Vec<String>) -> Result<(), String> {
    {
        let mut config = state.0.lock().unwrap();
        let id_set: std::collections::HashSet<&String> = ids.iter().collect();
        let mut reordered = Vec::with_capacity(config.links.len());
        for id in &ids {
            if let Some(link) = config.links.iter().find(|l| &l.id == id) {
                reordered.push(link.clone());
            }
        }
        for link in &config.links {
            if !id_set.contains(&link.id) {
                reordered.push(link.clone());
            }
        }
        config.links = reordered;
    }
    save_snapshot(&state)
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
    let result = {
        let mut config = state.0.lock().unwrap();
        config.settings = settings;
        config.settings.clone()
    };
    save_snapshot(&state)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_desktop_entry() {
        let raw = "[Desktop Entry]\nType=Application\nName=Foo\nExec=/usr/bin/foo --bar\nIcon=foo\n";
        let info = parse_desktop_file(raw).unwrap();
        assert_eq!(info.name, "Foo");
        assert_eq!(info.exec, "/usr/bin/foo --bar");
        assert_eq!(info.icon.as_deref(), Some("foo"));
    }

    #[test]
    fn strips_field_codes() {
        let raw = "[Desktop Entry]\nName=Foo\nExec=/usr/bin/foo %F %u %i\n";
        let info = parse_desktop_file(raw).unwrap();
        assert_eq!(info.exec, "/usr/bin/foo");
    }

    #[test]
    fn unescapes_desktop_escapes() {
        let raw = "[Desktop Entry]\nName=Foo\nExec=/usr/bin/foo\\sbar\\\\baz\n";
        let info = parse_desktop_file(raw).unwrap();
        // \s -> space, \\ -> backslash
        assert_eq!(info.exec, "/usr/bin/foo bar\\baz");
    }

    #[test]
    fn ignores_other_groups() {
        let raw = "[Desktop Entry]\nName=Foo\nExec=foo\n[Desktop Action open]\nName=Should Not Win\nExec=other\n";
        let info = parse_desktop_file(raw).unwrap();
        assert_eq!(info.name, "Foo");
        assert_eq!(info.exec, "foo");
    }

    #[test]
    fn errors_on_missing_required_fields() {
        let raw = "[Desktop Entry]\nName=Foo\n";
        assert!(parse_desktop_file(raw).is_err());
    }
}
