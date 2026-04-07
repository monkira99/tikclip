from unittest.mock import AsyncMock, patch

import pytest

from core.recorder import recording_manager
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


@pytest.mark.asyncio
async def test_on_autorecord_segment_end_starts_next_when_still_live():
    watcher = AccountWatcher()
    watcher.add_account(42, "host", auto_record=True)

    live = LiveStatus(
        username="host",
        is_live=True,
        room_id="r1",
        stream_url="https://cdn/next.flv",
        viewer_count=1,
    )

    with (
        patch("core.watcher.TikTokAPI") as MockAPI,
        patch("core.watcher.recording_manager") as rec_mgr,
    ):
        inst = MockAPI.return_value
        inst.check_live_status = AsyncMock(return_value=live)
        inst.aclose = AsyncMock()
        rec_mgr.start_recording = AsyncMock(return_value="new-rec-id")

        await watcher.on_autorecord_segment_end(42)

    rec_mgr.start_recording.assert_awaited_once()
    kwargs = rec_mgr.start_recording.call_args.kwargs
    assert kwargs["account_id"] == 42
    assert kwargs["username"] == "host"
    assert kwargs["stream_url"] == "https://cdn/next.flv"


@pytest.mark.asyncio
async def test_on_autorecord_segment_end_skips_when_not_auto_record():
    watcher = AccountWatcher()
    watcher.add_account(1, "x", auto_record=False)

    with patch("core.watcher.recording_manager") as rec_mgr:
        await watcher.on_autorecord_segment_end(1)

    rec_mgr.start_recording.assert_not_called()


@pytest.mark.asyncio
async def test_on_autorecord_segment_end_skips_when_stream_offline():
    watcher = AccountWatcher()
    watcher.add_account(2, "y", auto_record=True)

    offline = LiveStatus(
        username="y",
        is_live=False,
        room_id=None,
        stream_url=None,
        viewer_count=None,
    )

    with (
        patch("core.watcher.TikTokAPI") as MockAPI,
        patch("core.watcher.recording_manager") as rec_mgr,
    ):
        inst = MockAPI.return_value
        inst.check_live_status = AsyncMock(return_value=offline)
        inst.aclose = AsyncMock()

        await watcher.on_autorecord_segment_end(2)

    rec_mgr.start_recording.assert_not_called()


@pytest.mark.asyncio
async def test_poll_once_skips_tiktok_api_when_account_has_active_recording():
    watcher = AccountWatcher()
    watcher.add_account(99, "recuser", auto_record=False)

    with (
        patch.object(
            recording_manager,
            "has_active_recording_for_account",
            new_callable=AsyncMock,
            return_value=True,
        ),
        patch("core.watcher.TikTokAPI") as MockAPI,
        patch.object(ws_manager, "broadcast", new_callable=AsyncMock) as broadcast,
    ):
        await watcher._poll_once()

    MockAPI.assert_not_called()
    assert watcher._accounts[99].was_live is True
    status_calls = [c for c in broadcast.call_args_list if c.args[0] == "account_status"]
    assert status_calls
    assert status_calls[-1].args[1]["is_live"] is True
