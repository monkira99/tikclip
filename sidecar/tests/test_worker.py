import tempfile
from pathlib import Path

from core.worker import RecordingWorker


def test_build_ffmpeg_command_contains_ffmpeg_and_stream_url():
    tmp = Path(tempfile.mkdtemp())
    worker = RecordingWorker(
        recording_id="test-id",
        stream_url="https://example.com/live.flv",
        output_dir=tmp,
        username="user1",
        max_duration_seconds=120,
    )
    worker.file_path = str(tmp / "out.mp4")
    cmd = worker._build_ffmpeg_command()
    assert cmd[0] == "ffmpeg"
    assert "https://example.com/live.flv" in cmd
    assert "-i" in cmd
    assert cmd[cmd.index("-i") + 1] == "https://example.com/live.flv"
    assert "-f" in cmd
    assert cmd[cmd.index("-f") + 1] == "mp4"
    assert "+faststart" in cmd


def test_output_path_is_mp4_under_records():
    tmp = Path(tempfile.mkdtemp())
    worker = RecordingWorker(
        recording_id="rid",
        stream_url="https://example.com/stream",
        output_dir=tmp,
        username="alice",
        max_duration_seconds=60,
    )
    worker.file_path = str(worker._output_file_path())
    assert "records" in worker.file_path.replace("\\", "/")
    assert worker.file_path.endswith(".mp4")


def test_initial_status_pending():
    tmp = Path(tempfile.mkdtemp())
    worker = RecordingWorker(
        recording_id="rid",
        stream_url="https://example.com/stream",
        output_dir=tmp,
        username="u",
        max_duration_seconds=60,
    )
    assert worker.status == "pending"
