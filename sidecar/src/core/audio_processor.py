"""Extract audio from recordings, VAD (Silero), STT (gipformer via sherpa-onnx)."""

from __future__ import annotations

import asyncio
import logging
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import soundfile as sf

from config import settings
from core.model_manager import ModelManager
from ws.manager import ws_manager

logger = logging.getLogger(__name__)

SAMPLE_RATE = 16000


@dataclass
class SpeechSpan:
    """One speech interval with transcript (seconds, source timeline)."""

    start_sec: float
    end_sec: float
    text: str
    confidence: float | None = None


@dataclass
class AudioProcessor:
    recording_id: str
    username: str
    source_path: Path
    account_id: int
    status: str = "pending"
    progress_percent: float = 0.0
    segments: list[SpeechSpan] = field(default_factory=list)
    error_message: str | None = None

    def _base_payload(self) -> dict:
        return {
            "recording_id": self.recording_id,
            "account_id": self.account_id,
            "username": self.username,
        }

    def _extract_audio_sync(self, src: Path) -> Path:
        tmp = Path(
            tempfile.mkstemp(suffix=".wav", prefix="tikclip_audio_")[1],
        )
        cmd = [
            "ffmpeg",
            "-y",
            "-i",
            str(src),
            "-vn",
            "-ac",
            "1",
            "-ar",
            str(SAMPLE_RATE),
            "-f",
            "wav",
            str(tmp),
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=3600)
        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "").strip()
            raise RuntimeError(f"ffmpeg audio extract failed ({proc.returncode}): {err[:2000]}")
        return tmp

    def _merge_intervals_samples(
        self,
        intervals: list[tuple[int, int]],
        gap_samples: int,
    ) -> list[tuple[int, int]]:
        if not intervals:
            return []
        intervals = sorted(intervals, key=lambda x: x[0])
        merged: list[list[int]] = [list(intervals[0])]
        for s, e in intervals[1:]:
            if s - merged[-1][1] <= gap_samples:
                merged[-1][1] = max(merged[-1][1], e)
            else:
                merged.append([s, e])
        return [(a, b) for a, b in merged]

    def _vad_intervals_sync(self, samples: np.ndarray) -> list[tuple[int, int]]:
        import onnx_runtime_preload

        onnx_runtime_preload.preload_onnxruntime_shared_lib()

        samples = np.ascontiguousarray(samples, dtype=np.float32)
        mgr = ModelManager.get()
        vad = mgr.new_vad()
        window_size = int(vad.config.silero_vad.window_size)

        raw: list[tuple[int, int]] = []
        tail = samples

        def drain_segments() -> None:
            while not vad.empty():
                seg = vad.front
                start = int(seg.start)
                seg_samples = np.asarray(seg.samples, dtype=np.float32)
                end = start + len(seg_samples)
                raw.append((start, end))
                vad.pop()

        while len(tail) > window_size:
            vad.accept_waveform(tail[:window_size])
            tail = tail[window_size:]
            drain_segments()
        if len(tail) > 0:
            vad.accept_waveform(tail)
            drain_segments()
        vad.flush()
        drain_segments()

        gap_samples = int(settings.speech_merge_gap_sec * SAMPLE_RATE)
        return self._merge_intervals_samples(raw, gap_samples)

    def _transcribe_intervals_sync(
        self,
        samples: np.ndarray,
        intervals: list[tuple[int, int]],
    ) -> list[SpeechSpan]:
        import onnx_runtime_preload

        onnx_runtime_preload.preload_onnxruntime_shared_lib()

        recognizer = ModelManager.get().get_recognizer()
        out: list[SpeechSpan] = []
        for s, e in intervals:
            s = max(0, min(s, len(samples)))
            e = max(s, min(e, len(samples)))
            chunk = np.ascontiguousarray(samples[s:e], dtype=np.float32)
            if chunk.size == 0:
                continue
            stream = recognizer.create_stream()
            stream.accept_waveform(SAMPLE_RATE, chunk)
            recognizer.decode_streams([stream])
            text = stream.result.text.strip()
            out.append(
                SpeechSpan(
                    start_sec=s / SAMPLE_RATE,
                    end_sec=e / SAMPLE_RATE,
                    text=text,
                    confidence=None,
                )
            )
        return out

    async def process(self) -> list[SpeechSpan]:
        if not settings.audio_processing_enabled:
            return []

        self.status = "processing"
        self.segments.clear()
        self.error_message = None
        self.progress_percent = 0.0

        await ws_manager.broadcast(
            "audio_processing_started",
            {**self._base_payload(), "phase": "audio"},
        )

        wav_path: Path | None = None
        try:
            wav_path = await asyncio.to_thread(self._extract_audio_sync, self.source_path)
            samples, sr = await asyncio.to_thread(
                lambda: sf.read(str(wav_path), dtype="float32", always_2d=True),
            )
            samples = np.ascontiguousarray(samples[:, 0], dtype=np.float32)
            if int(sr) != SAMPLE_RATE:
                raise RuntimeError(
                    f"Expected ffmpeg output at {SAMPLE_RATE} Hz, got {sr}. "
                    "Check ffmpeg / audio track."
                )

            intervals = await asyncio.to_thread(self._vad_intervals_sync, samples)
            if not intervals:
                self.progress_percent = 100.0
                self.segments = []
                self.status = "completed"
                await ws_manager.broadcast(
                    "audio_processing_complete",
                    {**self._base_payload(), "total_segments": 0, "status": "no_speech"},
                )
                return []

            spans = await asyncio.to_thread(self._transcribe_intervals_sync, samples, intervals)
            n = len(spans)
            for i, sp in enumerate(spans):
                pct = 100.0 * (i + 1) / max(1, n)
                self.progress_percent = pct
                await ws_manager.broadcast(
                    "audio_processing_progress",
                    {
                        **self._base_payload(),
                        "progress_percent": pct,
                        "current_segment": i + 1,
                        "total_segments": n,
                    },
                )
                await ws_manager.broadcast(
                    "speech_segment_ready",
                    {
                        **self._base_payload(),
                        "start_sec": sp.start_sec,
                        "end_sec": sp.end_sec,
                        "text": sp.text,
                    },
                )

            self.segments = spans
            self.progress_percent = 100.0
            self.status = "completed"
            await ws_manager.broadcast(
                "audio_processing_complete",
                {
                    **self._base_payload(),
                    "total_segments": len(spans),
                    "status": "ok",
                },
            )
            return spans
        except Exception as e:
            logger.exception("Audio processing failed for %s", self.recording_id)
            self.error_message = str(e)
            self.status = "error"
            await ws_manager.broadcast(
                "audio_processing_complete",
                {
                    **self._base_payload(),
                    "total_segments": 0,
                    "status": "error",
                    "error_message": str(e),
                },
            )
            return []
        finally:
            if wav_path is not None and wav_path.is_file():
                try:
                    wav_path.unlink()
                except OSError:
                    pass
