mod app_paths;
mod commands;
mod db;
mod live_runtime;
mod recording_runtime;
mod tiktok;
mod time_hcm;
mod tray;
mod workflow;

use db::init::initialize_database;
use live_runtime::manager::LiveRuntimeManager;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, RunEvent};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub storage_path: PathBuf,
}

fn init_rust_logging() {
    let default_filter = "warn,tikclip_lib=info";
    let _ =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
            .format_timestamp_millis()
            .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_rust_logging();
    log::info!("app bootstrap started");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::accounts::list_accounts,
            commands::accounts::create_account,
            commands::accounts::delete_account,
            commands::accounts::sync_accounts_live_status,
            commands::clips::update_clip_caption,
            commands::clips::generate_clip_caption,
            commands::clips::suggest_product_for_clip,
            commands::products::list_products,
            commands::products::create_product,
            commands::products::update_product,
            commands::products::delete_product,
            commands::products::tag_clip_product,
            commands::products::fetch_product_from_url,
            commands::products::index_product_embeddings,
            commands::products::delete_product_embeddings,
            commands::dashboard::get_dashboard_stats,
            commands::flows::list_flows,
            commands::flows::create_flow,
            commands::flows::apply_flow_runtime_hint,
            commands::flows::set_flow_enabled,
            commands::flows::delete_flow,
            commands::flow_engine::get_flow_definition,
            commands::flow_engine::save_flow_node_draft,
            commands::flow_engine::publish_flow_definition,
            commands::flow_engine::restart_flow_run,
            commands::live_runtime::list_live_runtime_sessions,
            commands::live_runtime::list_live_runtime_logs,
            commands::live_runtime::trigger_start_live_detected,
            commands::live_runtime::mark_start_run_completed,
            commands::live_runtime::mark_source_offline,
            commands::recordings::list_active_rust_recordings,
            commands::notifications::insert_notification,
            commands::notifications::list_notifications,
            commands::notifications::mark_notification_read,
            commands::notifications::mark_all_notifications_read,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::storage::get_storage_stats,
            commands::storage::list_activity_feed,
            commands::storage::run_storage_cleanup_now,
            commands::paths::get_app_data_paths,
            commands::paths::open_path,
            commands::paths::storage_root_is_custom,
            commands::paths::pick_storage_root_folder,
            commands::paths::apply_storage_root,
            commands::paths::reset_storage_root_default,
        ])
        .setup(|app| {
            log::info!("tauri setup started");
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
            log::info!("storage root resolved: {}", storage_path.display());

            let db_path = storage_path.join("data").join("app.db");
            log::info!("initializing database: {}", db_path.display());
            let mut conn = initialize_database(&db_path).expect("failed to initialize database");
            let mut runtime_manager = LiveRuntimeManager::with_runtime_db_path(db_path.clone());
            runtime_manager.attach_storage_root(storage_path.clone());
            runtime_manager.attach_app_handle(app.handle().clone());
            log::info!("bootstrapping enabled live runtime flows");
            if let Err(err) = runtime_manager.bootstrap_enabled_flows(&mut conn) {
                log::warn!("failed to bootstrap enabled live runtime flows: {}", err);
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                storage_path: storage_path.clone(),
            });
            app.manage(runtime_manager);
            app.manage(commands::storage::StorageCleanupWorker::start(
                app.handle().clone(),
                db_path.clone(),
                storage_path.clone(),
            ));

            tray::setup_tray(app.handle())?;
            log::info!("tauri setup completed");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            log::info!("app exit requested; shutting down workers");
            let cleanup_worker = app_handle.state::<commands::storage::StorageCleanupWorker>();
            cleanup_worker.shutdown();
            let runtime_manager = app_handle.state::<LiveRuntimeManager>();
            let _ = runtime_manager.shutdown();
            log::info!("app shutdown completed");
        }
    });
}
