use crate::live_runtime::types::LiveRuntimeSessionSnapshot;
#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

#[cfg(test)]
static TEARDOWN_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static TEARDOWN_TEST_GUARD: Mutex<()> = Mutex::new(());
#[cfg(test)]
thread_local! {
    static COUNT_TEARDOWNS_FOR_CURRENT_TEST: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub struct TeardownTestGuard {
    _guard: MutexGuard<'static, ()>,
    previous_enabled: bool,
}

#[cfg(test)]
impl Drop for TeardownTestGuard {
    fn drop(&mut self) {
        COUNT_TEARDOWNS_FOR_CURRENT_TEST.with(|enabled| enabled.set(self.previous_enabled));
    }
}

#[cfg(test)]
pub fn reset_teardown_call_count_for_test() {
    TEARDOWN_CALL_COUNT.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub fn teardown_call_count_for_test() -> usize {
    TEARDOWN_CALL_COUNT.load(Ordering::SeqCst)
}

#[cfg(test)]
pub fn lock_teardown_test_guard_for_test() -> TeardownTestGuard {
    let guard = TEARDOWN_TEST_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous_enabled = COUNT_TEARDOWNS_FOR_CURRENT_TEST.with(|enabled| {
        let previous = enabled.get();
        enabled.set(true);
        previous
    });
    TeardownTestGuard {
        _guard: guard,
        previous_enabled,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn should_start_new_run(
    current_room_id: Option<&str>,
    last_completed_room_id: Option<&str>,
    source_is_live: bool,
) -> bool {
    if !source_is_live {
        return true;
    }

    match (current_room_id, last_completed_room_id) {
        (Some(current), Some(last)) => current != last,
        _ => true,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn runtime_current_node_for_status(status: &str) -> Option<&'static str> {
    match status.trim() {
        "watching" => Some("start"),
        "recording" => Some("record"),
        "processing" => Some("clip"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRuntimeSession {
    flow_id: i64,
    flow_name: String,
    username: String,
    lookup_key: String,
    generation: u64,
    poll_generation: u64,
    stopped: bool,
    last_error: Option<String>,
    active_flow_run_id: Option<i64>,
    last_completed_room_id: Option<String>,
    downstream_status: Option<String>,
}

impl LiveRuntimeSession {
    pub fn new(
        flow_id: i64,
        flow_name: String,
        username: String,
        lookup_key: String,
        generation: u64,
        last_completed_room_id: Option<String>,
    ) -> Self {
        Self {
            flow_id,
            flow_name,
            username,
            lookup_key,
            generation,
            poll_generation: 0,
            stopped: false,
            last_error: None,
            active_flow_run_id: None,
            last_completed_room_id,
            downstream_status: None,
        }
    }

    pub fn lookup_key(&self) -> &str {
        self.lookup_key.as_str()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn poll_generation(&self) -> u64 {
        self.poll_generation
    }

    pub fn bump_poll_generation(&mut self) -> u64 {
        self.poll_generation += 1;
        self.poll_generation
    }

    pub fn set_poll_generation(&mut self, generation: u64) {
        self.poll_generation = generation;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_polling(&self) -> bool {
        !self.stopped && self.active_flow_run_id.is_none()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_completed_room_id(&self) -> Option<&str> {
        self.last_completed_room_id.as_deref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn active_flow_run_id(&self) -> Option<i64> {
        self.active_flow_run_id
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_flow_run_started(&mut self, flow_run_id: i64) {
        self.active_flow_run_id = Some(flow_run_id);
        self.downstream_status = None;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_flow_run_completed(&mut self, room_id: Option<&str>) {
        self.active_flow_run_id = None;
        self.last_completed_room_id = room_id.map(str::to_string);
        self.downstream_status = None;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_downstream_stage(&mut self, flow_run_id: i64, status: &str) {
        self.active_flow_run_id = Some(flow_run_id);
        self.downstream_status = Some(status.to_string());
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn mark_source_offline(&mut self) {
        self.active_flow_run_id = None;
        self.last_completed_room_id = None;
        self.downstream_status = None;
    }

    #[allow(dead_code)]
    pub fn cancel(&mut self) {
        self.stopped = true;
    }

    pub fn teardown(&mut self) {
        self.active_flow_run_id = None;
        self.downstream_status = None;
        self.stopped = true;
        #[cfg(test)]
        {
            COUNT_TEARDOWNS_FOR_CURRENT_TEST.with(|enabled| {
                if enabled.get() {
                    TEARDOWN_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.stopped = true;
        self.downstream_status = None;
        self.last_error = Some(error.into());
    }

    pub fn snapshot(&self) -> LiveRuntimeSessionSnapshot {
        LiveRuntimeSessionSnapshot {
            flow_id: self.flow_id,
            flow_name: self.flow_name.clone(),
            username: self.username.clone(),
            lookup_key: self.lookup_key.clone(),
            generation: self.generation,
            status: if self.stopped {
                if self.last_error.is_some() {
                    "error".to_string()
                } else {
                    "stopped".to_string()
                }
            } else {
                if self.active_flow_run_id.is_some() {
                    self.downstream_status
                        .clone()
                        .unwrap_or_else(|| "recording".to_string())
                } else {
                    "watching".to_string()
                }
            },
            last_error: self.last_error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        lock_teardown_test_guard_for_test, reset_teardown_call_count_for_test,
        runtime_current_node_for_status, should_start_new_run, teardown_call_count_for_test,
        LiveRuntimeSession,
    };

    #[test]
    fn live_runtime_session_snapshot_reports_generation() {
        let session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        let snapshot = session.snapshot();
        assert_eq!(snapshot.flow_id, 22);
        assert_eq!(snapshot.generation, 3);
        assert_eq!(snapshot.lookup_key, "shop_abc");
        assert_eq!(snapshot.status, "watching");
    }

    #[test]
    fn live_runtime_session_cancel_marks_session_stopped() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        session.cancel();

        let snapshot = session.snapshot();
        assert_eq!(snapshot.status, "stopped");
    }

    #[test]
    fn live_runtime_session_marks_completed_room_and_returns_to_polling() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        session.mark_flow_run_started(99);
        session.mark_flow_run_completed(Some("7312345"));

        assert!(session.is_polling());
        assert_eq!(session.last_completed_room_id(), Some("7312345"));
    }

    #[test]
    fn live_runtime_session_snapshot_reports_downstream_status_when_run_stays_active() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        session.mark_downstream_stage(99, "processing");

        let snapshot = session.snapshot();
        assert_eq!(snapshot.status, "processing");
        assert!(!session.is_polling());
        assert_eq!(session.active_flow_run_id(), Some(99));
    }

    #[test]
    fn live_runtime_session_snapshot_reports_recording_when_active_run_exists() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        session.mark_flow_run_started(99);

        let snapshot = session.snapshot();
        assert_eq!(snapshot.status, "recording");
    }

    #[test]
    fn live_runtime_session_snapshot_reports_error_when_failed() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        session.fail("lease conflict");

        let snapshot = session.snapshot();
        assert_eq!(snapshot.status, "error");
    }

    #[test]
    fn live_runtime_session_teardown_invokes_hook() {
        let _guard = lock_teardown_test_guard_for_test();
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );
        reset_teardown_call_count_for_test();

        session.teardown();

        assert_eq!(teardown_call_count_for_test(), 1);
        assert_eq!(session.snapshot().status, "stopped");
    }

    #[test]
    fn should_start_new_run_blocks_same_room_after_completion() {
        assert!(!should_start_new_run(
            Some("7312345"),
            Some("7312345"),
            true
        ));
        assert!(should_start_new_run(Some("7319999"), Some("7312345"), true));
        assert!(should_start_new_run(
            Some("7312345"),
            Some("7312345"),
            false
        ));
    }

    #[test]
    fn runtime_current_node_for_status_maps_start_record_and_processing_stages() {
        assert_eq!(runtime_current_node_for_status("watching"), Some("start"));
        assert_eq!(runtime_current_node_for_status("recording"), Some("record"));
        assert_eq!(runtime_current_node_for_status("processing"), Some("clip"));
        assert_eq!(runtime_current_node_for_status("error"), None);
    }

    #[test]
    fn live_runtime_session_bumps_poll_generation() {
        let mut session = LiveRuntimeSession::new(
            22,
            "Flow".to_string(),
            "shop_abc".to_string(),
            "shop_abc".to_string(),
            3,
            None,
        );

        assert_eq!(session.poll_generation(), 0);
        assert_eq!(session.bump_poll_generation(), 1);
        assert_eq!(session.bump_poll_generation(), 2);
        assert_eq!(session.poll_generation(), 2);
    }
}
