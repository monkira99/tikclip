"""Configure tikclip.* loggers (stderr) without breaking uvicorn access logs."""

from __future__ import annotations

import errno
import logging
import sys
from typing import Any

from config import settings


class _SafeTextIO:
    """Swallow EPIPE / BrokenPipeError when the parent closes stdout/stderr (e.g. Tauri sidecar)."""

    __slots__ = ("_real",)

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


def install_stdio_broken_pipe_guards() -> None:
    """Wrap stdout and stderr; uvicorn access logs use stdout, app loggers use stderr."""
    if not isinstance(sys.stderr, _SafeTextIO):
        sys.stderr = _SafeTextIO(sys.__stderr__)
    if not isinstance(sys.stdout, _SafeTextIO):
        sys.stdout = _SafeTextIO(sys.__stdout__)


def setup_sidecar_logging() -> None:
    install_stdio_broken_pipe_guards()
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
