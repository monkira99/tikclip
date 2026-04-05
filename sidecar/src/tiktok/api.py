"""TikTok live status via live page HTML + webcast room info."""

from __future__ import annotations

import re
from dataclasses import dataclass

import httpx

from .stream import pick_stream_url_from_room_data


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
)


class TikTokAPI:
    BASE_URL = "https://www.tiktok.com"

    def __init__(self, cookies: dict | None = None, proxy: str | None = None):
        self._cookies = cookies or {}
        self._proxy = proxy
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(
                timeout=15.0,
                proxy=self._proxy,
                cookies=self._cookies,
                headers={
                    "User-Agent": (
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                        "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                    ),
                    "Referer": "https://www.tiktok.com/",
                },
                follow_redirects=True,
            )
        return self._client

    async def check_live_status(self, username: str) -> LiveStatus:
        try:
            room_info = await self._fetch_room_info(username)
            live_room = room_info.get("LiveRoomInfo") or {}
            status_code = live_room.get("status", 4)
            is_live = status_code == 2
            raw_room_id = room_info.get("room_id")
            room_id = str(raw_room_id) if raw_room_id else None

            viewer_count = None
            title = None
            stream_url = None
            if is_live:
                stats = live_room.get("liveRoomStats") or {}
                uc = stats.get("userCount")
                if uc is not None:
                    try:
                        viewer_count = int(uc)
                    except (TypeError, ValueError):
                        viewer_count = None
                title = (
                    live_room.get("title")
                    or live_room.get("liveRoomName")
                    or live_room.get("liveRoomTitle")
                )
                if isinstance(title, str) and not title.strip():
                    title = None
                stream_url = pick_stream_url_from_room_data(room_info)

            return LiveStatus(
                username=username,
                is_live=is_live,
                room_id=room_id,
                stream_url=stream_url,
                viewer_count=viewer_count,
                title=title,
            )
        except Exception:
            return LiveStatus(username=username, is_live=False)

    async def _fetch_room_info(self, username: str) -> dict:
        client = await self._get_client()
        clean = username.lstrip("@")
        page_url = f"{self.BASE_URL}/@{clean}/live"
        response = await client.get(page_url)
        response.raise_for_status()
        text = response.text

        room_id = self._extract_room_id(text)
        if not room_id:
            return {"LiveRoomInfo": {"status": 4}, "room_id": None}

        info_url = f"https://webcast.tiktok.com/webcast/room/info/?room_id={room_id}"
        info_response = await client.get(info_url)
        info_response.raise_for_status()
        body = info_response.json()
        data = body.get("data") or {}
        if not isinstance(data, dict):
            data = {}
        merged = dict(data)
        merged["room_id"] = room_id
        return merged

    def _extract_room_id(self, html: str) -> str | None:
        for pattern in _ROOM_ID_PATTERNS:
            match = pattern.search(html)
            if match:
                return match.group(1)
        return None

    async def aclose(self) -> None:
        if self._client is not None:
            await self._client.aclose()
            self._client = None

    async def close(self) -> None:
        await self.aclose()
