/** Mirrors `sidecar/src/config.py` defaults when SQLite has no row. */
export const DEFAULTS = {
  geminiEmbeddingModel: "gemini-embedding-2-preview",
  geminiEmbeddingDim: "1536",
  autoTagClipFrames: "4",
  autoTagClipMaxScore: "0.35",
  suggestWeightImage: "0.6",
  suggestWeightText: "0.4",
  suggestMinFusedScore: "0.25",
  suggestImageEmbedFocusPrompt:
    "Focus on the main product in this image for similarity to product catalog photos.",
} as const;

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
