use super::*;

#[test]
fn restored_last_completed_room_id_prevents_duplicate_run_for_same_on_air_room() {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    in_memory_runtime_schema(&conn);
    insert_flow(&conn, 1, true, "shop_abc");
    conn.execute(
            "INSERT INTO accounts (id, username, display_name, type, created_at, updated_at) \
             VALUES (1, 'shop_abc', 'Shop ABC', 'monitored', datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert account");
    conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, ended_at, trigger_reason) \
             VALUES (11, 1, 1, 'completed', datetime('now','+7 hours'), datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert completed flow run");
    conn.execute(
            "INSERT INTO recordings (account_id, room_id, status, duration_seconds, file_size_bytes, flow_id, flow_run_id, created_at, started_at) \
             VALUES (1, '7312345', 'done', 0, 0, 1, 11, datetime('now','+7 hours'), datetime('now','+7 hours'))",
            [],
        )
        .expect("insert recording");
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
        .expect("handle live");

    assert_eq!(flow_run_id, None);
    let running_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1 AND status = 'running'",
            [],
            |row| row.get(0),
        )
        .expect("count running runs");
    assert_eq!(running_count, 0);
}

#[test]
fn handle_live_detected_does_not_create_run_when_stream_url_is_missing() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    let flow_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: None,
                viewer_count: Some(77),
            },
        )
        .expect("handle live without stream url");

    assert_eq!(flow_run_id, None);
    assert!(manager.session_is_polling_for_test(1));
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(10));
    assert!(logs.iter().any(|entry| {
        entry.event == "stream_url_missing"
            && entry.code.as_deref() == Some("start.stream_url_missing")
            && entry.stage == "start"
    }));
    let run_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("count flow runs");
    assert_eq!(run_count, 0);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn successful_record_completion_keeps_flow_run_alive_for_downstream_stages() {
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
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 1,
                outcome: RecordingOutcome::Success,
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
        .expect("created flow run");

    manager
        .complete_active_run(&mut conn, 1, Some("7312345"))
        .expect("complete run");

    let flow_run_status: String = conn
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");
    assert_eq!(flow_run_status, "running");
    assert!(!manager.session_is_polling_for_test(1));
    assert_eq!(
        manager.session_active_flow_run_id_for_test(1),
        Some(flow_run_id)
    );

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn successful_record_completion_without_explicit_room_id_keeps_active_run_alive() {
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
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 1,
                outcome: RecordingOutcome::Success,
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
        .expect("created flow run");

    manager
        .complete_active_run(&mut conn, 1, None)
        .expect("complete run");

    let flow_run_status: String = conn
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");
    assert_eq!(flow_run_status, "running");
    assert_eq!(
        manager.session_active_flow_run_id_for_test(1),
        Some(flow_run_id)
    );

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_live_detected_creates_rust_owned_recording_row_for_active_run() {
    let (conn, path) = open_temp_db();
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

    let row: (i64, Option<i64>, Option<String>, String) = conn
            .query_row(
                "SELECT flow_id, flow_run_id, room_id, status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read rust-owned recording row");

    assert_eq!(row.0, 1);
    assert_eq!(row.1, Some(flow_run_id));
    assert_eq!(row.2.as_deref(), Some("7312345"));
    assert_eq!(row.3, "recording");

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn complete_active_run_finalizes_rust_owned_recording_row_and_advances_stage() {
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
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 1,
                outcome: RecordingOutcome::Success,
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
        .expect("created flow run");

    manager
        .complete_active_run(&mut conn, 1, Some("7312345"))
        .expect("complete run");

    let row: (String, Option<String>) = conn
            .query_row(
                "SELECT status, ended_at FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read finalized recording row");

    assert_eq!(row.0, "done");
    assert!(row.1.is_some());
    assert!(!manager.session_is_polling_for_test(1));
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
    assert!(logs.iter().any(|entry| {
        entry.event == "record_completed"
            && entry.flow_run_id == Some(flow_run_id)
            && entry.stage == "record"
    }));
    assert_eq!(
        manager.session_active_flow_run_id_for_test(1),
        Some(flow_run_id)
    );

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn offline_transition_allows_same_room_id_to_start_again_after_stage_reset() {
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
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 1,
                outcome: RecordingOutcome::Success,
            })
        }),
    );

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
        .expect("first live detect")
        .expect("first run created");
    manager
        .complete_active_run(&mut conn, 1, Some("7312345"))
        .expect("complete first run");
    let duplicate_before_reset = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(88),
            },
        )
        .expect("second live detect before reset");
    assert_eq!(duplicate_before_reset, None);

    manager.mark_source_offline(1).expect("mark source offline");
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(20));
    assert!(logs.iter().any(|entry| {
        entry.event == "source_offline_marked"
            && entry.stage == "session"
            && entry
                .context
                .get("flow_id")
                .and_then(|value| value.as_i64())
                == Some(1)
    }));

    let second_run_id = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(99),
            },
        )
        .expect("live detect after offline")
        .expect("second run created");

    assert_ne!(first_run_id, second_run_id);

    drop(conn);
    let _ = std::fs::remove_file(path);
}
