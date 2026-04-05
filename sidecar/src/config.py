from pathlib import Path

from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18321
    port_fallback_range_start: int = 18322
    port_fallback_range_end: int = 18330
    storage_path: Path = Path.home() / "TikTokApp"
    log_level: str = "info"
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

    model_config = {"env_prefix": "TIKCLIP_"}


settings = Settings()
