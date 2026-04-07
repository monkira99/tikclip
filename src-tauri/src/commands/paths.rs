use crate::app_paths;
use crate::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State, WebviewWindow};

#[derive(Debug, Serialize)]
pub struct AppDataPaths {
    /// Single configured root (`TIKCLIP_STORAGE_PATH`).
    pub storage_root: String,
    /// `{root}/data` — SQLite (`app.db`).
    pub data_dir: String,
    /// `{root}/clips/{username}/{date}/*.mp4`
    pub clips_dir: String,
    /// `{root}/records/{username}/{date}/*.mp4` (raw recordings, stream copy).
    pub records_dir: String,
}

#[tauri::command]
pub fn get_app_data_paths(state: State<'_, AppState>) -> Result<AppDataPaths, String> {
    let root = state
        .storage_path
        .canonicalize()
        .unwrap_or_else(|_| state.storage_path.clone());
    let data = root.join("data");
    let clips = root.join("clips");
    let records = root.join("records");
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&clips).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&records).map_err(|e| e.to_string())?;

    Ok(AppDataPaths {
        storage_root: path_display(&root),
        data_dir: path_display(&data),
        clips_dir: path_display(&clips),
        records_dir: path_display(&records),
    })
}

fn path_display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Opens a file or folder in the system file manager / default handler (no webview shell plugin).
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".to_string());
    }
    let p = PathBuf::from(trimmed);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", p.display()));
    }
    open::that(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn storage_root_is_custom(app: AppHandle) -> Result<bool, String> {
    let cfg_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(app_paths::custom_root_config_exists(&cfg_dir))
}

#[tauri::command]
pub fn pick_storage_root_folder(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Chọn thư mục gốc TikClip")
        .blocking_pick_folder();
    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp.into_path().map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Writes `storage_root.json` and restarts the app so DB and sidecar reload from the new root.
#[tauri::command]
pub fn apply_storage_root(app: AppHandle, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Đường dẫn trống".to_string());
    }
    let pb = PathBuf::from(trimmed);
    let canonical = std::fs::canonicalize(&pb).map_err(|e| e.to_string())?;
    if !canonical.is_dir() {
        return Err("Không phải thư mục hợp lệ".to_string());
    }
    let app_config = app.path().app_config_dir().map_err(|e| e.to_string())?;
    app_paths::write_custom_root(&app_config, canonical)?;
    app.restart();
}

/// Removes the override and restarts; next launch uses `~/.tikclip` (and legacy migration if needed).
#[tauri::command]
pub fn reset_storage_root_default(app: AppHandle) -> Result<(), String> {
    let app_config = app.path().app_config_dir().map_err(|e| e.to_string())?;
    app_paths::clear_custom_root(&app_config)?;
    app.restart();
}
