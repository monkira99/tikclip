use rusqlite::Connection;
use std::path::Path;

pub fn initialize_database(db_path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );",
    )?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let migrations: Vec<(i64, &str)> = vec![
        (1, include_str!("migrations/001_initial.sql")),
        (2, include_str!("migrations/002_sidecar_recording_id.sql")),
        (3, include_str!("migrations/003_timestamps_gmt_plus_7.sql")),
        (4, include_str!("migrations/004_product_enhancements.sql")),
        (5, include_str!("migrations/005_product_media_files.sql")),
        (6, include_str!("migrations/006_speech_segments.sql")),
        (7, include_str!("migrations/007_flows.sql")),
        (8, include_str!("migrations/008_flow_engine_rebuild.sql")),
        (
            9,
            include_str!("migrations/009_repair_flow_foreign_keys.sql"),
        ),
        (
            10,
            include_str!("migrations/010_product_embedding_vectors.sql"),
        ),
        (11, include_str!("migrations/011_external_recording_id.sql")),
    ];

    for (version, sql) in migrations {
        if version > current_version {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initialize_database;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_path(name: &str) -> PathBuf {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "tikclip-db-init-test-{}-{}-{}-{}.db",
            name,
            std::process::id(),
            counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn migration_008_canonicalizes_start_and_record_configs() {
        let db_path = temp_db_path("migration-008");
        {
            let conn = Connection::open(&db_path).expect("open legacy db");
            conn.execute_batch(include_str!("migrations/001_initial.sql"))
                .expect("run migration 001");
            conn.execute_batch(include_str!("migrations/002_sidecar_recording_id.sql"))
                .expect("run migration 002");
            conn.execute_batch(include_str!("migrations/003_timestamps_gmt_plus_7.sql"))
                .expect("run migration 003");
            conn.execute_batch(include_str!("migrations/004_product_enhancements.sql"))
                .expect("run migration 004");
            conn.execute_batch(include_str!("migrations/005_product_media_files.sql"))
                .expect("run migration 005");
            conn.execute_batch(include_str!("migrations/006_speech_segments.sql"))
                .expect("run migration 006");
            conn.execute_batch(include_str!("migrations/007_flows.sql"))
                .expect("run migration 007");
            conn.execute_batch("CREATE TABLE schema_version (version INTEGER PRIMARY KEY);")
                .expect("create schema_version");
            for version in 1..=7 {
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )
                .expect("insert schema version");
            }

            conn.execute(
                "INSERT INTO accounts (id, username, cookies_json, proxy_url, auto_record) VALUES (1, '  @Shop_Abc  ', '{}', 'http://127.0.0.1:9000', 1)",
                [],
            )
            .expect("insert account");
            conn.execute(
                "INSERT INTO flows (id, account_id, name, enabled, status, current_node, last_live_at, last_run_at, last_error, created_at, updated_at) VALUES (1, 1, 'f', 1, 'idle', NULL, NULL, NULL, NULL, datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
                [],
            )
            .expect("insert legacy flow");
            conn.execute(
                "INSERT INTO flow_node_configs (flow_id, node_key, config_json, updated_at) VALUES (1, 'record', '{\"maxDurationSeconds\":61}', datetime('now', '+7 hours'))",
                [],
            )
            .expect("insert legacy record config");
        }

        let conn = initialize_database(&db_path).expect("migrate db");

        let start_config: String = conn
            .query_row(
                "SELECT draft_config_json FROM flow_nodes WHERE flow_id = 1 AND node_key = 'start'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated start config");
        let record_config: String = conn
            .query_row(
                "SELECT draft_config_json FROM flow_nodes WHERE flow_id = 1 AND node_key = 'record'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated record config");

        assert_eq!(
            start_config,
            r#"{"username":"Shop_Abc","cookies_json":"{}","proxy_url":"http://127.0.0.1:9000","poll_interval_seconds":60,"retry_limit":0,"last_live_at":null,"last_run_at":null,"last_error":null}"#
        );
        assert_eq!(record_config, r#"{"max_duration_minutes":2}"#);

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn initialize_database_allows_recordings_row_with_flow_id_after_flow_engine_rebuild() {
        let db_path = temp_db_path("migration-008-recordings-flow-fk");

        let conn = initialize_database(&db_path).expect("initialize db");
        conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) VALUES (1, 'shop_abc', 'Shop', 'monitored', datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
            [],
        )
        .expect("insert account");
        conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) VALUES (1, 'Flow', 1, 'idle', 1, 1, datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
            [],
        )
        .expect("insert flow");
        conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) VALUES (1, 1, 1, 'running', datetime('now', '+7 hours'), 'test')",
            [],
        )
        .expect("insert flow run");

        conn.execute(
            "INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, created_at, started_at) VALUES (1, '7312345', 'recording', 0, 0, 1, 1, datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
            [],
        )
        .expect("insert recording with flow_id");

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn migration_011_renames_recording_external_key_without_losing_values() {
        let db_path = temp_db_path("migration-011-external-recording-id");
        {
            let conn = Connection::open(&db_path).expect("open legacy db");
            conn.execute_batch(include_str!("migrations/001_initial.sql"))
                .expect("run migration 001");
            conn.execute_batch(include_str!("migrations/002_sidecar_recording_id.sql"))
                .expect("run migration 002");
            conn.execute_batch(include_str!("migrations/003_timestamps_gmt_plus_7.sql"))
                .expect("run migration 003");
            conn.execute_batch(include_str!("migrations/004_product_enhancements.sql"))
                .expect("run migration 004");
            conn.execute_batch(include_str!("migrations/005_product_media_files.sql"))
                .expect("run migration 005");
            conn.execute_batch(include_str!("migrations/006_speech_segments.sql"))
                .expect("run migration 006");
            conn.execute_batch(include_str!("migrations/007_flows.sql"))
                .expect("run migration 007");
            conn.execute_batch(include_str!("migrations/008_flow_engine_rebuild.sql"))
                .expect("run migration 008");
            conn.execute_batch(include_str!("migrations/009_repair_flow_foreign_keys.sql"))
                .expect("run migration 009");
            conn.execute_batch(include_str!("migrations/010_product_embedding_vectors.sql"))
                .expect("run migration 010");
            conn.execute_batch("CREATE TABLE schema_version (version INTEGER PRIMARY KEY);")
                .expect("create schema_version");
            for version in 1..=10 {
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )
                .expect("insert schema version");
            }
            conn.execute(
                "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
                 VALUES (1, 'shop_abc', 'Shop', 'monitored', datetime('now', '+7 hours'), datetime('now', '+7 hours'))",
                [],
            )
            .expect("insert account");
            conn.execute(
                "INSERT INTO recordings (account_id, status, sidecar_recording_id) \
                 VALUES (1, 'recording', 'ext-legacy-123')",
                [],
            )
            .expect("insert legacy recording");
        }

        let conn = initialize_database(&db_path).expect("migrate db");
        let external_id: String = conn
            .query_row(
                "SELECT external_recording_id FROM recordings WHERE account_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read migrated external recording id");
        assert_eq!(external_id, "ext-legacy-123");

        let column_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('recordings') \
                 WHERE name = 'sidecar_recording_id'",
                [],
                |row| row.get(0),
            )
            .expect("count legacy column");
        assert_eq!(column_count, 0);

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
    }
}
