use super::*;

#[test]
fn publish_failure_due_to_reconcile_conflict_does_not_advance_published_db_state() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    insert_flow_with_username(&conn, 2, "shop_xyz");
    let runtime_manager = LiveRuntimeManager::new();

    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow 1 session");
    runtime_manager
        .start_flow_session(&conn, 2)
        .expect("start flow 2 session");
    conn.execute(
        "UPDATE flows SET published_version = 3, draft_version = 4 WHERE id = 1",
        [],
    )
    .expect("seed versions");
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
        [r#"{"username":"shop_xyz"}"#],
    )
    .expect("set conflicting draft");

    let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

    assert!(err.contains("username lease already held"));
    let (published_version, draft_version, published_username): (i64, i64, String) = conn
        .query_row(
            "SELECT f.published_version, f.draft_version, json_extract(n.published_config_json, '$.username') \
             FROM flows f JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
             WHERE f.id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read persisted publish state");
    assert_eq!(published_version, 3);
    assert_eq!(draft_version, 4);
    assert_eq!(published_username, "shop_abc");

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_failure_due_to_reconcile_conflict_leaves_no_active_old_session_running() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    insert_flow_with_username(&conn, 2, "shop_xyz");
    let runtime_manager = LiveRuntimeManager::new();

    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow 1 session");
    runtime_manager
        .start_flow_session(&conn, 2)
        .expect("start flow 2 session");
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
        [r#"{"username":"shop_xyz"}"#],
    )
    .expect("set conflicting draft");

    let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

    assert!(err.contains("username lease already held"));
    let flow_one_snapshot = runtime_manager
        .list_sessions()
        .into_iter()
        .find(|snapshot| snapshot.flow_id == 1)
        .expect("failed runtime snapshot should remain");
    assert_eq!(flow_one_snapshot.status, "error");
    assert_eq!(flow_one_snapshot.lookup_key, "shop_abc");
    assert!(flow_one_snapshot
        .last_error
        .unwrap_or_default()
        .contains("username lease already held"));

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_failure_due_to_reconcile_conflict_sets_runtime_error_state() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    insert_flow_with_username(&conn, 2, "shop_xyz");
    let runtime_manager = LiveRuntimeManager::new();

    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow 1 session");
    runtime_manager
        .start_flow_session(&conn, 2)
        .expect("start flow 2 session");
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = 1 AND node_key = 'start'",
        [r#"{"username":"shop_xyz"}"#],
    )
    .expect("set conflicting draft");

    let err = publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1).unwrap_err();

    assert!(err.contains("username lease already held"));
    let (status, current_node, last_error): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT status, current_node, json_extract(published_config_json, '$.last_error') \
             FROM flows f LEFT JOIN flow_nodes n ON n.flow_id = f.id AND n.node_key = 'start' \
             WHERE f.id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read runtime error state");
    assert_eq!(status, "error");
    assert_eq!(current_node.as_deref(), Some("start"));
    assert!(last_error
        .unwrap_or_default()
        .contains("username lease already held"));

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_runtime_reconcile_keeps_one_fresh_poll_task() {
    let (mut conn, path) = open_temp_db();
    insert_flow_with_username(&conn, 1, "shop_abc");
    let runtime_manager = LiveRuntimeManager::new();
    runtime_manager
        .start_flow_session(&conn, 1)
        .expect("start flow session");

    assert_eq!(runtime_manager.session_generation_for_test(1), Some(1));
    assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);

    publish_flow_with_runtime_reconcile(&mut conn, &runtime_manager, 1)
        .expect("publish with reconcile");

    assert!(runtime_manager.session_has_poll_task_for_test(1));
    assert_eq!(runtime_manager.active_poll_task_count_for_test(), 1);
    assert_eq!(runtime_manager.session_generation_for_test(1), Some(2));
    assert_eq!(
        runtime_manager.cancelled_poll_generations_for_test(1),
        vec![1]
    );

    drop(conn);
    let _ = std::fs::remove_file(&path);
}
