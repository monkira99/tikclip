from typing import Literal

from pydantic import BaseModel, Field, field_validator


class HealthResponse(BaseModel):
    status: str = "ok"
    version: str = "0.1.0"
    active_recordings: int = 0
    ws_connections: int = 0


class AccountStatusRequest(BaseModel):
    username: str
    cookies_json: str | None = None
    proxy_url: str | None = None


class AccountStatusResponse(BaseModel):
    username: str
    is_live: bool
    room_id: str | None = None
    stream_url: str | None = None
    viewer_count: int | None = None


class WatchAccountRequest(BaseModel):
    account_id: int
    username: str
    auto_record: bool = False
    cookies_json: str | None = None
    proxy_url: str | None = None


class StartRecordingRequest(BaseModel):
    account_id: int
    username: str
    room_id: str | None = None
    stream_url: str | None = None
    cookies_json: str | None = None
    proxy_url: str | None = None
    max_duration_seconds: int | None = None


class StopRecordingRequest(BaseModel):
    recording_id: str


class RecordingStatusResponse(BaseModel):
    recording_id: str
    account_id: int
    username: str
    status: str
    duration_seconds: int = 0
    file_size_bytes: int = 0
    file_path: str | None = None
    error_message: str | None = None


class ProcessVideoRequest(BaseModel):
    recording_id: str
    username: str
    file_path: str
    account_id: int
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0


class ClipOutput(BaseModel):
    index: int
    path: str
    thumbnail_path: str
    start_sec: float
    end_sec: float
    duration_sec: float
    transcript_text: str | None = None


class SpeechSegmentOutput(BaseModel):
    start_sec: float
    end_sec: float
    text: str
    confidence: float | None = None


class ProcessingStatusResponse(BaseModel):
    recording_id: str
    account_id: int
    username: str
    status: str
    progress_percent: float = 0.0
    clips: list[ClipOutput] = Field(default_factory=list)
    error_message: str | None = None
    speech_segments: list[SpeechSegmentOutput] = Field(default_factory=list)


class AudioProcessingStatusResponse(BaseModel):
    recording_id: str
    status: str
    progress_percent: float = 0.0
    total_segments: int = 0
    segments: list[SpeechSegmentOutput] = Field(default_factory=list)
    error_message: str | None = None


class ModelStatusResponse(BaseModel):
    vad_ready: bool = False
    stt_ready: bool = False
    stt_quantize: str = "unknown"
    vad_model_path: str | None = None
    stt_model_dir: str | None = None
    stt_loaded: bool = False


class ProcessVideoAcceptedResponse(BaseModel):
    recording_id: str
    status: str = "accepted"
    message: str = "Processing started"


class TrimClipRequest(BaseModel):
    source_path: str
    start_sec: float
    end_sec: float
    account_id: int
    recording_id: int


class TrimClipResponse(BaseModel):
    file_path: str
    thumbnail_path: str
    duration_sec: float


class FetchProductRequest(BaseModel):
    url: str
    cookies_json: str | None = None
    download_media: bool = True


class FetchedProductMediaFile(BaseModel):
    kind: Literal["image", "video"]
    path: str
    source_url: str


class FetchedProductData(BaseModel):
    name: str | None = None
    description: str | None = None
    price: float | None = None
    image_url: str | None = None
    category: str | None = None
    tiktok_shop_id: str | None = None
    image_urls: list[str] = Field(default_factory=list)
    video_urls: list[str] = Field(default_factory=list)
    media_files: list[FetchedProductMediaFile] = Field(default_factory=list)


class FetchProductResponse(BaseModel):
    success: bool
    incomplete: bool = False
    data: FetchedProductData | None = None
    error: str | None = None


class StorageStatsResponse(BaseModel):
    recordings_bytes: int = 0
    recordings_count: int = 0
    clips_bytes: int = 0
    clips_count: int = 0
    products_bytes: int = 0
    total_bytes: int = 0
    quota_bytes: int | None = None
    usage_percent: float = 0.0


class CleanupRunRequest(BaseModel):
    """Optional overrides for a single cleanup run. Omitted fields use process settings."""

    raw_retention_days: int | None = Field(default=None, ge=0)
    archive_retention_days: int | None = Field(default=None, ge=0)


class CleanupRunResponse(BaseModel):
    deleted_recordings: int = 0
    deleted_clips: int = 0
    freed_bytes: int = 0


class ProductEmbeddingMediaItem(BaseModel):
    kind: Literal["image", "video"]
    path: str
    source_url: str = ""


class IndexProductEmbeddingsRequest(BaseModel):
    product_id: int = Field(ge=1)
    product_name: str = ""
    product_description: str = ""
    items: list[ProductEmbeddingMediaItem] = Field(default_factory=list)


class IndexProductEmbeddingsResponse(BaseModel):
    indexed: int = 0
    skipped: int = 0
    errors: list[str] = Field(default_factory=list)
    message: str | None = None


class DeleteProductEmbeddingsRequest(BaseModel):
    product_id: int = Field(ge=1)


class DeleteProductEmbeddingsResponse(BaseModel):
    ok: bool = True


class ProductEmbeddingSearchRequest(BaseModel):
    query: str = ""
    top_k: int = Field(default=10, ge=1, le=100)

    @field_validator("query")
    @classmethod
    def strip_query(cls, v: str) -> str:
        return v.strip()


class ProductEmbeddingSearchByMediaRequest(BaseModel):
    path: str
    kind: Literal["image", "video"] = "image"
    top_k: int = Field(default=10, ge=1, le=100)
    companion_text: str | None = Field(
        default=None,
        description="Optional focus prompt with image/video for Gemini multimodal embed.",
    )


class ProductEmbeddingSearchHit(BaseModel):
    product_id: int
    score: float
    image_path: str
    source_url: str | None = None
    product_name: str | None = None
    modality: str | None = None
    product_text: str | None = None
    product_description: str | None = None


class ProductEmbeddingSearchResponse(BaseModel):
    hits: list[ProductEmbeddingSearchHit] = Field(default_factory=list)


class ProductEmbeddingIndexedProductRow(BaseModel):
    product_id: int = Field(ge=0)
    image_doc_count: int = Field(default=0, ge=0)
    video_doc_count: int = Field(default=0, ge=0)
    text_doc_count: int = Field(default=0, ge=0)
    product_name: str | None = None


class ProductEmbeddingsIndexedSummaryResponse(BaseModel):
    product_vector_enabled: bool = True
    store_ready: bool = False
    vector_store_relative: str = "vector/product_media"
    total_documents_scanned: int = 0
    scan_truncated: bool = False
    product_count: int = 0
    products: list[ProductEmbeddingIndexedProductRow] = Field(default_factory=list)
    message: str | None = None


class ClipSuggestProductRequest(BaseModel):
    video_path: str
    thumbnail_path: str | None = None
    transcript_text: str | None = None


class ClipSuggestTextHit(BaseModel):
    product_id: int
    score: float
    product_name: str | None = None
    product_description: str | None = None


class ClipSuggestVoteRow(BaseModel):
    product_id: int
    vote_count: int


class ClipSuggestImageEvidenceHit(BaseModel):
    """One zvec hit for a single query frame: pairs with parent row ``media_relative_path``."""

    product_id: int
    score: float
    product_name: str | None = None
    product_description: str | None = None
    catalog_media_relative_path: str | None = Field(
        default=None,
        description="Storage-relative path to the indexed product image/video that matched.",
    )
    catalog_source_url: str | None = Field(
        default=None,
        description="Original source URL of the catalog media, if indexed with one.",
    )
    catalog_modality: Literal["image", "video"] | None = Field(
        default=None,
        description="Whether the matched catalog vector came from an image or video file.",
    )


class ClipSuggestTranscriptSegmentRow(BaseModel):
    """One transcript slice and its best text-index match (evidence)."""

    segment_index: int
    segment_text: str
    outcome: Literal["hit", "no_hit", "error"] = "hit"
    error: str | None = None
    best_product_id: int | None = None
    best_score: float | None = None
    best_product_name: str | None = None
    matched_product_description: str | None = None


class ClipSuggestProductRankRow(BaseModel):
    """Product ranking row with absolute scores and weighted final_score."""

    product_id: int
    product_name: str | None = None
    frame_hit_count: int = Field(
        default=0,
        description="Number of frames (top-k) where this product appeared.",
    )
    avg_frame_distance: float | None = Field(
        default=None,
        description="Mean best-per-frame zvec cosine distance (lower = closer match).",
    )
    image_score: float = Field(
        default=0.0,
        description="1 - avg_frame_distance; 0 when no frame hits. Absolute [0,1].",
    )
    transcript_text_score: float | None = Field(
        default=None,
        description="Raw score from full-transcript text search (higher = better).",
    )
    text_score: float = Field(
        default=0.0,
        description="Same as transcript_text_score, or 0 when absent. Absolute [0,1].",
    )
    final_score: float = Field(
        default=0.0,
        description="w_img * image_score + w_txt * text_score. Ranked descending.",
    )


class ClipSuggestFrameRow(BaseModel):
    """One thumbnail or extracted frame and its best zvec hit (if any)."""

    index: int
    source: Literal["thumbnail", "extracted"]
    media_relative_path: str
    outcome: Literal["hit", "no_hit", "error"]
    error: str | None = None
    top_product_id: int | None = None
    top_score: float | None = None
    top_product_name: str | None = None
    matched_product_description: str | None = Field(
        default=None,
        description="Indexed product description on the top image hit (after re-index with media).",
    )
    image_evidence_hits: list[ClipSuggestImageEvidenceHit] = Field(
        default_factory=list,
        description=(
            "Top zvec hits for this frame: query image is ``media_relative_path``; "
            "each hit includes the matched catalog media path."
        ),
    )


class ClipSuggestProductResponse(BaseModel):
    matched: bool = False
    product_id: int | None = None
    product_name: str | None = None
    best_score: float | None = None
    frames_used: int = 0
    skipped_reason: str | None = None
    video_relative_path: str | None = None
    thumbnail_used: bool = False
    extracted_frame_count: int = 0
    frames_searched: int = 0
    config_target_extracted_frames: int = 0
    config_max_score_threshold: float = 0.0
    suggest_weight_image: float = 0.6
    suggest_weight_text: float = 0.4
    suggest_min_fused_score: float = 0.25
    suggest_image_embed_focus_prompt: str = ""
    pick_method: (
        Literal[
            "majority_vote",
            "min_distance_tiebreak",
            "weighted_fusion",
            "unified_score",
        ]
        | None
    ) = None
    votes_by_product: list[ClipSuggestVoteRow] = Field(
        default_factory=list,
        description="Legacy: vote count = number of frames where this product was top-1. "
        "Prefer product_ranks (mean frame score + transcript score).",
    )
    product_ranks: list[ClipSuggestProductRankRow] = Field(
        default_factory=list,
        description="Products ranked by final_score descending (absolute scoring system).",
    )
    transcript_segment_evidence: list[ClipSuggestTranscriptSegmentRow] = Field(
        default_factory=list,
        description="Transcript split into segments; each mapped to best text-index product + "
        "matched description.",
    )
    candidate_product_id: int | None = None
    candidate_product_name: str | None = None
    candidate_score: float | None = Field(
        default=None,
        description="Set when a winner was chosen but rejected (score above threshold).",
    )
    frame_rows: list[ClipSuggestFrameRow] = Field(default_factory=list)
    text_search_hits: list[ClipSuggestTextHit] = Field(default_factory=list)
    text_search_used: bool = False
    fusion_method: str | None = None
    debug_extracted_frames_dir: str | None = Field(
        default=None,
        description="When debug_keep_suggest_clip_frames is on: storage-relative path to the "
        "folder containing extracted frame_*.jpg (and README.txt).",
    )
