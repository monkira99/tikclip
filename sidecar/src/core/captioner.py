from __future__ import annotations


def _collapse_spaces(value: str) -> str:
    return " ".join(value.split())


def _transcript_snippet(transcript_text: str, max_words: int = 14) -> str:
    words = _collapse_spaces(transcript_text).split(" ")
    kept = [w for w in words if w][:max_words]
    return " ".join(kept)


def generate_caption(username: str, transcript_text: str, clip_title: str) -> str:
    user = _collapse_spaces(username).lstrip("@").strip()
    title = _collapse_spaces(clip_title).strip()
    transcript = _transcript_snippet(transcript_text)

    if not user:
        user = "creator"
    if not title:
        title = "Live clip"
    if not transcript:
        transcript = "Highlights from the stream"

    return f"{title} | {transcript} | @{user}"
