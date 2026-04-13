"""Preload onnxruntime shared library so sherpa-onnx resolves @rpath on macOS."""

from __future__ import annotations

import ctypes
import sys
from pathlib import Path


def preload_onnxruntime_shared_lib() -> None:
    try:
        import onnxruntime as ort
    except ImportError:
        return
    capi = Path(ort.__file__).resolve().parent / "capi"
    if not capi.is_dir():
        return
    if sys.platform != "darwin":
        return
    candidates = sorted(capi.glob("libonnxruntime.*.dylib"))
    if not candidates:
        candidates = list(capi.glob("libonnxruntime.dylib"))
    for p in candidates:
        try:
            ctypes.CDLL(str(p), mode=ctypes.RTLD_GLOBAL)
            return
        except OSError:
            continue
