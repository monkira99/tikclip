import json
from typing import Annotated

from fastapi import APIRouter, Body, Depends, HTTPException, Query

from ..core.watcher import account_watcher
from ..models.schemas import (
    AccountStatusRequest,
    AccountStatusResponse,
    WatchAccountRequest,
)

router = APIRouter()


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
    result = await account_watcher.check_account(
        body.username,
        cookies,
        body.proxy_url,
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
