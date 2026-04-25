import { invoke } from "@tauri-apps/api/core";

import { sidecarJson } from "@/lib/api/sidecar-client";
import type { Clip, ClipCaptionStatus, ClipFilters } from "@/types";

export async function listClips(): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips");
}

export async function listClipsFiltered(filters: ClipFilters): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips_filtered", {
    input: {
      status: filters.status === "all" ? null : filters.status,
      account_id: filters.accountId,
      scene_type: filters.sceneType === "all" ? null : filters.sceneType,
      date_from: filters.dateFrom,
      date_to: filters.dateTo,
      search: filters.search || null,
      sort_by: filters.sortBy,
      sort_order: filters.sortOrder,
    },
  });
}

export async function getClipById(clipId: number): Promise<Clip> {
  return invoke<Clip>("get_clip_by_id", { clipId });
}

export async function updateClipStatus(clipId: number, newStatus: string): Promise<void> {
  await invoke("update_clip_status", { clipId, newStatus });
}

export async function updateClipTitle(clipId: number, title: string): Promise<void> {
  await invoke("update_clip_title", { clipId, title });
}

export async function updateClipNotes(clipId: number, notes: string): Promise<void> {
  await invoke("update_clip_notes", { clipId, notes });
}

export async function updateClipCaption(
  clipId: number,
  captionText: string | null,
  captionStatus: ClipCaptionStatus,
  captionError?: string | null,
): Promise<void> {
  await invoke("update_clip_caption", {
    clipId,
    captionText,
    captionStatus,
    captionError: captionError ?? null,
  });
}

export async function batchUpdateClipStatus(clipIds: number[], newStatus: string): Promise<void> {
  await invoke("batch_update_clip_status", { clipIds, newStatus });
}

export async function batchDeleteClips(clipIds: number[]): Promise<void> {
  await invoke("batch_delete_clips", { clipIds });
}

export async function trimClip(body: {
  source_path: string;
  start_sec: number;
  end_sec: number;
  account_id: number;
  recording_id: number;
}): Promise<{ file_path: string; thumbnail_path: string; duration_sec: number }> {
  return sidecarJson("/api/clips/trim", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function insertTrimmedClip(input: {
  recording_id: number;
  account_id: number;
  file_path: string;
  thumbnail_path: string;
  duration_sec: number;
  start_sec: number;
  end_sec: number;
}): Promise<number> {
  return invoke<number>("insert_trimmed_clip", { input });
}

export type ClipSuggestImageEvidenceHit = {
  product_id: number;
  score: number;
  product_name: string | null;
  product_description: string | null;
  /** Ảnh/video catalog đã index, đường dẫn tương đối storage (cặp với query = media_relative_path của frame row). */
  catalog_media_relative_path?: string | null;
  catalog_source_url?: string | null;
  catalog_modality?: "image" | "video" | null;
};

export type ClipSuggestFrameRow = {
  index: number;
  source: "thumbnail" | "extracted";
  media_relative_path: string;
  outcome: "hit" | "no_hit" | "error";
  error: string | null;
  top_product_id: number | null;
  top_score: number | null;
  top_product_name: string | null;
  matched_product_description?: string | null;
  image_evidence_hits?: ClipSuggestImageEvidenceHit[];
};

export type ClipSuggestVoteRow = {
  product_id: number;
  vote_count: number;
};

export type ClipSuggestTextHit = {
  product_id: number;
  score: number;
  product_name: string | null;
  product_description?: string | null;
};

export type ClipSuggestTranscriptSegmentRow = {
  segment_index: number;
  segment_text: string;
  outcome: "hit" | "no_hit" | "error";
  error: string | null;
  best_product_id: number | null;
  best_score: number | null;
  best_product_name: string | null;
  matched_product_description: string | null;
};

export type ClipSuggestProductRankRow = {
  product_id: number;
  product_name: string | null;
  frame_hit_count: number;
  /** Mean best-per-frame cosine distance (lower = closer). */
  avg_frame_distance: number | null;
  /** 1 - avg_frame_distance; 0 khi không có frame hit. [0,1] */
  image_score: number;
  /** Raw score từ full-transcript text search (higher = better). */
  transcript_text_score: number | null;
  /** = transcript_text_score hoặc 0. [0,1] */
  text_score: number;
  /** w_img * image_score + w_txt * text_score. Xếp hạng giảm dần. */
  final_score: number;
};

export type ClipSuggestProductResult = {
  matched: boolean;
  product_id: number | null;
  product_name: string | null;
  best_score: number | null;
  frames_used: number;
  skipped_reason: string | null;
  video_relative_path: string | null;
  thumbnail_used: boolean;
  extracted_frame_count: number;
  frames_searched: number;
  config_target_extracted_frames: number;
  config_max_score_threshold: number;
  suggest_weight_image: number;
  suggest_weight_text: number;
  suggest_min_fused_score: number;
  /** Prompt đang dùng kèm ảnh khi embed frame (echo từ cấu hình). */
  suggest_image_embed_focus_prompt?: string;
  pick_method: "majority_vote" | "min_distance_tiebreak" | "weighted_fusion" | "unified_score" | null;
  votes_by_product: ClipSuggestVoteRow[];
  product_ranks?: ClipSuggestProductRankRow[];
  transcript_segment_evidence?: ClipSuggestTranscriptSegmentRow[];
  candidate_product_id: number | null;
  candidate_product_name: string | null;
  candidate_score: number | null;
  frame_rows: ClipSuggestFrameRow[];
  text_search_hits: ClipSuggestTextHit[];
  text_search_used: boolean;
  fusion_method: string | null;
  /** Khi bật lưu frame debug: đường dẫn tương đối từ thư mục lưu trữ tới thư mục chứa frame_*.jpg */
  debug_extracted_frames_dir?: string | null;
};

export async function suggestProductForClip(body: {
  video_path: string;
  thumbnail_path?: string | null;
  transcript_text?: string | null;
}): Promise<ClipSuggestProductResult> {
  return sidecarJson<ClipSuggestProductResult>("/api/clips/suggest-product", {
    method: "POST",
    body: JSON.stringify({
      video_path: body.video_path,
      thumbnail_path: body.thumbnail_path ?? null,
      transcript_text: body.transcript_text ?? null,
    }),
  });
}

export type GenerateCaptionResult = {
  clip_id: number;
  caption_text: string;
};

export async function generateCaptionForClip(body: {
  clip_id: number;
  username: string;
  transcript_text?: string | null;
  clip_title?: string | null;
}): Promise<GenerateCaptionResult> {
  return sidecarJson<GenerateCaptionResult>("/api/captions/generate", {
    method: "POST",
    body: JSON.stringify({
      clip_id: body.clip_id,
      username: body.username,
      transcript_text: body.transcript_text ?? null,
      clip_title: body.clip_title ?? null,
    }),
  });
}
