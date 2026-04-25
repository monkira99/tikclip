use super::*;

#[test]
fn log_runtime_event_appends_entry_to_manager_buffer() {
    let manager = LiveRuntimeManager::new();

    manager.log_runtime_event_for_test(7, Some(42), "record", "record_spawned", "Spawned worker");

    let logs = manager.list_runtime_logs_for_test(None, None);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].flow_id, 7);
    assert_eq!(logs[0].event, "record_spawned");
}

#[test]
fn log_runtime_event_keeps_buffered_entry_when_emit_fails() {
    let manager = LiveRuntimeManager::new();
    manager.fail_runtime_log_emit_for_test("simulated emit failure");

    let result = manager.log_runtime_event(
        7,
        Some(42),
        None,
        "record",
        "record_spawned",
        FlowRuntimeLogLevel::Info,
        None,
        "Spawned worker",
        serde_json::json!({}),
    );

    assert!(result.is_ok());
    let logs = manager.list_runtime_logs_for_test(Some(7), None);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event, "record_spawned");
}
