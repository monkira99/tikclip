use super::LiveRuntimeManager;
use crate::db::init::initialize_database;
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::live_runtime::session::{
    lock_teardown_test_guard_for_test, reset_teardown_call_count_for_test,
    teardown_call_count_for_test,
};
use crate::recording_runtime::types::RecordingOutcome;
use crate::tiktok::types::LiveStatus;
use rusqlite::{params, Connection};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

fn open_temp_db() -> (Connection, PathBuf) {
    let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "tikclip-live-runtime-manager-test-{}-{}-{}.db",
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

fn insert_flow(conn: &Connection, flow_id: i64, enabled: bool, username: &str) {
    conn.execute(
            "INSERT INTO flows (id, name, enabled, status, published_version, draft_version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'idle', 1, 1, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!("Flow {flow_id}"), if enabled { 1 } else { 0 }],
        )
        .expect("insert flow");
    conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'start', 1, ?2, ?2, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id, format!(r#"{{"username":"{username}"}}"#)],
        )
        .expect("insert start node");
    conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json, draft_updated_at, published_at) \
             VALUES (?1, 'record', 2, '{\"max_duration_minutes\":5}', '{\"max_duration_minutes\":5}', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            params![flow_id],
        )
        .expect("insert record node");
}

fn sidecar_request_server() -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind sidecar test server");
    let addr = listener.local_addr().expect("test server addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_thread = Arc::clone(&requests);
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .expect("set read timeout");
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        buffer.extend_from_slice(&chunk[..n]);
                        if n < chunk.len() {
                            break;
                        }
                    }
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            || err.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(_) => break,
                }
            }
            requests_for_thread
                .lock()
                .expect("lock requests")
                .push(String::from_utf8_lossy(&buffer).to_string());
            let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                );
        }
    });
    (format!("http://{}", addr), requests, handle)
}

fn in_memory_runtime_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'monitored',
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                auto_record INTEGER NOT NULL DEFAULT 0,
                priority INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE flows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'idle',
                current_node TEXT,
                published_version INTEGER NOT NULL DEFAULT 1,
                draft_version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
            );
            CREATE TABLE flow_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                node_key TEXT NOT NULL,
                position INTEGER NOT NULL,
                draft_config_json TEXT NOT NULL DEFAULT '{}',
                published_config_json TEXT NOT NULL DEFAULT '{}',
                draft_updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                published_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                UNIQUE(flow_id, node_key)
            );
            CREATE TABLE flow_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                definition_version INTEGER NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                trigger_reason TEXT,
                error TEXT
            );
            CREATE TABLE flow_node_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                flow_run_id INTEGER NOT NULL REFERENCES flow_runs(id) ON DELETE CASCADE,
                flow_id INTEGER NOT NULL REFERENCES flows(id) ON DELETE CASCADE,
                node_key TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT,
                ended_at TEXT,
                input_json TEXT,
                output_json TEXT,
                error TEXT
            );
            CREATE TABLE recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                room_id TEXT,
                status TEXT NOT NULL DEFAULT 'recording',
                started_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                ended_at TEXT,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                file_path TEXT,
                file_size_bytes INTEGER NOT NULL DEFAULT 0,
                stream_url TEXT,
                bitrate TEXT,
                error_message TEXT,
                auto_process INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
                flow_id INTEGER REFERENCES flows(id) ON DELETE SET NULL,
                flow_run_id INTEGER REFERENCES flow_runs(id) ON DELETE SET NULL
            );",
    )
    .expect("create runtime schema");
}

mod events;
mod polling;
mod recordings;
mod runs;
mod sessions;
