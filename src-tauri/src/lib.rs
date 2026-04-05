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
    // Collect server info while holding locks, then drop before building menu items
    let server_info: Vec<(String, String, String)> = {
        let config_state = app.try_state::<ConfigState>().unwrap();
        let manager = app.try_state::<ServerManager>().unwrap();
        let config = config_state.0.lock().unwrap();

        let mut status_map = std::collections::HashMap::new();
        {
            let mut running = manager.0.lock().unwrap();
            let mut crashed = Vec::new();
            for (id, server) in running.iter_mut() {
                match server.child.try_wait() {
                    Ok(Some(_)) => {
                        crashed.push(id.clone());
                        status_map.insert(id.clone(), "crashed");
                    }
                    Ok(None) => {
                        status_map.insert(id.clone(), "running");
                    }
                    Err(_) => {
                        crashed.push(id.clone());
                        status_map.insert(id.clone(), "crashed");
                    }
                }
            }
            for id in crashed {
                running.remove(&id);
            }
        }

        config.servers.iter().map(|s| {
            let state = status_map.get(&s.id).map(|s| s.to_string()).unwrap_or_else(|| "stopped".to_string());
            (s.id.clone(), s.name.clone(), state)
        }).collect()
    };
    // All locks dropped here

    let show = MenuItemBuilder::with_id("show", "Show Raz").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let mut builder = MenuBuilder::new(app)
        .item(&show)
        .item(&quit);

    if !server_info.is_empty() {
        builder = builder.separator();

        for (id, name, state) in &server_info {
            let icon = if state == "running" { "\u{25CF}" } else { "\u{25CB}" };
            let label = format!("{} {}", icon, name);
            let item = MenuItemBuilder::with_id(format!("server-{}", id), label)
                .enabled(false)
                .build(app)?;
            builder = builder.item(&item);
        }
    }

    builder.build()
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
            servers::start_server,
            servers::stop_server,
            servers::get_server_output,
            servers::get_all_server_statuses,
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
            sidebar::get_monitor_statuses,
            commands::get_settings,
            commands::update_settings,
            commands::create_desktop_entry,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Auto-launch servers and start monitor loop
            let config_state = handle.try_state::<ConfigState>().unwrap();
            let manager = handle.try_state::<ServerManager>().unwrap();
            let monitor_state = handle.try_state::<MonitorState>().unwrap();
            servers::auto_launch_servers(&config_state, &manager, &handle);
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
                            std::process::exit(0);
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
