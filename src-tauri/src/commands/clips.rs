use crate::db::models::{Clip, SpeechSegment};
use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::Result as SqlResult;
use rusqlite::{params, OptionalExtension, Row};
use serde::Deserialize;
use tauri::State;

use super::recordings::{sync_recording_from_sidecar_conn, SyncRecordingFromSidecarInput};

fn map_clip_row(row: &Row) -> SqlResult<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        recording_id: row.get(1)?,
        account_id: row.get(2)?,
        account_username: row.get(3)?,
        title: row.get(4)?,
        file_path: row.get(5)?,
        thumbnail_path: row.get(6)?,
        duration_seconds: row.get(7)?,
        file_size_bytes: row.get(8)?,
        start_time: row.get(9)?,
        end_time: row.get(10)?,
        status: row.get(11)?,
        quality_score: row.get(12)?,
        scene_type: row.get(13)?,
        ai_tags_json: row.get(14)?,
        notes: row.get(15)?,
        flow_id: row.get(16)?,
        transcript_text: row.get(17)?,
        caption_text: row.get(18)?,
        caption_status: row.get(19)?,
        caption_error: row.get(20)?,
        caption_generated_at: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

/// Lists clips joined with `accounts.username` as `account_username`.
/// Column order: clips columns plus flow/caption fields, joined username.
#[tauri::command]
pub fn list_clips(state: State<'_, AppState>) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             c.id, c.recording_id, c.account_id, a.username, \
             c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
             c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
             c.notes, c.flow_id, c.transcript_text, c.caption_text, c.caption_status, c.caption_error, c.caption_generated_at, c.created_at, c.updated_at \
             FROM clips c \
             INNER JOIN accounts a ON a.id = c.account_id \
             ORDER BY c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], map_clip_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListClipsFilteredInput {
    pub status: Option<String>,
    pub account_id: Option<i64>,
    pub scene_type: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[tauri::command]
pub fn list_clips_filtered(
    state: State<'_, AppState>,
    input: ListClipsFilteredInput,
) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut sql = String::from(
        "SELECT \
         c.id, c.recording_id, c.account_id, a.username, \
         c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
         c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
         c.notes, c.flow_id, c.transcript_text, c.caption_text, c.caption_status, c.caption_error, c.caption_generated_at, c.created_at, c.updated_at \
         FROM clips c \
         INNER JOIN accounts a ON a.id = c.account_id \
         WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref status) = input.status {
        if status != "all" {
            sql.push_str(&format!(" AND c.status = ?{idx}"));
            params_vec.push(Box::new(status.clone()));
            idx += 1;
        }
    }
    if let Some(account_id) = input.account_id {
        sql.push_str(&format!(" AND c.account_id = ?{idx}"));
        params_vec.push(Box::new(account_id));
        idx += 1;
    }
    if let Some(ref scene_type) = input.scene_type {
        if scene_type != "all" {
            sql.push_str(&format!(" AND c.scene_type = ?{idx}"));
            params_vec.push(Box::new(scene_type.clone()));
            idx += 1;
        }
    }
    if let Some(ref date_from) = input.date_from {
        sql.push_str(&format!(" AND c.created_at >= ?{idx}"));
        params_vec.push(Box::new(date_from.clone()));
        idx += 1;
    }
    if let Some(ref date_to) = input.date_to {
        sql.push_str(&format!(" AND c.created_at <= ?{idx}"));
        params_vec.push(Box::new(format!("{date_to} 23:59:59")));
        idx += 1;
    }
    if let Some(ref search) = input.search {
        if !search.trim().is_empty() {
            let pattern = format!("%{}%", search.trim());
            sql.push_str(&format!(
                " AND (c.title LIKE ?{idx} OR c.notes LIKE ?{})",
                idx + 1
            ));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
    }

    let sort_col = match input.sort_by.as_deref() {
        Some("duration") => "c.duration_seconds",
        Some("file_size") => "c.file_size_bytes",
        Some("title") => "c.title",
        _ => "c.created_at",
    };
    let sort_dir = match input.sort_order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };
    sql.push_str(&format!(" ORDER BY {sort_col} {sort_dir}"));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_refs.as_slice(), map_clip_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn get_clip_by_id(state: State<'_, AppState>, clip_id: i64) -> Result<Clip, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT \
         c.id, c.recording_id, c.account_id, a.username, \
         c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
         c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
         c.notes, c.flow_id, c.transcript_text, c.caption_text, c.caption_status, c.caption_error, c.caption_generated_at, c.created_at, c.updated_at \
         FROM clips c \
         INNER JOIN accounts a ON a.id = c.account_id \
         WHERE c.id = ?1",
        [clip_id],
        map_clip_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_clip_status(
    state: State<'_, AppState>,
    clip_id: i64,
    new_status: String,
) -> Result<(), String> {
    let valid = ["draft", "ready", "posted", "archived"];
    if !valid.contains(&new_status.as_str()) {
        return Err(format!("Invalid status: {new_status}"));
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            &format!(
                "UPDATE clips SET status = ?1, updated_at = {} WHERE id = ?2",
                SQL_NOW_HCM
            ),
            params![&new_status, clip_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err(format!("Clip {clip_id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn update_clip_title(
    state: State<'_, AppState>,
    clip_id: i64,
    title: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE clips SET title = ?1, updated_at = {} WHERE id = ?2",
            SQL_NOW_HCM
        ),
        params![&title, clip_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_clip_notes(
    state: State<'_, AppState>,
    clip_id: i64,
    notes: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "UPDATE clips SET notes = ?1, updated_at = {} WHERE id = ?2",
            SQL_NOW_HCM
        ),
        params![&notes, clip_id],
    )
    .map_err(|e| e.to_string())?;
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

    let conn = state.db.lock().map_err(|e| e.to_string())?;
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
pub fn batch_update_clip_status(
    state: State<'_, AppState>,
    clip_ids: Vec<i64>,
    new_status: String,
) -> Result<(), String> {
    let valid = ["draft", "ready", "posted", "archived"];
    if !valid.contains(&new_status.as_str()) {
        return Err(format!("Invalid status: {new_status}"));
    }
    if clip_ids.is_empty() {
        return Ok(());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let placeholders: Vec<String> = (1..=clip_ids.len())
        .map(|i| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "UPDATE clips SET status = ?1, updated_at = {} WHERE id IN ({})",
        SQL_NOW_HCM,
        placeholders.join(", ")
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(new_status));
    for id in &clip_ids {
        params_vec.push(Box::new(*id));
    }
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_delete_clips(state: State<'_, AppState>, clip_ids: Vec<i64>) -> Result<(), String> {
    if clip_ids.is_empty() {
        return Ok(());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let placeholders: Vec<String> = (1..=clip_ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT file_path, thumbnail_path FROM clips WHERE id IN ({})",
        placeholders.join(", ")
    );
    let params_refs: Vec<Box<dyn rusqlite::types::ToSql>> = clip_ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let refs: Vec<&dyn rusqlite::types::ToSql> = params_refs.iter().map(|p| p.as_ref()).collect();

    let file_rows: Vec<(String, Option<String>)> = {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    for (fp, tp) in &file_rows {
        let _ = std::fs::remove_file(fp);
        if let Some(t) = tp {
            let _ = std::fs::remove_file(t);
        }
    }

    let del_sql = format!(
        "DELETE FROM clips WHERE id IN ({})",
        placeholders.join(", ")
    );
    conn.execute(&del_sql, refs.as_slice())
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertTrimmedClipInput {
    pub recording_id: i64,
    pub account_id: i64,
    pub file_path: String,
    pub thumbnail_path: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub end_sec: f64,
}

#[tauri::command]
pub fn insert_trimmed_clip(
    state: State<'_, AppState>,
    input: InsertTrimmedClipInput,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let file_size: i64 = std::fs::metadata(&input.file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let duration = input.duration_sec.round().max(0.0) as i64;
    let thumb = input.thumbnail_path.trim();
    let thumb_opt = if thumb.is_empty() { None } else { Some(thumb) };

    conn.execute(
        &format!(
            "INSERT INTO clips (\
               recording_id, account_id, title, file_path, thumbnail_path, \
               duration_seconds, file_size_bytes, start_time, end_time, status, created_at, updated_at\
             ) VALUES (?1, ?2, 'Trimmed clip', ?3, ?4, ?5, ?6, ?7, ?8, 'draft', {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            input.recording_id,
            input.account_id,
            &input.file_path,
            thumb_opt,
            duration,
            file_size,
            input.start_sec,
            input.end_sec,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertClipFromSidecarInput {
    pub sidecar_recording_id: String,
    pub account_id: i64,
    pub file_path: String,
    pub thumbnail_path: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub end_sec: f64,
    pub transcript_text: Option<String>,
}

/// Persist one generated clip when the sidecar emits `clip_ready` over WebSocket.
#[tauri::command]
pub fn insert_clip_from_sidecar(
    state: State<'_, AppState>,
    input: InsertClipFromSidecarInput,
) -> Result<i64, String> {
    if input.sidecar_recording_id.trim().is_empty() {
        return Err("sidecar_recording_id is required".to_string());
    }
    if input.file_path.trim().is_empty() {
        return Err("file_path is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut recording_row: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT id, flow_id FROM recordings WHERE sidecar_recording_id = ?1",
            [&input.sidecar_recording_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if recording_row.is_none() {
        sync_recording_from_sidecar_conn(
            &conn,
            &SyncRecordingFromSidecarInput {
                sidecar_recording_id: input.sidecar_recording_id.clone(),
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
                "SELECT id, flow_id FROM recordings WHERE sidecar_recording_id = ?1",
                [&input.sidecar_recording_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?,
        );
    }

    let (recording_id, flow_id) = recording_row.expect("recording row must exist after upsert");

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
               duration_seconds, file_size_bytes, start_time, end_time, status, flow_id, transcript_text, created_at, updated_at\
             ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, 'ready', ?9, ?10, {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            recording_id,
            input.account_id,
            &input.file_path,
            thumb_opt,
            duration_seconds,
            file_size_bytes,
            input.start_sec,
            input.end_sec,
            flow_id,
            transcript,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

fn map_speech_segment_row(row: &Row) -> SqlResult<SpeechSegment> {
    Ok(SpeechSegment {
        id: row.get(0)?,
        recording_id: row.get(1)?,
        start_time: row.get(2)?,
        end_time: row.get(3)?,
        text: row.get(4)?,
        confidence: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InsertSpeechSegmentInput {
    pub sidecar_recording_id: String,
    /// Used to create a stub `recordings` row if missing (same as clip insert).
    pub account_id: i64,
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,
    pub confidence: Option<f64>,
}

/// Persist one speech segment when the sidecar emits `speech_segment_ready` over WebSocket.
#[tauri::command]
pub fn insert_speech_segment(
    state: State<'_, AppState>,
    input: InsertSpeechSegmentInput,
) -> Result<i64, String> {
    if input.sidecar_recording_id.trim().is_empty() {
        return Err("sidecar_recording_id is required".to_string());
    }
    if input.account_id <= 0 {
        return Err("account_id is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut rec_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
            [&input.sidecar_recording_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if rec_id.is_none() {
        sync_recording_from_sidecar_conn(
            &conn,
            &SyncRecordingFromSidecarInput {
                sidecar_recording_id: input.sidecar_recording_id.clone(),
                account_id: input.account_id,
                status: "done".to_string(),
                duration_seconds: 0,
                file_size_bytes: 0,
                file_path: None,
                error_message: None,
            },
        )?;
        rec_id = Some(
            conn.query_row(
                "SELECT id FROM recordings WHERE sidecar_recording_id = ?1",
                [&input.sidecar_recording_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?,
        );
    }

    let recording_id = rec_id.expect("recording row must exist after upsert");

    conn.execute(
        &format!(
            "INSERT INTO speech_segments (recording_id, start_time, end_time, text, confidence, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, {})",
            SQL_NOW_HCM
        ),
        params![
            recording_id,
            input.start_time,
            input.end_time,
            input.text,
            input.confidence,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn list_speech_segments(
    state: State<'_, AppState>,
    recording_id: i64,
) -> Result<Vec<SpeechSegment>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, recording_id, start_time, end_time, text, confidence, created_at \
             FROM speech_segments WHERE recording_id = ?1 ORDER BY start_time ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([recording_id], map_speech_segment_row)
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
