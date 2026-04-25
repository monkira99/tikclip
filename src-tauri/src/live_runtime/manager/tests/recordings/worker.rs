use super::*;

#[test]
fn handle_live_detected_starts_rust_owned_execution_without_waiting_for_sidecar_finish() {
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

    let row: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, file_path, sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read recording row");

    assert_eq!(row.0, "recording");
    assert!(row
        .1
        .as_deref()
        .is_some_and(|value| value.contains("/records/") && value.ends_with(".mp4")));
    assert!(row
        .2
        .as_deref()
        .is_some_and(|value| value.starts_with("rust-recording-")));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rust_owned_recording_worker_completes_without_sidecar_finish_signal() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
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
                file_size_bytes: 321,
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

    manager.drain_worker_threads_for_test();
    let reader = crate::db::init::initialize_database(&path).expect("open reader");
    let status: String = reader
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("query finalized recording status");
    let flow_run_status: String = reader
        .query_row(
            "SELECT status FROM flow_runs WHERE id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read flow run status");

    assert_eq!(status, "done");
    assert_eq!(flow_run_status, "running");

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn manager_with_runtime_db_path_does_not_skip_rust_execution() {
    let (conn, path) = open_temp_db();
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

    let recording_key: String = conn
            .query_row(
                "SELECT sidecar_recording_id FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
                [flow_run_id],
                |row| row.get(0),
            )
            .expect("read recording key");

    assert!(recording_key.starts_with("rust-recording-"));

    drop(conn);
    let _ = std::fs::remove_file(path);
}
