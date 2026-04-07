use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [&key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, {})
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = {}",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
