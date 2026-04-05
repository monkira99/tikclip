from pathlib import Path

from core.processor import VideoProcessor


def test_processor_initializes():
    p = VideoProcessor(
        recording_id="rid-1",
        username="alice",
        source_path=Path("/tmp/does-not-exist.mp4"),
        account_id=42,
        clip_min_duration=15,
        clip_max_duration=90,
        scene_threshold=27.0,
        date_str="2026-04-05",
    )
    assert p.recording_id == "rid-1"
    assert p.username == "alice"
    assert p.status == "pending"
    assert p.clips == []
    assert p.progress_percent == 0.0


def test_build_clip_path():
    root = Path("/storage")
    path = VideoProcessor.build_clip_path(root, "user", "2026-04-05", 7)
    assert path == Path("/storage/clips/user/2026-04-05/clip_007.mp4")
