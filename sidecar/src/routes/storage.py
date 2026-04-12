import logging
from pathlib import Path

from fastapi import APIRouter

from config import settings
from core.cleanup import cleanup_worker
from models.schemas import CleanupRunResponse, StorageStatsResponse

logger = logging.getLogger(__name__)
router = APIRouter()


def _dir_size(path: Path) -> tuple[int, int]:
    """Return (total_bytes, file_count) for a directory tree."""
    total = 0
    count = 0
    if not path.is_dir():
        return 0, 0
    for f in path.rglob("*"):
        if f.is_file():
            try:
                total += f.stat().st_size
                count += 1
            except OSError:
                pass
    return total, count


@router.get("/api/storage/stats", response_model=StorageStatsResponse)
async def storage_stats():
    root = settings.storage_path

    rec_bytes, rec_count = _dir_size(root / "recordings")
    clip_bytes, clip_count = _dir_size(root / "clips")
    prod_bytes, _ = _dir_size(root / "products")

    total = rec_bytes + clip_bytes + prod_bytes
    quota = int(settings.storage_quota_gb * 1_073_741_824) if settings.storage_quota_gb else None
    usage_pct = (total / quota * 100) if quota and quota > 0 else 0.0

    return StorageStatsResponse(
        recordings_bytes=rec_bytes,
        recordings_count=rec_count,
        clips_bytes=clip_bytes,
        clips_count=clip_count,
        products_bytes=prod_bytes,
        total_bytes=total,
        quota_bytes=quota,
        usage_percent=round(usage_pct, 1),
    )


@router.post("/api/storage/cleanup-run", response_model=CleanupRunResponse)
async def run_cleanup_now():
    """Trigger one cleanup cycle (same logic as the background worker)."""
    summary = await cleanup_worker.run_once()
    return CleanupRunResponse(**summary)
