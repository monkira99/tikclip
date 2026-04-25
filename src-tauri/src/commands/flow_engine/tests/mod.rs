use super::{
    apply_flow_node_draft, get_flow_definition_with_conn, publish_flow_definition_with_conn,
    publish_flow_with_runtime_reconcile, restart_flow_run_with_conn,
};
use crate::db::init::initialize_database;
use crate::live_runtime::manager::LiveRuntimeManager;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

fn open_temp_db() -> (Connection, PathBuf) {
    let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "tikclip-flow-engine-test-{}-{}-{}.db",
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

fn insert_flow_with_nodes(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO flows (name, enabled, status) VALUES ('t', 1, 'idle')",
        [],
    )
    .expect("insert flow");
    let flow_id = conn.last_insert_rowid();

    for (node_key, position, config_json) in [
        ("start", 1i64, r#"{"username":"shop_abc"}"#),
        ("record", 2i64, r#"{"max_duration_minutes":5}"#),
        ("clip", 3i64, "{}"),
        ("caption", 4i64, "{}"),
        ("upload", 5i64, "{}"),
    ] {
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![flow_id, node_key, position, config_json],
        )
        .expect("insert flow node");
    }

    flow_id
}

fn insert_flow_with_username(conn: &Connection, flow_id: i64, username: &str) {
    conn.execute(
        "INSERT INTO flows (id, name, enabled, status, published_version, draft_version) VALUES (?1, ?2, 1, 'idle', 1, 1)",
        params![flow_id, format!("Flow {flow_id}")],
    )
    .expect("insert flow");

    for (node_key, position, config_json) in [
        ("start", 1i64, format!(r#"{{"username":"{username}"}}"#)),
        ("record", 2i64, r#"{"max_duration_minutes":5}"#.to_string()),
        ("clip", 3i64, "{}".to_string()),
        ("caption", 4i64, "{}".to_string()),
        ("upload", 5i64, "{}".to_string()),
    ] {
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![flow_id, node_key, position, config_json],
        )
        .expect("insert flow node");
    }
}

mod drafts;
mod publish;
mod queries;
mod reconcile;
mod restart;
