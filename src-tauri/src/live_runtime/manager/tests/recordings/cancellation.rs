use super::*;

#[test]
fn cancelled_session_path_tears_down_exactly_once() {
    let _guard = lock_teardown_test_guard_for_test();
    reset_teardown_call_count_for_test();
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::with_recording_process_runner_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingProcessRunner::from_fn(|input, output_path| {
            let cancelled = Arc::new(Mutex::new(false));
            let cancelled_for_wait = Arc::clone(&cancelled);
            let cancelled_for_cancel = Arc::clone(&cancelled);
            let output_path = output_path.to_string();
            let input = input.clone();
            Ok(
                crate::recording_runtime::worker::RecordingProcessHandle::from_parts(
                    Box::new(move || {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        let outcome = if *cancelled_for_wait.lock().expect("lock cancel flag") {
                            RecordingOutcome::Cancelled
                        } else {
                            RecordingOutcome::Success
                        };
                        Ok(crate::recording_runtime::types::RecordingFinishInput {
                            account_id: input.account_id,
                            flow_id: input.flow_id,
                            flow_run_id: input.flow_run_id,
                            external_recording_id: input.external_recording_id.clone(),
                            room_id: input.room_id.clone(),
                            file_path: Some(output_path.clone()),
                            error_message: None,
                            duration_seconds: 0,
                            file_size_bytes: 0,
                            outcome,
                        })
                    }),
                    Arc::new(move || {
                        *cancelled_for_cancel.lock().expect("lock cancel flag") = true;
                        Ok(())
                    }),
                ),
            )
        }),
    );

    manager.start_flow_session(&conn, 1).expect("start session");
    manager
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

    manager.stop_flow_session(1).expect("cancel flow session");
    manager.drain_worker_threads_for_test();

    assert_eq!(teardown_call_count_for_test(), 1);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn cancel_finalization_marks_recording_record_node_and_flow_run_cancelled() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
            std::thread::sleep(std::time::Duration::from_millis(300));
            Ok(crate::recording_runtime::types::RecordingFinishInput {
                account_id: input.account_id,
                flow_id: input.flow_id,
                flow_run_id: input.flow_run_id,
                external_recording_id: input.external_recording_id.clone(),
                room_id: input.room_id.clone(),
                file_path: Some(output_path.to_string()),
                error_message: None,
                duration_seconds: 0,
                file_size_bytes: 0,
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
        .expect("created run");

    manager.stop_flow_session(1).expect("cancel flow session");
    manager.drain_worker_threads_for_test();

    let recording_status: String = conn
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read recording status");
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

    assert_eq!(recording_status, "cancelled");
    assert_eq!(flow_run_status, "cancelled");
    assert_eq!(record_node_status, "cancelled");

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_and_shutdown_cancel_active_rust_worker_execution_deterministically() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::with_recording_runner_autospawn_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingRunner::from_fn(|input, output_path| {
            std::thread::sleep(std::time::Duration::from_millis(500));
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

    let stopped = manager.stop_flow_session(1).expect("stop flow session");
    assert_eq!(stopped.len(), 1);
    manager.drain_worker_threads_for_test();

    let reader = crate::db::init::initialize_database(&path).expect("open reader");
    let recording_status: String = reader
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read cancelled recording status");

    assert_eq!(recording_status, "cancelled");

    drop(reader);
    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_flow_session_cancels_process_even_when_cancel_handle_registers_late() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let cancel_call_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let manager = LiveRuntimeManager::with_recording_process_runner_for_test(
        path.clone(),
        crate::recording_runtime::worker::RecordingProcessRunner::from_fn({
            let cancel_call_count = Arc::clone(&cancel_call_count);
            move |input, output_path| {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let output_path = output_path.to_string();
                let input = input.clone();
                let cancel_call_count = Arc::clone(&cancel_call_count);
                Ok(
                    crate::recording_runtime::worker::RecordingProcessHandle::from_parts(
                        Box::new(move || {
                            std::thread::sleep(std::time::Duration::from_millis(250));
                            Ok(crate::recording_runtime::types::RecordingFinishInput {
                                account_id: input.account_id,
                                flow_id: input.flow_id,
                                flow_run_id: input.flow_run_id,
                                external_recording_id: input.external_recording_id.clone(),
                                room_id: input.room_id.clone(),
                                file_path: Some(output_path.clone()),
                                error_message: None,
                                duration_seconds: 0,
                                file_size_bytes: 0,
                                outcome: RecordingOutcome::Success,
                            })
                        }),
                        Arc::new(move || {
                            cancel_call_count.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }),
                    ),
                )
            }
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

    std::thread::sleep(std::time::Duration::from_millis(50));
    manager.stop_flow_session(1).expect("stop flow session");
    manager.drain_worker_threads_for_test();

    assert_eq!(cancel_call_count.load(Ordering::SeqCst), 1);

    let reader = crate::db::init::initialize_database(&path).expect("open reader");
    let recording_status: String = reader
        .query_row(
            "SELECT status FROM recordings WHERE flow_run_id = ?1 ORDER BY id DESC LIMIT 1",
            [flow_run_id],
            |row| row.get(0),
        )
        .expect("read recording status");
    assert_eq!(recording_status, "cancelled");

    drop(reader);
    drop(conn);
    let _ = std::fs::remove_file(path);
}
