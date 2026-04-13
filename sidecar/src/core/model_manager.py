"""Download and lazy-load sherpa-onnx VAD + gipformer STT models (ONNX)."""

from __future__ import annotations

import asyncio
import logging
import threading
from pathlib import Path
from typing import Any

from config import settings
from ws.manager import ws_manager

logger = logging.getLogger(__name__)

GIPFORMER_REPO = "g-group-ai-lab/gipformer-65M-rnnt"
GIPFORMER_FILES_FP32 = {
    "encoder": "encoder-epoch-35-avg-6.onnx",
    "decoder": "decoder-epoch-35-avg-6.onnx",
    "joiner": "joiner-epoch-35-avg-6.onnx",
    "tokens": "tokens.txt",
}
GIPFORMER_FILES_INT8 = {
    "encoder": "encoder-epoch-35-avg-6.int8.onnx",
    "decoder": "decoder-epoch-35-avg-6.int8.onnx",
    "joiner": "joiner-epoch-35-avg-6.int8.onnx",
    "tokens": "tokens.txt",
}

# Sherpa-onnx release asset (same URL as upstream examples).
SILERO_VAD_FILENAME = "silero_vad.onnx"
SILERO_VAD_URL = (
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx"
)


def _detect_quantize() -> str:
    q = (settings.stt_quantize or "auto").strip().lower()
    if q in ("fp32", "float32"):
        return "fp32"
    if q in ("int8",):
        return "int8"
    if q != "auto":
        logger.warning("Unknown TIKCLIP_STT_QUANTIZE=%r, using auto", settings.stt_quantize)
    try:
        import onnxruntime as ort

        providers = ort.get_available_providers()
        if "CUDAExecutionProvider" in providers:
            return "fp32"
    except Exception:
        logger.debug("Could not probe onnxruntime providers", exc_info=True)
    return "int8"


def _download_silero_vad(dest_dir: Path) -> Path:
    dest_dir.mkdir(parents=True, exist_ok=True)
    out = dest_dir / SILERO_VAD_FILENAME
    if out.is_file() and out.stat().st_size > 0:
        return out

    import httpx

    total: int | None = None

    def _report(downloaded: int) -> None:
        payload: dict[str, Any] = {
            "model_name": "silero_vad",
            "progress_percent": 0.0,
            "downloaded_bytes": downloaded,
            "total_bytes": total,
        }
        if total and total > 0:
            payload["progress_percent"] = min(100.0, 100.0 * downloaded / total)
        try:
            loop = asyncio.get_running_loop()
            loop.create_task(ws_manager.broadcast("model_download_progress", payload))
        except RuntimeError:
            pass

    with (
        httpx.Client(follow_redirects=True, timeout=120.0) as client,
        client.stream("GET", SILERO_VAD_URL) as r,
    ):
        r.raise_for_status()
        cl = r.headers.get("content-length")
        if cl and cl.isdigit():
            total = int(cl)
        tmp = out.with_suffix(".tmp")
        n = 0
        with open(tmp, "wb") as f:
            for chunk in r.iter_bytes():
                f.write(chunk)
                n += len(chunk)
                if total and n % (256 * 1024) < len(chunk):
                    _report(n)
        tmp.replace(out)
    _report(out.stat().st_size)
    return out


def _download_gipformer(dest_dir: Path, quantize: str) -> dict[str, Path]:
    from huggingface_hub import hf_hub_download

    dest_dir.mkdir(parents=True, exist_ok=True)
    files = GIPFORMER_FILES_FP32 if quantize == "fp32" else GIPFORMER_FILES_INT8
    paths: dict[str, Path] = {}
    n_files = len(files)
    for i, (key, filename) in enumerate(files.items(), start=1):
        path_str = hf_hub_download(
            repo_id=GIPFORMER_REPO,
            filename=filename,
            local_dir=str(dest_dir),
        )
        paths[key] = Path(path_str)
        pct = 100.0 * i / n_files
        payload = {
            "model_name": f"gipformer_{quantize}",
            "progress_percent": pct,
            "downloaded_bytes": None,
            "total_bytes": None,
        }
        try:
            loop = asyncio.get_running_loop()
            loop.create_task(ws_manager.broadcast("model_download_progress", payload))
        except RuntimeError:
            pass
    return paths


class ModelManager:
    """Process-wide model paths + lazy singleton for offline recognizer only."""

    _instance: ModelManager | None = None
    _instance_lock = threading.Lock()

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._recognizer: Any = None
        self._quantize: str | None = None
        self._vad_path: Path | None = None
        self._stt_paths: dict[str, Path] | None = None

    @classmethod
    def get(cls) -> ModelManager:
        with cls._instance_lock:
            if cls._instance is None:
                cls._instance = ModelManager()
            return cls._instance

    def quantize(self) -> str:
        if self._quantize is None:
            self._quantize = _detect_quantize()
        return self._quantize

    def ensure_vad_model(self) -> Path:
        root = settings.models_path / "silero_vad"
        return _download_silero_vad(root)

    def ensure_stt_models(self) -> dict[str, Path]:
        q = self.quantize()
        root = settings.models_path / "gipformer" / q
        return _download_gipformer(root, q)

    def new_vad(self) -> Any:
        """Fresh VoiceActivityDetector per recording (not shared across files)."""
        import onnx_runtime_preload

        onnx_runtime_preload.preload_onnxruntime_shared_lib()
        import sherpa_onnx

        model = self.ensure_vad_model()
        self._vad_path = model
        config = sherpa_onnx.VadModelConfig()
        config.sample_rate = 16000
        config.silero_vad.model = str(model)
        config.silero_vad.threshold = 0.5
        config.silero_vad.min_silence_duration = 0.25
        config.silero_vad.min_speech_duration = 0.25
        config.silero_vad.max_speech_duration = 30.0
        return sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=120)

    def get_recognizer(self) -> Any:
        import onnx_runtime_preload

        onnx_runtime_preload.preload_onnxruntime_shared_lib()
        import sherpa_onnx

        with self._lock:
            if self._recognizer is not None:
                return self._recognizer
            paths = self.ensure_stt_models()
            self._stt_paths = paths
            self._recognizer = sherpa_onnx.OfflineRecognizer.from_transducer(
                encoder=str(paths["encoder"]),
                decoder=str(paths["decoder"]),
                joiner=str(paths["joiner"]),
                tokens=str(paths["tokens"]),
                num_threads=settings.stt_num_threads,
                sample_rate=16000,
                feature_dim=80,
                decoding_method="modified_beam_search",
            )
            return self._recognizer

    def status(self) -> dict[str, Any]:
        q = self.quantize()
        vad_path = settings.models_path / "silero_vad" / SILERO_VAD_FILENAME
        vad_ready = vad_path.is_file() and vad_path.stat().st_size > 0
        stt_root = settings.models_path / "gipformer" / q
        files = GIPFORMER_FILES_FP32 if q == "fp32" else GIPFORMER_FILES_INT8
        stt_ready = all((stt_root / fn).is_file() for fn in files.values())
        return {
            "vad_ready": vad_ready,
            "stt_ready": stt_ready,
            "stt_quantize": q,
            "vad_model_path": str(vad_path) if vad_ready else None,
            "stt_model_dir": str(stt_root),
            "stt_loaded": self._recognizer is not None,
        }
