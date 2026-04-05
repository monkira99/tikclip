from fastapi import APIRouter

from ..models.schemas import HealthResponse
from ..ws.manager import ws_manager

router = APIRouter()


@router.get("/api/health", response_model=HealthResponse)
async def health_check():
    return HealthResponse(
        status="ok",
        version="0.1.0",
        active_recordings=0,
        ws_connections=ws_manager.active_count,
    )
