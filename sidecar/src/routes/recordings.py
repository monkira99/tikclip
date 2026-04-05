import json

from fastapi import APIRouter, HTTPException

from ..core.recorder import recording_manager
from ..models.schemas import (
    RecordingStatusResponse,
    StartRecordingRequest,
    StopRecordingRequest,
)
from ..tiktok.stream import StreamResolver

router = APIRouter()


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


def _status_to_response(d: dict) -> RecordingStatusResponse:
    return RecordingStatusResponse(
        recording_id=d["recording_id"],
        account_id=d["account_id"],
        username=d["username"],
        status=d["status"],
        duration_seconds=d.get("duration_seconds", 0),
        file_size_bytes=d.get("file_size_bytes", 0),
        file_path=d.get("file_path"),
        error_message=d.get("error_message"),
    )


@router.post("/api/recording/start", response_model=RecordingStatusResponse)
async def start_recording(body: StartRecordingRequest):
    stream_url = body.stream_url
    if not stream_url:
        if not body.room_id:
            raise HTTPException(
                status_code=400,
                detail="Provide stream_url or room_id to start recording",
            )
        cookies = _parse_cookies(body.cookies_json)
        resolver = StreamResolver(cookies=cookies, proxy=body.proxy_url)
        stream_url = await resolver.get_stream_url(body.room_id)
        if not stream_url:
            raise HTTPException(
                status_code=400,
                detail="Could not resolve stream URL for this room",
            )

    try:
        recording_id = await recording_manager.start_recording(
            account_id=body.account_id,
            username=body.username,
            stream_url=stream_url,
            max_duration_seconds=body.max_duration_seconds,
        )
    except RuntimeError as e:
        raise HTTPException(status_code=503, detail=str(e)) from e

    meta = recording_manager.get_status(recording_id)
    assert meta is not None
    return _status_to_response(meta)


@router.post("/api/recording/stop", response_model=RecordingStatusResponse)
async def stop_recording(body: StopRecordingRequest):
    ok = await recording_manager.stop_recording(body.recording_id)
    if not ok:
        raise HTTPException(status_code=404, detail="Recording not found")
    meta = recording_manager.get_status(body.recording_id)
    assert meta is not None
    return _status_to_response(meta)


@router.get("/api/recording/status", response_model=list[RecordingStatusResponse])
async def list_recording_status():
    return [_status_to_response(d) for d in recording_manager.get_all_status()]


@router.get("/api/recording/status/{recording_id}", response_model=RecordingStatusResponse)
async def get_recording_status(recording_id: str):
    meta = recording_manager.get_status(recording_id)
    if meta is None:
        raise HTTPException(status_code=404, detail="Recording not found")
    return _status_to_response(meta)
