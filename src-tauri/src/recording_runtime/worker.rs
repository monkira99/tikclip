use super::types::{
    RecordingExecution, RecordingFinishInput, RecordingOutcome, RecordingStartInput,
};
use crate::time_hcm::now_timestamp_hcm;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

type RunnerFn =
    dyn Fn(&RecordingStartInput, &str) -> Result<RecordingFinishInput, String> + Send + Sync;

type SpawnFn =
    dyn Fn(&RecordingStartInput, &str) -> Result<RecordingProcessHandle, String> + Send + Sync;

#[derive(Clone)]
pub struct RecordingProcessRunner {
    spawn: Arc<SpawnFn>,
}

pub struct RecordingProcessHandle {
    wait: Box<dyn FnOnce() -> Result<RecordingFinishInput, String> + Send>,
    cancel: Arc<dyn Fn() -> Result<(), String> + Send + Sync>,
}

impl RecordingProcessHandle {
    #[allow(dead_code)]
    pub(crate) fn from_parts(
        wait: Box<dyn FnOnce() -> Result<RecordingFinishInput, String> + Send>,
        cancel: Arc<dyn Fn() -> Result<(), String> + Send + Sync>,
    ) -> Self {
        Self { wait, cancel }
    }

    pub fn wait(self) -> Result<RecordingFinishInput, String> {
        (self.wait)()
    }

    #[allow(dead_code)]
    pub fn cancel(&self) -> Result<(), String> {
        (self.cancel)()
    }

    pub fn cancel_handle(&self) -> Arc<dyn Fn() -> Result<(), String> + Send + Sync> {
        Arc::clone(&self.cancel)
    }
}

#[derive(Clone)]
pub struct RecordingRunner {
    #[allow(dead_code)]
    run: Arc<RunnerFn>,
}

impl RecordingRunner {
    pub fn ffmpeg() -> Self {
        Self {
            run: Arc::new(run_ffmpeg_recording),
        }
    }

    #[cfg(test)]
    pub fn from_fn<F>(func: F) -> Self
    where
        F: Fn(&RecordingStartInput, &str) -> Result<RecordingFinishInput, String>
            + Send
            + Sync
            + 'static,
    {
        Self {
            run: Arc::new(func),
        }
    }

    #[allow(dead_code)]
    pub fn run(
        &self,
        input: &RecordingStartInput,
        output_path: &str,
    ) -> Result<RecordingFinishInput, String> {
        (self.run)(input, output_path)
    }
}

impl RecordingProcessRunner {
    pub fn ffmpeg() -> Self {
        Self {
            spawn: Arc::new(spawn_ffmpeg_recording),
        }
    }

    #[cfg(test)]
    pub fn from_fn<F>(func: F) -> Self
    where
        F: Fn(&RecordingStartInput, &str) -> Result<RecordingProcessHandle, String>
            + Send
            + Sync
            + 'static,
    {
        Self {
            spawn: Arc::new(func),
        }
    }

    pub fn spawn(
        &self,
        input: &RecordingStartInput,
        output_path: &str,
    ) -> Result<RecordingProcessHandle, String> {
        (self.spawn)(input, output_path)
    }
}

#[allow(dead_code)]
pub fn build_ffmpeg_argv(input: &RecordingStartInput, output_path: &str) -> Vec<String> {
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.stream_url.clone(),
        "-t".to_string(),
        input.max_duration_seconds.to_string(),
        output_path.to_string(),
    ]
}

#[allow(dead_code)]
pub fn build_ffmpeg_command(
    input: &RecordingStartInput,
    output_path: &str,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("ffmpeg");
    command.args(build_ffmpeg_argv(input, output_path));
    command
}

#[allow(dead_code)]
pub fn build_recording_output_path(input: &RecordingStartInput) -> String {
    std::env::temp_dir()
        .join("tikclip-recordings")
        .join(format!(
            "flow-{}-run-{}-{}.mp4",
            input.flow_id, input.flow_run_id, input.external_recording_id
        ))
        .to_string_lossy()
        .into_owned()
}

#[allow(dead_code)]
pub fn generate_external_recording_id(flow_run_id: i64) -> String {
    format!("rust-recording-{flow_run_id}-{}", now_timestamp_hcm())
}

fn run_ffmpeg_recording(
    input: &RecordingStartInput,
    output_path: &str,
) -> Result<RecordingFinishInput, String> {
    let status = std::process::Command::new("ffmpeg")
        .args(build_ffmpeg_argv(input, output_path))
        .status()
        .map_err(|e| e.to_string())?;
    let success = status.success();
    Ok(RecordingFinishInput {
        account_id: input.account_id,
        flow_id: input.flow_id,
        flow_run_id: input.flow_run_id,
        external_recording_id: input.external_recording_id.clone(),
        room_id: input.room_id.clone(),
        file_path: Some(output_path.to_string()),
        error_message: if success {
            None
        } else {
            Some(format!("ffmpeg exited with status {status}"))
        },
        duration_seconds: input.max_duration_seconds,
        file_size_bytes: 0,
        outcome: if success {
            RecordingOutcome::Success
        } else {
            RecordingOutcome::Failed
        },
    })
}

fn spawn_ffmpeg_recording(
    input: &RecordingStartInput,
    output_path: &str,
) -> Result<RecordingProcessHandle, String> {
    let child = Command::new("ffmpeg")
        .args(build_ffmpeg_argv(input, output_path))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let child = Arc::new(Mutex::new(child));
    let wait_child = Arc::clone(&child);
    let cancel_child = Arc::clone(&child);
    let finish_input = input.clone();
    let output = output_path.to_string();

    Ok(RecordingProcessHandle {
        wait: Box::new(move || wait_ffmpeg_child(wait_child, &finish_input, output.as_str())),
        cancel: Arc::new(move || cancel_ffmpeg_child(&cancel_child)),
    })
}

fn wait_ffmpeg_child(
    child: Arc<Mutex<Child>>,
    input: &RecordingStartInput,
    output_path: &str,
) -> Result<RecordingFinishInput, String> {
    let status = child
        .lock()
        .map_err(|e| e.to_string())?
        .wait()
        .map_err(|e| e.to_string())?;
    let outcome = if status.success() {
        RecordingOutcome::Success
    } else {
        RecordingOutcome::Failed
    };
    Ok(RecordingFinishInput {
        account_id: input.account_id,
        flow_id: input.flow_id,
        flow_run_id: input.flow_run_id,
        external_recording_id: input.external_recording_id.clone(),
        room_id: input.room_id.clone(),
        file_path: Some(output_path.to_string()),
        error_message: if matches!(outcome, RecordingOutcome::Failed) {
            Some(format!("ffmpeg exited with status {status}"))
        } else {
            None
        },
        duration_seconds: input.max_duration_seconds,
        file_size_bytes: 0,
        outcome,
    })
}

fn cancel_ffmpeg_child(child: &Arc<Mutex<Child>>) -> Result<(), String> {
    child
        .lock()
        .map_err(|e| e.to_string())?
        .kill()
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn build_recording_execution(input: &RecordingStartInput) -> RecordingExecution {
    RecordingExecution {
        start: input.clone(),
        output_path: build_recording_output_path(input),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_ffmpeg_argv, build_recording_execution, build_recording_output_path,
        generate_external_recording_id, RecordingProcessHandle, RecordingProcessRunner,
        RecordingRunner,
    };
    use crate::recording_runtime::types::{
        RecordingFinishInput, RecordingOutcome, RecordingStartInput,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn build_ffmpeg_argv_includes_input_duration_and_output_path() {
        let input = RecordingStartInput {
            account_id: 9,
            flow_id: 3,
            flow_run_id: 11,
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            max_duration_seconds: 300,
            external_recording_id: "ext-123".to_string(),
        };

        let argv = build_ffmpeg_argv(&input, "/tmp/out.mp4");

        assert_eq!(
            argv,
            vec![
                "-y".to_string(),
                "-i".to_string(),
                "https://example.com/live.flv".to_string(),
                "-t".to_string(),
                "300".to_string(),
                "/tmp/out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn build_recording_output_path_uses_flow_run_and_external_recording_id() {
        let input = RecordingStartInput {
            account_id: 9,
            flow_id: 3,
            flow_run_id: 11,
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            max_duration_seconds: 300,
            external_recording_id: "ext-123".to_string(),
        };

        let path = build_recording_output_path(&input);

        assert!(path.contains("flow-3-run-11-ext-123.mp4"));
    }

    #[test]
    fn generate_external_recording_id_includes_flow_run_id() {
        let external_id = generate_external_recording_id(11);

        assert!(external_id.starts_with("rust-recording-11-"));
    }

    #[test]
    fn recording_runner_from_fn_returns_finish_payload() {
        let input = RecordingStartInput {
            account_id: 9,
            flow_id: 3,
            flow_run_id: 11,
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            max_duration_seconds: 300,
            external_recording_id: "ext-123".to_string(),
        };
        let runner = RecordingRunner::from_fn(|input, output_path| {
            Ok(RecordingFinishInput {
                account_id: input.account_id,
                flow_id: input.flow_id,
                flow_run_id: input.flow_run_id,
                external_recording_id: input.external_recording_id.clone(),
                room_id: input.room_id.clone(),
                file_path: Some(output_path.to_string()),
                error_message: None,
                duration_seconds: input.max_duration_seconds,
                file_size_bytes: 7,
                outcome: RecordingOutcome::Success,
            })
        });

        let result = runner.run(&input, "/tmp/out.mp4").expect("runner result");

        assert_eq!(result.file_path.as_deref(), Some("/tmp/out.mp4"));
        assert_eq!(result.outcome, RecordingOutcome::Success);
    }

    #[test]
    fn build_recording_execution_uses_output_path_helper() {
        let input = RecordingStartInput {
            account_id: 9,
            flow_id: 3,
            flow_run_id: 11,
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            max_duration_seconds: 300,
            external_recording_id: "ext-123".to_string(),
        };

        let execution = build_recording_execution(&input);

        assert_eq!(execution.start.flow_run_id, 11);
        assert!(execution.output_path.contains("flow-3-run-11-ext-123.mp4"));
    }

    #[test]
    fn recording_process_runner_from_fn_supports_cancel_and_wait() {
        let input = RecordingStartInput {
            account_id: 9,
            flow_id: 3,
            flow_run_id: 11,
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            max_duration_seconds: 300,
            external_recording_id: "ext-123".to_string(),
        };
        let cancelled = Arc::new(Mutex::new(false));
        let cancelled_flag = Arc::clone(&cancelled);
        let runner = RecordingProcessRunner::from_fn(move |input, output_path| {
            let cancelled_for_wait = Arc::clone(&cancelled_flag);
            let cancelled_for_cancel = Arc::clone(&cancelled_flag);
            Ok(RecordingProcessHandle {
                wait: Box::new({
                    let output_path = output_path.to_string();
                    let input = input.clone();
                    move || {
                        let was_cancelled = *cancelled_for_wait.lock().unwrap();
                        Ok(RecordingFinishInput {
                            account_id: input.account_id,
                            flow_id: input.flow_id,
                            flow_run_id: input.flow_run_id,
                            external_recording_id: input.external_recording_id.clone(),
                            room_id: input.room_id.clone(),
                            file_path: Some(output_path),
                            error_message: None,
                            duration_seconds: input.max_duration_seconds,
                            file_size_bytes: 0,
                            outcome: if was_cancelled {
                                RecordingOutcome::Cancelled
                            } else {
                                RecordingOutcome::Success
                            },
                        })
                    }
                }),
                cancel: Arc::new(move || {
                    *cancelled_for_cancel.lock().unwrap() = true;
                    Ok(())
                }),
            })
        });

        let handle = runner.spawn(&input, "/tmp/out.mp4").expect("spawn handle");
        handle.cancel().expect("cancel handle");
        let result = handle.wait().expect("wait result");

        assert_eq!(result.outcome, RecordingOutcome::Cancelled);
    }
}
