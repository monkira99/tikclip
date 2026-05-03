use super::*;

#[test]
fn successful_finalization_with_processing_failure_marks_pipeline_failed_without_sidecar_handoff() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
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
            "SELECT external_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
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
        .expect("finalize success with processing failure handling");

    let flow_run_status: String = conn
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");
    let clip_node_status: String = conn
        .query_row(
            "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'clip' ORDER BY id DESC LIMIT 1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read failed clip node run");
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
    assert_eq!(clip_node_status, "failed");
    assert_eq!(flow_status.0, "error");
    assert_eq!(flow_status.1.as_deref(), Some("record"));
    assert_eq!(
        flow_status.2.as_deref(),
        Some("Rust audio processing failed")
    );
    assert_eq!(snapshot.status, "error");
    assert_eq!(snapshot.current_node.as_deref(), Some("record"));

    let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
    assert!(logs.iter().any(|entry| {
        entry.event == "record_completed"
            && entry.flow_run_id == Some(flow_run_id)
            && entry.external_recording_id.as_deref() == Some(external_recording_id.as_str())
            && entry.stage == "record"
    }));
    assert!(!logs
        .iter()
        .any(|entry| entry.event.starts_with("sidecar_handoff")));

    drop(conn);
    let _ = std::fs::remove_file(path);
}
