from unittest.mock import AsyncMock, patch

import pytest

from core.watcher import AccountWatcher
from tiktok.api import LiveStatus
from ws.manager import ws_manager


@pytest.mark.asyncio
async def test_check_account_returns_dict_from_tiktok_api():
    watcher = AccountWatcher()
    mock_status = LiveStatus(
        username="u1",
        is_live=True,
        room_id="123",
        stream_url="https://example.com/live.flv",
        viewer_count=42,
    )
    with patch("core.watcher.TikTokAPI") as MockAPI:
        inst = MockAPI.return_value
        inst.check_live_status = AsyncMock(return_value=mock_status)
        inst.aclose = AsyncMock()
        result = await watcher.check_account("u1", None, None)

    assert result == {
        "username": "u1",
        "is_live": True,
        "room_id": "123",
        "stream_url": "https://example.com/live.flv",
        "viewer_count": 42,
    }
    inst.check_live_status.assert_awaited_once_with("u1")
    inst.aclose.assert_awaited_once()


@pytest.mark.asyncio
async def test_poll_once_emits_account_live_on_transition_only():
    watcher = AccountWatcher()
    watcher.add_account(7, "broadcaster", auto_record=False)

    live = LiveStatus(
        username="broadcaster",
        is_live=True,
        room_id="999",
        stream_url="https://cdn/stream.flv",
        viewer_count=100,
    )

    with (
        patch("core.watcher.TikTokAPI") as MockAPI,
        patch.object(ws_manager, "broadcast", new_callable=AsyncMock) as broadcast,
    ):
        inst = MockAPI.return_value
        inst.check_live_status = AsyncMock(return_value=live)
        inst.aclose = AsyncMock()

        await watcher._poll_once()

        assert broadcast.await_count >= 2
        types = [c.args[0] for c in broadcast.call_args_list]
        assert "account_live" in types
        assert "account_status" in types
        live_call = next(c for c in broadcast.call_args_list if c.args[0] == "account_live")
        data = live_call.args[1]
        assert data["account_id"] == 7
        assert data["username"] == "broadcaster"
        assert data["room_id"] == "999"
        assert watcher._accounts[7].was_live is True

        broadcast.reset_mock()
        await watcher._poll_once()
        broadcast.assert_awaited_once()
        assert broadcast.call_args[0][0] == "account_status"
        st = broadcast.call_args[0][1]
        assert st["account_id"] == 7
        assert st["is_live"] is True
