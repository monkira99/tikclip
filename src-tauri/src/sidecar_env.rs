//! Build `TIKCLIP_*` environment variables from SQLite `app_settings` plus app paths.
//! Process env overrides `sidecar/.env` (pydantic-settings precedence).

use rusqlite::Connection;
use std::path::Path;

fn get_setting_trimmed(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(val) => {
            let t = val.trim().to_string();
            Ok(if t.is_empty() { None } else { Some(t) })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

fn push_int_if_valid(
    env: &mut Vec<(String, String)>,
    conn: &Connection,
    db_key: &str,
    tikclip_key: &str,
) -> Result<(), String> {
    if let Some(t) = get_setting_trimmed(conn, db_key).map_err(|e| e.to_string())? {
        if t.parse::<i64>().is_err() {
            return Err(format!("{db_key} must be an integer, got {t:?}"));
        }
        env.push((tikclip_key.to_string(), t));
    }
    Ok(())
}

fn push_float_if_valid(
    env: &mut Vec<(String, String)>,
    conn: &Connection,
    db_key: &str,
    tikclip_key: &str,
) -> Result<(), String> {
    if let Some(t) = get_setting_trimmed(conn, db_key).map_err(|e| e.to_string())? {
        if t.parse::<f64>().is_err() {
            return Err(format!("{db_key} must be a number, got {t:?}"));
        }
        env.push((tikclip_key.to_string(), t));
    }
    Ok(())
}

/// Environment variables passed to the Python sidecar. Always includes `TIKCLIP_STORAGE_PATH`.
pub fn build_sidecar_env(conn: &Connection, storage_path: &Path) -> Result<Vec<(String, String)>, String> {
    let resolved = storage_path
        .canonicalize()
        .unwrap_or_else(|_| storage_path.to_path_buf());
    let mut env: Vec<(String, String)> = vec![(
        "TIKCLIP_STORAGE_PATH".to_string(),
        resolved.to_string_lossy().into_owned(),
    )];

    push_int_if_valid(
        &mut env,
        conn,
        "poll_interval",
        "TIKCLIP_POLL_INTERVAL_SECONDS",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "max_concurrent",
        "TIKCLIP_MAX_CONCURRENT_RECORDINGS",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "clip_min_duration",
        "TIKCLIP_CLIP_MIN_DURATION",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "clip_max_duration",
        "TIKCLIP_CLIP_MAX_DURATION",
    )?;
    push_float_if_valid(
        &mut env,
        conn,
        "max_storage_gb",
        "TIKCLIP_STORAGE_QUOTA_GB",
    )?;

    Ok(env)
}
