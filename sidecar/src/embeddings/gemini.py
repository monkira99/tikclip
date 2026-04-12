from __future__ import annotations

import asyncio
import logging
from pathlib import Path

import httpx
from google import genai
from google.genai import types

logger = logging.getLogger(__name__)

# Gemini embedding API: semantic similarity aligns with product / clip matching.
_EMBED_TASK_TYPE = "SEMANTIC_SIMILARITY"

# Skip very large files to avoid OOM (video cap aligns with Gemini guidance).
_MAX_EMBED_BYTES_IMAGE = 20 * 1024 * 1024
_MAX_EMBED_BYTES_VIDEO = 80 * 1024 * 1024


def _mime_for_path(path: Path, kind: str) -> str:
    suf = path.suffix.lower()
    if kind == "video":
        if suf == ".mov":
            return "video/quicktime"
        return "video/mp4"
    mapping = {
        ".jpg": "image/jpeg",
        ".jpeg": "image/jpeg",
        ".png": "image/png",
        ".webp": "image/webp",
        ".gif": "image/gif",
    }
    return mapping.get(suf, "application/octet-stream")


def _embedding_values_from_response(resp: types.EmbedContentResponse) -> list[float]:
    embs = resp.embeddings
    if not embs:
        msg = "Gemini embed_content returned no embeddings"
        raise ValueError(msg)
    first = embs[0]
    vals = first.values
    if not vals:
        msg = "Gemini embedding has empty values"
        raise ValueError(msg)
    return [float(x) for x in vals]


def _embed_config(output_dimensionality: int) -> types.EmbedContentConfig:
    return types.EmbedContentConfig(
        output_dimensionality=output_dimensionality,
        task_type=_EMBED_TASK_TYPE,
    )


def _embed_text_sync(
    *,
    api_key: str,
    model: str,
    text: str,
    output_dimensionality: int,
) -> list[float]:
    client = genai.Client(api_key=api_key)
    result = client.models.embed_content(
        model=model,
        contents=text.strip(),
        config=_embed_config(output_dimensionality),
    )
    return _embedding_values_from_response(result)


def _embed_multimodal_sync(
    *,
    api_key: str,
    model: str,
    media_bytes: bytes,
    mime_type: str,
    output_dimensionality: int,
    text: str | None,
) -> list[float]:
    client = genai.Client(api_key=api_key)
    parts: list[types.Part] = []
    if text and text.strip():
        parts.append(types.Part(text=text.strip()))
    parts.append(types.Part.from_bytes(data=media_bytes, mime_type=mime_type))
    result = client.models.embed_content(
        model=model,
        contents=[types.Content(parts=parts)],
        config=_embed_config(output_dimensionality),
    )
    return _embedding_values_from_response(result)


async def embed_text(
    _http: httpx.AsyncClient,
    *,
    api_key: str,
    model: str,
    text: str,
    output_dimensionality: int,
) -> list[float]:
    t = text.strip()
    if not t:
        msg = "Empty text for embedding"
        raise ValueError(msg)
    return await asyncio.to_thread(
        _embed_text_sync,
        api_key=api_key,
        model=model,
        text=t,
        output_dimensionality=output_dimensionality,
    )


async def embed_file(
    _http: httpx.AsyncClient,
    *,
    api_key: str,
    model: str,
    path: Path,
    kind: str,
    output_dimensionality: int,
    product_name: str | None = None,
) -> list[float]:
    if not path.is_file():
        msg = f"Media file not found: {path}"
        raise FileNotFoundError(msg)
    max_bytes = _MAX_EMBED_BYTES_VIDEO if kind == "video" else _MAX_EMBED_BYTES_IMAGE
    size = path.stat().st_size
    if size > max_bytes:
        msg = f"File too large for embedding ({size} bytes): {path}"
        raise ValueError(msg)
    raw = path.read_bytes()
    mime = _mime_for_path(path, kind)
    if mime == "application/octet-stream":
        logger.warning("Unknown media type for %s (kind=%s); Gemini may reject", path, kind)
    label = (product_name or "").strip() or None
    return await asyncio.to_thread(
        _embed_multimodal_sync,
        api_key=api_key,
        model=model,
        media_bytes=raw,
        mime_type=mime,
        output_dimensionality=output_dimensionality,
        text=label,
    )
