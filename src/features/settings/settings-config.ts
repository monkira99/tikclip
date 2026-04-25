/** Mirrors `sidecar/src/config.py` defaults when SQLite has no row. */
export const DEFAULTS = {
  maxConcurrent: "5",
  pollInterval: "30",
  clipMin: "15",
  clipMax: "90",
  /** Minutes per recording when auto-record does not override (maps to TIKCLIP_MAX_DURATION_MINUTES). */
  recordingMaxMinutes: "5",
  geminiEmbeddingModel: "gemini-embedding-2-preview",
  geminiEmbeddingDim: "1536",
  autoTagClipFrames: "4",
  autoTagClipMaxScore: "0.35",
  suggestWeightImage: "0.6",
  suggestWeightText: "0.4",
  suggestMinFusedScore: "0.25",
  suggestImageEmbedFocusPrompt:
    "Focus on the main product in this image for similarity to product catalog photos.",
  speechMergeGapSec: "0.5",
  speechCutToleranceSec: "1.5",
  sttNumThreads: "4",
} as const;

export const KEY_SPEECH_MERGE_GAP = "speech_merge_gap_sec";
export const KEY_SPEECH_CUT_TOLERANCE = "speech_cut_tolerance_sec";
export const KEY_STT_NUM_THREADS = "stt_num_threads";
export const KEY_STT_QUANTIZE = "stt_quantize";

export const KEY_PRODUCT_VECTOR = "product_vector_enabled";
export const KEY_GEMINI_API_KEY = "gemini_api_key";
export const KEY_GEMINI_EMBEDDING_MODEL = "gemini_embedding_model";
export const KEY_GEMINI_EMBEDDING_DIM = "gemini_embedding_dimensions";
export const KEY_AUTO_TAG_CLIP = "auto_tag_clip_product_enabled";
export const KEY_AUTO_TAG_FRAMES = "auto_tag_clip_frame_count";
export const KEY_AUTO_TAG_MAX_SCORE = "auto_tag_clip_max_score";
export const KEY_SUGGEST_WEIGHT_IMAGE = "suggest_weight_image";
export const KEY_SUGGEST_WEIGHT_TEXT = "suggest_weight_text";
export const KEY_SUGGEST_MIN_FUSED_SCORE = "suggest_min_fused_score";
export const KEY_DEBUG_KEEP_SUGGEST_FRAMES = "debug_keep_suggest_clip_frames";
export const KEY_SUGGEST_IMAGE_EMBED_FOCUS_PROMPT = "suggest_image_embed_focus_prompt";

export const KEY_RAW_RETENTION = "TIKCLIP_RAW_RETENTION_DAYS";
export const KEY_ARCHIVE_RETENTION = "TIKCLIP_ARCHIVE_RETENTION_DAYS";
export const KEY_STORAGE_WARN = "TIKCLIP_STORAGE_WARN_PERCENT";
export const KEY_STORAGE_CLEANUP = "TIKCLIP_STORAGE_CLEANUP_PERCENT";
