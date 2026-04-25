use super::*;

#[test]
fn restart_flow_run_updates_runtime_session_state_coherently() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    let runtime_manager = LiveRuntimeManager::new();

    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow session");
    let first_run_id = runtime_manager
        .handle_live_detected(
            &conn,
            1,
            &crate::tiktok::types::LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(77),
            },
        )
        .expect("handle live")
        .expect("create initial run");
    conn.execute(
        "UPDATE flows SET status = 'processing', current_node = 'clip' WHERE id = 1",
        [],
    )
    .expect("set stale runtime state");

    let result =
        restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).expect("restart flow run");

    assert_eq!(result.flow_id, 1);
    assert!(result.restarted);
    assert_eq!(runtime_manager.session_active_flow_run_id_for_test(1), None);
    assert!(runtime_manager.session_is_polling_for_test(1));
    assert!(runtime_manager.session_has_poll_task_for_test(1));
    assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);
    assert_eq!(
        runtime_manager.cancelled_poll_generations_for_test(1),
        vec![1]
    );

    let (run_status, run_error): (String, Option<String>) = conn
        .query_row(
            "SELECT status, error FROM flow_runs WHERE id = ?1",
            [first_run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read cancelled run");
    assert_eq!(run_status, "cancelled");
    assert_eq!(run_error.as_deref(), Some("Publish restart"));

    let (flow_status, current_node): (String, Option<String>) = conn
        .query_row(
            "SELECT status, current_node FROM flows WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read flow runtime state");
    assert_eq!(flow_status, "watching");
    assert_eq!(current_node.as_deref(), Some("start"));

    let snapshot = runtime_manager
        .take_latest_runtime_event_for_test()
        .expect("runtime event");
    assert_eq!(snapshot.status, "watching");
    assert_eq!(snapshot.current_node.as_deref(), Some("start"));
    assert_eq!(snapshot.active_flow_run_id, None);
    assert_eq!(runtime_manager.session_active_flow_run_id_for_test(1), None);

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn restart_flow_run_fails_cleanly_without_mutating_db_when_session_missing() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    let runtime_manager = LiveRuntimeManager::new();
    conn.execute(
        "INSERT INTO flow_runs (id, flow_id, definition_version, status, started_at, trigger_reason) \
         VALUES (11, 1, 1, 'running', datetime('now','+7 hours'), 'test')",
        [],
    )
    .expect("insert running flow run");
    conn.execute(
        "INSERT INTO flow_node_runs (flow_run_id, flow_id, node_key, status, started_at) \
         VALUES (11, 1, 'record', 'running', datetime('now','+7 hours'))",
        [],
    )
    .expect("insert running node run");

    let err = restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).unwrap_err();

    assert!(err.contains("missing live runtime session"));
    let runs: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, status FROM flow_runs WHERE flow_id = 1 ORDER BY id ASC")
            .expect("prepare flow run query");
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query flow runs")
            .map(|row| row.expect("map flow run row"))
            .collect()
    };
    assert_eq!(runs, vec![(11, "running".to_string())]);

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn restart_flow_run_refreshes_poll_task() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    let runtime_manager = LiveRuntimeManager::new();
    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow session");

    assert_eq!(runtime_manager.session_generation_for_test(1), Some(1));
    assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);

    restart_flow_run_with_conn(&mut conn, &runtime_manager, 1).expect("restart flow run");

    assert!(runtime_manager.session_has_poll_task_for_test(1));
    assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);
    assert_eq!(runtime_manager.session_generation_for_test(1), Some(2));
    assert!(runtime_manager.session_is_polling_for_test(1));
    assert_eq!(
        runtime_manager.cancelled_poll_generations_for_test(1),
        vec![1]
    );

    drop(conn);
    let _ = std::fs::remove_file(&path);
}
