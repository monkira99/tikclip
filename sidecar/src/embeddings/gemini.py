from __future__ import annotations

import base64
import logging
from pathlib import Path
from typing import Any, cast

import httpx

logger = logging.getLogger(__name__)

# Generative Language API (REST). Header auth matches official curl examples.
_GEMINI_BASE = "https://generativelanguage.googleapis.com/v1beta"

# Skip very large files to avoid OOM (video cap aligns with Gemini ~120s guidance).
_MAX_EMBED_BYTES_IMAGE = 20 * 1024 * 1024
_MAX_EMBED_BYTES_VIDEO = 80 * 1024 * 1024


def _model_resource(model: str) -> str:
    m = model.strip()
    if m.startswith("models/"):
        return m
    return f"models/{m}"


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


def _parse_embedding_vector(payload: object) -> list[float]:
    if not isinstance(payload, dict):
        msg = "Gemini embed response: expected JSON object"
        raise ValueError(msg)
    data = cast(dict[str, Any], payload)
    emb = data.get("embedding")
    if isinstance(emb, dict):
        emb_d = cast(dict[str, Any], emb)
        values = emb_d.get("values")
        if isinstance(values, list):
            return [float(x) for x in values]
    # Some clients return a list of embeddings
    embs = data.get("embeddings")
    if isinstance(embs, list) and embs:
        first = embs[0]
        if isinstance(first, dict):
            fd = cast(dict[str, Any], first)
            vals = fd.get("values")
            if isinstance(vals, list):
                return [float(x) for x in vals]
    msg = "Gemini embed response: missing embedding.values"
    raise ValueError(msg)


async def embed_content(
    client: httpx.AsyncClient,
    *,
    api_key: str,
    model: str,
    parts: list[dict[str, object]],
    output_dimensionality: int,
) -> list[float]:
    mid = model.removeprefix("models/")
    url = f"{_GEMINI_BASE}/models/{mid}:embedContent"
    body: dict[str, object] = {
        "model": _model_resource(model),
        "content": {"parts": parts},
        "outputDimensionality": output_dimensionality,
    }
    headers = {
        "Content-Type": "application/json",
        "x-goog-api-key": api_key,
    }
    resp = await client.post(url, headers=headers, json=body, timeout=180.0)
    if resp.status_code >= 400:
        text = resp.text[:2000]
        msg = f"Gemini embedContent HTTP {resp.status_code}: {text}"
        raise ValueError(msg)
    return _parse_embedding_vector(resp.json())


async def embed_text(
    client: httpx.AsyncClient,
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
    return await embed_content(
        client,
        api_key=api_key,
        model=model,
        parts=[{"text": t}],
        output_dimensionality=output_dimensionality,
    )


async def embed_file(
    client: httpx.AsyncClient,
    *,
    api_key: str,
    model: str,
    path: Path,
    kind: str,
    output_dimensionality: int,
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
    b64 = base64.standard_b64encode(raw).decode("ascii")
    mime = _mime_for_path(path, kind)
    if mime == "application/octet-stream":
        logger.warning("Unknown media type for %s (kind=%s); Gemini may reject", path, kind)
    parts: list[dict[str, object]] = [
        {
            "inline_data": {
                "mime_type": mime,
                "data": b64,
            },
        },
    ]
    return await embed_content(
        client,
        api_key=api_key,
        model=model,
        parts=parts,
        output_dimensionality=output_dimensionality,
    )
