use super::*;

#[test]
fn publish_flow_definition_rejects_empty_start_username() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
        params![r#"{"username":"   @   "}"#, flow_id],
    )
    .expect("update start draft");

    let err = publish_flow_definition_with_conn(&mut conn, flow_id).unwrap_err();

    assert!(err.contains("username is required"));
    let published_username: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read published start");
    assert_eq!(published_username, r#"{"username":"shop_abc"}"#);

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_definition_rejects_malformed_record_duration() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
        params![r#"{"max_duration_minutes":"oops"}"#, flow_id],
    )
    .expect("update record draft");

    let err = publish_flow_definition_with_conn(&mut conn, flow_id).unwrap_err();

    assert!(err.contains("invalid record config"));
    let published_record: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read published record");
    assert_eq!(published_record, r#"{"max_duration_minutes":5}"#);

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_definition_canonicalizes_start_username_before_persist() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
        params![
            r#"{"username":" @shop_abc ","cookies_json":"{}","proxy_url":"","poll_interval_seconds":20,"retry_limit":3}"#,
            flow_id
        ],
    )
    .expect("update start draft");

    publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

    let published_username: String = conn
        .query_row(
            "SELECT json_extract(published_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read canonical username");
    assert_eq!(published_username, "shop_abc");

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_definition_canonicalizes_start_legacy_keys_to_snake_case() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'start'",
        params![
            r#"{"username":" @shop_abc ","cookiesJson":"{}","proxyUrl":"http://127.0.0.1:9000","pollIntervalSeconds":20,"retryLimit":4}"#,
            flow_id
        ],
    )
    .expect("update start draft");

    publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

    let published_config: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read published start config");
    assert_eq!(
        published_config,
        r#"{"cookies_json":"{}","poll_interval_seconds":20,"proxy_url":"http://127.0.0.1:9000","retry_limit":4,"username":"shop_abc"}"#
    );

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_definition_canonicalizes_record_legacy_keys_to_snake_case() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flow_nodes SET draft_config_json = ?1 WHERE flow_id = ?2 AND node_key = 'record'",
        params![r#"{"maxDurationSeconds":61}"#, flow_id],
    )
    .expect("update record draft");

    publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

    let published_config: String = conn
        .query_row(
            "SELECT published_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'record'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read published record config");
    assert_eq!(
        published_config,
        r#"{"max_duration_minutes":2,"speech_merge_gap_sec":0.5,"stt_num_threads":4,"stt_quantize":"auto"}"#
    );

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn publish_flow_definition_advances_published_version_to_new_runtime_version() {
    let (mut conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);
    conn.execute(
        "UPDATE flows SET published_version = 2, draft_version = 3 WHERE id = ?1",
        [flow_id],
    )
    .expect("seed versions");

    let result = publish_flow_definition_with_conn(&mut conn, flow_id).expect("publish flow");

    assert_eq!(result.flow_id, flow_id);
    assert!(!result.is_running);
    let (published_version, draft_version): (i64, i64) = conn
        .query_row(
            "SELECT published_version, draft_version FROM flows WHERE id = ?1",
            [flow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read versions");
    assert_eq!(published_version, 4);
    assert_eq!(draft_version, 4);

    drop(conn);
    let _ = std::fs::remove_file(&path);
}
