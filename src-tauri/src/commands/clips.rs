use crate::time_hcm::SQL_NOW_HCM;
use crate::workflow::clip_node::product_suggest::{
    self, ClipSuggestProductResponse, SuggestProductForClipInput,
};
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::recordings::{sync_recording_from_external_key_conn, SyncRecordingInput};

fn update_clip_caption_conn(
    conn: &Connection,
    clip_id: i64,
    caption_text: Option<String>,
    caption_status: &str,
    caption_error: Option<String>,
) -> Result<(), String> {
    let status = caption_status.trim();
    let valid = ["pending", "generating", "completed", "failed"];
    if !valid.contains(&status) {
        return Err(format!("Invalid caption_status: {caption_status}"));
    }

    let text_norm = caption_text
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let err_norm = caption_error
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if status == "completed" && text_norm.is_none() {
        return Err("caption_text is required when caption_status is completed".to_string());
    }

    let changed = conn
        .execute(
            &format!(
                "UPDATE clips SET \
                 caption_text = ?1, \
                 caption_status = ?2, \
                 caption_error = CASE \
                    WHEN ?2 IN ('pending', 'generating', 'completed') THEN NULL \
                    WHEN ?2 = 'failed' THEN COALESCE(?3, caption_error) \
                    ELSE caption_error \
                 END, \
                 caption_generated_at = CASE \
                    WHEN ?2 = 'completed' THEN {} \
                    WHEN caption_status = 'completed' THEN NULL \
                    ELSE caption_generated_at \
                 END, \
                 updated_at = {} \
                 WHERE id = ?4",
                SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![text_norm, status, err_norm, clip_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("Clip {clip_id} not found"));
    }

    Ok(())
}

#[tauri::command]
pub fn update_clip_caption(
    state: State<'_, AppState>,
    clip_id: i64,
    caption_text: Option<String>,
    caption_status: String,
    caption_error: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    update_clip_caption_conn(
        &conn,
        clip_id,
        caption_text,
        caption_status.as_str(),
        caption_error,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GenerateClipCaptionInput {
    pub clip_id: i64,
    pub username: String,
    pub transcript_text: Option<String>,
    pub clip_title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateClipCaptionOutput {
    pub clip_id: i64,
    pub caption_text: String,
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn transcript_snippet(transcript_text: &str, max_words: usize) -> String {
    collapse_spaces(transcript_text)
        .split(' ')
        .filter(|word| !word.is_empty())
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_caption_text(username: &str, transcript_text: &str, clip_title: &str) -> String {
    let mut user = collapse_spaces(username)
        .trim_start_matches('@')
        .trim()
        .to_string();
    let mut title = collapse_spaces(clip_title).trim().to_string();
    let mut transcript = transcript_snippet(transcript_text, 14);

    if user.is_empty() {
        user = "creator".to_string();
    }
    if title.is_empty() {
        title = "Live clip".to_string();
    }
    if transcript.is_empty() {
        transcript = "Highlights from the stream".to_string();
    }

    format!("{title} | {transcript} | @{user}")
}

#[tauri::command]
pub fn generate_clip_caption(
    state: State<'_, AppState>,
    input: GenerateClipCaptionInput,
) -> Result<GenerateClipCaptionOutput, String> {
    if input.clip_id <= 0 {
        return Err("clip_id must be positive".to_string());
    }
    if input.username.trim().is_empty() {
        return Err("username is required".to_string());
    }

    let caption_text = generate_caption_text(
        input.username.as_str(),
        input.transcript_text.as_deref().unwrap_or(""),
        input.clip_title.as_deref().unwrap_or(""),
    );

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    update_clip_caption_conn(
        &conn,
        input.clip_id,
        Some(caption_text.clone()),
        "completed",
        None,
    )?;

    Ok(GenerateClipCaptionOutput {
        clip_id: input.clip_id,
        caption_text,
    })
}

#[tauri::command]
pub fn suggest_product_for_clip(
    state: State<'_, AppState>,
    input: SuggestProductForClipInput,
) -> Result<ClipSuggestProductResponse, String> {
    product_suggest::suggest_product_for_clip_with_db_lock(
        &state.db,
        state.storage_path.as_path(),
        &input,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertGeneratedClipInput {
    pub external_recording_id: String,
    pub account_id: i64,
    pub file_path: String,
    pub thumbnail_path: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub end_sec: f64,
    pub transcript_text: Option<String>,
}

/// Persist one clip generated by the Rust clip pipeline.
pub(crate) fn insert_generated_clip_with_conn(
    conn: &rusqlite::Connection,
    input: &InsertGeneratedClipInput,
) -> Result<i64, String> {
    if input.external_recording_id.trim().is_empty() {
        return Err("external_recording_id is required".to_string());
    }
    if input.file_path.trim().is_empty() {
        return Err("file_path is required".to_string());
    }

    let mut recording_row: Option<(i64, i64, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT id, account_id, flow_id, flow_run_id FROM recordings WHERE external_recording_id = ?1",
            [&input.external_recording_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if recording_row.is_none() {
        sync_recording_from_external_key_conn(
            conn,
            &SyncRecordingInput {
                external_recording_id: input.external_recording_id.clone(),
                account_id: input.account_id,
                status: "done".to_string(),
                duration_seconds: 0,
                file_size_bytes: 0,
                file_path: None,
                error_message: None,
            },
        )?;
        recording_row = Some(
            conn.query_row(
                "SELECT id, account_id, flow_id, flow_run_id FROM recordings WHERE external_recording_id = ?1",
                [&input.external_recording_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| e.to_string())?,
        );
    }

    let (recording_id, recording_account_id, flow_id, flow_run_id) =
        recording_row.expect("recording row must exist after upsert");

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM clips WHERE recording_id = ?1 AND file_path = ?2",
            params![recording_id, &input.file_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let file_size_bytes: i64 = std::fs::metadata(&input.file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let duration_seconds = input.duration_sec.round().max(0.0) as i64;

    let thumb = input.thumbnail_path.trim();
    let thumb_opt = if thumb.is_empty() { None } else { Some(thumb) };
    let transcript = input
        .transcript_text
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    conn.execute(
        &format!(
            "INSERT INTO clips (\
               recording_id, account_id, title, file_path, thumbnail_path, \
               duration_seconds, file_size_bytes, start_time, end_time, status, flow_id, flow_run_id, transcript_text, created_at, updated_at\
             ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, 'ready', ?9, ?10, ?11, {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            recording_id,
            recording_account_id,
            &input.file_path,
            thumb_opt,
            duration_seconds,
            file_size_bytes,
            input.start_sec,
            input.end_sec,
            flow_id,
            flow_run_id,
            transcript,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::{
        generate_caption_text, insert_generated_clip_with_conn, transcript_snippet,
        InsertGeneratedClipInput,
    };
    use crate::commands::recordings::upsert_rust_recording;
    use crate::db::init::initialize_database;
    use crate::recording_runtime::types::RustRecordingUpsertInput;
    use rusqlite::{params, Connection};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn open_temp_db() -> (Connection, PathBuf) {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tikclip-clips-test-{}-{}-{}.db",
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let conn = initialize_database(&path).expect("init db");
        (conn, path)
    }

    #[test]
    fn generate_caption_text_uses_runtime_caption_fallbacks() {
        let caption = generate_caption_text(" @shop_abc ", "", "");

        assert_eq!(
            caption,
            "Live clip | Highlights from the stream | @shop_abc"
        );
    }

    #[test]
    fn transcript_snippet_keeps_first_fourteen_words() {
        let transcript = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen";

        assert_eq!(
            transcript_snippet(transcript, 14),
            "one two three four five six seven eight nine ten eleven twelve thirteen fourteen"
        );
    }

    #[test]
    fn insert_generated_clip_uses_recording_owner_found_by_external_key() {
        let (conn, path) = open_temp_db();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (3, 'Flow 3', 1, 'processing', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (12, 3, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert flow run");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, 'shop_abc', 'Shop ABC', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account one");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (2, 'shop_xyz', 'Shop XYZ', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account two");
        upsert_rust_recording(
            &conn,
            &RustRecordingUpsertInput {
                account_id: 1,
                flow_id: 3,
                flow_run_id: Some(12),
                external_recording_id: "ext-123".to_string(),
                runtime_status: "done".to_string(),
                room_id: Some("7312345".to_string()),
                file_path: Some("/tmp/recording.mp4".to_string()),
                error_message: None,
                duration_seconds: 120,
                file_size_bytes: 0,
            },
        )
        .expect("insert rust recording");

        let clip_id = insert_generated_clip_with_conn(
            &conn,
            &InsertGeneratedClipInput {
                external_recording_id: "ext-123".to_string(),
                account_id: 2,
                file_path: "/tmp/clip.mp4".to_string(),
                thumbnail_path: String::new(),
                duration_sec: 10.0,
                start_sec: 0.0,
                end_sec: 10.0,
                transcript_text: None,
            },
        )
        .expect("insert generated clip");

        let row: (i64, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT account_id, flow_id, flow_run_id FROM clips WHERE id = ?1",
                params![clip_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read inserted clip row");

        assert_eq!(row.0, 1);
        assert_eq!(row.1, Some(3));
        assert_eq!(row.2, Some(12));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
