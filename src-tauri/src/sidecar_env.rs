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

fn push_bool_setting(
    env: &mut Vec<(String, String)>,
    conn: &Connection,
    db_key: &str,
    tikclip_key: &str,
) -> Result<(), String> {
    if let Some(t) = get_setting_trimmed(conn, db_key).map_err(|e| e.to_string())? {
        let lower = t.to_ascii_lowercase();
        let enabled = matches!(lower.as_str(), "1" | "true" | "yes" | "on");
        env.push((
            tikclip_key.to_string(),
            if enabled {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ));
    }
    Ok(())
}

/// Environment variables passed to the Python sidecar. Always includes `TIKCLIP_STORAGE_PATH`.
pub fn build_sidecar_env(
    conn: &Connection,
    storage_path: &Path,
) -> Result<Vec<(String, String)>, String> {
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
    if let Some(t) =
        get_setting_trimmed(conn, "recording_max_minutes").map_err(|e| e.to_string())?
    {
        if t.parse::<i64>().is_err() {
            return Err(format!(
                "recording_max_minutes must be an integer, got {t:?}"
            ));
        }
        env.push(("TIKCLIP_MAX_DURATION_MINUTES".to_string(), t));
    } else if let Some(h) =
        get_setting_trimmed(conn, "recording_max_hours").map_err(|e| e.to_string())?
    {
        // Legacy Settings key (hours) → minutes for sidecar env.
        if let Ok(hours) = h.parse::<i64>() {
            if hours > 0 {
                env.push((
                    "TIKCLIP_MAX_DURATION_MINUTES".to_string(),
                    (hours * 60).to_string(),
                ));
            }
        } else {
            return Err(format!("recording_max_hours must be an integer, got {h:?}"));
        }
    }
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
    push_float_if_valid(&mut env, conn, "max_storage_gb", "TIKCLIP_STORAGE_QUOTA_GB")?;
    push_bool_setting(
        &mut env,
        conn,
        "auto_process_after_record",
        "TIKCLIP_AUTO_PROCESS_AFTER_RECORD",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "TIKCLIP_RAW_RETENTION_DAYS",
        "TIKCLIP_RAW_RETENTION_DAYS",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "TIKCLIP_ARCHIVE_RETENTION_DAYS",
        "TIKCLIP_ARCHIVE_RETENTION_DAYS",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "TIKCLIP_STORAGE_WARN_PERCENT",
        "TIKCLIP_STORAGE_WARN_PERCENT",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "TIKCLIP_STORAGE_CLEANUP_PERCENT",
        "TIKCLIP_STORAGE_CLEANUP_PERCENT",
    )?;

    push_bool_setting(
        &mut env,
        conn,
        "product_vector_enabled",
        "TIKCLIP_PRODUCT_VECTOR_ENABLED",
    )?;
    if let Some(t) = get_setting_trimmed(conn, "gemini_api_key").map_err(|e| e.to_string())? {
        env.push(("TIKCLIP_GEMINI_API_KEY".to_string(), t));
    }
    if let Some(t) =
        get_setting_trimmed(conn, "gemini_embedding_model").map_err(|e| e.to_string())?
    {
        env.push(("TIKCLIP_GEMINI_EMBEDDING_MODEL".to_string(), t));
    }
    push_int_if_valid(
        &mut env,
        conn,
        "gemini_embedding_dimensions",
        "TIKCLIP_GEMINI_EMBEDDING_DIMENSIONS",
    )?;

    push_bool_setting(
        &mut env,
        conn,
        "auto_tag_clip_product_enabled",
        "TIKCLIP_AUTO_TAG_CLIP_PRODUCT_ENABLED",
    )?;
    push_int_if_valid(
        &mut env,
        conn,
        "auto_tag_clip_frame_count",
        "TIKCLIP_AUTO_TAG_CLIP_FRAME_COUNT",
    )?;
    push_float_if_valid(
        &mut env,
        conn,
        "auto_tag_clip_max_score",
        "TIKCLIP_AUTO_TAG_CLIP_MAX_SCORE",
    )?;

    Ok(env)
}
