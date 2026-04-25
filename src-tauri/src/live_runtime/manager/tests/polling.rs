use super::*;

#[test]
fn poll_loop_autonomous_live_detect_starts_run() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
        room_id: "7312345".to_string(),
        stream_url: Some("https://example.com/live.flv".to_string()),
        viewer_count: Some(77),
    }))]);

    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("run one poll iteration");

    assert!(manager.session_active_flow_run_id_for_test(1).is_some());

    let run_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_runs WHERE flow_id = 1 AND status = 'running'",
            [],
            |row| row.get(0),
        )
        .expect("count running runs");
    assert_eq!(run_count, 1);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn poll_loop_live_without_stream_keeps_watching() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
        room_id: "7312345".to_string(),
        stream_url: None,
        viewer_count: Some(77),
    }))]);

    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("run one poll iteration");

    assert!(manager.session_is_polling_for_test(1));
    assert_eq!(manager.session_active_flow_run_id_for_test(1), None);
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
fn poll_loop_offline_resets_dedupe_and_allows_same_room_again() {
    let (mut conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
        room_id: "7312345".to_string(),
        stream_url: Some("https://example.com/live.flv".to_string()),
        viewer_count: Some(77),
    }))]);
    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("first live poll iteration");
    let first_run_id = manager
        .session_active_flow_run_id_for_test(1)
        .expect("first run id");

    manager
        .fail_active_run(&mut conn, 1, Some("7312345"), Some("test failure"))
        .expect("finalize first run to watching state");

    let duplicate_before_offline = manager
        .handle_live_detected(
            &conn,
            1,
            &LiveStatus {
                room_id: "7312345".to_string(),
                stream_url: Some("https://example.com/live.flv".to_string()),
                viewer_count: Some(88),
            },
        )
        .expect("duplicate detect before offline reset");
    assert_eq!(duplicate_before_offline, None);

    manager.with_stubbed_live_status_for_test(vec![
        Ok(None),
        Ok(Some(LiveStatus {
            room_id: "7312345".to_string(),
            stream_url: Some("https://example.com/live.flv".to_string()),
            viewer_count: Some(99),
        })),
    ]);

    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("offline poll iteration");
    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("live poll iteration after offline");

    let second_run_id = manager
        .session_active_flow_run_id_for_test(1)
        .expect("second run id");
    assert_ne!(first_run_id, second_run_id);

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn poll_loop_stale_iteration_is_dropped_before_apply() {
    let (conn, path) = open_temp_db();
    insert_flow(&conn, 1, true, "shop_abc");
    let manager = LiveRuntimeManager::new();

    manager.start_flow_session(&conn, 1).expect("start session");
    manager.with_stubbed_live_status_for_test(vec![Ok(Some(LiveStatus {
        room_id: "7312345".to_string(),
        stream_url: Some("https://example.com/live.flv".to_string()),
        viewer_count: Some(77),
    }))]);
    let manager_for_hook = manager.clone();
    manager.with_before_poll_apply_hook_for_test(move || {
        manager_for_hook
            .stop_flow_session(1)
            .expect("stop session in stale hook");
    });

    manager
        .run_one_poll_iteration_for_test(&conn, 1)
        .expect("run one poll iteration");

    assert_eq!(manager.session_active_flow_run_id_for_test(1), None);
    assert!(manager.list_sessions().is_empty());
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
