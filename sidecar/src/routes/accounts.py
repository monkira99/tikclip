"""HTTP surface for account checks and watcher registration.

Routes stay thin: validate input, delegate to ``account_watcher`` / TikTok helpers.
Workflow orchestration lives in the Tauri app + SQLite engine, not here.
"""

import json
import logging
from typing import Annotated

from fastapi import APIRouter, Body, Depends, HTTPException, Query

from core.watcher import account_watcher
from models.schemas import (
    AccountStatusRequest,
    AccountStatusResponse,
    WatchAccountRequest,
)
from tiktok.api import cookie_key_summary

router = APIRouter()
logger = logging.getLogger("tikclip.routes.accounts")


def _resolve_watch_request(
    account_id: int | None = Query(None),
    username: str | None = Query(None),
    auto_record: bool = Query(False),
    cookies_json: str | None = Query(None),
    proxy_url: str | None = Query(None),
    body: Annotated[WatchAccountRequest | None, Body()] = None,
) -> WatchAccountRequest:
    if body is not None:
        return body
    if account_id is not None and username is not None:
        return WatchAccountRequest(
            account_id=account_id,
            username=username,
            auto_record=auto_record,
            cookies_json=cookies_json,
            proxy_url=proxy_url,
        )
    raise HTTPException(
        status_code=400,
        detail="Provide account_id and username in JSON body or query parameters",
    )


def _parse_cookies(cookies_json: str | None) -> dict | None:
    if not cookies_json:
        return None
    try:
        data = json.loads(cookies_json)
    except json.JSONDecodeError as e:
        raise HTTPException(status_code=400, detail=f"Invalid cookies_json: {e}") from e
    if not isinstance(data, dict):
        raise HTTPException(status_code=400, detail="cookies_json must be a JSON object")
    return data


@router.post("/api/accounts/check-status", response_model=AccountStatusResponse)
async def check_account_status(body: AccountStatusRequest):
    cookies = _parse_cookies(body.cookies_json)
    logger.info(
        "HTTP check-status username=%s cookies=%s",
        body.username.lstrip("@"),
        cookie_key_summary(cookies),
    )
    result = await account_watcher.check_account(
        body.username,
        cookies,
        body.proxy_url,
    )
    logger.info(
        "HTTP check-status result username=%s is_live=%s room_id=%s",
        result.get("username"),
        result.get("is_live"),
        result.get("room_id"),
    )
    return AccountStatusResponse(
        username=result["username"],
        is_live=result["is_live"],
        room_id=result.get("room_id"),
        stream_url=result.get("stream_url"),
        viewer_count=result.get("viewer_count"),
    )


@router.post("/api/accounts/watch")
async def watch_account(req: Annotated[WatchAccountRequest, Depends(_resolve_watch_request)]):
    logger.info(
        "HTTP watch account_id=%s username=%s auto_record=%s cookies=%s",
        req.account_id,
        req.username.lstrip("@"),
        req.auto_record,
        cookie_key_summary(_parse_cookies(req.cookies_json) if req.cookies_json else None),
    )
    account_watcher.add_account(
        req.account_id,
        req.username,
        cookies_json=req.cookies_json,
        proxy_url=req.proxy_url,
        auto_record=req.auto_record,
    )
    return {"ok": True, "account_id": req.account_id}


@router.delete("/api/accounts/watch/{account_id}")
async def unwatch_account(account_id: int):
    if not account_watcher.remove_account(account_id):
        raise HTTPException(status_code=404, detail="Account not watched")
    return {"ok": True}


@router.get("/api/accounts/live-overview")
async def live_overview():
    """Snapshot of last poll results; use when desktop WebSocket does not reach the UI."""
    rows = account_watcher.live_overview()
    logger.debug("live-overview %s account(s)", len(rows))
    return {"accounts": rows}


@router.post("/api/accounts/poll-now")
async def poll_now():
    """Trigger an immediate poll of all watched accounts (used on app startup)."""
    logger.info("poll-now triggered")
    await account_watcher.poll_now()
    rows = account_watcher.live_overview()
    logger.info(
        "poll-now done %s account(s): %s",
        len(rows),
        ", ".join(f"{r['username']}={'live' if r['is_live'] else 'off'}" for r in rows),
    )
    return {"accounts": rows}
