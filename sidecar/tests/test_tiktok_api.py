from unittest.mock import AsyncMock, patch

import pytest

from tiktok.api import TikTokAPI, normalize_tiktok_cookies


@pytest.mark.asyncio
async def test_check_live_status_returns_response():
    api = TikTokAPI()
    with (
        patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock,
        patch.object(api, "_room_is_broadcasting", new_callable=AsyncMock) as alive,
    ):
        mock.return_value = {
            "LiveRoomInfo": {
                "status": 2,
                "ownerInfo": {"uniqueId": "testuser"},
                "liveRoomStats": {"userCount": 1500},
            },
            "room_id": "12345",
        }
        alive.return_value = False
        result = await api.check_live_status("testuser")
        assert result.is_live is True
        assert result.room_id == "12345"
        assert result.viewer_count == 1500


@pytest.mark.asyncio
async def test_check_live_status_flat_payload_uses_root_status():
    api = TikTokAPI()
    with (
        patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock,
        patch.object(api, "_room_is_broadcasting", new_callable=AsyncMock) as alive,
    ):
        mock.return_value = {"room_id": "99", "status": 2, "user_count": 42, "title": "  Hi  "}
        alive.return_value = False
        result = await api.check_live_status("u")
        assert result.is_live is True
        assert result.room_id == "99"
        assert result.viewer_count == 42
        assert result.title == "Hi"


@pytest.mark.asyncio
async def test_check_live_status_check_alive_true():
    api = TikTokAPI()
    with (
        patch.object(api, "_fetch_room_info", new_callable=AsyncMock) as mock,
        patch.object(api, "_room_is_broadcasting", new_callable=AsyncMock) as alive,
    ):
        mock.return_value = {"room_id": "1", "title": "Hello"}
        alive.return_value = True
        result = await api.check_live_status("u")
        assert result.is_live is True
        assert result.title == "Hello"


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


def test_normalize_tiktok_copies_sessionid_from_ss():
    out = normalize_tiktok_cookies({"sessionid_ss": "abc", "tt-target-idc": "alisg"})
    assert out["sessionid_ss"] == "abc"
    assert out["sessionid"] == "abc"
    assert out["tt-target-idc"] == "alisg"
