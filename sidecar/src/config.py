from pathlib import Path

from pydantic import Field
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
    debug_tiktok: bool = Field(
        default=False,
        description=(
            "If true, log a truncated HTML snippet when the live page has no room_id "
            "(stderr only; does not write files)."
        ),
    )
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

    model_config = SettingsConfigDict(
        env_prefix="TIKCLIP_",
        # Loaded on sidecar start (incl. Tauri-spawned python); no uv --env-file needed.
        env_file=_SIDECAR_ROOT / ".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )


settings = Settings()
