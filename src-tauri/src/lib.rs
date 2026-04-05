mod db;
mod sidecar;

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
        .invoke_handler(tauri::generate_handler![sidecar::get_sidecar_status])
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
