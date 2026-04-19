use crate::db::models::Account;
use crate::time_hcm::{now_timestamp_hcm, SQL_NOW_HCM};
use crate::AppState;
use log::{debug, info, warn};
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
    debug!(
        "list_accounts: {} row(s) — {}",
        out.len(),
        out.iter()
            .map(|a| format!("id={} user={} is_live={}", a.id, a.username, a.is_live))
            .collect::<Vec<_>>()
            .join(", ")
    );
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
    let ts = now_timestamp_hcm();
    conn.execute(
        "INSERT INTO accounts (username, display_name, type, cookies_json, proxy_url, auto_record, priority, notes, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            username,
            input.display_name,
            input.account_type,
            input.cookies_json,
            input.proxy_url,
            if input.auto_record { 1i64 } else { 0i64 },
            input.priority,
            input.notes,
            ts.clone(),
            ts,
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

#[derive(Debug, Deserialize)]
pub struct LiveFlagRow {
    pub account_id: i64,
    pub is_live: bool,
}

/// Applies sidecar live flags in one SQLite transaction (avoids list_accounts interleaving between rows).
#[tauri::command]
pub fn sync_accounts_live_status(
    state: State<'_, AppState>,
    rows: Vec<LiveFlagRow>,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    debug!("sync_accounts_live_status: enter {} row(s)", rows.len());
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for row in &rows {
        let flag = if row.is_live { 1i64 } else { 0i64 };
        let n = tx
            .execute(
                &format!(
                    "UPDATE accounts SET \
                     is_live = ?1, \
                     last_checked_at = {}, \
                     last_live_at = CASE WHEN ?1 != 0 THEN {} ELSE last_live_at END, \
                     updated_at = {} \
                     WHERE id = ?2",
                    SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
                ),
                params![flag, row.account_id],
            )
            .map_err(|e| e.to_string())?;
        if n == 0 {
            warn!(
                "sync_accounts_live_status: unknown account_id={} (skipped)",
                row.account_id
            );
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    info!(
        "sync_accounts_live_status: committed {} live flag(s)",
        rows.len()
    );
    Ok(())
}

#[tauri::command]
pub fn update_account_live_status(
    state: State<'_, AppState>,
    id: i64,
    is_live: bool,
) -> Result<(), String> {
    debug!("update_account_live_status: enter id={id} is_live={is_live}");
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let flag = if is_live { 1i64 } else { 0i64 };
    let n = conn
        .execute(
            &format!(
                "UPDATE accounts SET \
                 is_live = ?1, \
                 last_checked_at = {}, \
                 last_live_at = CASE WHEN ?1 != 0 THEN {} ELSE last_live_at END, \
                 updated_at = {} \
                 WHERE id = ?2",
                SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
            ),
            params![flag, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        warn!(
            "update_account_live_status: no row updated (id={id} is_live={is_live}) — id missing in SQLite"
        );
        return Err(format!("account id {id} not found"));
    }
    info!("update_account_live_status: ok id={id} is_live={is_live} rows_updated={n}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::initialize_database;
    use crate::time_hcm::SQL_NOW_HCM;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn open_temp_db() -> (Connection, PathBuf) {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tikclip-accounts-test-{}-{}-{}.db",
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
    fn update_live_status_sets_is_live_column() {
        let (conn, path) = open_temp_db();
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, is_live) VALUES (?1, ?2, ?3, 0)",
            params!["u1", "d1", "monitored"],
        )
        .expect("insert");
        let id = conn.last_insert_rowid();
        let flag = 1i64;
        let n = conn
            .execute(
                &format!(
                    "UPDATE accounts SET \
                     is_live = ?1, \
                     last_checked_at = {}, \
                     last_live_at = CASE WHEN ?1 != 0 THEN {} ELSE last_live_at END, \
                     updated_at = {} \
                     WHERE id = ?2",
                    SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
                ),
                params![flag, id],
            )
            .expect("update");
        assert_eq!(n, 1);
        let live: i64 = conn
            .query_row(
                "SELECT is_live FROM accounts WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("select");
        assert_eq!(live, 1);
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn transaction_sync_updates_two_rows() {
        let (mut conn, path) = open_temp_db();
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, is_live) VALUES ('a','a','monitored',0)",
            [],
        )
        .expect("insert1");
        let id1 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, is_live) VALUES ('b','b','monitored',0)",
            [],
        )
        .expect("insert2");
        let id2 = conn.last_insert_rowid();
        let tx = conn.transaction().expect("tx");
        for (id, flag) in [(id1, 1i64), (id2, 1i64)] {
            tx.execute(
                &format!(
                    "UPDATE accounts SET is_live = ?1, last_checked_at = {}, \
                     last_live_at = CASE WHEN ?1 != 0 THEN {} ELSE last_live_at END, \
                     updated_at = {} WHERE id = ?2",
                    SQL_NOW_HCM, SQL_NOW_HCM, SQL_NOW_HCM
                ),
                params![flag, id],
            )
            .expect("upd");
        }
        tx.commit().expect("commit");
        let c1: i64 = conn
            .query_row(
                "SELECT is_live FROM accounts WHERE id=?1",
                params![id1],
                |r| r.get(0),
            )
            .unwrap();
        let c2: i64 = conn
            .query_row(
                "SELECT is_live FROM accounts WHERE id=?1",
                params![id2],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!((c1, c2), (1, 1));
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }
}
