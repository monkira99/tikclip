"""TikTok cookie helpers for HTTP clients (shared by api + stream)."""

from __future__ import annotations

import logging

logger = logging.getLogger("tikclip.tiktok")

__all__ = ["cookie_key_summary", "normalize_tiktok_cookies"]


def cookie_key_summary(cookies: dict | None) -> str:
    if not cookies:
        return "no cookies"
    return ",".join(sorted(str(k) for k in cookies))


def normalize_tiktok_cookies(cookies: dict | None) -> dict[str, str]:
    """Merge common TikTok web cookie aliases so httpx sends what www.tiktok.com expects."""
    if not cookies:
        return {}
    out: dict[str, str] = {}
    for k, v in cookies.items():
        if v is None:
            continue
        out[str(k)] = str(v)
    if "sessionid" not in out and "sessionid_ss" in out:
        out["sessionid"] = out["sessionid_ss"]
        logger.debug("normalized sessionid from sessionid_ss for TikTok web requests")
    return out
