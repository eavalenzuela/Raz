mod commands;
mod config;
mod servers;
mod sidebar;

use config::{load_config, ConfigState};
use servers::ServerManager;
use sidebar::MonitorState;
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{image::Image, Listener, Manager, WindowEvent};

fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    // The background watcher in servers.rs reaps crashed processes; here we
    // just snapshot which ids are currently in the running map.
    let server_info: Vec<(String, String, String)> = {
        let config_state = app.try_state::<ConfigState>().unwrap();
        let manager = app.try_state::<ServerManager>().unwrap();
        let config = config_state.0.lock().unwrap();
        let running = manager.running.lock().unwrap();
        let last_exit = manager.last_exit.lock().unwrap();
        let running_ids: std::collections::HashSet<String> = running.keys().cloned().collect();
        config
            .servers
            .iter()
            .map(|s| {
                let state = if running_ids.contains(&s.id) {
                    "running"
                } else if last_exit.get(&s.id).map(|e| e.crashed).unwrap_or(false) {
                    "crashed"
                } else {
                    "stopped"
                };
                (s.id.clone(), s.name.clone(), state.to_string())
            })
            .collect()
    };

    let show = MenuItemBuilder::with_id("show", "Show Raz").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let mut builder = MenuBuilder::new(app)
        .item(&show)
        .item(&quit);

    if !server_info.is_empty() {
        builder = builder.separator();

        for (id, name, state) in &server_info {
            let icon = match state.as_str() {
                "running" => "\u{25CF}",
                "crashed" => "\u{2715}",
                _ => "\u{25CB}",
            };
            let action = if state == "running" { "stop" } else { "start" };
            let label = format!("{} {} \u{2014} {}", icon, name, action);
            let item = MenuItemBuilder::with_id(format!("server-{}", id), label)
                .build(app)?;
            builder = builder.item(&item);
        }
    }

    builder.build()
}

/// Start a stopped server / stop a running one from its tray menu entry.
/// Runs on a thread because stopping waits out the SIGTERM grace period.
fn toggle_server_from_tray(handle: &tauri::AppHandle, server_id: &str) {
    let app = handle.clone();
    let id = server_id.to_string();
    std::thread::spawn(move || {
        let is_running = {
            let manager = app.state::<ServerManager>();
            let running = manager.running.lock().unwrap();
            running.contains_key(&id)
        };
        let result = if is_running {
            servers::stop_server(app.state(), app.clone(), id)
        } else {
            servers::start_server(app.state(), app.state(), app.clone(), id)
        };
        if let Err(e) = result {
            eprintln!("Tray server toggle failed: {}", e);
        }
    });
}

/// Persist the main window's size/position into settings so the next launch
/// can restore it. Skipped while minimized — that geometry is meaningless.
fn persist_window_geometry(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(size), Ok(pos)) = (window.outer_size(), window.outer_position()) else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    let Some(state) = app.try_state::<ConfigState>() else { return };
    let snapshot = {
        let mut config = state.0.lock().unwrap();
        config.settings.window_width = Some(size.width);
        config.settings.window_height = Some(size.height);
        config.settings.window_x = Some(pos.x);
        config.settings.window_y = Some(pos.y);
        config.clone()
    };
    let _ = config::save_config(&snapshot);
}

/// Restore the last-persisted window geometry, if any.
fn restore_window_geometry(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let settings = {
        let Some(state) = app.try_state::<ConfigState>() else { return };
        let config = state.0.lock().unwrap();
        config.settings.clone()
    };
    if let (Some(w), Some(h)) = (settings.window_width, settings.window_height) {
        if w >= 200 && h >= 150 {
            let _ = window.set_size(tauri::PhysicalSize::new(w, h));
        }
    }
    if let (Some(x), Some(y)) = (settings.window_x, settings.window_y) {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn update_tray_menu(handle: &tauri::AppHandle) {
    if let Some(tray) = handle.tray_by_id("main-tray") {
        if let Ok(menu) = build_tray_menu(handle) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn restore_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = load_config();
    let config_state = ConfigState(Mutex::new(config));
    let server_manager = ServerManager::new();
    let monitor_state = MonitorState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(config_state)
        .manage(server_manager)
        .manage(monitor_state)
        .invoke_handler(tauri::generate_handler![
            commands::quit_app,
            commands::get_apps,
            commands::add_app,
            commands::update_app,
            commands::remove_app,
            commands::duplicate_app,
            commands::reorder_apps,
            commands::launch_app,
            commands::open_app_directory,
            commands::import_desktop_file,
            commands::add_app_from_desktop,
            commands::resolve_icon,
            commands::read_icon_base64,
            commands::scan_desktop_files,
            commands::bulk_import_desktop,
            commands::get_links,
            commands::add_link,
            commands::update_link,
            commands::remove_link,
            commands::reorder_links,
            commands::open_link,
            commands::fetch_favicon,
            commands::fetch_url_metadata,
            servers::get_servers,
            servers::add_server,
            servers::update_server,
            servers::remove_server,
            servers::duplicate_server,
            servers::start_server,
            servers::stop_server,
            servers::restart_server,
            servers::get_server_output,
            servers::get_all_server_statuses,
            servers::clear_server_log,
            servers::export_server_log,
            servers::get_server_resources,
            servers::open_server_directory,
            sidebar::get_pinned,
            sidebar::pin_item,
            sidebar::unpin_item,
            sidebar::get_status_monitors,
            sidebar::add_status_monitor,
            sidebar::update_status_monitor,
            sidebar::remove_status_monitor,
            sidebar::set_monitor_enabled,
            sidebar::get_monitor_statuses,
            commands::get_settings,
            commands::update_settings,
            commands::create_desktop_entry,
            commands::export_config,
            commands::import_config,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Restore last window size/position before anything is visible.
            restore_window_geometry(&handle);

            // Auto-launch servers and start monitor loop
            let config_state = handle.try_state::<ConfigState>().unwrap();
            let manager = handle.try_state::<ServerManager>().unwrap();
            let monitor_state = handle.try_state::<MonitorState>().unwrap();
            servers::auto_launch_servers(&config_state, &manager, &handle);
            servers::start_server_watcher(&handle);
            sidebar::start_monitor_loop(&config_state, &monitor_state, &handle);

            // Build tray menu with server statuses
            let menu = build_tray_menu(&handle)?;

            // Build tray icon
            let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

            let handle_for_menu = handle.clone();
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Raz")
                .menu(&menu)
                .on_menu_event(move |_app: &tauri::AppHandle, event| {
                    match event.id().as_ref() {
                        "show" => {
                            restore_window(&handle_for_menu);
                        }
                        "quit" => {
                            persist_window_geometry(&handle_for_menu);
                            // Don't orphan managed servers on quit.
                            servers::shutdown_all_servers(&handle_for_menu);
                            std::process::exit(0);
                        }
                        id if id.starts_with("server-") => {
                            let server_id = id.trim_start_matches("server-");
                            toggle_server_from_tray(&handle_for_menu, server_id);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        restore_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // Listen for server state changes to update the tray menu
            let handle_for_tray = handle.clone();
            handle.listen("tray-update", move |_| {
                update_tray_menu(&handle_for_tray);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Capture geometry before minimizing/closing so the next
                // launch opens where the user left the window.
                persist_window_geometry(window.app_handle());
                let minimize = window
                    .app_handle()
                    .try_state::<ConfigState>()
                    .map(|s| s.0.lock().unwrap().settings.minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    api.prevent_close();
                    let _ = window.minimize();
                    let _ = window.set_skip_taskbar(true);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            // Covers the exit path where the window is closed with
            // minimize-to-tray disabled — don't orphan managed servers.
            if let tauri::RunEvent::Exit = event {
                servers::shutdown_all_servers(app_handle);
            }
        });
}
