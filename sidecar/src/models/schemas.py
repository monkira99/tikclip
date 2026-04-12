from pydantic import BaseModel, Field


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
