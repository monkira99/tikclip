import pytest
from unittest.mock import AsyncMock, patch

from src.tiktok.api import TikTokAPI


@pytest.mark.asyncio
async def test_check_live_status_returns_response():
    api = TikTokAPI()
    with patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock:
        mock.return_value = {
            "LiveRoomInfo": {
                "status": 2,
                "ownerInfo": {"uniqueId": "testuser"},
                "liveRoomStats": {"userCount": 1500},
            },
            "room_id": "12345",
        }
        result = await api.check_live_status("testuser")
        assert result.is_live is True
        assert result.room_id == "12345"
        assert result.viewer_count == 1500


@pytest.mark.asyncio
async def test_check_live_status_offline():
    api = TikTokAPI()
    with patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock:
        mock.return_value = {
            "LiveRoomInfo": {"status": 4},
            "room_id": None,
        }
        result = await api.check_live_status("testuser")
        assert result.is_live is False
        assert result.room_id is None
