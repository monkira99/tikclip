use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "storage_root.json";

#[derive(Debug, Serialize, Deserialize)]
struct StorageRootFile {
    root: String,
}

pub fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(CONFIG_FILENAME)
}

pub fn custom_root_config_exists(app_config_dir: &Path) -> bool {
    config_path(app_config_dir).is_file()
}

fn read_custom_root(app_config_dir: &Path) -> Option<PathBuf> {
    let p = config_path(app_config_dir);
    let text = fs::read_to_string(&p).ok()?;
    let cfg: StorageRootFile = serde_json::from_str(&text).ok()?;
    let root = PathBuf::from(cfg.root.trim());
    root.is_absolute().then_some(root)
}

/// Persisted override lives in `app_config_dir/storage_root.json` (outside the data tree).
pub fn write_custom_root(app_config_dir: &Path, root: PathBuf) -> Result<(), String> {
    fs::create_dir_all(app_config_dir).map_err(|e| e.to_string())?;
    let cfg = StorageRootFile {
        root: root.to_string_lossy().into_owned(),
    };
    fs::write(
        config_path(app_config_dir),
        serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn clear_custom_root(app_config_dir: &Path) -> Result<(), String> {
    let path = config_path(app_config_dir);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resolve data root: optional JSON override, else `~/.tikclip` with legacy `TikTokApp` migration.
pub fn resolve_storage_root(
    home_dir: PathBuf,
    app_data_dir: PathBuf,
    app_config_dir: PathBuf,
) -> Result<PathBuf, String> {
    if let Some(custom) = read_custom_root(&app_config_dir) {
        ensure_layout_dirs(&custom)?;
        return Ok(custom);
    }

    resolve_default_tikclip_with_legacy_migration(home_dir, app_data_dir)
}

fn ensure_layout_dirs(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("clips")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("records")).map_err(|e| e.to_string())?;
    Ok(())
}

/// Default data root: `~/.tikclip`. If an older install only has data under app data `TikTokApp/`,
/// copy that tree here once so existing users keep their DB and files.
fn resolve_default_tikclip_with_legacy_migration(
    home_dir: PathBuf,
    app_data_dir: PathBuf,
) -> Result<PathBuf, String> {
    let new_root = home_dir.join(".tikclip");
    let legacy = app_data_dir.join("TikTokApp");
    let new_db = new_root.join("data").join("app.db");
    let legacy_db = legacy.join("data").join("app.db");

    if new_db.is_file() {
        ensure_layout_dirs(&new_root)?;
        return Ok(new_root);
    }

    if legacy_db.is_file() {
        fs::create_dir_all(&new_root).map_err(|e| e.to_string())?;
        copy_dir_all(&legacy, &new_root).map_err(|e| e.to_string())?;
        log::info!(
            "TikClip: migrated storage from {} to {}",
            legacy.display(),
            new_root.display()
        );
        ensure_layout_dirs(&new_root)?;
        return Ok(new_root);
    }

    fs::create_dir_all(new_root.join("data")).map_err(|e| e.to_string())?;
    ensure_layout_dirs(&new_root)?;
    Ok(new_root)
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "copy_dir_all: source is not a directory",
        ));
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}
