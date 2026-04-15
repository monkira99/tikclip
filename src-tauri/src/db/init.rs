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
