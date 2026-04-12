"""TikTok live status via live page HTML + webcast APIs.

Webcast query params align with common TikTok web usage (e.g. aid=1988) as in
tiktok-live-recorder — without ``aid``, ``room/info`` often returns HTTP 400.
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

import httpx

from config import settings
from tiktok.cookies import cookie_key_summary, normalize_tiktok_cookies
from tiktok.http_transport import (
    TikTokHttpStatusError,
    TikTokHttpTransport,
    create_tiktok_transport,
)
from tiktok.stream import pick_stream_url_from_room_data

logger = logging.getLogger("tikclip.tiktok")

__all__ = ["LiveStatus", "TikTokAPI", "cookie_key_summary", "normalize_tiktok_cookies"]


@dataclass
class LiveStatus:
    username: str
    is_live: bool
    room_id: str | None = None
    stream_url: str | None = None
    viewer_count: int | None = None
    title: str | None = None


_ROOM_ID_PATTERNS = (
    re.compile(r'"roomId"\s*:\s*"(\d+)"'),
    re.compile(r'"room_id"\s*:\s*"(\d+)"'),
    re.compile(r'"room_id"\s*:\s*(\d+)'),
    re.compile(r"room_id=(\d+)"),
    re.compile(r"roomId=(\d+)"),
    re.compile(r'"id_str"\s*:\s*"(\d+)"'),
    re.compile(r"room/(\d{10,})"),
    re.compile(r'"web_rid"\s*:\s*"(\d+)"'),
)

# Cap saved debug HTML so a huge response cannot fill the disk.
_DEBUG_TIKTOK_HTML_MAX_BYTES = 512 * 1024


def _save_debug_tiktok_live_html(username: str, html: str) -> Path | None:
    """Write live page HTML for debug_tiktok (HTTP errors or block/WAF-style pages)."""
    try:
        root = settings.storage_path.resolve() / "debug" / "tiktok_live_html"
        root.mkdir(parents=True, exist_ok=True)
        safe_chars = (c if c.isalnum() or c in "-_" else "_" for c in username.strip())
        safe = "".join(safe_chars)[:64] or "unknown"
        name = f"{datetime.now().strftime('%Y%m%d-%H%M%S')}_{safe}.html"
        path = root / name
        raw = html.encode("utf-8", errors="replace")
        if len(raw) > _DEBUG_TIKTOK_HTML_MAX_BYTES:
            raw = raw[:_DEBUG_TIKTOK_HTML_MAX_BYTES]
        path.write_bytes(raw)
        return path
    except OSError as e:
        logger.warning("debug_tiktok: could not save HTML file: %s", e)
        return None


def _live_page_html_suggests_error_or_block(html: str) -> bool:
    """Plain offline pages are not saved; WAF/challenge-style HTML is."""
    h = html.lower().replace("\u2019", "'")
    markers = (
        "wafchallenge",
        "_wafchallengeid",
        "slardar_us_waf",
        "verify you're human",
        "access denied",
        "unusual traffic",
        "captcha",
        "please wait",
    )
    return any(m in h for m in markers)


class TikTokAPI:
    BASE_URL = "https://www.tiktok.com"
    WEBCAST_BASE = "https://webcast.tiktok.com"
    # TikTok web client uses aid=1988 on webcast APIs (see tiktok-live-recorder and TikTok web).

    def __init__(self, cookies: dict | None = None, proxy: str | None = None):
        self._cookies = normalize_tiktok_cookies(cookies)
        self._proxy = proxy
        self._http: TikTokHttpTransport | None = None

    async def _get_http(self) -> TikTokHttpTransport:
        if self._http is None:
            self._http = create_tiktok_transport(self._cookies, self._proxy)
        return self._http

    def _webcast_region_param(self) -> str:
        """Rough region hint for check_alive (matches common tt-target-idc values)."""
        idc = (self._cookies.get("tt-target-idc") or "").lower()
        if "alisg" in idc:
            return "SG"
        if "useast" in idc:
            return "US"
        if "eu" in idc or "gcp" in idc:
            return "EU"
        return "CH"

    async def _fetch_webcast_room_payload(self, http: TikTokHttpTransport, room_id: str) -> dict:
        """Room details; requires aid=1988 or TikTok returns 400."""
        params = {"aid": "1988", "room_id": room_id}
        info_response = await http.get(f"{self.WEBCAST_BASE}/webcast/room/info/", params=params)
        if info_response.is_success:
            body = info_response.json()
            data = body.get("data") or {}
            if not isinstance(data, dict):
                data = {}
            logger.debug(
                "webcast room/info ok room_id=%s keys=%s",
                room_id,
                list(body.keys())[:12],
            )
            merged = dict(data)
            merged["room_id"] = room_id
            return merged

        preview = (info_response.text or "")[:300].replace("\n", " ")
        logger.warning(
            "webcast room/info failed room_id=%s status=%s preview=%r",
            room_id,
            info_response.status_code,
            preview,
        )
        return await self._fetch_webcast_check_alive_fallback(http, room_id)

    async def _fetch_webcast_check_alive_fallback(
        self, http: TikTokHttpTransport, room_id: str
    ) -> dict:
        """Used when room/info errors; same pattern as tiktok-live-recorder check_alive."""
        region = self._webcast_region_param()
        params = {
            "aid": "1988",
            "region": region,
            "room_ids": room_id,
            "user_is_login": "true",
        }
        r = await http.get(f"{self.WEBCAST_BASE}/webcast/room/check_alive/", params=params)
        if not r.is_success:
            logger.warning(
                "webcast check_alive failed room_id=%s status=%s",
                room_id,
                r.status_code,
            )
            return {"LiveRoomInfo": {"status": 4}, "room_id": room_id}

        try:
            body = r.json()
        except Exception:
            logger.warning("webcast check_alive invalid JSON room_id=%s", room_id)
            return {"LiveRoomInfo": {"status": 4}, "room_id": room_id}

        rows = body.get("data") or []
        alive = bool(rows and isinstance(rows, list) and rows[0].get("alive"))
        logger.info(
            "webcast check_alive fallback room_id=%s region=%s alive=%s",
            room_id,
            region,
            alive,
        )
        if alive:
            return {"LiveRoomInfo": {"status": 2}, "room_id": room_id}
        return {"LiveRoomInfo": {"status": 4}, "room_id": room_id}

    async def _room_is_broadcasting(self, http: TikTokHttpTransport, room_id: str) -> bool:
        """Authoritative on-air flag (same endpoint family as tiktok-live-recorder)."""
        region = self._webcast_region_param()
        params = {
            "aid": "1988",
            "region": region,
            "room_ids": room_id,
            "user_is_login": "true",
        }
        r = await http.get(f"{self.WEBCAST_BASE}/webcast/room/check_alive/", params=params)
        if not r.is_success:
            logger.debug(
                "check_alive room_id=%s status=%s region=%s",
                room_id,
                r.status_code,
                region,
            )
            return False
        try:
            body = r.json()
        except Exception:
            return False
        rows = body.get("data") or []
        alive = bool(rows and isinstance(rows, list) and rows[0].get("alive"))
        logger.debug("check_alive room_id=%s region=%s alive=%s", room_id, region, alive)
        return alive

    @staticmethod
    def _viewer_count_from_merged(merged: dict) -> int | None:
        lr = merged.get("LiveRoomInfo") if isinstance(merged.get("LiveRoomInfo"), dict) else {}
        stats = (lr or {}).get("liveRoomStats") or {}
        uc = stats.get("userCount")
        if uc is not None:
            try:
                return int(uc)
            except (TypeError, ValueError):
                pass
        for key in ("user_count", "viewer_count"):
            v = merged.get(key)
            if v is not None:
                try:
                    return int(v)
                except (TypeError, ValueError):
                    pass
        return None

    @staticmethod
    def _title_from_merged(merged: dict) -> str | None:
        lr = merged.get("LiveRoomInfo") if isinstance(merged.get("LiveRoomInfo"), dict) else {}
        for t in (
            (lr or {}).get("title"),
            (lr or {}).get("liveRoomName"),
            (lr or {}).get("liveRoomTitle"),
            merged.get("title"),
        ):
            if isinstance(t, str) and t.strip():
                return t.strip()
        return None

    @staticmethod
    def _status_int_from_merged(merged: dict) -> int | None:
        """Some payloads use LiveRoomInfo.status or data.status (2 = live)."""
        lr = merged.get("LiveRoomInfo") if isinstance(merged.get("LiveRoomInfo"), dict) else {}
        st = (lr or {}).get("status")
        if st is not None:
            try:
                return int(st)
            except (TypeError, ValueError):
                pass
        st2 = merged.get("status")
        if st2 is not None:
            try:
                return int(st2)
            except (TypeError, ValueError):
                pass
        return None

    async def _room_id_via_sign_service(
        self, http: TikTokHttpTransport, username: str
    ) -> str | None:
        """Third-party sign URL then TikTok signed path (opt-in); sends unique_id off-device."""
        base = (settings.tiktok_room_sign_base_url or "").strip().rstrip("/")
        if not base:
            return None
        sign_url = f"{base}/tiktok/room/api/sign"
        sign_resp = await http.get(sign_url, params={"unique_id": username})
        if not sign_resp.is_success:
            logger.warning(
                "tiktok sign API failed status=%s url=%s",
                sign_resp.status_code,
                sign_resp.url,
            )
            return None
        try:
            sign_body = sign_resp.json()
        except Exception:
            logger.warning("tiktok sign API response not JSON url=%s", sign_resp.url)
            return None

        signed_path = sign_body.get("signed_path")
        if signed_path is None and isinstance(sign_body.get("data"), dict):
            signed_path = sign_body["data"].get("signed_path")
        if not isinstance(signed_path, str) or not signed_path.strip():
            return None
        path = signed_path.strip()
        if not path.startswith("/"):
            path = f"/{path}"

        page_resp = await http.get(f"{self.BASE_URL}{path}")
        text = page_resp.text or ""
        if not text.strip():
            logger.warning("tiktok signed URL empty body status=%s", page_resp.status_code)
            return None
        if "please wait" in text.lower():
            logger.warning(
                "tiktok signed URL looks WAF-blocked (Please wait) username=%s",
                username,
            )
            return None

        if page_resp.is_success:
            try:
                detail = page_resp.json()
            except Exception:
                detail = None
            if isinstance(detail, dict):
                rid = self._room_id_from_signed_json(detail)
                if rid:
                    return rid

        return self._extract_room_id(text)

    @staticmethod
    def _room_id_from_signed_json(payload: dict) -> str | None:
        data = payload.get("data")
        if isinstance(data, dict):
            user = data.get("user")
            if isinstance(user, dict):
                for key in ("roomId", "room_id", "roomIdStr", "id_str"):
                    v = user.get(key)
                    if v is not None and str(v).strip():
                        return str(v).strip()
        user = payload.get("user")
        if isinstance(user, dict):
            for key in ("roomId", "room_id"):
                v = user.get(key)
                if v is not None and str(v).strip():
                    return str(v).strip()
        return None

    async def check_live_status(self, username: str) -> LiveStatus:
        clean = username.lstrip("@")
        ck = cookie_key_summary(self._cookies if self._cookies else None)
        logger.info("check_live_status start username=%s cookies=%s", clean, ck)
        try:
            room_info = await self._fetch_room_info(clean)
            raw_room_id = room_info.get("room_id")
            room_id = str(raw_room_id) if raw_room_id else None
            if not room_id:
                logger.info("check_live_status done username=%s is_live=False (no room_id)", clean)
                return LiveStatus(username=clean, is_live=False, room_id=None)

            http = await self._get_http()
            is_live = await self._room_is_broadcasting(http, room_id)
            status_hint = self._status_int_from_merged(room_info)
            if not is_live and status_hint == 2:
                logger.info(
                    "check_alive=false but merged status=2; treating as live room_id=%s",
                    room_id,
                )
                is_live = True

            stream_candidate = pick_stream_url_from_room_data(room_info)
            viewer_count = self._viewer_count_from_merged(room_info) if is_live else None
            title = self._title_from_merged(room_info) if is_live else None
            stream_url = stream_candidate if is_live else None

            logger.info(
                "check_live_status done username=%s is_live=%s room_id=%s status_hint=%s",
                clean,
                is_live,
                room_id,
                status_hint,
            )
            return LiveStatus(
                username=clean,
                is_live=is_live,
                room_id=room_id,
                stream_url=stream_url,
                viewer_count=viewer_count,
                title=title,
            )
        except TikTokHttpStatusError as e:
            body_preview = (e.text or "")[:200].replace("\n", " ")
            logger.warning(
                "check_live_status HTTP error username=%s url=%s status=%s body_preview=%r",
                clean,
                e.url,
                e.status_code,
                body_preview,
            )
            return LiveStatus(username=clean, is_live=False)
        except (httpx.RequestError, ConnectionError, TimeoutError, OSError) as e:
            logger.warning("check_live_status network error username=%s: %s", clean, e)
            return LiveStatus(username=clean, is_live=False)
        except Exception:
            logger.exception("check_live_status unexpected error username=%s", clean)
            return LiveStatus(username=clean, is_live=False)

    async def _fetch_room_info(self, username: str) -> dict:
        http = await self._get_http()

        if settings.tiktok_room_sign_enabled:
            signed_rid = await self._room_id_via_sign_service(http, username)
            if signed_rid:
                return await self._fetch_webcast_room_payload(http, signed_rid)

        page_url = f"{self.BASE_URL}/@{username}/live"
        try:
            response = await http.get(page_url)
            response.raise_for_status()
        except TikTokHttpStatusError as e:
            if settings.debug_tiktok:
                body = e.text or ""
                saved = _save_debug_tiktok_live_html(username, body) if body.strip() else None
                if saved is not None:
                    logger.warning(
                        "debug_tiktok: saved HTTP %s /@%s/live response to %s",
                        e.status_code,
                        username,
                        saved,
                    )
                elif body.strip():
                    snippet = body[:800].replace("\n", " ")
                    logger.warning(
                        "debug_tiktok: HTTP %s body (truncated, file save failed?): %s",
                        e.status_code,
                        snippet,
                    )
                else:
                    logger.warning(
                        "debug_tiktok: HTTP %s /@%s/live (empty body)",
                        e.status_code,
                        username,
                    )
            raise
        text = response.text
        logger.debug(
            "live page GET username=%s status=%s bytes=%s final_url=%s",
            username,
            response.status_code,
            len(text),
            response.url,
        )

        room_id = self._extract_room_id(text)
        if not room_id:
            logger.warning(
                "no room_id in /@%s/live HTML (offline, private, block, or page layout changed)",
                username,
            )
            if settings.debug_tiktok and _live_page_html_suggests_error_or_block(text):
                saved = _save_debug_tiktok_live_html(username, text)
                if saved is not None:
                    n = len(text.encode("utf-8", errors="replace"))
                    cap = _DEBUG_TIKTOK_HTML_MAX_BYTES
                    logger.warning(
                        "debug_tiktok: saved suspected block/error HTML for @%s (%s bytes%s) to %s",
                        username,
                        n,
                        f", truncated to {cap}" if n > cap else "",
                        saved,
                    )
                else:
                    snippet = text[:800].replace("\n", " ")
                    logger.warning("debug_tiktok HTML snippet (truncated): %s", snippet)
            return {"LiveRoomInfo": {"status": 4}, "room_id": None}

        return await self._fetch_webcast_room_payload(http, room_id)

    def _extract_room_id(self, html: str) -> str | None:
        for pattern in _ROOM_ID_PATTERNS:
            match = pattern.search(html)
            if match:
                rid = match.group(1)
                logger.debug("room_id matched pattern=%s value=%s", pattern.pattern[:40], rid)
                return rid
        return None

    async def aclose(self) -> None:
        if self._http is not None:
            await self._http.aclose()
            self._http = None

    async def close(self) -> None:
        await self.aclose()
