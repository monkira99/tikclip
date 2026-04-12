"""Periodic storage cleanup: raw recordings by retention + quota warnings over WebSocket.

We delete old raw media under ``records/`` (current worker output) and legacy ``recordings/``.
``raw_retention_days`` is interpreted as **local calendar days** since the file's last
modified date (``st_mtime`` → system local timezone), not a rolling 24h wall-clock window.
Example: with retention ``1``, a file whose local modification **date** is yesterday or
earlier is removed when cleanup runs today (even if the file is under 24h old in seconds).

Automatic deletion of exported clips by age is not done here: the sidecar cannot
correlate files on disk with clip rows (draft/ready/archived) in the Tauri DB.
Use ``archive_retention_days > 0`` only as a future hook; until desktop-driven
purge exists, values > 0 are ignored (see ``_maybe_delete_archived_clips``).
"""

from __future__ import annotations

import asyncio
import logging
import os
from datetime import datetime
from pathlib import Path

from config import settings
from ws.manager import ws_manager

logger = logging.getLogger("tikclip.cleanup")


def _file_local_calendar_age_days(path: Path) -> int:
    """Whole local calendar days between file mtime's date and today (system local TZ)."""
    try:
        mtime = datetime.fromtimestamp(path.stat().st_mtime)
    except OSError:
        return 0
    now = datetime.now()
    return (now.date() - mtime.date()).days


def _dir_total_bytes(path: Path) -> int:
    total = 0
    if not path.is_dir():
        return 0
    for f in path.rglob("*"):
        if f.is_file():
            try:
                total += f.stat().st_size
            except OSError:
                pass
    return total


def _delete_old_under_dir(rec_dir: Path, retention_days: int) -> tuple[int, int]:
    """Delete old raw media files under one directory tree."""
    if retention_days <= 0 or not rec_dir.is_dir():
        logger.debug(
            "cleanup raw skip dir=%s retention_days=%s (not applicable or missing)",
            rec_dir,
            retention_days,
        )
        return 0, 0
    count = 0
    freed = 0
    for f in rec_dir.rglob("*"):
        if f.is_file() and f.suffix.lower() in (".flv", ".mp4", ".ts", ".mkv", ".m4a", ".aac"):
            cal_days = _file_local_calendar_age_days(f)
            if cal_days >= retention_days:
                try:
                    size = f.stat().st_size
                    f.unlink()
                    freed += size
                    count += 1
                except OSError as e:
                    logger.warning("Failed to delete recording %s: %s", f, e)
    logger.debug(
        "cleanup raw summary dir=%s deleted_files=%s freed_bytes=%s "
        "retention_local_calendar_days=%s",
        rec_dir,
        count,
        freed,
        retention_days,
    )
    return count, freed


def _delete_old_recordings(root: Path, retention_days: int) -> tuple[int, int]:
    """Delete old raw media under ``records/`` and legacy ``recordings/``."""
    c1, f1 = _delete_old_under_dir(root / "records", retention_days)
    c2, f2 = _delete_old_under_dir(root / "recordings", retention_days)
    return c1 + c2, f1 + f2


def _maybe_delete_archived_clips(_root: Path, retention_days: int) -> tuple[int, int]:
    """Reserved for desktop-coordinated archival purge (requires DB). No-op today."""
    if retention_days <= 0:
        return 0, 0
    logger.info(
        "archive_retention_days=%s ignored: clip retention purge needs Tauri/SQLite correlation; "
        "not deleting under clips/ from sidecar",
        retention_days,
    )
    return 0, 0


class StorageCleanupWorker:
    def __init__(self) -> None:
        self._task: asyncio.Task[None] | None = None
        self._running = False

    async def start(self) -> None:
        if os.environ.get("PYTEST_CURRENT_TEST"):
            logger.debug("StorageCleanupWorker not started under pytest")
            return
        if self._running:
            return
        self._running = True
        self._task = asyncio.create_task(self._loop())
        logger.info(
            "StorageCleanupWorker started (interval=%dm)",
            settings.cleanup_interval_minutes,
        )

    async def stop(self) -> None:
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
            self._task = None
        logger.info("StorageCleanupWorker stopped")

    async def run_once(
        self,
        *,
        raw_retention_days: int | None = None,
        archive_retention_days: int | None = None,
    ) -> dict:
        """Run cleanup cycle once. Returns summary dict.

        Per-run overrides (e.g. from POST /api/storage/cleanup-run) take precedence over
        loaded settings for raw/archive retention only; quota broadcast still uses settings.
        """
        root = settings.storage_path.resolve()
        total_deleted_rec = 0
        total_deleted_clips = 0
        total_freed = 0

        raw_days = (
            raw_retention_days if raw_retention_days is not None else settings.raw_retention_days
        )
        arch_days = (
            archive_retention_days
            if archive_retention_days is not None
            else settings.archive_retention_days
        )

        logger.debug(
            "cleanup cycle start root=%s raw_retention_days=%s archive_retention_days=%s "
            "storage_quota_gb=%s warn_pct=%s cleanup_pct=%s",
            root,
            raw_days,
            arch_days,
            settings.storage_quota_gb,
            settings.storage_warn_percent,
            settings.storage_cleanup_percent,
        )

        rec_count, rec_freed = await asyncio.to_thread(_delete_old_recordings, root, raw_days)
        total_deleted_rec += rec_count
        total_freed += rec_freed
        logger.debug(
            "cleanup after raw retention deleted_recordings=%s freed_bytes=%s",
            rec_count,
            rec_freed,
        )

        clip_count, clip_freed = await asyncio.to_thread(
            _maybe_delete_archived_clips, root, arch_days
        )
        total_deleted_clips += clip_count
        total_freed += clip_freed

        if settings.storage_quota_gb and settings.storage_quota_gb > 0:
            quota_bytes = int(settings.storage_quota_gb * 1_073_741_824)
            current = await asyncio.to_thread(_dir_total_bytes, root)
            usage_pct = current / quota_bytes * 100 if quota_bytes > 0 else 0.0

            logger.debug(
                "cleanup quota check total_under_root=%s quota_bytes=%s usage_pct=%.2f "
                "warn_threshold=%s cleanup_threshold=%s",
                current,
                quota_bytes,
                usage_pct,
                settings.storage_warn_percent,
                settings.storage_cleanup_percent,
            )

            if usage_pct >= settings.storage_warn_percent:
                critical = usage_pct >= settings.storage_cleanup_percent
                logger.debug(
                    "cleanup broadcasting storage_warning usage_pct=%s critical=%s",
                    round(usage_pct, 1),
                    critical,
                )
                await ws_manager.broadcast(
                    "storage_warning",
                    {
                        "usage_percent": round(usage_pct, 1),
                        "quota_bytes": quota_bytes,
                        "total_bytes": current,
                        "critical": critical,
                    },
                )

        summary = {
            "deleted_recordings": total_deleted_rec,
            "deleted_clips": total_deleted_clips,
            "freed_bytes": total_freed,
        }

        logger.debug(
            "cleanup cycle end deleted_recordings=%s deleted_clips=%s freed_bytes=%s",
            total_deleted_rec,
            total_deleted_clips,
            total_freed,
        )

        if total_freed > 0:
            await ws_manager.broadcast("cleanup_completed", summary)

        return summary

    async def _loop(self) -> None:
        while self._running:
            try:
                await self.run_once()
            except Exception:
                logger.exception("Cleanup cycle failed")
            await asyncio.sleep(max(60, settings.cleanup_interval_minutes * 60))


cleanup_worker = StorageCleanupWorker()
