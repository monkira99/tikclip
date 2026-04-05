from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict

# sidecar/src/config.py -> repo sidecar/ (where pyproject.toml and .env live)
_SIDECAR_ROOT = Path(__file__).resolve().parent.parent


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18321
    port_fallback_range_start: int = 18322
    port_fallback_range_end: int = 18330
    storage_path: Path = Path.home() / "TikTokApp"
    log_level: str = "info"
    # TIKCLIP_DEBUG_TIKTOK=1 — log short HTML snippet when room_id parse fails (no secrets).
    debug_tiktok: bool = False
    poll_interval_seconds: int = 30
    max_concurrent_recordings: int = 5
    max_duration_hours: int = 4
    max_file_size_gb: int = 4
    retry_attempts: int = 3
    clip_min_duration: int = 15
    clip_max_duration: int = 90
    scene_threshold: float = 30.0
    auto_process_after_record: bool = True
    auto_cleanup_raw: bool = True
    raw_retention_days: int = 7

    model_config = SettingsConfigDict(
        env_prefix="TIKCLIP_",
        # Loaded on sidecar start (incl. Tauri-spawned python); no uv --env-file needed.
        env_file=_SIDECAR_ROOT / ".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )


settings = Settings()
