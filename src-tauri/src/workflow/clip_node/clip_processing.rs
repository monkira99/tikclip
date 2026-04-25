use crate::commands::clips::{insert_clip_from_sidecar_with_conn, InsertClipFromSidecarInput};
use crate::commands::flows::append_pipeline_hint_node_run;
use crate::time_hcm::now_timestamp_hcm;
use crate::workflow::clip_node::ClipConfig;
use crate::workflow::record_node::SpeechSpan;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone)]
pub struct ClipProcessingConfig {
    pub clip_min_duration: i64,
    pub clip_max_duration: i64,
    pub scene_threshold: f64,
    pub speech_cut_tolerance_sec: f64,
    pub storage_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ClipProcessingInput {
    pub external_recording_id: String,
    pub account_id: i64,
    pub username: String,
    pub source_path: PathBuf,
    pub speech_segments: Vec<SpeechSpan>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RustClipReadyPayload {
    pub clip_id: i64,
    pub recording_id: String,
    pub account_id: i64,
    pub username: String,
    pub clip_index: i64,
    pub path: String,
    pub thumbnail_path: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub duration_sec: f64,
    pub transcript_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClipProcessingResult {
    pub clips: Vec<RustClipReadyPayload>,
}

impl ClipProcessingConfig {
    pub fn from_clip_config(storage_root: &Path, config: &ClipConfig) -> Self {
        Self {
            clip_min_duration: config.clip_min_duration,
            clip_max_duration: config.clip_max_duration,
            scene_threshold: config.scene_threshold,
            speech_cut_tolerance_sec: config.speech_cut_tolerance_sec,
            storage_root: storage_root.to_path_buf(),
        }
    }
}

pub fn process_recording_clips(
    conn: &Connection,
    app_handle: Option<&AppHandle>,
    input: &ClipProcessingInput,
    config: &ClipProcessingConfig,
) -> Result<ClipProcessingResult, String> {
    if !input.source_path.is_file() {
        return Err(format!(
            "Source file not found: {}",
            input.source_path.display()
        ));
    }

    let total_duration = probe_duration_seconds(&input.source_path)?;
    let scenes = detect_scenes(&input.source_path, config.scene_threshold, total_duration)?;
    let grouped = group_scenes_with_speech(&scenes, &input.speech_segments, total_duration, config);
    if grouped.is_empty() {
        return Ok(ClipProcessingResult { clips: Vec::new() });
    }

    let date_str = today_ymd_hcm();
    let out_dir = config
        .storage_root
        .join("clips")
        .join(&input.username)
        .join(date_str);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let start_idx = next_clip_file_index(&out_dir)?;
    let mut clips = Vec::with_capacity(grouped.len());

    for (offset, (start_sec, end_sec)) in grouped.into_iter().enumerate() {
        let duration_sec = (end_sec - start_sec).max(0.0);
        if duration_sec <= 0.0 {
            continue;
        }
        let clip_index = start_idx + i64::try_from(offset).map_err(|e| e.to_string())?;
        let clip_path = out_dir.join(format!("clip_{clip_index:03}.mp4"));
        let thumbnail_path = clip_path.with_extension("jpg");
        extract_clip(&input.source_path, &clip_path, start_sec, duration_sec)?;
        extract_thumbnail(&clip_path, &thumbnail_path, duration_sec)?;

        let transcript_text = transcript_for_clip_range(&input.speech_segments, start_sec, end_sec);
        let clip_id = insert_clip_from_sidecar_with_conn(
            conn,
            &InsertClipFromSidecarInput {
                sidecar_recording_id: input.external_recording_id.clone(),
                account_id: input.account_id,
                file_path: clip_path.to_string_lossy().into_owned(),
                thumbnail_path: thumbnail_path.to_string_lossy().into_owned(),
                duration_sec,
                start_sec,
                end_sec,
                transcript_text: transcript_text.clone(),
            },
        )?;
        append_pipeline_hint_node_run(conn, "clip_ready", clip_id)?;

        let payload = RustClipReadyPayload {
            clip_id,
            recording_id: input.external_recording_id.clone(),
            account_id: input.account_id,
            username: input.username.clone(),
            clip_index,
            path: clip_path.to_string_lossy().into_owned(),
            thumbnail_path: thumbnail_path.to_string_lossy().into_owned(),
            start_sec,
            end_sec,
            duration_sec,
            transcript_text,
        };
        if let Some(app) = app_handle {
            app.emit("rust-clip-ready", payload.clone())
                .map_err(|e| e.to_string())?;
        }
        clips.push(payload);
    }

    Ok(ClipProcessingResult { clips })
}

fn probe_duration_seconds(video_path: &Path) -> Result<f64, String> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(video_path)
        .output()
        .map_err(|e| format!("ffprobe failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .lines()
        .last()
        .unwrap_or("")
        .parse::<f64>()
        .map_err(|_| format!("Could not parse duration from ffprobe output: {stdout:?}"))
}

fn detect_scenes(
    video_path: &Path,
    scene_threshold: f64,
    total_duration: f64,
) -> Result<Vec<(f64, f64)>, String> {
    if total_duration <= 0.0 {
        return Ok(Vec::new());
    }
    let ffmpeg_threshold = (scene_threshold / 100.0).clamp(0.01, 1.0);
    let filter = format!("select='gt(scene,{ffmpeg_threshold:.4})',showinfo");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(video_path)
        .arg("-vf")
        .arg(filter)
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .map_err(|e| format!("ffmpeg scene detect failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg scene detect failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut boundaries: Vec<f64> = stderr
        .lines()
        .filter_map(parse_showinfo_pts_time)
        .filter(|t| *t > 0.0 && *t < total_duration)
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    Ok(intervals_from_internal_cuts(&boundaries, total_duration))
}

fn parse_showinfo_pts_time(line: &str) -> Option<f64> {
    let (_, rest) = line.split_once("pts_time:")?;
    let token = rest.split_whitespace().next()?;
    token.parse::<f64>().ok()
}

fn group_scenes_with_speech(
    segments: &[(f64, f64)],
    speech_segments: &[SpeechSpan],
    total_duration: f64,
    config: &ClipProcessingConfig,
) -> Vec<(f64, f64)> {
    let visual = group_scenes(segments, total_duration, config);
    if speech_segments.is_empty() || total_duration <= 0.0 {
        return visual;
    }
    let gaps = speech_gap_intervals(speech_segments, total_duration);
    let bounds = raw_scene_boundary_times(segments);
    let safe = hybrid_internal_cuts(&bounds, &gaps, config.speech_cut_tolerance_sec);
    if safe.is_empty() {
        return visual;
    }
    let raw_parts = intervals_from_internal_cuts(&safe, total_duration);
    if raw_parts.is_empty() {
        return visual;
    }
    let hybrid = group_consecutive_ranges(&raw_parts, config);
    if hybrid.is_empty() {
        visual
    } else {
        hybrid
    }
}

fn speech_gap_intervals(spans: &[SpeechSpan], total_duration: f64) -> Vec<(f64, f64)> {
    if total_duration <= 0.0 {
        return Vec::new();
    }
    if spans.is_empty() {
        return vec![(0.0, total_duration)];
    }
    let mut ordered = spans.to_vec();
    ordered.sort_by(|a, b| a.start_sec.total_cmp(&b.start_sec));
    let mut gaps = Vec::new();
    if ordered[0].start_sec > 1e-3 {
        gaps.push((0.0, ordered[0].start_sec));
    }
    for pair in ordered.windows(2) {
        gaps.push((pair[0].end_sec, pair[1].start_sec));
    }
    if ordered
        .last()
        .map(|span| span.end_sec < total_duration - 1e-3)
        .unwrap_or(false)
    {
        gaps.push((
            ordered.last().map(|span| span.end_sec).unwrap_or(0.0),
            total_duration,
        ));
    }
    gaps.into_iter().filter(|(a, b)| b - a > 1e-3).collect()
}

fn raw_scene_boundary_times(segments: &[(f64, f64)]) -> Vec<f64> {
    if segments.len() < 2 {
        return Vec::new();
    }
    let mut out: Vec<f64> = segments
        .iter()
        .take(segments.len() - 1)
        .map(|(_, end)| *end)
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    out
}

fn hybrid_internal_cuts(scene_bounds: &[f64], gaps: &[(f64, f64)], tolerance_sec: f64) -> Vec<f64> {
    let min_overlap = 0.02;
    let mut out = Vec::new();
    for &t in scene_bounds {
        for &(g0, g1) in gaps {
            let lo = t - tolerance_sec;
            let hi = t + tolerance_sec;
            let overlap = g1.min(hi) - g0.max(lo);
            if overlap >= min_overlap {
                out.push(t);
                break;
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    out
}

fn intervals_from_internal_cuts(cuts: &[f64], total_duration: f64) -> Vec<(f64, f64)> {
    let mut inner: Vec<f64> = cuts
        .iter()
        .copied()
        .filter(|c| *c > 0.0 && *c < total_duration)
        .collect();
    inner.sort_by(f64::total_cmp);
    inner.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    let mut points = Vec::with_capacity(inner.len() + 2);
    points.push(0.0);
    points.extend(inner);
    points.push(total_duration);
    points
        .windows(2)
        .filter_map(|pair| (pair[1] > pair[0]).then_some((pair[0], pair[1])))
        .collect()
}

fn group_consecutive_ranges(
    parts: &[(f64, f64)],
    config: &ClipProcessingConfig,
) -> Vec<(f64, f64)> {
    if parts.is_empty() {
        return Vec::new();
    }
    let mut merged = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let start = parts[i].0;
        let mut end = parts[i].1;
        i += 1;
        while i < parts.len() && end - start < config.clip_min_duration as f64 {
            end = parts[i].1;
            i += 1;
        }
        while i < parts.len() && parts[i].1 - start <= config.clip_max_duration as f64 {
            end = parts[i].1;
            i += 1;
        }
        merged.extend(split_long_segment(start, end, config));
    }
    merged
}

fn group_scenes(
    segments: &[(f64, f64)],
    total_duration: f64,
    config: &ClipProcessingConfig,
) -> Vec<(f64, f64)> {
    if total_duration <= 0.0 {
        return Vec::new();
    }
    if segments.is_empty() {
        return split_long_segment(0.0, total_duration, config);
    }
    group_consecutive_ranges(segments, config)
}

fn split_long_segment(start: f64, end: f64, config: &ClipProcessingConfig) -> Vec<(f64, f64)> {
    let max_duration = config.clip_max_duration as f64;
    if end <= start {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut t = start;
    while end - t > max_duration {
        out.push((t, t + max_duration));
        t += max_duration;
    }
    let remainder = end - t;
    if remainder >= config.clip_min_duration as f64 || out.is_empty() {
        if remainder > 0.0 {
            out.push((t, end));
        }
    } else if let Some((last_start, _)) = out.pop() {
        out.push((last_start, end));
    }
    out
}

fn transcript_for_clip_range(
    speech_segments: &[SpeechSpan],
    start_sec: f64,
    end_sec: f64,
) -> Option<String> {
    let texts: Vec<&str> = speech_segments
        .iter()
        .filter(|span| span.end_sec > start_sec && span.start_sec < end_sec)
        .map(|span| span.text.trim())
        .filter(|text| !text.is_empty())
        .collect();
    if texts.is_empty() {
        None
    } else {
        Some(texts.join(" "))
    }
}

fn extract_clip(src: &Path, dest: &Path, start_sec: f64, duration_sec: f64) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(start_sec.to_string())
        .arg("-i")
        .arg(src)
        .arg("-t")
        .arg(duration_sec.to_string())
        .arg("-c")
        .arg("copy")
        .arg("-avoid_negative_ts")
        .arg("make_zero")
        .arg(dest)
        .output()
        .map_err(|e| format!("ffmpeg clip extract failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg clip extract failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn extract_thumbnail(video_path: &Path, dest: &Path, clip_duration_sec: f64) -> Result<(), String> {
    let offset = clip_duration_sec.mul_add(0.5, 0.0).clamp(0.0, 1.0);
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(offset.to_string())
        .arg("-i")
        .arg(video_path)
        .arg("-vframes")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg(dest)
        .output()
        .map_err(|e| format!("ffmpeg thumbnail failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg thumbnail failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn next_clip_file_index(out_dir: &Path) -> Result<i64, String> {
    if !out_dir.is_dir() {
        return Ok(1);
    }
    let mut max_index = 0;
    for entry in fs::read_dir(out_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        if let Some(index) = parse_clip_index(&entry.file_name().to_string_lossy()) {
            max_index = max_index.max(index);
        }
    }
    Ok(max_index + 1)
}

fn parse_clip_index(name: &str) -> Option<i64> {
    let stem = name
        .strip_suffix(".mp4")
        .or_else(|| name.strip_suffix(".MP4"))
        .or_else(|| name.strip_suffix(".jpg"))
        .or_else(|| name.strip_suffix(".JPG"))?;
    let n = stem.strip_prefix("clip_")?;
    if n.len() != 3 || !n.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    n.parse::<i64>().ok()
}

fn today_ymd_hcm() -> String {
    now_timestamp_hcm()
        .split_once(' ')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(now_timestamp_hcm)
}

#[cfg(test)]
mod tests {
    use super::{
        group_consecutive_ranges, group_scenes_with_speech, parse_clip_index,
        transcript_for_clip_range, ClipProcessingConfig,
    };
    use crate::workflow::record_node::SpeechSpan;
    use std::path::PathBuf;

    fn config() -> ClipProcessingConfig {
        ClipProcessingConfig {
            clip_min_duration: 15,
            clip_max_duration: 90,
            scene_threshold: 30.0,
            speech_cut_tolerance_sec: 1.5,
            storage_root: PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn group_consecutive_ranges_merges_short_parts_and_splits_long_ones() {
        let grouped =
            group_consecutive_ranges(&[(0.0, 5.0), (5.0, 20.0), (20.0, 140.0)], &config());
        assert_eq!(grouped, vec![(0.0, 20.0), (20.0, 110.0), (110.0, 140.0)]);
    }

    #[test]
    fn speech_gaps_can_replace_visual_grouping() {
        let spans = vec![
            SpeechSpan {
                start_sec: 0.0,
                end_sec: 48.0,
                text: "first".to_string(),
                confidence: None,
            },
            SpeechSpan {
                start_sec: 52.0,
                end_sec: 130.0,
                text: "second".to_string(),
                confidence: None,
            },
        ];
        let grouped =
            group_scenes_with_speech(&[(0.0, 50.0), (50.0, 130.0)], &spans, 130.0, &config());
        assert_eq!(grouped, vec![(0.0, 50.0), (50.0, 130.0)]);
    }

    #[test]
    fn transcript_for_clip_joins_overlapping_non_empty_text() {
        let spans = vec![
            SpeechSpan {
                start_sec: 1.0,
                end_sec: 3.0,
                text: " hello ".to_string(),
                confidence: None,
            },
            SpeechSpan {
                start_sec: 4.0,
                end_sec: 5.0,
                text: "world".to_string(),
                confidence: None,
            },
        ];
        assert_eq!(
            transcript_for_clip_range(&spans, 2.0, 4.5).as_deref(),
            Some("hello world")
        );
    }

    #[test]
    fn parse_clip_index_reads_mp4_and_jpg_names() {
        assert_eq!(parse_clip_index("clip_007.mp4"), Some(7));
        assert_eq!(parse_clip_index("clip_008.jpg"), Some(8));
        assert_eq!(parse_clip_index("other_008.jpg"), None);
    }
}
