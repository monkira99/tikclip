mod commands;
mod db;
mod sidecar;
mod tray;

use db::init::initialize_database;
use rusqlite::Connection;
use sidecar::SidecarManager;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub storage_path: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            sidecar::get_sidecar_status,
            commands::accounts::list_accounts,
            commands::accounts::create_account,
            commands::accounts::delete_account,
            commands::accounts::update_account_live_status,
            commands::clips::list_clips,
            commands::settings::get_setting,
            commands::settings::set_setting,
        ])
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("failed to get app data dir");
            let storage_path = app_data.join("TikTokApp");
            std::fs::create_dir_all(&storage_path).ok();

            let db_path = storage_path.join("data").join("app.db");
            let conn = initialize_database(&db_path).expect("failed to initialize database");

            app.manage(AppState {
                db: Mutex::new(conn),
                storage_path,
            });

            let sidecar = SidecarManager::new();
            if let Err(e) = sidecar.start() {
                eprintln!("sidecar start failed: {e}");
            }
            app.manage(sidecar);

            tray::setup_tray(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
