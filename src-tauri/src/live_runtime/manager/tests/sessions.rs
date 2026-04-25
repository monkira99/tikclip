use super::*;

#[test]
fn bootstrap_enabled_flows_starts_enabled_flows_once() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, false, "shop_xyz");
    let manager = LiveRuntimeManager::new();

    manager
        .bootstrap_enabled_flows(&mut conn)
        .expect("first bootstrap");
    manager
        .bootstrap_enabled_flows(&mut conn)
        .expect("second bootstrap");

    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].flow_id, 1);
    assert_eq!(sessions[0].lookup_key, "shop_abc");
    assert_eq!(sessions[0].generation, 1);
    let logs = manager.list_runtime_logs_for_test(Some(1), Some(10));
    assert!(logs.iter().any(|entry| {
        entry.event == "session_bootstrap_started"
            && entry.stage == "session"
            && entry
                .context
                .get("flow_id")
                .and_then(|value| value.as_i64())
                == Some(1)
    }));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bootstrap_enabled_flows_cancels_orphaned_running_db_run() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (41, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert running flow run");
    conn.execute(
        "INSERT INTO accounts (id, username, display_name) VALUES (1, 'shop_abc', 'shop_abc')",
        [],
    )
    .expect("insert account");
    conn.execute(
            "INSERT INTO recordings (id, account_id, room_id, status, duration_seconds, flow_id, flow_run_id) \
             VALUES (91, 1, '7312345', 'recording', 0, 1, 41)",
            [],
        )
        .expect("insert orphaned recording row");
    let manager = LiveRuntimeManager::new();

    manager
        .bootstrap_enabled_flows(&mut conn)
        .expect("bootstrap enabled flows");

    assert_eq!(manager.session_active_flow_run_id_for_test(1), None);
    assert!(manager.session_is_polling_for_test(1));
    let (run_status, run_error): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error FROM flow_runs WHERE id = 41",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read flow run");
    assert_eq!(run_status, "cancelled");
    assert_eq!(run_error.as_deref(), Some("Interrupted by app restart"));
    let (recording_status, recording_error): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error_message FROM recordings WHERE id = 91",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read recording row");
    assert_eq!(recording_status, "cancelled");
    assert_eq!(
        recording_error.as_deref(),
        Some("Interrupted by app restart")
    );

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bootstrap_enabled_flows_skips_bad_flow_and_starts_valid_enabled_flow() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_xyz");
    conn.execute(
        "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = 2 AND node_key = 'start'",
        [r#"{"username":"   @   "}"#],
    )
    .expect("break flow config");
    let manager = LiveRuntimeManager::new();

    manager
        .bootstrap_enabled_flows(&mut conn)
        .expect("bootstrap should continue");

    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].flow_id, 1);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn acquire_username_lease_rejects_second_flow_with_same_lookup_key() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "Shop_ABC");
    insert_flow(&conn, 2, true, "@shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start first");
    let err = manager.start_flow_session(&conn, 2).unwrap_err();

    assert!(err.contains("username lease already held"));
    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 2);
    let failed = sessions
        .iter()
        .find(|snapshot| snapshot.flow_id == 2)
        .expect("failed snapshot retained");
    assert_eq!(failed.status, "error");
    assert!(failed
        .last_error
        .clone()
        .unwrap_or_default()
        .contains("username lease already held"));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reconcile_flow_after_publish_restarts_session_once() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start initial");
    manager.reconcile_flow(&conn, 1).expect("reconcile");

    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].flow_id, 1);
    assert_eq!(sessions[0].generation, 2);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reconcile_flow_restores_active_flow_run_id_from_running_db_run() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    conn.execute(
            "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
             VALUES (51, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
            [],
        )
        .expect("insert running flow run");

    manager.reconcile_flow(&conn, 1).expect("reconcile flow");

    assert_eq!(manager.session_active_flow_run_id_for_test(1), Some(51));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reconcile_flow_surfaces_failure_snapshot_when_replacement_fails() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_xyz");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start first");
    manager.start_flow_session(&conn, 2).expect("start second");
    conn.execute(
        "UPDATE flow_nodes SET published_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
        [r#"{"username":"shop_xyz"}"#],
    )
    .expect("change username to conflicting lease");

    let err = manager.reconcile_flow(&conn, 1).unwrap_err();

    assert!(err.contains("username lease already held"));
    let sessions = manager.list_sessions();
    assert_eq!(sessions.len(), 2);
    let session = sessions
        .iter()
        .find(|snapshot| snapshot.flow_id == 1)
        .expect("failed runtime snapshot retained");
    assert_eq!(session.lookup_key, "shop_abc");
    assert_eq!(session.generation, 1);
    assert_eq!(session.status, "error");
    assert!(session
        .last_error
        .clone()
        .unwrap_or_default()
        .contains("username lease already held"));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn shutdown_clears_sessions_and_leases() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start flow");
    manager.shutdown().expect("shutdown");

    assert!(manager.list_sessions().is_empty());
    manager.start_flow_session(&conn, 1).expect("start again");
    assert_eq!(manager.list_sessions().len(), 1);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_and_shutdown_invoke_session_teardown_hook() {
    let _guard = lock_teardown_test_guard_for_test();
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_xyz");
    let manager = LiveRuntimeManager::new();
    reset_teardown_call_count_for_test();

    manager.start_flow_session(&conn, 1).expect("start first");
    manager.start_flow_session(&conn, 2).expect("start second");
    let stopped = manager.stop_flow_session(1).expect("stop one session");

    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0].status, "stopped");
    assert_eq!(teardown_call_count_for_test(), 1);
    let shutdown = manager.shutdown().expect("shutdown");
    assert_eq!(shutdown.len(), 1);
    assert_eq!(shutdown[0].status, "stopped");
    assert_eq!(teardown_call_count_for_test(), 2);
    assert!(manager.list_sessions().is_empty());

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn start_flow_session_emits_lease_conflict_runtime_log() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    insert_flow(&conn, 2, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager
        .start_flow_session(&conn, 1)
        .expect("start first session");
    let error = manager
        .start_flow_session(&conn, 2)
        .expect_err("second session should hit username lease conflict");

    assert!(error.contains("username lease already held"));
    let logs = manager.list_runtime_logs_for_test(Some(2), Some(10));
    assert!(logs.iter().any(|entry| {
        entry.event == "lease_conflict"
            && entry.code.as_deref() == Some("start.username_conflict")
            && entry.stage == "start"
            && entry
                .context
                .get("lookup_key")
                .and_then(|value| value.as_str())
                == Some("shop_abc")
    }));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn start_flow_session_creates_single_poll_task_for_enabled_flow() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager
        .start_flow_session(&conn, 1)
        .expect("start enabled flow session");
    manager
        .start_flow_session(&conn, 1)
        .expect("second start should not duplicate task");

    assert!(manager.session_has_poll_task_for_test(1));
    assert_eq!(manager.active_poll_task_count_for_test(), 1);
    assert_eq!(manager.session_generation_for_test(1), Some(1));

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_flow_session_cancels_and_removes_poll_task() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start flow");
    manager.stop_flow_session(1).expect("stop flow");

    assert!(!manager.session_has_poll_task_for_test(1));
    assert_eq!(manager.active_poll_task_count_for_test(), 0);
    assert_eq!(manager.cancelled_poll_generations_for_test(1), vec![1]);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reconcile_flow_replaces_poll_task_without_duplicates() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start flow");
    assert_eq!(manager.session_generation_for_test(1), Some(1));
    assert_eq!(manager.active_poll_task_count_for_test(), 1);

    manager.reconcile_flow(&conn, 1).expect("reconcile flow");

    assert!(manager.session_has_poll_task_for_test(1));
    assert_eq!(manager.active_poll_task_count_for_test(), 1);
    assert_eq!(manager.session_generation_for_test(1), Some(2));
    assert_eq!(manager.cancelled_poll_generations_for_test(1), vec![1]);

    drop(conn);
    let _ = std::fs::remove_file(path);
}
