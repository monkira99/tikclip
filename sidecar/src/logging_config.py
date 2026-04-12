"""Configure tikclip.* loggers (stderr) without breaking uvicorn access logs."""

from __future__ import annotations

import errno
import logging
import sys
from typing import Any

from config import settings


def install_stderr_broken_pipe_guard() -> None:
    """When the parent closes stderr (e.g. Tauri restarting the sidecar), logging must not raise.

    Otherwise ``logging`` prints ``--- Logging error ---`` and a traceback on every emit/flush.
    """
    if getattr(sys.stderr, "_tikclip_stderr_guard", False):
        return

    real = sys.__stderr__

    class _SafeStderr:
        __slots__ = ("_real",)
        _tikclip_stderr_guard = True

        def __init__(self, stream: Any) -> None:
            self._real = stream

        def write(self, s: str) -> int:
            try:
                return self._real.write(s)
            except BrokenPipeError:
                return len(s)
            except OSError as exc:
                if exc.errno == errno.EPIPE:
                    return len(s)
                raise

        def flush(self) -> None:
            try:
                self._real.flush()
            except BrokenPipeError:
                pass
            except OSError as exc:
                if exc.errno != errno.EPIPE:
                    raise

        def __getattr__(self, name: str) -> Any:
            return getattr(self._real, name)

    sys.stderr = _SafeStderr(real)


def setup_sidecar_logging() -> None:
    install_stderr_broken_pipe_guard()
    level = getattr(logging, settings.log_level.upper(), logging.INFO)
    fmt = logging.Formatter(
        "%(asctime)s | %(levelname)-5s | %(name)s | %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(fmt)

    root_tikclip = logging.getLogger("tikclip")
    root_tikclip.setLevel(level)
    if not root_tikclip.handlers:
        root_tikclip.addHandler(handler)
    root_tikclip.propagate = False

    # Modules using logging.getLogger(__name__) live under these prefixes (not under tikclip.*).
    for prefix in ("routes", "tiktok", "embeddings", "core"):
        lg = logging.getLogger(prefix)
        lg.setLevel(level)
        if not lg.handlers:
            lg.addHandler(handler)
        lg.propagate = False
