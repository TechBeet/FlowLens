mod commands;
mod state;

use flowlens_core::BroadcastEventBus;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let event_bus = BroadcastEventBus::new(1024);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state::AppState::new(event_bus))
        .invoke_handler(tauri::generate_handler![
            commands::start_capture,
            commands::stop_capture,
            commands::get_stats,
            commands::list_devices,
        ])
        .setup(|_app| {
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}