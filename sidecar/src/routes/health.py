from fastapi import APIRouter

from ..core.recorder import recording_manager
from ..models.schemas import HealthResponse
from ..ws.manager import ws_manager

router = APIRouter()


@router.get("/api/health", response_model=HealthResponse)
async def health_check():
    return HealthResponse(
        status="ok",
        version="0.1.0",
        active_recordings=recording_manager.active_count,
        ws_connections=ws_manager.active_count,
    )
