use crate::db::models::Account;
use crate::AppState;
use rusqlite::params;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
pub struct CreateAccountInput {
    pub username: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub cookies_json: Option<String>,
    pub proxy_url: Option<String>,
    pub auto_record: bool,
    pub priority: i32,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, username, display_name, avatar_url, type, tiktok_uid, cookies_json, proxy_url, \
             auto_record, auto_record_schedule, priority, is_live, last_live_at, last_checked_at, \
             notes, created_at, updated_at FROM accounts \
             ORDER BY priority DESC, username ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                avatar_url: row.get(3)?,
                account_type: row.get(4)?,
                tiktok_uid: row.get(5)?,
                cookies_json: row.get(6)?,
                proxy_url: row.get(7)?,
                auto_record: row.get::<_, i64>(8)? != 0,
                auto_record_schedule: row.get(9)?,
                priority: row.get(10)?,
                is_live: row.get::<_, i64>(11)? != 0,
                last_live_at: row.get(12)?,
                last_checked_at: row.get(13)?,
                notes: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    input: CreateAccountInput,
) -> Result<i64, String> {
    if input.account_type != "own" && input.account_type != "monitored" {
        return Err("type must be 'own' or 'monitored'".to_string());
    }
    let username = input.username.trim();
    if username.is_empty() {
        return Err("username is required".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO accounts (username, display_name, type, cookies_json, proxy_url, auto_record, priority, notes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            username,
            input.display_name,
            input.account_type,
            input.cookies_json,
            input.proxy_url,
            if input.auto_record { 1i64 } else { 0i64 },
            input.priority,
            input.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let n = conn
        .execute("DELETE FROM accounts WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("account id {id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn update_account_live_status(
    state: State<'_, AppState>,
    id: i64,
    is_live: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flag = if is_live { 1i64 } else { 0i64 };
    let n = conn
        .execute(
            "UPDATE accounts SET \
             is_live = ?1, \
             last_checked_at = datetime('now'), \
             last_live_at = CASE WHEN ?1 != 0 THEN datetime('now') ELSE last_live_at END, \
             updated_at = datetime('now') \
             WHERE id = ?2",
            params![flag, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("account id {id} not found"));
    }
    Ok(())
}
