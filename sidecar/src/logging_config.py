"""Configure tikclip.* loggers (stderr) without breaking uvicorn access logs."""

from __future__ import annotations

import logging
import sys

from config import settings


def setup_sidecar_logging() -> None:
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
