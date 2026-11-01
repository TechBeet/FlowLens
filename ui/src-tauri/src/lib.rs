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
            commands::export_har,
            commands::install_ca,
            commands::uninstall_ca,
            commands::check_ca_status,
            commands::get_ca_pem,
            commands::pause_session,
            commands::release_session,
            commands::modify_session,
            commands::list_paused_sessions,
            commands::add_rewrite_rule,
            commands::remove_rewrite_rule,
            commands::toggle_rewrite_rule,
            commands::list_rewrite_rules,
            commands::replay_request,
            commands::compare_sessions,
        ])
        .setup(|_app| {
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}