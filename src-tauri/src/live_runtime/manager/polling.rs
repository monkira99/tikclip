#[cfg(not(test))]
use super::store::open_runtime_connection;
use super::{
    store::{load_flow_runtime_config, FlowRuntimeConfig},
    ActivePollTaskHandle, LiveRuntimeManager, PollIterationToken,
};
use crate::live_runtime::logs::FlowRuntimeLogLevel;
use crate::tiktok::types::LiveStatus;
use crate::time_hcm::{now_timestamp_hcm, timestamp_after_seconds_hcm};
use rusqlite::Connection;
use std::sync::{atomic::Ordering, Arc};

impl LiveRuntimeManager {
    fn resolve_live_status_for_poll(
        &self,
        config: &FlowRuntimeConfig,
    ) -> Result<Option<LiveStatus>, String> {
        #[cfg(test)]
        {
            if let Some(next) = self
                .stubbed_live_statuses
                .lock()
                .map_err(|e| e.to_string())?
                .pop_front()
            {
                return next;
            }
        }

        crate::tiktok::check_live::resolve_live_status(&crate::tiktok::types::LiveCheckConfig {
            username: config.username.as_str(),
            cookies_json: config.cookies_json.as_str(),
            proxy_url: config.proxy_url.as_deref(),
        })
    }

    fn poll_iteration_token(&self, flow_id: i64) -> Result<Option<PollIterationToken>, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let Some(task) = state.active_poll_tasks_by_flow.get(&flow_id) else {
            return Ok(None);
        };
        Ok(Some(PollIterationToken {
            generation: task.generation,
            cancelled: Arc::clone(&task.cancelled),
        }))
    }

    fn is_poll_iteration_stale(
        &self,
        flow_id: i64,
        token: &PollIterationToken,
    ) -> Result<bool, String> {
        if token.cancelled.load(Ordering::SeqCst) {
            return Ok(true);
        }

        let state = self.state.lock().map_err(|e| e.to_string())?;
        let Some(task) = state.active_poll_tasks_by_flow.get(&flow_id) else {
            return Ok(true);
        };
        if task.generation != token.generation || task.cancelled.load(Ordering::SeqCst) {
            return Ok(true);
        }

        let Some(session) = state.sessions_by_flow.get(&flow_id) else {
            return Ok(true);
        };
        Ok(!session.is_polling())
    }

    pub fn poll_flow_once(&self, conn: &Connection, flow_id: i64) -> Result<(), String> {
        let config = load_flow_runtime_config(conn, flow_id)?;
        if !config.enabled {
            return Ok(());
        }

        let Some(token) = self.poll_iteration_token(flow_id)? else {
            return Ok(());
        };
        if self.is_poll_iteration_stale(flow_id, &token)? {
            return Ok(());
        }

        let live_status = match self.resolve_live_status_for_poll(&config) {
            Ok(status) => status,
            Err(err) => {
                self.mark_poll_retry(flow_id, config.poll_interval_seconds)?;
                let _ = self.log_runtime_event(
                    flow_id,
                    None,
                    None,
                    "start",
                    "poll_failed",
                    FlowRuntimeLogLevel::Warn,
                    Some("start.poll_failed"),
                    "Live check failed; watcher will retry on the next poll",
                    serde_json::json!({
                        "error": err,
                    }),
                );
                self.emit_runtime_update_for_flow(conn, flow_id)?;
                return Ok(());
            }
        };
        #[cfg(test)]
        {
            if let Some(hook) = self
                .before_poll_apply_hook
                .lock()
                .map_err(|e| e.to_string())?
                .clone()
            {
                hook();
            }
        }
        if self.is_poll_iteration_stale(flow_id, &token)? {
            return Ok(());
        }

        if let Some(status) = live_status {
            self.mark_poll_checked(flow_id, true, config.poll_interval_seconds)?;
            let _ = self.handle_live_detected(conn, flow_id, &status)?;
        } else {
            self.mark_poll_checked(flow_id, false, config.poll_interval_seconds)?;
            self.mark_source_offline(flow_id)?;
        }

        Ok(())
    }

    fn mark_poll_checked(
        &self,
        flow_id: i64,
        live: bool,
        poll_interval_seconds: i64,
    ) -> Result<(), String> {
        let checked_at = now_timestamp_hcm();
        let next_poll_at = timestamp_after_seconds_hcm(poll_interval_seconds);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_poll_checked(checked_at, live, next_poll_at, poll_interval_seconds);
        }
        Ok(())
    }

    fn mark_poll_retry(&self, flow_id: i64, poll_interval_seconds: i64) -> Result<(), String> {
        let checked_at = now_timestamp_hcm();
        let next_poll_at = timestamp_after_seconds_hcm(poll_interval_seconds);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = state.sessions_by_flow.get_mut(&flow_id) {
            session.mark_poll_retry(checked_at, next_poll_at, poll_interval_seconds);
        }
        Ok(())
    }

    pub(super) fn spawn_poll_loop_worker(
        &self,
        flow_id: i64,
        poll_interval_seconds: i64,
        handle: ActivePollTaskHandle,
    ) {
        #[cfg(test)]
        {
            let _ = (flow_id, poll_interval_seconds, handle);
        }

        #[cfg(not(test))]
        {
            let Some(db_path) = self.runtime_db_path.clone() else {
                return;
            };
            let runtime_manager = self.clone();
            let poll_interval = std::time::Duration::from_secs(poll_interval_seconds.max(1) as u64);
            let token = PollIterationToken {
                generation: handle.generation,
                cancelled: Arc::clone(&handle.cancelled),
            };

            std::thread::spawn(move || loop {
                let is_stale = runtime_manager
                    .is_poll_iteration_stale(flow_id, &token)
                    .unwrap_or(true);
                if is_stale {
                    break;
                }

                let conn = match open_runtime_connection(&db_path) {
                    Ok(conn) => conn,
                    Err(err) => {
                        log::warn!(
                            "flow_runtime poll worker failed opening DB for flow {}: {}",
                            flow_id,
                            err
                        );
                        break;
                    }
                };
                if let Err(err) = runtime_manager.poll_flow_once(&conn, flow_id) {
                    log::warn!(
                        "flow_runtime poll worker iteration failed for flow {}: {}",
                        flow_id,
                        err
                    );
                }

                let is_stale = runtime_manager
                    .is_poll_iteration_stale(flow_id, &token)
                    .unwrap_or(true);
                if is_stale {
                    break;
                }
                std::thread::sleep(poll_interval);
            });
        }
    }
}
