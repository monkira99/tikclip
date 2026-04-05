"""Resolve TikTok live stream URLs from webcast room info."""

from __future__ import annotations

import httpx

_STREAM_QUALITY_ORDER = ("FULL_HD1", "HD1", "SD1", "SD2")


def pick_stream_url_from_room_data(data: dict) -> str | None:
    """Pick best FLV or HLS URL from merged webcast room info payload."""
    stream_url = data.get("stream_url") or {}
    if not isinstance(stream_url, dict):
        return None

    flv_pull = stream_url.get("flv_pull_url") or {}
    if isinstance(flv_pull, dict):
        for quality in _STREAM_QUALITY_ORDER:
            url = flv_pull.get(quality)
            if isinstance(url, str) and url:
                return url

    hls_pull = stream_url.get("hls_pull_url_map") or stream_url.get("hls_pull_url") or {}
    if isinstance(hls_pull, dict):
        for quality in _STREAM_QUALITY_ORDER:
            url = hls_pull.get(quality)
            if isinstance(url, str) and url:
                return url

    raw_flv = stream_url.get("flv_pull_url")
    raw_hls = stream_url.get("hls_pull_url")
    if isinstance(raw_flv, str) and raw_flv:
        return raw_flv
    if isinstance(raw_hls, str) and raw_hls:
        return raw_hls
    return None


class StreamResolver:
    """Resolves FLV/HLS stream URL for a TikTok live room."""

    def __init__(self, cookies: dict | None = None, proxy: str | None = None):
        self._cookies = cookies or {}
        self._proxy = proxy

    async def get_stream_url(self, room_id: str) -> str | None:
        async with httpx.AsyncClient(
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
        ) as client:
            url = f"https://webcast.tiktok.com/webcast/room/info/?room_id={room_id}"
            response = await client.get(url)
            response.raise_for_status()
            payload = response.json()
            data = payload.get("data") or {}
            return pick_stream_url_from_room_data(data)
