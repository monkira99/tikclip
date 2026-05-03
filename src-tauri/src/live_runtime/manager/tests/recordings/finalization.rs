use super::*;

#[test]
fn finalize_active_recording_uses_durable_external_recording_key() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_xyz");
    let manager = LiveRuntimeManager::new();

    manager
        .start_flow_session(&conn, 1)
        .expect("start first session");
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

    conn.execute(
        "UPDATE accounts SET username = 'shop_moved' WHERE id = 1",
        [],
    )
    .expect("move account away from original flow");

    manager
        .finalize_recording_by_key(
            &mut conn,
            &external_recording_id,
            Some("7312345"),
            true,
            None,
        )
        .expect("finalize by durable key");

    let row: (String, Option<i64>) = conn
        .query_row(
            "SELECT status, flow_run_id FROM recordings WHERE external_recording_id = ?1",
            [&external_recording_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read finalized row");

    assert_eq!(row.0, "done");
    assert_eq!(row.1, Some(flow_run_id));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn successful_finalization_with_downstream_failure_marks_session_error() {
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
        .expect("finalize success");

    assert_eq!(
        manager.session_active_flow_run_id_for_test(1),
        Some(flow_run_id)
    );
    assert!(!manager.session_is_polling_for_test(1));
    let snapshot = manager
        .list_sessions()
        .into_iter()
        .find(|snapshot| snapshot.flow_id == 1)
        .expect("session snapshot");
    assert_eq!(snapshot.status, "error");

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn successful_finalization_emits_runtime_update_event_for_downstream_failure() {
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
        .expect("finalize success");

    let snapshot = manager
        .take_latest_runtime_event_for_test()
        .expect("runtime event snapshot");
    assert_eq!(snapshot.flow_id, 1);
    assert_eq!(snapshot.status, "error");
    assert_eq!(snapshot.current_node.as_deref(), Some("record"));
    assert_eq!(snapshot.active_flow_run_id, None);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn finalize_by_external_recording_key_updates_targeted_run_not_latest_running_run() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    let first_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("handle first live")
        .expect("created first run");
    let first_recording_key: String = conn
            .query_row(
                "SELECT external_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [first_run_id],
                |row| row.get(0),
            )
            .expect("read first recording key");

    conn.execute(
        "UPDATE flow_runs SET status = 'running' WHERE id = ?1",
        [first_run_id],
    )
    .expect("keep first run running for targeted finalize test");
    conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) VALUES (999, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert newer running flow run");
    conn.execute(
            "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) VALUES (999, 1, 'record', 'running', datetime('now','+7 hours'))",
            [],
        )
        .expect("insert newer running record node");

    manager
        .finalize_recording_by_key(&mut conn, &first_recording_key, Some("7312345"), true, None)
        .expect("finalize first recording by key");

    let first_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'record' ORDER BY id DESC LIMIT 1",
                [first_run_id],
                |row| row.get(0),
            )
            .expect("read first node status");
    let second_node_status: String = conn
            .query_row(
                "SELECT status FROM flow_node_runs WHERE flow_run_id = 999 AND node_key = 'record' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("read second node status");

    assert_eq!(first_node_status, "completed");
    assert_eq!(second_node_status, "running");

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_and_shutdown_cancel_tracked_rust_recording_execution_and_finalize_rows() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_xyz");
    let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            Ok(crate::recording_runtime::types::RecordingFinishInput {
                account_id: input.account_id,
                flow_id: input.flow_id,
                flow_run_id: input.flow_run_id,
                external_recording_id: input.external_recording_id.clone(),
                room_id: input.room_id.clone(),
                file_path: Some(output_path.to_string()),
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 0,
                outcome: RecordingOutcome::Success,
            })
        }),
    );

    manager
        .start_flow_session(&conn, 1)
        .expect("start first session");
    manager
        .start_flow_session(&conn, 2)
        .expect("start second session");
    let first_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("handle first live")
        .expect("first run created");
    let second_run_id = manager
        .handle_live_detected(
            &conn,
            2,
            &LiveStatus {
                room_id: "8888".to_string(),
                stream_url: Some("https://example.com/other.flv".to_string()),
                viewer_count: Some(55),
            },
        )
        .expect("handle second live")
        .expect("second run created");

    let stopped = manager.stop_flow_session(1).expect("stop first flow");
    assert_eq!(stopped.len(), 1);
    let shutdown = manager.shutdown().expect("shutdown manager");
    assert_eq!(shutdown.len(), 1);
    manager.drain_worker_threads_for_test();

    let reader = crate::db::init::initialize_database(&path).expect("open reader");
    let first_status: String = reader
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
            [first_run_id],
            |row| row.get(0),
        )
        .expect("read first recording status");
    let second_status: String = reader
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
            [second_run_id],
            |row| row.get(0),
        )
        .expect("read second recording status");

    assert_eq!(first_status, "cancelled");
    assert_eq!(second_status, "cancelled");

    manager.drain_worker_threads_for_test();
    drop(reader);
    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn failure_finalization_closes_record_node_and_flow_run_failed() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::with_recording_runner_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
            Ok(crate::recording_runtime::types::RecordingFinishInput {
                account_id: input.account_id,
                flow_id: input.flow_id,
                flow_run_id: input.flow_run_id,
                external_recording_id: input.external_recording_id.clone(),
                room_id: input.room_id.clone(),
                file_path: Some(output_path.to_string()),
                error_message: Some("ffmpeg failed".to_string()),
                duration_seconds: 0,
                file_size_bytes: 0,
                outcome: RecordingOutcome::Failed,
            })
        }),
    );

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
        .expect("created run");
    let external_recording_id: String = conn
        .query_row(
            "SELECT external_recording_id FROM recordings WHERE flow_run_id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read recording key");

    manager
        .finalize_recording_by_key(
            &mut conn,
            &external_recording_id,
            Some("7312345"),
            false,
            Some("ffmpeg failed"),
        )
        .expect("finalize failure");

    let flow_run_status: String = conn
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");
    let record_node_status: String = conn
        .query_row(
            "SELECT status FROM flow_node_runs WHERE flow_run_id = ?1 AND node_key = 'record'",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read record node status");

    assert_eq!(flow_run_status, "failed");
    assert_eq!(record_node_status, "failed");

    drop(conn);
    let _ = std::fs::remove_file(path);
}
