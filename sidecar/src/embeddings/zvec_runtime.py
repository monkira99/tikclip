"""One-time zvec engine configuration; call before any collection open/create (zvec docs)."""

from __future__ import annotations

import zvec
from zvec import LogLevel, LogType

from config import settings


def setup_zvec() -> None:
    raw = (settings.log_level or "info").strip().upper()
    if raw == "WARNING":
        raw = "WARN"
    if raw not in {"DEBUG", "INFO", "WARN", "ERROR", "FATAL"}:
        zlvl = LogLevel.WARN
    else:
        zlvl = getattr(LogLevel, raw)
    zvec.init(log_type=LogType.CONSOLE, log_level=zlvl)
