use crate::live_runtime::normalize::canonicalize_username;
use crate::time_hcm::now_timestamp_hcm;
use rusqlite::{params, Connection, OptionalExtension};

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveAccountResult {
    Existing { account_id: i64 },
    Created { account_id: i64 },
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_or_create_account_for_username(
    conn: &Connection,
    canonical_username: &str,
) -> Result<ResolveAccountResult, String> {
    let canonical = canonicalize_username(canonical_username)
        .map_err(|_| "username is required".to_string())?;
    let matched_ids = find_account_ids_by_lookup_key(conn, canonical.lookup_key.as_str())?;

    match matched_ids.as_slice() {
        [account_id] => Ok(ResolveAccountResult::Existing {
            account_id: *account_id,
        }),
        [] => {
            let ts = now_timestamp_hcm();
            conn.execute(
                "INSERT INTO accounts (username, display_name, type, auto_record, priority, created_at, updated_at) \
                 VALUES (?1, ?2, 'monitored', 0, 0, ?3, ?4)",
                params![canonical.canonical, canonical.canonical, ts.clone(), ts],
            )
            .map_err(|e| e.to_string())?;
            Ok(ResolveAccountResult::Created {
                account_id: conn.last_insert_rowid(),
            })
        }
        _ => Err(format!(
            "duplicate accounts for username {canonical_username}"
        )),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn find_account_by_start_username(
    conn: &Connection,
    start_username: Option<&str>,
) -> Result<Option<(i64, String)>, String> {
    let Some(start_username) = start_username else {
        return Ok(None);
    };
    let Ok(canonical) = canonicalize_username(start_username) else {
        return Ok(None);
    };
    let lookup_key = canonical.lookup_key;
    let matched_accounts = find_accounts_by_lookup_key(conn, lookup_key.as_str())?;

    match matched_accounts.as_slice() {
        [] => Ok(None),
        [account] => Ok(Some(account.clone())),
        _ => Err(format!("duplicate accounts for username {start_username}")),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn find_flow_id_for_account(conn: &Connection, account_id: i64) -> Result<Option<i64>, String> {
    let account_username: Option<String> = conn
        .query_row(
            "SELECT username FROM accounts WHERE id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(account_username) = account_username else {
        return Ok(None);
    };
    let Ok(canonical) = canonicalize_username(&account_username) else {
        return Ok(None);
    };
    let lookup_key = canonical.lookup_key;

    let mut stmt = conn
        .prepare(
            "SELECT n.flow_id, json_extract(n.published_config_json, '$.username') \
             FROM flow_nodes n \
             JOIN flows f ON f.id = n.flow_id \
             WHERE n.node_key = 'start' AND f.enabled = 1 \
             ORDER BY n.flow_id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut matched_flow_ids = Vec::new();
    for row in rows {
        let (flow_id, start_username) = row.map_err(|e| e.to_string())?;
        let Some(start_username) = start_username else {
            continue;
        };
        let Ok(canonical) = canonicalize_username(&start_username) else {
            continue;
        };
        let flow_lookup_key = canonical.lookup_key;
        if flow_lookup_key == lookup_key {
            matched_flow_ids.push(flow_id);
        }
    }

    match matched_flow_ids.as_slice() {
        [] => Ok(None),
        [flow_id] => Ok(Some(*flow_id)),
        _ => Err(format!(
            "multiple enabled flows match account_id {account_id}"
        )),
    }
}

fn find_account_ids_by_lookup_key(conn: &Connection, lookup_key: &str) -> Result<Vec<i64>, String> {
    Ok(find_accounts_by_lookup_key(conn, lookup_key)?
        .into_iter()
        .map(|(account_id, _)| account_id)
        .collect())
}

fn find_accounts_by_lookup_key(
    conn: &Connection,
    lookup_key: &str,
) -> Result<Vec<(i64, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id, username FROM accounts ORDER BY id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut matched_accounts = Vec::new();
    for row in rows {
        let (account_id, username) = row.map_err(|e| e.to_string())?;
        let existing_lookup_key = canonicalize_username(&username)
            .map_err(|_| format!("invalid existing username {username}"))?
            .lookup_key;
        if existing_lookup_key == lookup_key {
            matched_accounts.push((account_id, username));
        }
    }

    Ok(matched_accounts)
}

#[cfg(test)]
mod tests {
    use super::{
        find_account_by_start_username, find_flow_id_for_account,
        resolve_or_create_account_for_username, ResolveAccountResult,
    };
    use crate::db::init::initialize_database;
    use rusqlite::params;
    use std::path::PathBuf;

    fn open_temp_db(name: &str) -> (rusqlite::Connection, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "tikclip-account-binding-{name}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let conn = initialize_database(&path).expect("init db");
        (conn, path)
    }

    #[test]
    fn resolve_or_create_account_for_username_creates_missing_monitored_account() {
        let (conn, path) = open_temp_db("create");

        let result = resolve_or_create_account_for_username(&conn, " @Shop_ABC ").unwrap();

        let account_id = match result {
            ResolveAccountResult::Created { account_id } => account_id,
            other => panic!("expected created result, got {other:?}"),
        };
        let row: (String, String) = conn
            .query_row(
                "SELECT username, type FROM accounts WHERE id = ?1",
                [account_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(row.0, "Shop_ABC");
        assert_eq!(row.1, "monitored");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_or_create_account_for_username_returns_existing_account_for_canonical_username() {
        let (conn, path) = open_temp_db("existing");
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, created_at, updated_at) \
             VALUES (?1, ?2, 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params!["@Shop_ABC", "A"],
        )
        .unwrap();
        let existing_id = conn.last_insert_rowid();

        let result = resolve_or_create_account_for_username(&conn, "shop_abc").unwrap();

        assert_eq!(
            result,
            ResolveAccountResult::Existing {
                account_id: existing_id,
            }
        );

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_or_create_account_for_username_errors_on_normalized_duplicates() {
        let (conn, path) = open_temp_db("duplicate");
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, created_at, updated_at) \
             VALUES (?1, ?2, 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params!["Shop_ABC", "A"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, created_at, updated_at) \
             VALUES (?1, ?2, 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params!["@shop_abc", "B"],
        )
        .unwrap();

        let err = resolve_or_create_account_for_username(&conn, "shop_abc").unwrap_err();

        assert!(err.contains("duplicate accounts"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn find_account_by_start_username_errors_on_normalized_duplicates() {
        let (conn, path) = open_temp_db("find-account-duplicate");
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, created_at, updated_at) \
             VALUES (?1, ?2, 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params!["Shop_ABC", "A"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO accounts (username, display_name, type, created_at, updated_at) \
             VALUES (?1, ?2, 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params!["@shop_abc", "B"],
        )
        .unwrap();

        let err = find_account_by_start_username(&conn, Some("shop_abc")).unwrap_err();

        assert!(err.contains("duplicate accounts"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn find_flow_id_for_account_matches_start_username_with_shared_canonical_rule() {
        let (conn, path) = open_temp_db("find-flow");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, '@Shop_ABC', 'A', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (11, 'Flow', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (11, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();

        let flow_id = find_flow_id_for_account(&conn, 1).unwrap();

        assert_eq!(flow_id, Some(11));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn find_flow_id_for_account_prefers_enabled_flow_when_disabled_flow_shares_username() {
        let (conn, path) = open_temp_db("find-flow-enabled-only");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, '@Shop_ABC', 'A', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (10, 'Disabled Flow', 0, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (11, 'Enabled Flow', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (10, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (11, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();

        let flow_id = find_flow_id_for_account(&conn, 1).unwrap();

        assert_eq!(flow_id, Some(11));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn find_flow_id_for_account_errors_when_multiple_enabled_flows_share_username() {
        let (conn, path) = open_temp_db("find-flow-conflict");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, '@Shop_ABC', 'A', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (11, 'Flow A', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (12, 'Flow B', 1, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (11, 'start', 1, '{\"username\":\"shop_abc\"}', '{\"username\":\"shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (12, 'start', 1, '{\"username\":\"@shop_abc\"}', '{\"username\":\"@shop_abc\"}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .unwrap();

        let err = find_flow_id_for_account(&conn, 1).unwrap_err();

        assert!(err.contains("multiple enabled flows"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
