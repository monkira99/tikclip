"""Async HTTP for TikTok: prefer curl_cffi TLS/browser impersonation; fallback httpx."""

from __future__ import annotations

import json
import logging
import sys
from dataclasses import dataclass
from typing import Any, Protocol

import httpx

from config import settings

logger = logging.getLogger("tikclip.tiktok")


def _browser_platform() -> str:
    if sys.platform == "win32":
        return '"Windows"'
    if sys.platform.startswith("linux"):
        return '"Linux"'
    return '"macOS"'


def _browser_user_agent() -> str:
    if sys.platform == "win32":
        return (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        )
    if sys.platform.startswith("linux"):
        return (
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        )
    return (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    )


# Browser-like headers aligned with tiktok-live-recorder / real Chrome navigation.
_TIKTOK_NAV_HEADERS: dict[str, str] = {
    "Accept": (
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,"
        "image/apng,application/json,text/plain,*/*;q=0.8"
    ),
    "Accept-Language": "en-US,en;q=0.9",
    "Origin": "https://www.tiktok.com",
    "Referer": "https://www.tiktok.com/",
    "Sec-Ch-Ua": '"Chromium";v="131", "Not_A Brand";v="24", "Google Chrome";v="131"',
    "Sec-Ch-Ua-Mobile": "?0",
    "Sec-Ch-Ua-Platform": _browser_platform(),
    "Sec-Fetch-Dest": "document",
    "Sec-Fetch-Mode": "navigate",
    "Sec-Fetch-Site": "same-origin",
    "Sec-Fetch-User": "?1",
    "Upgrade-Insecure-Requests": "1",
    "User-Agent": _browser_user_agent(),
}

_TIKTOK_BINARY_HEADERS: dict[str, str] = {
    "Accept": "*/*",
    "Accept-Language": "en-US,en;q=0.9",
    "Referer": "https://www.tiktok.com/",
    "Origin": "https://www.tiktok.com",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "no-cors",
    "Sec-Fetch-Site": "cross-site",
    "User-Agent": _TIKTOK_NAV_HEADERS["User-Agent"],
}


class TikTokHttpStatusError(Exception):
    """HTTP error from TikTok transport (mirrors raise_for_status use)."""

    def __init__(self, status_code: int, url: str, text: str = "") -> None:
        self.status_code = status_code
        self.url = url
        self.text = text
        super().__init__(f"HTTP {status_code} {url}")


@dataclass
class TikTokHttpResponse:
    status_code: int
    text: str
    url: str

    @property
    def is_success(self) -> bool:
        return 200 <= self.status_code < 300

    def raise_for_status(self) -> None:
        if not self.is_success:
            raise TikTokHttpStatusError(self.status_code, self.url, self.text)

    def json(self) -> Any:
        return json.loads(self.text)


class TikTokHttpTransport(Protocol):
    async def get(
        self,
        url: str,
        *,
        params: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
    ) -> TikTokHttpResponse: ...

    async def get_bytes(
        self,
        url: str,
        *,
        headers: dict[str, str] | None = None,
    ) -> tuple[int, bytes, str, str | None]:
        """HTTP status, body, final URL, content-type (lowercase or None)."""
        ...

    async def aclose(self) -> None: ...


class HttpxTikTokTransport:
    def __init__(self, cookies: dict[str, str], proxy: str | None, timeout_seconds: float) -> None:
        self._client = httpx.AsyncClient(
            timeout=timeout_seconds,
            proxy=proxy,
            cookies=cookies,
            headers=dict(_TIKTOK_NAV_HEADERS),
            follow_redirects=True,
        )

    async def get(
        self,
        url: str,
        *,
        params: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
    ) -> TikTokHttpResponse:
        merged = dict(_TIKTOK_NAV_HEADERS)
        if headers:
            merged.update(headers)
        r = await self._client.get(url, params=params, headers=merged)
        return TikTokHttpResponse(r.status_code, r.text, str(r.url))

    async def get_bytes(
        self,
        url: str,
        *,
        headers: dict[str, str] | None = None,
    ) -> tuple[int, bytes, str, str | None]:
        merged = dict(_TIKTOK_BINARY_HEADERS)
        if headers:
            merged.update(headers)
        r = await self._client.get(url, headers=merged)
        ct = r.headers.get("content-type")
        ct = ct.split(";")[0].strip().lower() if ct else None
        return r.status_code, r.content, str(r.url), ct

    async def aclose(self) -> None:
        await self._client.aclose()


class CurlCffiTikTokTransport:
    """curl_cffi AsyncSession with Chrome TLS/JA3 impersonation (tiktok-live-recorder style)."""

    def __init__(
        self,
        cookies: dict[str, str],
        proxy: str | None,
        impersonate: str,
        session_factory: Any,
        timeout_seconds: float,
    ) -> None:
        self._session_factory = session_factory
        self._cookies = cookies
        self._proxy = proxy
        self._impersonate = impersonate
        self._timeout = timeout_seconds
        self._session: Any = None

    async def _ensure(self) -> Any:
        if self._session is None:
            from curl_cffi.const import CurlOpt, CurlSslVersion

            # Match tiktok-live-recorder HttpClient (non-Termux): HTTP/1.1 + TLS 1.2 cap.
            self._session = self._session_factory(
                timeout=self._timeout,
                proxy=self._proxy,
                http_version="v1",
                curl_options={CurlOpt.SSLVERSION: CurlSslVersion.TLSv1_2},
            )
        return self._session

    async def get(
        self,
        url: str,
        *,
        params: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
    ) -> TikTokHttpResponse:
        sess = await self._ensure()
        merged = dict(_TIKTOK_NAV_HEADERS)
        if headers:
            merged.update(headers)
        r = await sess.get(
            url,
            params=params,
            headers=merged,
            cookies=self._cookies or None,
            impersonate=self._impersonate,
        )
        return TikTokHttpResponse(r.status_code, r.text, str(r.url))

    async def get_bytes(
        self,
        url: str,
        *,
        headers: dict[str, str] | None = None,
    ) -> tuple[int, bytes, str, str | None]:
        sess = await self._ensure()
        merged = dict(_TIKTOK_BINARY_HEADERS)
        if headers:
            merged.update(headers)
        r = await sess.get(
            url,
            headers=merged,
            cookies=self._cookies or None,
            impersonate=self._impersonate,
        )
        raw_ct = r.headers.get("content-type") if hasattr(r.headers, "get") else None
        if raw_ct is None and hasattr(r.headers, "get_header"):
            raw_ct = r.headers.get_header("content-type")  # type: ignore[union-attr]
        ct = raw_ct.split(";")[0].strip().lower() if isinstance(raw_ct, str) else None
        return r.status_code, r.content, str(r.url), ct

    async def aclose(self) -> None:
        if self._session is not None:
            await self._session.close()
            self._session = None


def create_tiktok_transport(cookies: dict[str, str], proxy: str | None) -> TikTokHttpTransport:
    backend = (settings.tiktok_http_backend or "curl_cffi").strip().lower()
    if backend == "curl_cffi":
        try:
            from curl_cffi.requests import AsyncSession as CurlAsyncSession
        except ImportError:
            logger.warning("curl_cffi not installed; falling back to httpx for TikTok HTTP")
        else:
            return CurlCffiTikTokTransport(
                cookies,
                proxy,
                settings.tiktok_curl_impersonate,
                CurlAsyncSession,
                settings.tiktok_http_timeout_seconds,
            )
    return HttpxTikTokTransport(cookies, proxy, settings.tiktok_http_timeout_seconds)
