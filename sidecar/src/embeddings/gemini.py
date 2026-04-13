from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Literal

import httpx
from google import genai
from google.genai import types

logger = logging.getLogger(__name__)

# Skip very large files to avoid OOM (video cap aligns with Gemini guidance).
_MAX_EMBED_BYTES_IMAGE = 20 * 1024 * 1024
_MAX_EMBED_BYTES_VIDEO = 80 * 1024 * 1024


def _is_embedding_v2(model: str) -> bool:
    return "embedding-2" in model.lower()


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


def _embed_config_for_multimodal(
    *,
    output_dimensionality: int,
    model: str,
) -> types.EmbedContentConfig:
    if _is_embedding_v2(model):
        return types.EmbedContentConfig(output_dimensionality=output_dimensionality)
    return types.EmbedContentConfig(
        output_dimensionality=output_dimensionality,
        task_type="SEMANTIC_SIMILARITY",
    )


def _embed_config_for_text(
    *,
    output_dimensionality: int,
    model: str,
    role: Literal["query", "document"],
    title: str | None,
    raw_text: str,
) -> tuple[str, types.EmbedContentConfig]:
    """Return (contents, config) for embed_content."""
    if _is_embedding_v2(model):
        if role == "query":
            return f"task: search result | query: {raw_text}", types.EmbedContentConfig(
                output_dimensionality=output_dimensionality,
            )
        ti = (title or "").strip() or "none"
        tb = raw_text.strip() or "none"
        return f"title: {ti} | text: {tb}", types.EmbedContentConfig(
            output_dimensionality=output_dimensionality,
        )
    if role == "query":
        return raw_text, types.EmbedContentConfig(
            output_dimensionality=output_dimensionality,
            task_type="RETRIEVAL_QUERY",
        )
    body = raw_text.strip() or "none"
    cfg = types.EmbedContentConfig(
        output_dimensionality=output_dimensionality,
        task_type="RETRIEVAL_DOCUMENT",
    )
    t = (title or "").strip()
    if t:
        cfg.title = t
    return body, cfg


def _embed_text_sync(
    *,
    api_key: str,
    model: str,
    contents: str,
    config: types.EmbedContentConfig,
) -> list[float]:
    client = genai.Client(api_key=api_key)
    result = client.models.embed_content(
        model=model,
        contents=contents,
        config=config,
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
    cfg = _embed_config_for_multimodal(output_dimensionality=output_dimensionality, model=model)
    result = client.models.embed_content(
        model=model,
        contents=[types.Content(parts=parts)],
        config=cfg,
    )
    return _embedding_values_from_response(result)


async def embed_text(
    _http: httpx.AsyncClient,
    *,
    api_key: str,
    model: str,
    text: str,
    output_dimensionality: int,
    role: Literal["query", "document"] = "query",
    title: str | None = None,
) -> list[float]:
    raw = text.strip()
    if role == "query" and not raw:
        msg = "Empty text for embedding"
        raise ValueError(msg)
    if role == "document" and not raw and not (title or "").strip():
        msg = "Empty document for embedding"
        raise ValueError(msg)
    contents, cfg = _embed_config_for_text(
        output_dimensionality=output_dimensionality,
        model=model,
        role=role,
        title=title,
        raw_text=raw if raw else "",
    )
    return await asyncio.to_thread(
        _embed_text_sync,
        api_key=api_key,
        model=model,
        contents=contents,
        config=cfg,
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
