use super::*;

#[test]
fn successful_finalization_triggers_sidecar_processing_handoff_with_external_key() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let (sidecar_base, requests, server_handle) = sidecar_request_server();
    conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('sidecar_port', ?1, datetime('now','+7 hours'))",
            [sidecar_base.trim_start_matches("http://127.0.0.1:")],
        )
        .expect("insert sidecar port setting");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    let flow_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("handle live")
        .expect("created flow run");
    let recording: (String, String) = conn
            .query_row(
                "SELECT sidecar_recording_id, file_path FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read recording row");

    manager
        .finalize_recording_by_key(&mut conn, &recording.0, Some("7312345"), true, None)
        .expect("finalize success");

    server_handle.join().expect("join sidecar server");
    let requests = requests.lock().expect("lock requests");
    let payload = requests.first().expect("captured handoff request");
    let body = payload.split("\r\n\r\n").nth(1).expect("http request body");
    let body_json: serde_json::Value =
        serde_json::from_str(body).expect("parse handoff request json");

    assert!(payload.contains("POST /api/video/process HTTP/1.1"));
    assert_eq!(
        body_json.get("recording_id").and_then(|v| v.as_str()),
        Some(recording.0.as_str())
    );
    assert_eq!(
        body_json.get("account_id").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        body_json.get("username").and_then(|v| v.as_str()),
        Some("shop_abc")
    );
    assert_eq!(
        body_json.get("file_path").and_then(|v| v.as_str()),
        Some(recording.1.as_str())
    );
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
    let record_completed_index = logs
        .iter()
        .position(|entry| {
            entry.event == "record_completed"
                && entry.flow_run_id == Some(flow_run_id)
                && entry.external_recording_id.as_deref() == Some(recording.0.as_str())
                && entry.stage == "record"
        })
        .expect("record_completed log index");
    let handoff_completed_index = logs
        .iter()
        .position(|entry| {
            entry.event == "sidecar_handoff_completed"
                && entry.flow_run_id == Some(flow_run_id)
                && entry.external_recording_id.as_deref() == Some(recording.0.as_str())
                && entry.stage == "clip"
        })
        .expect("sidecar_handoff_completed log index");
    assert!(record_completed_index < handoff_completed_index);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn handoff_failure_marks_clip_stage_failed_and_emits_runtime_update() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('sidecar_port', '1', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert invalid sidecar port");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    let flow_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("handle live")
        .expect("created flow run");
    let external_recording_id: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

    manager
        .finalize_recording_by_key(
            &mut conn,
            &external_recording_id,
            Some("7312345"),
            true,
            None,
        )
        .expect("finalize success with handoff failure handling");

    let flow_run_status: String = conn
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");
    let clip_node_status: Option<String> = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'clip' ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .ok();
    let flow_status: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, current_node, json_extract(published_config_json, '$.last_error') FROM flows f JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' WHERE f.id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read flow status");
    let snapshot = manager
        .take_latest_runtime_event_for_test()
        .expect("runtime event snapshot");

    assert_eq!(flow_run_status, "failed");
    assert_eq!(clip_node_status.as_deref(), Some("failed"));
    assert_eq!(flow_status.0, "error");
    assert_eq!(flow_status.1.as_deref(), Some("clip"));
    let last_error = flow_status.2.unwrap_or_default();
    assert!(!last_error.is_empty());
    assert!(
        last_error.contains("error")
            || last_error.contains("refused")
            || last_error.contains("connect")
    );
    assert_eq!(snapshot.status, "error");
    assert_eq!(snapshot.current_node.as_deref(), Some("clip"));
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
    let record_completed_index = logs
        .iter()
        .position(|entry| {
            entry.event == "record_completed"
                && entry.flow_run_id == Some(flow_run_id)
                && entry.external_recording_id.as_deref() == Some(external_recording_id.as_str())
                && entry.stage == "record"
        })
        .expect("record_completed log index");
    let handoff_failed_index = logs
        .iter()
        .position(|entry| {
            entry.event == "sidecar_handoff_failed"
                && entry.flow_run_id == Some(flow_run_id)
                && entry.external_recording_id.as_deref() == Some(external_recording_id.as_str())
                && entry.stage == "clip"
                && entry.code.as_deref() == Some("handoff.http_failed")
        })
        .expect("sidecar_handoff_failed log index");
    assert!(record_completed_index < handoff_failed_index);

    drop(conn);
    let _ = std::fs::remove_file(path);
}
