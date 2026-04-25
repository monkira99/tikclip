use crate::time_hcm::SQL_NOW_HCM;
use rusqlite::{params, Connection};
use serde::Serialize;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, VadModelConfig,
    VoiceActivityDetector, Wave,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

use super::RecordConfig;

const SAMPLE_RATE: i32 = 16_000;
const SILERO_VAD_FILENAME: &str = "silero_vad.onnx";
const SILERO_VAD_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";
const GIPFORMER_REPO_URL: &str =
    "https://huggingface.co/g-group-ai-lab/gipformer-65M-rnnt/resolve/main";

#[derive(Debug, Clone, PartialEq, Eq)]
enum SttQuantize {
    Fp32,
    Int8,
}

#[derive(Debug, Clone)]
pub struct AudioProcessingConfig {
    pub enabled: bool,
    pub speech_merge_gap_sec: f32,
    pub stt_num_threads: i32,
    quantize: SttQuantize,
    pub models_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeechSpan {
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
    pub confidence: Option<f64>,
}

pub fn replace_recording_speech_segments(
    conn: &Connection,
    recording_id: i64,
    spans: &[SpeechSpan],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM speech_segments WHERE recording_id = ?1",
        [recording_id],
    )
    .map_err(|e| e.to_string())?;

    for span in spans {
        conn.execute(
            &format!(
                "INSERT INTO speech_segments (recording_id, start_time, end_time, text, confidence, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, {})",
                SQL_NOW_HCM
            ),
            params![
                recording_id,
                span.start_sec,
                span.end_sec,
                span.text,
                span.confidence,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SttModelPaths {
    encoder: PathBuf,
    decoder: PathBuf,
    joiner: PathBuf,
    tokens: PathBuf,
}

#[derive(Clone)]
struct CachedRecognizer {
    key: String,
    recognizer: Arc<OfflineRecognizer>,
}

static RECOGNIZER_CACHE: OnceLock<Mutex<Option<CachedRecognizer>>> = OnceLock::new();

impl AudioProcessingConfig {
    pub fn from_record_config(
        _conn: &Connection,
        storage_path: &Path,
        config: &RecordConfig,
    ) -> Result<Self, String> {
        let models_path = storage_path.join("models");
        Ok(Self {
            enabled: true,
            speech_merge_gap_sec: config.speech_merge_gap_sec,
            stt_num_threads: config.stt_num_threads,
            quantize: parse_quantize(config.stt_quantize.as_str())?,
            models_path,
        })
    }
}

pub fn process_recording_audio(
    source_path: &Path,
    config: &AudioProcessingConfig,
) -> Result<Vec<SpeechSpan>, String> {
    if !config.enabled {
        return Ok(Vec::new());
    }
    if !source_path.is_file() {
        return Err(format!("Source file not found: {}", source_path.display()));
    }

    let wav_path = extract_audio_to_temp_wav(source_path)?;
    let result = process_wav_audio(&wav_path, config);
    let _ = fs::remove_file(&wav_path);
    result
}

fn process_wav_audio(
    wav_path: &Path,
    config: &AudioProcessingConfig,
) -> Result<Vec<SpeechSpan>, String> {
    let wav = Wave::read(&wav_path.to_string_lossy())
        .ok_or_else(|| format!("Could not read WAV: {}", wav_path.display()))?;
    if wav.sample_rate() != SAMPLE_RATE {
        return Err(format!(
            "Expected ffmpeg output at {SAMPLE_RATE} Hz, got {}",
            wav.sample_rate()
        ));
    }

    let samples = wav.samples();
    let intervals = vad_intervals(samples, config)?;
    if intervals.is_empty() {
        return Ok(Vec::new());
    }
    transcribe_intervals(samples, &intervals, config)
}

fn extract_audio_to_temp_wav(source_path: &Path) -> Result<PathBuf, String> {
    let tmp_path = std::env::temp_dir().join(format!("tikclip_audio_{}.wav", Uuid::new_v4()));
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(source_path)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg(SAMPLE_RATE.to_string())
        .arg("-f")
        .arg("wav")
        .arg(&tmp_path)
        .status()
        .map_err(|e| format!("ffmpeg audio extract failed: {e}"))?;
    if !status.success() {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("ffmpeg audio extract failed with status {status}"));
    }
    Ok(tmp_path)
}

fn vad_intervals(
    samples: &[f32],
    config: &AudioProcessingConfig,
) -> Result<Vec<(usize, usize)>, String> {
    let vad_path = ensure_vad_model(config)?;
    let mut vad_config = VadModelConfig {
        sample_rate: SAMPLE_RATE,
        num_threads: config.stt_num_threads,
        ..Default::default()
    };
    vad_config.silero_vad.model = Some(vad_path.to_string_lossy().into_owned());
    vad_config.silero_vad.threshold = 0.5;
    vad_config.silero_vad.min_silence_duration = 0.25;
    vad_config.silero_vad.min_speech_duration = 0.25;
    vad_config.silero_vad.max_speech_duration = 30.0;
    vad_config.silero_vad.window_size = 512;

    let vad = VoiceActivityDetector::create(&vad_config, 120.0)
        .ok_or_else(|| "Failed to create sherpa-onnx VAD".to_string())?;
    let window_size = usize::try_from(vad_config.silero_vad.window_size)
        .map_err(|_| "Invalid VAD window_size".to_string())?;
    if window_size == 0 {
        return Err("Invalid VAD window_size".to_string());
    }

    let mut raw = Vec::new();
    let mut offset = 0;
    while offset + window_size < samples.len() {
        vad.accept_waveform(&samples[offset..offset + window_size]);
        offset += window_size;
        drain_vad_segments(&vad, &mut raw);
    }
    if offset < samples.len() {
        vad.accept_waveform(&samples[offset..]);
        drain_vad_segments(&vad, &mut raw);
    }
    vad.flush();
    drain_vad_segments(&vad, &mut raw);

    let gap_samples = (config.speech_merge_gap_sec.max(0.0) * SAMPLE_RATE as f32).round() as usize;
    Ok(merge_intervals(raw, gap_samples))
}

fn drain_vad_segments(vad: &VoiceActivityDetector, out: &mut Vec<(usize, usize)>) {
    while let Some(segment) = vad.front() {
        let start = usize::try_from(segment.start()).unwrap_or(0);
        let end = start.saturating_add(segment.samples().len());
        out.push((start, end));
        vad.pop();
    }
}

fn merge_intervals(mut intervals: Vec<(usize, usize)>, gap_samples: usize) -> Vec<(usize, usize)> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by_key(|(start, _)| *start);
    let mut merged = vec![intervals[0]];
    for (start, end) in intervals.into_iter().skip(1) {
        let (_, previous_end) = merged.last_mut().expect("merged is not empty");
        if start.saturating_sub(*previous_end) <= gap_samples {
            *previous_end = (*previous_end).max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn transcribe_intervals(
    samples: &[f32],
    intervals: &[(usize, usize)],
    config: &AudioProcessingConfig,
) -> Result<Vec<SpeechSpan>, String> {
    let recognizer = get_recognizer(config)?;
    let mut out = Vec::with_capacity(intervals.len());
    for &(start, end) in intervals {
        let s = start.min(samples.len());
        let e = end.min(samples.len()).max(s);
        if s == e {
            continue;
        }
        let stream = recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE, &samples[s..e]);
        recognizer.decode(&stream);
        let text = stream
            .get_result()
            .map(|result| result.text.trim().to_string())
            .unwrap_or_default();
        out.push(SpeechSpan {
            start_sec: s as f64 / SAMPLE_RATE as f64,
            end_sec: e as f64 / SAMPLE_RATE as f64,
            text,
            confidence: None,
        });
    }
    Ok(out)
}

fn get_recognizer(config: &AudioProcessingConfig) -> Result<Arc<OfflineRecognizer>, String> {
    let paths = ensure_stt_models(config)?;
    let key = format!(
        "{}|{}|{}|{}|{}",
        paths.encoder.display(),
        paths.decoder.display(),
        paths.joiner.display(),
        paths.tokens.display(),
        config.stt_num_threads
    );
    let cache = RECOGNIZER_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().map_err(|e| e.to_string())?;
    if let Some(cached) = guard.as_ref() {
        if cached.key == key {
            return Ok(Arc::clone(&cached.recognizer));
        }
    }

    let mut recognizer_config = OfflineRecognizerConfig::default();
    recognizer_config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(paths.encoder.to_string_lossy().into_owned()),
        decoder: Some(paths.decoder.to_string_lossy().into_owned()),
        joiner: Some(paths.joiner.to_string_lossy().into_owned()),
    };
    recognizer_config.model_config.tokens = Some(paths.tokens.to_string_lossy().into_owned());
    recognizer_config.model_config.num_threads = config.stt_num_threads;
    recognizer_config.decoding_method = Some("modified_beam_search".to_string());

    let recognizer = Arc::new(
        OfflineRecognizer::create(&recognizer_config)
            .ok_or_else(|| "Failed to create sherpa-onnx offline recognizer".to_string())?,
    );
    *guard = Some(CachedRecognizer {
        key,
        recognizer: Arc::clone(&recognizer),
    });
    Ok(recognizer)
}

fn ensure_vad_model(config: &AudioProcessingConfig) -> Result<PathBuf, String> {
    let path = config
        .models_path
        .join("silero_vad")
        .join(SILERO_VAD_FILENAME);
    ensure_downloaded(&path, SILERO_VAD_URL)?;
    Ok(path)
}

fn ensure_stt_models(config: &AudioProcessingConfig) -> Result<SttModelPaths, String> {
    let (dir_name, files) = match config.quantize {
        SttQuantize::Fp32 => (
            "fp32",
            [
                ("encoder", "encoder-epoch-35-avg-6.onnx"),
                ("decoder", "decoder-epoch-35-avg-6.onnx"),
                ("joiner", "joiner-epoch-35-avg-6.onnx"),
                ("tokens", "tokens.txt"),
            ],
        ),
        SttQuantize::Int8 => (
            "int8",
            [
                ("encoder", "encoder-epoch-35-avg-6.int8.onnx"),
                ("decoder", "decoder-epoch-35-avg-6.int8.onnx"),
                ("joiner", "joiner-epoch-35-avg-6.int8.onnx"),
                ("tokens", "tokens.txt"),
            ],
        ),
    };
    let root = config.models_path.join("gipformer").join(dir_name);
    for (_, filename) in files {
        let path = root.join(filename);
        let url = format!("{GIPFORMER_REPO_URL}/{filename}");
        ensure_downloaded(&path, &url)?;
    }
    Ok(SttModelPaths {
        encoder: root.join(files[0].1),
        decoder: root.join(files[1].1),
        joiner: root.join(files[2].1),
        tokens: root.join(files[3].1),
    })
}

fn ensure_downloaded(path: &Path, url: &str) -> Result<(), String> {
    if path.is_file() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid model path: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    let bytes = reqwest::blocking::Client::new()
        .get(url)
        .send()
        .map_err(|e| format!("download {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download {url}: {e}"))?
        .bytes()
        .map_err(|e| format!("read download {url}: {e}"))?;
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    match conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(test)]
fn read_bool(conn: &Connection, key: &str, default: bool) -> Result<bool, String> {
    Ok(read_setting(conn, key)?
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default))
}

#[cfg(test)]
fn read_i32(conn: &Connection, key: &str, default: i32) -> Result<i32, String> {
    match read_setting(conn, key)? {
        Some(value) => value
            .parse::<i32>()
            .map_err(|_| format!("{key} must be an integer, got {value:?}")),
        None => Ok(default),
    }
}

#[cfg(test)]
fn read_f32(conn: &Connection, key: &str, default: f32) -> Result<f32, String> {
    match read_setting(conn, key)? {
        Some(value) => value
            .parse::<f32>()
            .map_err(|_| format!("{key} must be a number, got {value:?}")),
        None => Ok(default),
    }
}

fn parse_quantize(value: &str) -> Result<SttQuantize, String> {
    match value.to_ascii_lowercase().as_str() {
        "fp32" | "float32" => Ok(SttQuantize::Fp32),
        // The Rust static sherpa build is CPU-only here, so auto follows the CPU int8 path.
        "auto" | "int8" => Ok(SttQuantize::Int8),
        other => Err(format!(
            "stt_quantize must be auto, fp32, or int8, got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_intervals, read_bool, read_f32, read_i32};
    use rusqlite::Connection;

    fn conn_with_settings() -> Connection {
        let conn = Connection::open_in_memory().expect("open db");
        conn.execute(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT)",
            [],
        )
        .expect("create settings");
        conn
    }

    #[test]
    fn merge_intervals_combines_short_gaps() {
        let merged = merge_intervals(vec![(10, 20), (22, 30), (50, 60)], 3);
        assert_eq!(merged, vec![(10, 30), (50, 60)]);
    }

    #[test]
    fn settings_readers_use_defaults_when_missing() {
        let conn = conn_with_settings();
        assert!(read_bool(&conn, "audio_processing_enabled", true).expect("bool"));
        assert_eq!(read_i32(&conn, "stt_num_threads", 4).expect("int"), 4);
        assert_eq!(
            read_f32(&conn, "speech_merge_gap_sec", 0.5).expect("float"),
            0.5
        );
    }
}
