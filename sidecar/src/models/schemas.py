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


class ProcessingStatusResponse(BaseModel):
    recording_id: str
    account_id: int
    username: str
    status: str
    progress_percent: float = 0.0
    clips: list[ClipOutput] = Field(default_factory=list)
    error_message: str | None = None


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


class ProductEmbeddingSearchHit(BaseModel):
    product_id: int
    score: float
    image_path: str
    source_url: str | None = None
    product_name: str | None = None
    modality: str | None = None


class ProductEmbeddingSearchResponse(BaseModel):
    hits: list[ProductEmbeddingSearchHit] = Field(default_factory=list)


class ClipSuggestProductRequest(BaseModel):
    video_path: str
    thumbnail_path: str | None = None


class ClipSuggestProductResponse(BaseModel):
    product_id: int | None = None
    product_name: str | None = None
    best_score: float | None = None
    frames_used: int = 0
    skipped_reason: str | None = None
