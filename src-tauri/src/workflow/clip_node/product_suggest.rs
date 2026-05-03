use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use uuid::Uuid;

use super::common::{
    bool_setting, float_setting, int_setting, resolve_storage_media_path, storage_relative,
    string_setting,
};
use super::product_vectors;

const TEXT_PATH_PLACEHOLDER: &str = "__text__";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SuggestProductForClipInput {
    pub video_path: String,
    pub thumbnail_path: Option<String>,
    pub transcript_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestImageEvidenceHit {
    pub product_id: i64,
    pub score: f64,
    pub product_name: Option<String>,
    pub product_description: Option<String>,
    pub catalog_media_relative_path: Option<String>,
    pub catalog_source_url: Option<String>,
    pub catalog_modality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestFrameRow {
    pub index: i64,
    pub source: String,
    pub media_relative_path: String,
    pub outcome: String,
    pub error: Option<String>,
    pub top_product_id: Option<i64>,
    pub top_score: Option<f64>,
    pub top_product_name: Option<String>,
    pub matched_product_description: Option<String>,
    pub image_evidence_hits: Vec<ClipSuggestImageEvidenceHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestVoteRow {
    pub product_id: i64,
    pub vote_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestTextHit {
    pub product_id: i64,
    pub score: f64,
    pub product_name: Option<String>,
    pub product_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestTranscriptSegmentRow {
    pub segment_index: i64,
    pub segment_text: String,
    pub outcome: String,
    pub error: Option<String>,
    pub best_product_id: Option<i64>,
    pub best_score: Option<f64>,
    pub best_product_name: Option<String>,
    pub matched_product_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestProductRankRow {
    pub product_id: i64,
    pub product_name: Option<String>,
    pub frame_hit_count: i64,
    pub avg_frame_distance: Option<f64>,
    pub image_score: f64,
    pub transcript_text_score: Option<f64>,
    pub text_score: f64,
    pub final_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipSuggestProductResponse {
    pub matched: bool,
    pub product_id: Option<i64>,
    pub product_name: Option<String>,
    pub best_score: Option<f64>,
    pub frames_used: i64,
    pub skipped_reason: Option<String>,
    pub video_relative_path: Option<String>,
    pub thumbnail_used: bool,
    pub extracted_frame_count: i64,
    pub frames_searched: i64,
    pub config_target_extracted_frames: i64,
    pub config_max_score_threshold: f64,
    pub suggest_weight_image: f64,
    pub suggest_weight_text: f64,
    pub suggest_min_fused_score: f64,
    pub suggest_image_embed_focus_prompt: String,
    pub pick_method: Option<String>,
    pub votes_by_product: Vec<ClipSuggestVoteRow>,
    pub product_ranks: Vec<ClipSuggestProductRankRow>,
    pub transcript_segment_evidence: Vec<ClipSuggestTranscriptSegmentRow>,
    pub candidate_product_id: Option<i64>,
    pub candidate_product_name: Option<String>,
    pub candidate_score: Option<f64>,
    pub frame_rows: Vec<ClipSuggestFrameRow>,
    pub text_search_hits: Vec<ClipSuggestTextHit>,
    pub text_search_used: bool,
    pub fusion_method: Option<String>,
    pub debug_extracted_frames_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct SuggestConfig {
    auto_tag_enabled: bool,
    product_vector_enabled: bool,
    gemini_api_key_present: bool,
    frame_count: i64,
    max_score: f64,
    weight_image: f64,
    weight_text: f64,
    min_fused_score: f64,
    image_focus_prompt: String,
    debug_keep_frames: bool,
}

impl SuggestConfig {
    fn from_conn(conn: &Connection) -> Result<Self, String> {
        Ok(Self {
            auto_tag_enabled: bool_setting(conn, "auto_tag_clip_product_enabled", false)?,
            product_vector_enabled: bool_setting(conn, "product_vector_enabled", false)?,
            gemini_api_key_present: string_setting(conn, "gemini_api_key")?.is_some(),
            frame_count: int_setting(conn, "auto_tag_clip_frame_count", 4)?.clamp(1, 12),
            max_score: float_setting(conn, "auto_tag_clip_max_score", 0.35)?,
            weight_image: float_setting(conn, "suggest_weight_image", 0.6)?,
            weight_text: float_setting(conn, "suggest_weight_text", 0.4)?,
            min_fused_score: float_setting(conn, "suggest_min_fused_score", 0.25)?,
            image_focus_prompt: string_setting(conn, "suggest_image_embed_focus_prompt")?
                .unwrap_or_else(|| {
                    "Focus on the main product in this image for similarity to product catalog photos."
                        .to_string()
                }),
            debug_keep_frames: bool_setting(conn, "debug_keep_suggest_clip_frames", false)?,
        })
    }

    fn base_response(&self) -> ClipSuggestProductResponse {
        ClipSuggestProductResponse {
            config_target_extracted_frames: self.frame_count,
            config_max_score_threshold: self.max_score,
            suggest_weight_image: self.weight_image,
            suggest_weight_text: self.weight_text,
            suggest_min_fused_score: self.min_fused_score,
            suggest_image_embed_focus_prompt: self.image_focus_prompt.clone(),
            ..ClipSuggestProductResponse::default()
        }
    }
}

pub fn suggest_product_for_clip(
    conn: &Connection,
    storage_root: &Path,
    input: &SuggestProductForClipInput,
) -> Result<ClipSuggestProductResponse, String> {
    suggest_product_for_clip_inner(&SearchContext::Conn(conn), storage_root, input)
}

pub fn suggest_product_for_clip_with_db_lock(
    db: &Mutex<Connection>,
    storage_root: &Path,
    input: &SuggestProductForClipInput,
) -> Result<ClipSuggestProductResponse, String> {
    suggest_product_for_clip_inner(&SearchContext::Shared(db), storage_root, input)
}

enum SearchContext<'a> {
    Conn(&'a Connection),
    Shared(&'a Mutex<Connection>),
}

impl SearchContext<'_> {
    fn config(&self) -> Result<SuggestConfig, String> {
        match self {
            SearchContext::Conn(conn) => SuggestConfig::from_conn(conn),
            SearchContext::Shared(db) => {
                let conn = db.lock().map_err(|e| e.to_string())?;
                SuggestConfig::from_conn(&conn)
            }
        }
    }

    fn search_by_text(
        &self,
        query: &str,
        top_k: i64,
    ) -> Result<Vec<product_vectors::ProductEmbeddingSearchHit>, String> {
        match self {
            SearchContext::Conn(conn) => product_vectors::search_by_text(conn, query, top_k),
            SearchContext::Shared(db) => {
                product_vectors::search_by_text_with_db_lock(db, query, top_k)
            }
        }
    }

    fn search_by_media_path(
        &self,
        storage_root: &Path,
        media_path: &str,
        kind: &str,
        top_k: i64,
        companion_text: Option<&str>,
    ) -> Result<Vec<product_vectors::ProductEmbeddingSearchHit>, String> {
        match self {
            SearchContext::Conn(conn) => product_vectors::search_by_media_path(
                conn,
                storage_root,
                media_path,
                kind,
                top_k,
                companion_text,
            ),
            SearchContext::Shared(db) => product_vectors::search_by_media_path_with_db_lock(
                db,
                storage_root,
                media_path,
                kind,
                top_k,
                companion_text,
            ),
        }
    }
}

fn suggest_product_for_clip_inner(
    search: &SearchContext<'_>,
    storage_root: &Path,
    input: &SuggestProductForClipInput,
) -> Result<ClipSuggestProductResponse, String> {
    let config = search.config()?;
    let mut response = config.base_response();
    log::info!(
        "clip product suggest started video_path_present={} thumbnail_present={} transcript_present={} frame_count={} weights=({:.2},{:.2})",
        !input.video_path.trim().is_empty(),
        input.thumbnail_path
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        input
            .transcript_text
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        config.frame_count,
        config.weight_image,
        config.weight_text
    );

    if !config.auto_tag_enabled {
        response.skipped_reason = Some("auto_tag_clip_product_enabled is off".to_string());
        log::info!("clip product suggest skipped reason=auto_tag_disabled");
        return Ok(response);
    }
    if !config.product_vector_enabled {
        response.skipped_reason = Some("product_vector_enabled is off".to_string());
        log::info!("clip product suggest skipped reason=product_vector_disabled");
        return Ok(response);
    }
    if !config.gemini_api_key_present {
        response.skipped_reason = Some("Gemini API key is not configured".to_string());
        log::info!("clip product suggest skipped reason=missing_gemini_api_key");
        return Ok(response);
    }

    let video = match resolve_storage_media_path(storage_root, input.video_path.as_str()) {
        Ok(path) => path,
        Err(err) => {
            log::warn!(
                "clip product suggest skipped reason=video_path_error error={}",
                err
            );
            response.skipped_reason = Some(err);
            return Ok(response);
        }
    };
    let video_rel = storage_relative(storage_root, &video);
    response.video_relative_path = Some(video_rel.clone());
    if !video.is_file() {
        response.skipped_reason = Some("clip video file not found".to_string());
        log::warn!(
            "clip product suggest skipped reason=video_file_not_found path={}",
            video.display()
        );
        return Ok(response);
    }

    let transcript = input
        .transcript_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let weight_image = config.weight_image;
    let weight_text = config.weight_text;
    if weight_image <= 0.0 && weight_text <= 0.0 {
        response.skipped_reason = Some(
            "suggest_weight_image and suggest_weight_text are both zero (nothing to score)"
                .to_string(),
        );
        log::info!("clip product suggest skipped reason=zero_weights");
        return Ok(response);
    }

    let mut work_dir: Option<PathBuf> = None;
    let mut extracted: Vec<PathBuf> = Vec::new();
    let mut frame_paths: Vec<PathBuf> = Vec::new();
    let mut thumbnail_used = false;

    let result = (|| {
        if weight_image > 0.0 {
            if let Some(thumbnail_raw) = input.thumbnail_path.as_deref() {
                if let Ok(thumb) = resolve_storage_media_path(storage_root, thumbnail_raw) {
                    if thumb.is_file() {
                        frame_paths.push(thumb);
                        thumbnail_used = true;
                    }
                }
            }

            let run_id = Uuid::new_v4().simple().to_string();
            let run_id_short = &run_id[..8];
            let dir = if config.debug_keep_frames {
                storage_root
                    .join("debug")
                    .join("suggest_clip_frames")
                    .join(format!("{}_{}", timestamp_run_prefix(), run_id_short))
            } else {
                storage_root
                    .join("tmp")
                    .join("clip_frames")
                    .join(Uuid::new_v4().to_string())
            };
            extracted = extract_frames_evenly(&video, config.frame_count, &dir)?;
            frame_paths.extend(extracted.iter().cloned());
            work_dir = Some(dir);
            log::info!(
                "clip product suggest frames prepared video={} thumbnail_used={} extracted={} total_frames={}",
                video_rel,
                thumbnail_used,
                extracted.len(),
                frame_paths.len()
            );
        }

        response.thumbnail_used = thumbnail_used;
        response.extracted_frame_count = i64::try_from(extracted.len()).unwrap_or(i64::MAX);
        if config.debug_keep_frames {
            if let Some(dir) = work_dir.as_ref() {
                response.debug_extracted_frames_dir = Some(storage_relative(storage_root, dir));
                let _ = fs::write(
                    dir.join("README.txt"),
                    format!(
                        "video_relative_path={video_rel}\nthumbnail_included={thumbnail_used}\nextracted_jpeg_count={}\n",
                        extracted.len()
                    ),
                );
            }
        }

        let mut text_hits = Vec::new();
        if weight_text > 0.0 && !transcript.is_empty() {
            text_hits = search.search_by_text(transcript, 5).unwrap_or_default();
            response.text_search_used = !text_hits.is_empty();
            log::info!(
                "clip product suggest transcript search completed transcript_chars={} hits={}",
                transcript.chars().count(),
                text_hits.len()
            );
            response.text_search_hits = text_hits
                .iter()
                .map(|hit| ClipSuggestTextHit {
                    product_id: hit.product_id,
                    score: hit.score,
                    product_name: hit.product_name.clone(),
                    product_description: hit.product_description.clone(),
                })
                .collect();
            response.transcript_segment_evidence =
                transcript_segment_evidence(search, transcript, &text_hits);
        }

        if weight_image > 0.0 && frame_paths.is_empty() {
            response.skipped_reason = Some("could not extract any frames".to_string());
            log::warn!("clip product suggest skipped reason=no_frames");
            response.product_ranks = build_rankings(
                response.frame_rows.as_slice(),
                text_hits.as_slice(),
                weight_image,
                weight_text,
            );
            return Ok(response);
        }

        let focus = config.image_focus_prompt.trim();
        let companion_text = if focus.is_empty() { None } else { Some(focus) };
        for (index, frame) in frame_paths.iter().enumerate() {
            let source = if thumbnail_used && index == 0 {
                "thumbnail"
            } else {
                "extracted"
            };
            let rel = storage_relative(storage_root, frame);
            match search.search_by_media_path(
                storage_root,
                frame.to_string_lossy().as_ref(),
                "image",
                3,
                companion_text,
            ) {
                Ok(hits) if hits.is_empty() => {
                    log::info!(
                        "clip product suggest frame no_hit index={} source={} path={}",
                        index,
                        source,
                        rel
                    );
                    response.frame_rows.push(ClipSuggestFrameRow {
                        index: i64::try_from(index).unwrap_or(i64::MAX),
                        source: source.to_string(),
                        media_relative_path: rel,
                        outcome: "no_hit".to_string(),
                        ..ClipSuggestFrameRow::default()
                    });
                }
                Ok(hits) => {
                    response.frames_searched += 1;
                    let top = &hits[0];
                    log::info!(
                        "clip product suggest frame hit index={} source={} top_product_id={} top_score={:.4}",
                        index,
                        source,
                        top.product_id,
                        top.score
                    );
                    let evidence = hits
                        .iter()
                        .take(3)
                        .map(|hit| ClipSuggestImageEvidenceHit {
                            product_id: hit.product_id,
                            score: hit.score,
                            product_name: hit.product_name.clone(),
                            product_description: hit.product_description.clone(),
                            catalog_media_relative_path: catalog_media_rel(
                                storage_root,
                                &hit.image_path,
                            ),
                            catalog_source_url: hit.source_url.clone(),
                            catalog_modality: catalog_modality(hit.modality.as_deref()),
                        })
                        .collect();
                    response.frame_rows.push(ClipSuggestFrameRow {
                        index: i64::try_from(index).unwrap_or(i64::MAX),
                        source: source.to_string(),
                        media_relative_path: rel,
                        outcome: "hit".to_string(),
                        top_product_id: Some(top.product_id),
                        top_score: Some(top.score),
                        top_product_name: top.product_name.clone(),
                        matched_product_description: top.product_description.clone(),
                        image_evidence_hits: evidence,
                        ..ClipSuggestFrameRow::default()
                    });
                }
                Err(err) => {
                    log::warn!(
                        "clip product suggest frame search failed index={} source={} error={}",
                        index,
                        source,
                        err
                    );
                    response.frame_rows.push(ClipSuggestFrameRow {
                        index: i64::try_from(index).unwrap_or(i64::MAX),
                        source: source.to_string(),
                        media_relative_path: rel,
                        outcome: "error".to_string(),
                        error: Some(err),
                        ..ClipSuggestFrameRow::default()
                    });
                }
            }
        }

        response.frames_used = i64::try_from(frame_paths.len()).unwrap_or(i64::MAX);
        response.product_ranks = build_rankings(
            response.frame_rows.as_slice(),
            text_hits.as_slice(),
            weight_image,
            weight_text,
        );
        response.votes_by_product = vote_rows(response.frame_rows.as_slice());
        response.pick_method = Some("unified_score".to_string());

        if response.product_ranks.is_empty() {
            response.skipped_reason = Some("no vector hits from frames or transcript".to_string());
            log::info!("clip product suggest skipped reason=no_vector_hits");
            return Ok(response);
        }

        let winner = response.product_ranks[0].clone();
        if winner.final_score < config.min_fused_score {
            response.skipped_reason = Some(format!(
                "final score {:.4} below minimum {:.4}",
                winner.final_score, config.min_fused_score
            ));
            response.candidate_product_id = Some(winner.product_id);
            response.candidate_product_name = winner.product_name;
            response.candidate_score = Some(winner.final_score);
            log::info!(
                "clip product suggest rejected product_id={} final_score={:.4} min_fused_score={:.4}",
                winner.product_id,
                winner.final_score,
                config.min_fused_score
            );
            return Ok(response);
        }
        if weight_image > 0.0 {
            if let Some(avg) = winner.avg_frame_distance {
                if avg > config.max_score {
                    response.skipped_reason = Some(format!(
                        "winner image distance {:.4} above threshold {:.4}",
                        avg, config.max_score
                    ));
                    response.candidate_product_id = Some(winner.product_id);
                    response.candidate_product_name = winner.product_name;
                    response.candidate_score = Some(winner.final_score);
                    log::info!(
                        "clip product suggest rejected product_id={} avg_frame_distance={:.4} max_score={:.4}",
                        winner.product_id,
                        avg,
                        config.max_score
                    );
                    return Ok(response);
                }
            }
        }

        response.matched = true;
        response.product_id = Some(winner.product_id);
        response.product_name = winner.product_name;
        response.best_score = Some(winner.final_score);
        log::info!(
            "clip product suggest matched product_id={} final_score={:.4} frames_used={} text_used={}",
            winner.product_id,
            winner.final_score,
            response.frames_used,
            response.text_search_used
        );
        Ok(response)
    })();

    if let Some(dir) = work_dir {
        if !config.debug_keep_frames {
            let _ = fs::remove_dir_all(dir);
        }
    }

    result
}

pub fn maybe_auto_tag_clip(
    conn: &Connection,
    storage_root: &Path,
    clip_id: i64,
    input: &SuggestProductForClipInput,
) -> Result<Option<i64>, String> {
    log::info!("clip auto-tag started clip_id={}", clip_id);
    let result = suggest_product_for_clip(conn, storage_root, input)?;
    let Some(product_id) = result.product_id else {
        log::info!(
            "clip auto-tag skipped clip_id={} reason={}",
            clip_id,
            result
                .skipped_reason
                .as_deref()
                .unwrap_or("no_product_match")
        );
        return Ok(None);
    };
    conn.execute(
        "INSERT OR IGNORE INTO clip_products (clip_id, product_id) VALUES (?1, ?2)",
        params![clip_id, product_id],
    )
    .map_err(|e| e.to_string())?;
    log::info!(
        "clip auto-tag linked clip_id={} product_id={}",
        clip_id,
        product_id
    );
    Ok(Some(product_id))
}

fn transcript_segment_evidence(
    search: &SearchContext<'_>,
    transcript: &str,
    text_hits: &[product_vectors::ProductEmbeddingSearchHit],
) -> Vec<ClipSuggestTranscriptSegmentRow> {
    let segments = split_transcript_segments(transcript, 8);
    if segments.is_empty() {
        return Vec::new();
    }
    if segments.len() == 1 {
        let segment = segments[0].clone();
        if let Some(hit) = text_hits.first() {
            return vec![ClipSuggestTranscriptSegmentRow {
                segment_index: 0,
                segment_text: segment,
                outcome: "hit".to_string(),
                best_product_id: Some(hit.product_id),
                best_score: Some(hit.score),
                best_product_name: hit.product_name.clone(),
                matched_product_description: hit.product_description.clone(),
                ..ClipSuggestTranscriptSegmentRow::default()
            }];
        }
        return vec![ClipSuggestTranscriptSegmentRow {
            segment_index: 0,
            segment_text: segment,
            outcome: "no_hit".to_string(),
            ..ClipSuggestTranscriptSegmentRow::default()
        }];
    }

    segments
        .into_iter()
        .enumerate()
        .map(
            |(index, segment)| match search.search_by_text(segment.as_str(), 1) {
                Ok(hits) if hits.is_empty() => ClipSuggestTranscriptSegmentRow {
                    segment_index: i64::try_from(index).unwrap_or(i64::MAX),
                    segment_text: segment,
                    outcome: "no_hit".to_string(),
                    ..ClipSuggestTranscriptSegmentRow::default()
                },
                Ok(hits) => {
                    let hit = &hits[0];
                    ClipSuggestTranscriptSegmentRow {
                        segment_index: i64::try_from(index).unwrap_or(i64::MAX),
                        segment_text: segment,
                        outcome: "hit".to_string(),
                        best_product_id: Some(hit.product_id),
                        best_score: Some(hit.score),
                        best_product_name: hit.product_name.clone(),
                        matched_product_description: hit.product_description.clone(),
                        ..ClipSuggestTranscriptSegmentRow::default()
                    }
                }
                Err(err) => ClipSuggestTranscriptSegmentRow {
                    segment_index: i64::try_from(index).unwrap_or(i64::MAX),
                    segment_text: segment,
                    outcome: "error".to_string(),
                    error: Some(err),
                    ..ClipSuggestTranscriptSegmentRow::default()
                },
            },
        )
        .collect()
}

pub(crate) fn build_rankings(
    frame_rows: &[ClipSuggestFrameRow],
    text_hits: &[product_vectors::ProductEmbeddingSearchHit],
    weight_image: f64,
    weight_text: f64,
) -> Vec<ClipSuggestProductRankRow> {
    let mut frame_best_by_product: HashMap<i64, HashMap<i64, f64>> = HashMap::new();
    let mut names: HashMap<i64, String> = HashMap::new();
    for row in frame_rows {
        if row.outcome != "hit" {
            continue;
        }
        for hit in &row.image_evidence_hits {
            if let Some(name) = hit.product_name.as_ref().filter(|s| !s.trim().is_empty()) {
                names.entry(hit.product_id).or_insert_with(|| name.clone());
            }
            let bests = frame_best_by_product.entry(hit.product_id).or_default();
            let current = bests.get(&row.index).copied();
            if current.is_none_or(|score| hit.score < score) {
                bests.insert(row.index, hit.score);
            }
        }
    }

    let mut image_scores = HashMap::new();
    let mut image_counts = HashMap::new();
    let mut image_distances = HashMap::new();
    for (product_id, per_frame) in frame_best_by_product {
        let values: Vec<f64> = per_frame.values().copied().collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        image_scores.insert(product_id, (1.0 - mean).max(0.0));
        image_counts.insert(product_id, i64::try_from(values.len()).unwrap_or(i64::MAX));
        image_distances.insert(product_id, mean);
    }

    let mut text_scores = HashMap::new();
    for hit in text_hits {
        text_scores.entry(hit.product_id).or_insert(hit.score);
        if let Some(name) = hit.product_name.as_ref().filter(|s| !s.trim().is_empty()) {
            names.entry(hit.product_id).or_insert_with(|| name.clone());
        }
    }

    let mut all_product_ids: HashSet<i64> = HashSet::new();
    all_product_ids.extend(image_scores.keys().copied());
    all_product_ids.extend(text_scores.keys().copied());
    let mut rows: Vec<ClipSuggestProductRankRow> = all_product_ids
        .into_iter()
        .map(|product_id| {
            let image_score = image_scores.get(&product_id).copied().unwrap_or(0.0);
            let text_score = text_scores.get(&product_id).copied().unwrap_or(0.0);
            ClipSuggestProductRankRow {
                product_id,
                product_name: names.get(&product_id).cloned(),
                frame_hit_count: image_counts.get(&product_id).copied().unwrap_or(0),
                avg_frame_distance: image_distances.get(&product_id).copied(),
                image_score,
                transcript_text_score: text_scores.get(&product_id).copied(),
                text_score,
                final_score: (weight_image * image_score) + (weight_text * text_score),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.final_score.total_cmp(&a.final_score));
    rows
}

fn vote_rows(frame_rows: &[ClipSuggestFrameRow]) -> Vec<ClipSuggestVoteRow> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    for row in frame_rows {
        if row.outcome == "hit" {
            if let Some(product_id) = row.top_product_id {
                *counts.entry(product_id).or_default() += 1;
            }
        }
    }
    let mut rows: Vec<ClipSuggestVoteRow> = counts
        .into_iter()
        .map(|(product_id, vote_count)| ClipSuggestVoteRow {
            product_id,
            vote_count,
        })
        .collect();
    rows.sort_by(|a, b| b.vote_count.cmp(&a.vote_count));
    rows
}

pub(crate) fn split_transcript_segments(text: &str, max_segments: usize) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<String> = trimmed
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();
    if parts.len() <= 1 {
        parts = split_sentences(trimmed);
    }
    if parts.is_empty() {
        return vec![trimmed.chars().take(2000).collect()];
    }
    let mut out = Vec::new();
    for part in parts {
        if part.len() < 6 {
            continue;
        }
        out.push(part.chars().take(2000).collect());
        if out.len() >= max_segments {
            break;
        }
    }
    if out.is_empty() {
        vec![trimmed.chars().take(2000).collect()]
    } else {
        out
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (pos, ch) in &chars {
        if !matches!(ch, '.' | '!' | '?') {
            continue;
        }
        let next_is_space = text[*pos + ch.len_utf8()..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
        if next_is_space {
            let sentence = text[start..*pos + ch.len_utf8()].trim();
            if !sentence.is_empty() {
                out.push(sentence.to_string());
            }
            start = *pos + ch.len_utf8();
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn extract_frames_evenly(
    video_path: &Path,
    count: i64,
    work_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    if count < 1 {
        return Ok(Vec::new());
    }
    let duration = probe_duration_seconds(video_path)?;
    if duration <= 0.0 {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for index in 0..count {
        let timestamp = duration * (index + 1) as f64 / (count + 1) as f64;
        let dest = work_dir.join(format!("frame_{index:02}.jpg"));
        let output = Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss")
            .arg(timestamp.to_string())
            .arg("-i")
            .arg(video_path)
            .arg("-vframes")
            .arg("1")
            .arg("-q:v")
            .arg("3")
            .arg(&dest)
            .output()
            .map_err(|e| format!("ffmpeg frame extract failed: {e}"))?;
        if output.status.success() && dest.is_file() {
            out.push(dest);
        }
    }
    Ok(out)
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

fn catalog_media_rel(storage_root: &Path, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == TEXT_PATH_PLACEHOLDER {
        return None;
    }
    Some(storage_relative(storage_root, Path::new(trimmed)))
}

fn catalog_modality(raw: Option<&str>) -> Option<String> {
    match raw {
        Some("image") => Some("image".to_string()),
        Some("video") => Some("video".to_string()),
        _ => None,
    }
}

fn timestamp_run_prefix() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

#[cfg(test)]
mod tests {
    use super::product_vectors::ProductEmbeddingSearchHit;
    use super::{
        build_rankings, resolve_storage_media_path, split_transcript_segments, ClipSuggestFrameRow,
        ClipSuggestImageEvidenceHit,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("tikclip-suggest-{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn split_transcript_segments_prefers_paragraphs_then_limits_count() {
        let text = "First product block.\n\nSecond product block.\n\nThird product block.";
        let segments = split_transcript_segments(text, 2);
        assert_eq!(
            segments,
            vec!["First product block.", "Second product block."]
        );
    }

    #[test]
    fn build_rankings_combines_image_and_text_scores() {
        let frame_rows = vec![ClipSuggestFrameRow {
            index: 0,
            outcome: "hit".to_string(),
            image_evidence_hits: vec![ClipSuggestImageEvidenceHit {
                product_id: 7,
                score: 0.2,
                product_name: Some("Bag".to_string()),
                ..ClipSuggestImageEvidenceHit::default()
            }],
            ..ClipSuggestFrameRow::default()
        }];
        let text_hits = vec![ProductEmbeddingSearchHit {
            product_id: 7,
            score: 0.8,
            product_name: Some("Bag".to_string()),
            ..ProductEmbeddingSearchHit::default()
        }];
        let ranks = build_rankings(&frame_rows, &text_hits, 0.6, 0.4);
        assert_eq!(ranks[0].product_id, 7);
        assert!((ranks[0].final_score - 0.8).abs() < 0.0001);
    }

    #[test]
    fn resolve_storage_media_path_rejects_paths_outside_storage_root() {
        let root = temp_dir("storage");
        let outside = temp_dir("outside").join("clip.mp4");
        fs::write(&outside, b"x").expect("outside file");
        let err = resolve_storage_media_path(&root, outside.to_string_lossy().as_ref())
            .expect_err("outside path should fail");
        assert_eq!(err, "Media path must be under storage root");
    }
}
