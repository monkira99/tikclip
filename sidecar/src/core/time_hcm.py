"""Wall-clock times in Asia/Ho_Chi_Minh (UTC+7), aligned with the desktop SQLite schema."""

from __future__ import annotations

from datetime import datetime
from zoneinfo import ZoneInfo

HCM_TZ = ZoneInfo("Asia/Ho_Chi_Minh")


def now_hcm() -> datetime:
    return datetime.now(HCM_TZ)


def today_ymd_hcm() -> str:
    return now_hcm().strftime("%Y-%m-%d")
