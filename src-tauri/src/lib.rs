mod commands;
mod config;
mod servers;
mod sidebar;

use config::{load_config, ConfigState};
use servers::ServerManager;
use sidebar::MonitorState;
use std::sync::Mutex;
use tauri::Manager;

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
            commands::get_links,
            commands::add_link,
            commands::update_link,
            commands::remove_link,
            commands::reorder_links,
            commands::open_link,
            servers::get_servers,
            servers::add_server,
            servers::update_server,
            servers::remove_server,
            servers::start_server,
            servers::stop_server,
            servers::get_server_output,
            servers::get_all_server_statuses,
            sidebar::get_pinned,
            sidebar::pin_item,
            sidebar::unpin_item,
            sidebar::get_status_monitors,
            sidebar::add_status_monitor,
            sidebar::update_status_monitor,
            sidebar::remove_status_monitor,
            sidebar::get_monitor_statuses,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let config_state = handle.try_state::<ConfigState>().unwrap();
            let manager = handle.try_state::<ServerManager>().unwrap();
            let monitor_state = handle.try_state::<MonitorState>().unwrap();
            servers::auto_launch_servers(&config_state, &manager, &handle);
            sidebar::start_monitor_loop(&config_state, &monitor_state, &handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
