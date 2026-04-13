from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict

# sidecar/src/config.py -> repo sidecar/ (where pyproject.toml and .env live)
_SIDECAR_ROOT = Path(__file__).resolve().parent.parent


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18321
    port_fallback_range_start: int = 18322
    # Wide range so another instance or many restarts can still bind.
    port_fallback_range_end: int = 18999
    # Desktop app uses ~/.tikclip by default; CLI / standalone may set TIKCLIP_STORAGE_PATH or .env.
    storage_path: Path = Path.home() / ".tikclip"
    log_level: str = "info"
    # TIKCLIP_DEBUG_TIKTOK=1 — save live-page HTML only on HTTP errors or suspected WAF/block HTML
    # under {TIKCLIP_STORAGE_PATH}/debug/tiktok_live_html/ (log path; no secrets in logs).
    debug_tiktok: bool = True
    # TikTok HTTP: curl_cffi = Chrome TLS impersonation (see tiktok-live-recorder); httpx = legacy.
    tiktok_http_backend: str = "curl_cffi"
    tiktok_curl_impersonate: str = "chrome131"
    # Opt-in third-party sign API (e.g. tikrec): sends unique_id off-device; then TikTok JSON.
    tiktok_room_sign_enabled: bool = True
    tiktok_room_sign_base_url: str = "https://tikrec.com"
    # TikTok HTTP request timeout (curl_cffi / httpx), seconds.
    tiktok_http_timeout_seconds: float = 45.0
    poll_interval_seconds: int = 30
    max_concurrent_recordings: int = 5
    # Max length of one live recording if API omits max_duration_seconds (Settings, minutes).
    max_duration_minutes: int = 5
    max_file_size_gb: int = 4
    # Set by desktop app from Settings → max storage (GB); enforcement TBD.
    storage_quota_gb: float | None = None
    retry_attempts: int = 3
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0
    auto_process_after_record: bool = True
    auto_cleanup_raw: bool = True
    raw_retention_days: int = 7
    # No clip DB here: default 0 skips age-based deletion under clips/.
    archive_retention_days: int = 0
    storage_warn_percent: int = 80
    storage_cleanup_percent: int = 95
    cleanup_interval_minutes: int = 30

    # Product media vector index (zvec + Gemini Embedding API). Driven by Tauri Settings → env.
    product_vector_enabled: bool = False
    gemini_api_key: str | None = None
    gemini_embedding_model: str = "gemini-embedding-2-preview"
    gemini_embedding_dimensions: int = 1536

    # After each new clip: extract frames → Gemini image embed → zvec; tag clip if match is strong.
    auto_tag_clip_product_enabled: bool = False
    auto_tag_clip_frame_count: int = 4
    auto_tag_clip_max_score: float = 0.35

    # Audio: VAD + STT (sherpa-onnx, gipformer ONNX). Models under models_path.
    audio_processing_enabled: bool = True
    speech_merge_gap_sec: float = 0.5
    speech_cut_tolerance_sec: float = 1.5
    stt_num_threads: int = 4
    # auto: fp32 when CUDA ExecutionProvider available, else int8.
    stt_quantize: str = "auto"
    models_path: Path = Path.home() / ".tikclip" / "models"

    model_config = SettingsConfigDict(
        env_prefix="TIKCLIP_",
        # Loaded on sidecar start (incl. Tauri-spawned python); no uv --env-file needed.
        env_file=_SIDECAR_ROOT / ".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )


settings = Settings()
