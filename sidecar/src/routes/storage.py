import logging
from pathlib import Path

from fastapi import APIRouter

from config import settings
from core.cleanup import cleanup_worker
from models.schemas import CleanupRunResponse, StorageStatsResponse

logger = logging.getLogger("tikclip.storage")
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


def _raw_recordings_usage(root: Path) -> tuple[int, int, int, int, int, int]:
    """Return (total_bytes, total_files, records_b, records_c, legacy_b, legacy_c)."""
    rb, rc = _dir_size(root / "records")
    lb, lc = _dir_size(root / "recordings")
    return rb + lb, rc + lc, rb, rc, lb, lc


@router.get("/api/storage/stats", response_model=StorageStatsResponse)
async def storage_stats():
    root = settings.storage_path.resolve()

    rec_bytes, rec_count, rec_dir_b, rec_dir_c, leg_b, leg_c = _raw_recordings_usage(root)
    clip_bytes, clip_count = _dir_size(root / "clips")
    prod_bytes, prod_count = _dir_size(root / "products")

    total = rec_bytes + clip_bytes + prod_bytes
    quota = int(settings.storage_quota_gb * 1_073_741_824) if settings.storage_quota_gb else None
    usage_pct = (total / quota * 100) if quota and quota > 0 else 0.0

    logger.debug(
        "GET /api/storage/stats root=%s | records/ bytes=%s files=%s | "
        "recordings/ bytes=%s files=%s | raw_total bytes=%s files=%s | "
        "clips/ bytes=%s files=%s | products/ bytes=%s files=%s | "
        "grand_total=%s | quota_gb=%s quota_bytes=%s usage_pct=%.2f",
        root,
        rec_dir_b,
        rec_dir_c,
        leg_b,
        leg_c,
        rec_bytes,
        rec_count,
        clip_bytes,
        clip_count,
        prod_bytes,
        prod_count,
        total,
        settings.storage_quota_gb,
        quota,
        usage_pct,
    )

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
    logger.debug(
        "POST /api/storage/cleanup-run root=%s raw_retention_days=%s archive_retention_days=%s",
        settings.storage_path.resolve(),
        settings.raw_retention_days,
        settings.archive_retention_days,
    )
    summary = await cleanup_worker.run_once()
    logger.debug(
        "cleanup-run done deleted_recordings=%s deleted_clips=%s freed_bytes=%s",
        summary.get("deleted_recordings"),
        summary.get("deleted_clips"),
        summary.get("freed_bytes"),
    )
    return CleanupRunResponse(**summary)
