use super::*;

#[test]
fn apply_flow_node_draft_canonicalizes_start_username() {
    let (conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);

    super::apply_flow_node_draft(
        &conn,
        flow_id,
        "start",
        r#"{"username":" @shop_abc ","cookies_json":"{}"}"#,
    )
    .expect("save draft");

    let draft_username: String = conn
        .query_row(
            "SELECT json_extract(draft_config_json, '$.username') FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read canonical draft username");
    assert_eq!(draft_username, "shop_abc");

    drop(conn);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn apply_flow_node_draft_allows_blank_start_username() {
    let (conn, path) = open_temp_db();
    let flow_id = insert_flow_with_nodes(&conn);

    apply_flow_node_draft(
        &conn,
        flow_id,
        "start",
        r#"{"username":"   ","cookiesJson":"{}","proxyUrl":"http://127.0.0.1:9000","pollIntervalSeconds":20,"retryLimit":4}"#,
    )
    .expect("save incomplete draft");

    let draft_config: String = conn
        .query_row(
            "SELECT draft_config_json FROM flow_nodes WHERE flow_id = ?1 AND node_key = 'start'",
            [flow_id],
            |row| row.get(0),
        )
        .expect("read draft config");
    assert_eq!(
        draft_config,
        r#"{"cookies_json":"{}","poll_interval_seconds":20,"proxy_url":"http://127.0.0.1:9000","retry_limit":4,"username":""}"#
    );

    drop(conn);
    let _ = std::fs::remove_file(&path);
}
