mod api_channels;
mod codex;
mod commands;
mod models;
mod quota;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::import_current_codex_account,
            commands::start_codex_login,
            commands::add_api_key_account,
            commands::add_token_account,
            commands::add_api_channel,
            commands::update_api_channel,
            commands::delete_api_channel,
            commands::switch_api_channel,
            commands::switch_account,
            commands::delete_account,
            commands::update_account_label,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::launch_instance,
            commands::stop_instance,
            commands::open_path,
            commands::get_instance_launch_command,
            commands::refresh_account_quota,
            commands::refresh_all_quotas
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
