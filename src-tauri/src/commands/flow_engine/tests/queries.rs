use super::*;

#[test]
fn flow_editor_payload_serializes_count_fields_in_snake_case() {
    let (conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);

    let payload = get_flow_definition_with_conn(&conn, flow_id).expect("load flow definition");
    let value = serde_json::to_value(&payload).expect("serialize flow editor payload");

    assert!(value.get("nodeRuns").is_some());
    assert!(value.get("recordings_count").is_some());
    assert!(value.get("clips_count").is_some());
    assert!(value.get("recordingsCount").is_none());
    assert!(value.get("clipsCount").is_none());

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn get_flow_definition_allows_blank_start_username_for_new_flow() {
    let (conn, path) = open_temp_db();
    conn.execute(
        "INSERT INTO flows (name, enabled, status, published_version, draft_version) VALUES ('t', 1, 'idle', 1, 1)",
        [],
    )
    .expect("insert flow");
    let flow_id = conn.last_insert_rowid();
    for (node_key, position, config_json) in [
        (
            "start",
            1i64,
            r#"{"username":"","cookies_json":"","proxy_url":"","poll_interval_seconds":60,"retry_limit":3}"#,
        ),
        ("record", 2i64, r#"{"max_duration_minutes":5}"#),
        ("clip", 3i64, "{}"),
        ("caption", 4i64, "{}"),
        ("upload", 5i64, "{}"),
    ] {
        conn.execute(
            "INSERT INTO flow_nodes (flow_id, node_key, position, draft_config_json, published_config_json) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![flow_id, node_key, position, config_json],
        )
        .expect("insert flow node");
    }

    let result = get_flow_definition_with_conn(&conn, flow_id);

    assert!(result.is_ok());

    drop(conn);
    let _ = std::fs::remove_file(&path);
}
