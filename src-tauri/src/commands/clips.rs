use crate::db::models::Clip;
use crate::AppState;
use rusqlite::Result as SqlResult;
use rusqlite::Row;
use tauri::State;

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
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

/// Lists clips joined with `accounts.username` as `account_username`.
/// Column order matches `clips` table in `db/migrations/001_initial.sql` plus joined username.
#[tauri::command]
pub fn list_clips(state: State<'_, AppState>) -> Result<Vec<Clip>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT \
             c.id, c.recording_id, c.account_id, a.username, \
             c.title, c.file_path, c.thumbnail_path, c.duration_seconds, c.file_size_bytes, \
             c.start_time, c.end_time, c.status, c.quality_score, c.scene_type, c.ai_tags_json, \
             c.notes, c.created_at, c.updated_at \
             FROM clips c \
             INNER JOIN accounts a ON a.id = c.account_id \
             ORDER BY c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| map_clip_row(row))
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
