"""Poll watched TikTok accounts for live status; optional auto-record."""

from __future__ import annotations

import asyncio
import json
import logging
from dataclasses import dataclass, replace

from config import settings
from core.recorder import recording_manager
from tiktok.api import TikTokAPI, cookie_key_summary
from tiktok.stream import StreamResolver
from ws.manager import ws_manager

logger = logging.getLogger("tikclip.watcher")


@dataclass
class WatchedAccount:
    username: str
    account_id: int
    cookies_json: str | None = None
    proxy_url: str | None = None
    auto_record: bool = False
    was_live: bool = False


def _parse_cookies_json(cookies_json: str | None) -> dict | None:
    if not cookies_json:
        return None
    data = json.loads(cookies_json)
    if not isinstance(data, dict):
        raise ValueError("cookies_json must be a JSON object")
    return data


class AccountWatcher:
    def __init__(self) -> None:
        self._accounts: dict[int, WatchedAccount] = {}
        self._running: bool = False
        self._task: asyncio.Task[None] | None = None
        self._lock = asyncio.Lock()

    def add_account(
        self,
        account_id: int,
        username: str,
        *,
        cookies_json: str | None = None,
        proxy_url: str | None = None,
        auto_record: bool = False,
    ) -> None:
        existing = self._accounts.get(account_id)
        self._accounts[account_id] = WatchedAccount(
            username=username,
            account_id=account_id,
            cookies_json=cookies_json,
            proxy_url=proxy_url,
            auto_record=auto_record,
            was_live=existing.was_live if existing is not None else False,
        )
        if existing is not None:
            logger.debug(
                "add_account re-register account_id=%s username=%s preserve_was_live=%s",
                account_id,
                username,
                existing.was_live,
            )

    def remove_account(self, account_id: int) -> bool:
        return self._accounts.pop(account_id, None) is not None

    def live_overview(self) -> list[dict]:
        """Last known live flags from the poller (for HTTP sync when WebSocket is unavailable)."""
        return [
            {
                "account_id": aid,
                "username": acc.username,
                "is_live": acc.was_live,
            }
            for aid, acc in sorted(self._accounts.items(), key=lambda x: x[0])
        ]

    def update_account(
        self,
        account_id: int,
        *,
        username: str | None = None,
        cookies_json: str | None = None,
        proxy_url: str | None = None,
        auto_record: bool | None = None,
    ) -> bool:
        acc = self._accounts.get(account_id)
        if acc is None:
            return False
        if username is not None:
            acc = replace(acc, username=username)
        if cookies_json is not None:
            acc = replace(acc, cookies_json=cookies_json)
        if proxy_url is not None:
            acc = replace(acc, proxy_url=proxy_url)
        if auto_record is not None:
            acc = replace(acc, auto_record=auto_record)
        self._accounts[account_id] = acc
        return True

    async def check_account(
        self,
        username: str,
        cookies: dict | None,
        proxy: str | None,
    ) -> dict:
        api = TikTokAPI(cookies=cookies, proxy=proxy)
        try:
            status = await api.check_live_status(username)
        finally:
            await api.aclose()
        return {
            "username": status.username,
            "is_live": status.is_live,
            "room_id": status.room_id,
            "stream_url": status.stream_url,
            "viewer_count": status.viewer_count,
        }

    async def start(self) -> None:
        async with self._lock:
            if self._task is not None and not self._task.done():
                return
            self._running = True
            self._task = asyncio.create_task(self._poll_loop())

    async def stop(self) -> None:
        async with self._lock:
            self._running = False
            t = self._task
            self._task = None
        if t is not None:
            t.cancel()
            try:
                await t
            except asyncio.CancelledError:
                pass

    async def _poll_loop(self) -> None:
        try:
            while self._running:
                await self._poll_once()
                await asyncio.sleep(settings.poll_interval_seconds)
        except asyncio.CancelledError:
            pass

    async def _poll_once(self) -> None:
        for account_id, acc in list(self._accounts.items()):
            cookies: dict | None
            try:
                cookies = _parse_cookies_json(acc.cookies_json)
            except (json.JSONDecodeError, ValueError) as e:
                logger.warning(
                    "account_id=%s username=%s invalid cookies_json: %s",
                    account_id,
                    acc.username,
                    e,
                )
                cookies = None
            result = await self.check_account(acc.username, cookies, acc.proxy_url)
            is_live = bool(result.get("is_live"))
            logger.info(
                "poll account_id=%s username=%s is_live=%s room_id=%s cookies=%s",
                account_id,
                acc.username,
                is_live,
                result.get("room_id"),
                cookie_key_summary(cookies),
            )
            room_id = result.get("room_id")
            if is_live and not acc.was_live:
                await ws_manager.broadcast(
                    "account_live",
                    {
                        "account_id": account_id,
                        "username": acc.username,
                        "room_id": room_id,
                        "stream_url": result.get("stream_url"),
                        "viewer_count": result.get("viewer_count"),
                    },
                )
                if acc.auto_record and room_id:
                    stream_url = result.get("stream_url")
                    if not stream_url:
                        resolver = StreamResolver(
                            cookies=cookies,
                            proxy=acc.proxy_url,
                        )
                        stream_url = await resolver.get_stream_url(str(room_id))
                    if stream_url:
                        try:
                            await recording_manager.start_recording(
                                account_id=account_id,
                                username=acc.username,
                                stream_url=stream_url,
                            )
                        except RuntimeError:
                            pass
            acc = replace(acc, was_live=is_live)
            self._accounts[account_id] = acc
            logger.info(
                "ws broadcast account_status account_id=%s is_live=%s ws_clients=%s",
                account_id,
                is_live,
                ws_manager.active_count,
            )
            await ws_manager.broadcast(
                "account_status",
                {
                    "account_id": account_id,
                    "username": acc.username,
                    "is_live": is_live,
                },
            )


account_watcher = AccountWatcher()
