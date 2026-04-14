"""Recording manager: worker pool, WebSocket events, lifecycle."""

from __future__ import annotations

import asyncio
import logging
import uuid

from config import settings
from core.worker import RecordingWorker
from ws.manager import ws_manager

logger = logging.getLogger("tikclip.recorder")


class RecordingManager:
    def __init__(self) -> None:
        self._workers: dict[str, RecordingWorker] = {}
        self._account_ids: dict[str, int] = {}
        self._lock = asyncio.Lock()

    @property
    def active_count(self) -> int:
        return sum(1 for w in self._workers.values() if w.status in ("pending", "recording"))

    async def has_active_recording_for_account(self, account_id: int) -> bool:
        """True while a worker for this account is pending or actively recording."""
        async with self._lock:
            return any(
                self._account_ids.get(rid) == account_id and w.status in ("pending", "recording")
                for rid, w in self._workers.items()
            )

    def _effective_max_duration(self, max_duration_seconds: int | None) -> int:
        if max_duration_seconds is not None:
            return max_duration_seconds
        return settings.max_duration_minutes * 60

    async def start_recording(
        self,
        account_id: int,
        username: str,
        stream_url: str,
        max_duration_seconds: int | None = None,
    ) -> str:
        async with self._lock:
            active = sum(1 for w in self._workers.values() if w.status in ("pending", "recording"))
            if active >= settings.max_concurrent_recordings:
                raise RuntimeError("Maximum concurrent recordings reached")
            recording_id = str(uuid.uuid4())
            worker = RecordingWorker(
                recording_id=recording_id,
                stream_url=stream_url,
                output_dir=settings.storage_path,
                username=username,
                max_duration_seconds=self._effective_max_duration(max_duration_seconds),
            )
            self._workers[recording_id] = worker
            self._account_ids[recording_id] = account_id

        asyncio.create_task(self._run_worker(recording_id, worker, account_id))
        return recording_id

    async def _broadcast_progress(
        self, recording_id: str, account_id: int, worker: RecordingWorker
    ) -> None:
        while worker.status in ("pending", "recording"):
            await asyncio.sleep(5)
            await ws_manager.broadcast(
                "recording_progress",
                {
                    "recording_id": recording_id,
                    "account_id": account_id,
                    **worker.to_dict(),
                },
            )

    async def _run_worker(
        self, recording_id: str, worker: RecordingWorker, account_id: int
    ) -> None:
        await ws_manager.broadcast(
            "recording_started",
            {
                "recording_id": recording_id,
                "account_id": account_id,
                **worker.to_dict(),
            },
        )
        progress_task = asyncio.create_task(
            self._broadcast_progress(recording_id, account_id, worker)
        )
        try:
            await worker.start()
        finally:
            progress_task.cancel()
            try:
                await progress_task
            except asyncio.CancelledError:
                pass
        await ws_manager.broadcast(
            "recording_finished",
            {
                "recording_id": recording_id,
                "account_id": account_id,
                **worker.to_dict(),
            },
        )
        if worker.status == "completed" and settings.auto_process_after_record and worker.file_path:
            try:
                from routes.clips import try_schedule_video_processing

                err = await try_schedule_video_processing(
                    recording_id=recording_id,
                    username=worker.username,
                    file_path=worker.file_path,
                    account_id=account_id,
                )
                if err:
                    logger.warning(
                        "auto clip processing skipped recording_id=%s reason=%s",
                        recording_id,
                        err,
                    )
            except Exception:
                logger.exception(
                    "auto clip processing failed recording_id=%s",
                    recording_id,
                )
        # Hit max duration (-t) while the host may still be live: poll once and start next segment.
        if worker.status == "completed":
            try:
                from core.watcher import account_watcher

                await account_watcher.on_autorecord_segment_end(account_id)
            except Exception:
                logger.exception(
                    "autorecord segment chain failed for account_id=%s",
                    account_id,
                )

    async def stop_recording(self, recording_id: str) -> bool:
        async with self._lock:
            worker = self._workers.get(recording_id)
        if worker is None:
            return False
        await worker.stop()
        return True

    def get_status(self, recording_id: str) -> dict | None:
        worker = self._workers.get(recording_id)
        if worker is None:
            return None
        account_id = self._account_ids.get(recording_id, 0)
        return {"account_id": account_id, **worker.to_dict()}

    def get_all_status(self) -> list[dict]:
        return [
            {"account_id": self._account_ids.get(rid, 0), **w.to_dict()}
            for rid, w in self._workers.items()
        ]


recording_manager = RecordingManager()
