use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub(crate) fn resolve_storage_media_path(
    storage_root: &Path,
    raw: &str,
) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Media path is required".to_string());
    }
    let root = storage_root
        .canonicalize()
        .map_err(|e| format!("Could not resolve storage root: {e}"))?;
    let raw_path = expand_home_path(trimmed);
    let candidate = if raw_path.is_absolute() {
        raw_path
    } else {
        root.join(raw_path)
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|e| format!("Could not resolve media path: {e}"))?;
    if !resolved.starts_with(&root) {
        return Err("Media path must be under storage root".to_string());
    }
    Ok(resolved)
}

pub(crate) fn storage_relative(storage_root: &Path, path: &Path) -> String {
    let root = storage_root
        .canonicalize()
        .unwrap_or_else(|_| storage_root.to_path_buf());
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    resolved
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned())
        })
}

fn expand_home_path(raw: &str) -> PathBuf {
    if raw == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(raw)
}

pub(crate) fn string_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let value = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

pub(crate) fn bool_setting(conn: &Connection, key: &str, default: bool) -> Result<bool, String> {
    let Some(value) = string_setting(conn, key)? else {
        return Ok(default);
    };
    let lower = value.to_ascii_lowercase();
    Ok(matches!(lower.as_str(), "1" | "true" | "yes" | "on"))
}

pub(crate) fn int_setting(conn: &Connection, key: &str, default: i64) -> Result<i64, String> {
    let Some(value) = string_setting(conn, key)? else {
        return Ok(default);
    };
    Ok(value.parse::<i64>().unwrap_or(default))
}

pub(crate) fn float_setting(conn: &Connection, key: &str, default: f64) -> Result<f64, String> {
    let Some(value) = string_setting(conn, key)? else {
        return Ok(default);
    };
    Ok(value.parse::<f64>().unwrap_or(default))
}
