mod app_paths;
mod commands;
mod db;
mod sidecar;
mod sidecar_env;
mod time_hcm;
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

fn init_rust_logging() {
    let default_filter = if cfg!(debug_assertions) {
        "warn,tikclip_lib::commands::accounts=info"
    } else {
        "warn,tikclip_lib::commands::accounts=warn"
    };
    let _ =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
            .format_timestamp_millis()
            .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_rust_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            sidecar::get_sidecar_status,
            sidecar::restart_sidecar,
            commands::accounts::list_accounts,
            commands::accounts::create_account,
            commands::accounts::delete_account,
            commands::accounts::update_account_live_status,
            commands::accounts::sync_accounts_live_status,
            commands::clips::insert_clip_from_sidecar,
            commands::clips::list_clips,
            commands::clips::list_clips_filtered,
            commands::clips::get_clip_by_id,
            commands::clips::update_clip_status,
            commands::clips::update_clip_title,
            commands::clips::update_clip_notes,
            commands::clips::batch_update_clip_status,
            commands::clips::batch_delete_clips,
            commands::clips::insert_trimmed_clip,
            commands::products::list_products,
            commands::products::get_product_by_id,
            commands::products::create_product,
            commands::products::update_product,
            commands::products::delete_product,
            commands::products::list_clip_products,
            commands::products::tag_clip_product,
            commands::products::untag_clip_product,
            commands::products::batch_tag_clip_products,
            commands::dashboard::get_dashboard_stats,
            commands::notifications::insert_notification,
            commands::notifications::list_notifications,
            commands::notifications::mark_notification_read,
            commands::notifications::mark_all_notifications_read,
            commands::recordings::sync_recording_from_sidecar,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::storage::delete_recording_files,
            commands::storage::list_recordings_for_cleanup,
            commands::storage::list_activity_feed,
            commands::paths::get_app_data_paths,
            commands::paths::open_path,
            commands::paths::storage_root_is_custom,
            commands::paths::pick_storage_root_folder,
            commands::paths::apply_storage_root,
            commands::paths::reset_storage_root_default,
        ])
        .setup(|app| {
            let home_dir = app.path().home_dir().expect("failed to get home dir");
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("failed to get app config dir");
            let storage_path =
                app_paths::resolve_storage_root(home_dir, app_data_dir, app_config_dir)
                    .expect("failed to resolve storage path");
            std::fs::create_dir_all(&storage_path).ok();

            let db_path = storage_path.join("data").join("app.db");
            let conn = initialize_database(&db_path).expect("failed to initialize database");
            let tikclip_env = sidecar_env::build_sidecar_env(&conn, &storage_path)
                .expect("failed to build sidecar env from settings");

            app.manage(AppState {
                db: Mutex::new(conn),
                storage_path: storage_path.clone(),
            });

            let sidecar = SidecarManager::new();
            if let Err(e) = sidecar.start(&tikclip_env) {
                eprintln!("sidecar start failed: {e}");
            }
            app.manage(sidecar);

            tray::setup_tray(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
