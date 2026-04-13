# Hybrid Product Suggest — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add text hybrid search (Gemini dense + BM25 sparse from STT transcript) combined with existing image search for improved product suggestion quality.

**Architecture:** Extend the single `product_media` zvec collection with text dense + sparse vectors. At suggest time, run text hybrid search (RRF reranker) in parallel with image search, then fuse scores with configurable weights. Gemini embeddings use asymmetric task types (RETRIEVAL_DOCUMENT / RETRIEVAL_QUERY for embedding-001, prefix-based for embedding-2).

**Tech Stack:** zvec 0.3.0 (BM25EmbeddingFunction, SPARSE_VECTOR_FP32, RrfReRanker, multi-vector query), Google Gemini Embedding API, FastAPI, Pydantic, Tauri/Rust SQLite settings, React/TypeScript frontend.

---

### Task 1: Update `gemini.py` — role-aware embed_text + model-aware task types

**Files:**
- Modify: `sidecar/src/embeddings/gemini.py`

- [ ] **Step 1: Add model detection helper and role-aware text formatting**

In `sidecar/src/embeddings/gemini.py`, replace the hardcoded `_EMBED_TASK_TYPE` and update `embed_text()` / `embed_file()`:

```python
from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Literal

import httpx
from google import genai
from google.genai import types

logger = logging.getLogger(__name__)

_MAX_EMBED_BYTES_IMAGE = 20 * 1024 * 1024
_MAX_EMBED_BYTES_VIDEO = 80 * 1024 * 1024


def _is_embedding_v2(model: str) -> bool:
    return "embedding-2" in model.lower()


def _format_text_for_role(
    text: str,
    role: Literal["query", "document"],
    model: str,
    *,
    title: str | None = None,
) -> tuple[str, types.EmbedContentConfig | None]:
    """Return (formatted_text, config) based on model generation and role.

    embedding-2: prefix-based formatting, no task_type in config.
    embedding-001: raw text, task_type in config.
    """
    t = text.strip()
    if _is_embedding_v2(model):
        if role == "document":
            ti = (title or "").strip() or "none"
            formatted = f"title: {ti} | text: {t}"
        else:
            formatted = f"task: search result | query: {t}"
        return formatted, None
    task = "RETRIEVAL_DOCUMENT" if role == "document" else "RETRIEVAL_QUERY"
    return t, types.EmbedContentConfig(task_type=task)


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


def _make_embed_config(
    output_dimensionality: int,
    *,
    task_type: str | None = None,
) -> types.EmbedContentConfig:
    kwargs: dict = {"output_dimensionality": output_dimensionality}
    if task_type:
        kwargs["task_type"] = task_type
    return types.EmbedContentConfig(**kwargs)


def _embed_text_sync(
    *,
    api_key: str,
    model: str,
    text: str,
    output_dimensionality: int,
    config_override: types.EmbedContentConfig | None = None,
) -> list[float]:
    client = genai.Client(api_key=api_key)
    cfg = config_override or _make_embed_config(output_dimensionality)
    if cfg.output_dimensionality is None:
        cfg = _make_embed_config(output_dimensionality, task_type=cfg.task_type)
    result = client.models.embed_content(
        model=model,
        contents=text,
        config=cfg,
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
    cfg = _make_embed_config(
        output_dimensionality,
        task_type="SEMANTIC_SIMILARITY" if not _is_embedding_v2(model) else None,
    )
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
    t = text.strip()
    if not t:
        msg = "Empty text for embedding"
        raise ValueError(msg)
    formatted, role_config = _format_text_for_role(t, role, model, title=title)
    cfg = role_config
    if cfg and cfg.output_dimensionality is None:
        cfg = _make_embed_config(output_dimensionality, task_type=cfg.task_type)
    return await asyncio.to_thread(
        _embed_text_sync,
        api_key=api_key,
        model=model,
        text=formatted,
        output_dimensionality=output_dimensionality,
        config_override=cfg,
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
```

- [ ] **Step 2: Verify existing callers still compile**

Run: `cd sidecar && uv run ruff check src/embeddings/gemini.py`
Expected: no errors (existing callers of `embed_text` pass no `role` → default `"query"` is backward-compatible; `embed_file` signature unchanged)

- [ ] **Step 3: Commit**

```bash
git add sidecar/src/embeddings/gemini.py
git commit -m "feat(embeddings): role-aware Gemini embed_text with asymmetric task types"
```

---

### Task 2: Add new settings — suggest fusion weights

**Files:**
- Modify: `sidecar/src/config.py`
- Modify: `src-tauri/src/sidecar_env.rs`

- [ ] **Step 1: Add settings to sidecar config**

In `sidecar/src/config.py`, add after the `auto_tag_clip_max_score` line:

```python
    # Weighted score fusion for hybrid suggest (image + text from STT transcript).
    suggest_weight_image: float = 0.6
    suggest_weight_text: float = 0.4
```

- [ ] **Step 2: Forward settings through Tauri sidecar env**

In `src-tauri/src/sidecar_env.rs`, add before the `push_bool_setting` for `audio_processing_enabled` block (around line 207):

```rust
    push_float_if_valid(
        &mut env,
        conn,
        "suggest_weight_image",
        "TIKCLIP_SUGGEST_WEIGHT_IMAGE",
    )?;
    push_float_if_valid(
        &mut env,
        conn,
        "suggest_weight_text",
        "TIKCLIP_SUGGEST_WEIGHT_TEXT",
    )?;
```

- [ ] **Step 3: Verify**

Run: `cd sidecar && uv run ruff check src/config.py`
Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings`
Expected: both pass

- [ ] **Step 4: Commit**

```bash
git add sidecar/src/config.py src-tauri/src/sidecar_env.rs
git commit -m "feat(config): add suggest_weight_image/text fusion settings"
```

---

### Task 3: Extend zvec schema + BM25 corpus management + text index/search

**Files:**
- Modify: `sidecar/src/embeddings/product_vector.py`

This is the largest task. It extends the zvec schema with text vectors, adds BM25 corpus caching, text indexing, and text hybrid search.

- [ ] **Step 1: Extend schema with new fields and vectors**

In `sidecar/src/embeddings/product_vector.py`, replace `_build_schema`:

```python
def _build_schema(dim: int) -> zvec.CollectionSchema:
    return zvec.CollectionSchema(
        name="product_media",
        fields=[
            zvec.FieldSchema(
                "product_id",
                zvec.DataType.INT64,
                nullable=False,
                index_param=zvec.InvertIndexParam(enable_range_optimization=True),
            ),
            zvec.FieldSchema("image_path", zvec.DataType.STRING, nullable=False),
            zvec.FieldSchema("source_url", zvec.DataType.STRING, nullable=True),
            zvec.FieldSchema("product_name", zvec.DataType.STRING, nullable=True),
            zvec.FieldSchema("modality", zvec.DataType.STRING, nullable=False),
            zvec.FieldSchema("product_text", zvec.DataType.STRING, nullable=True),
            zvec.FieldSchema("product_description", zvec.DataType.STRING, nullable=True),
        ],
        vectors=[
            zvec.VectorSchema(
                "embedding",
                zvec.DataType.VECTOR_FP32,
                dim,
                index_param=zvec.HnswIndexParam(metric_type=zvec.MetricType.COSINE),
            ),
            zvec.VectorSchema(
                "text_dense",
                zvec.DataType.VECTOR_FP32,
                dim,
                index_param=zvec.HnswIndexParam(metric_type=zvec.MetricType.COSINE),
            ),
            zvec.VectorSchema(
                "text_sparse",
                zvec.DataType.SPARSE_VECTOR_FP32,
            ),
        ],
    )
```

- [ ] **Step 2: Add BM25 corpus cache module-level state**

Add after the existing `_coll_lock` / `_coll_cache` declarations:

```python
_bm25_lock = threading.Lock()
_bm25_doc: zvec.BM25EmbeddingFunction | None = None
_bm25_query: zvec.BM25EmbeddingFunction | None = None
_bm25_corpus_size: int = 0


def _load_corpus_texts(coll: zvec.Collection) -> list[str]:
    """Load all product_text values from text docs in the collection."""
    docs = coll.query(
        filter="product_text IS NOT NULL",
        topk=10000,
        output_fields=["product_text"],
    )
    return [
        str(d.fields["product_text"])
        for d in docs
        if d.fields and d.fields.get("product_text")
    ]


def _rebuild_bm25(coll: zvec.Collection) -> None:
    global _bm25_doc, _bm25_query, _bm25_corpus_size  # noqa: PLW0603
    corpus = _load_corpus_texts(coll)
    if not corpus:
        _bm25_doc = None
        _bm25_query = None
        _bm25_corpus_size = 0
        return
    _bm25_doc = zvec.BM25EmbeddingFunction(
        corpus=corpus, encoding_type="document", language="en",
    )
    _bm25_query = zvec.BM25EmbeddingFunction(
        corpus=corpus, encoding_type="query", language="en",
    )
    _bm25_corpus_size = len(corpus)


def invalidate_bm25_cache() -> None:
    global _bm25_doc, _bm25_query, _bm25_corpus_size  # noqa: PLW0603
    with _bm25_lock:
        _bm25_doc = None
        _bm25_query = None
        _bm25_corpus_size = 0


def ensure_bm25_ready(coll: zvec.Collection) -> tuple[zvec.BM25EmbeddingFunction, zvec.BM25EmbeddingFunction]:
    """Return (bm25_doc, bm25_query), rebuilding if stale."""
    with _bm25_lock:
        if _bm25_doc is not None and _bm25_query is not None:
            return _bm25_doc, _bm25_query
        _rebuild_bm25(coll)
        if _bm25_doc is None or _bm25_query is None:
            msg = "No product text indexed yet; cannot build BM25 corpus"
            raise RuntimeError(msg)
        return _bm25_doc, _bm25_query
```

- [ ] **Step 3: Add text indexing function**

Add after the existing `index_product_media` function:

```python
async def index_product_text(
    *,
    product_id: int,
    product_name: str,
    product_description: str,
    http: httpx.AsyncClient,
) -> IndexSummary:
    if not settings.product_vector_enabled:
        return IndexSummary(message="Product vector indexing is disabled in settings")
    if not settings.gemini_api_key:
        return IndexSummary(message="Gemini API key is not configured")

    name = (product_name or "").strip()
    desc = (product_description or "").strip()
    if not name and not desc:
        return IndexSummary(message="No product name or description to index")

    try:
        coll = await asyncio.to_thread(get_or_open_collection_sync)
    except (RuntimeError, ValueError) as exc:
        return IndexSummary(errors=[str(exc)], message=str(exc))

    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    raw_text = f"{name} {desc}".strip()

    try:
        dense_vec = await gemini.embed_text(
            http,
            api_key=api_key,
            model=model,
            text=desc if desc else name,
            output_dimensionality=dim,
            role="document",
            title=name or None,
        )
    except (OSError, ValueError) as exc:
        return IndexSummary(errors=[str(exc)], message="Dense text embedding failed")

    if len(dense_vec) != dim:
        msg = f"Text embedding length {len(dense_vec)} != configured {dim}"
        return IndexSummary(errors=[msg], message=msg)

    def _build_sparse_and_upsert() -> None:
        bm25_doc, _ = ensure_bm25_ready(coll)
        sparse_vec = bm25_doc.embed(raw_text)

        doc_id = f"t{product_id}"
        coll.delete_by_filter(f"product_id = {int(product_id)} AND modality = 'text'")
        coll.upsert([
            zvec.Doc(
                id=doc_id,
                vectors={
                    "text_dense": dense_vec,
                    "text_sparse": sparse_vec,
                },
                fields={
                    "product_id": int(product_id),
                    "image_path": "",
                    "product_name": name or None,
                    "product_description": desc or None,
                    "product_text": raw_text,
                    "modality": "text",
                },
            ),
        ])
        coll.flush()
        invalidate_bm25_cache()

    try:
        await asyncio.to_thread(_build_sparse_and_upsert)
    except Exception as exc:
        logger.exception("zvec text upsert failed for product %s", product_id)
        return IndexSummary(errors=[str(exc)], message="Text vector upsert failed")

    return IndexSummary(indexed=1)
```

- [ ] **Step 4: Add text hybrid search function**

Add after `search_by_media_path`:

```python
async def search_by_transcript(
    *,
    transcript: str,
    top_k: int,
    http: httpx.AsyncClient,
) -> list[SearchHit]:
    if not settings.product_vector_enabled:
        msg = "Product vector search is disabled in settings"
        raise ValueError(msg)
    if not settings.gemini_api_key:
        msg = "Gemini API key is not configured"
        raise ValueError(msg)

    t = transcript.strip()
    if not t:
        return []

    coll = await asyncio.to_thread(get_or_open_collection_for_query_sync)
    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    dense_vec = await gemini.embed_text(
        http,
        api_key=api_key,
        model=model,
        text=t,
        output_dimensionality=dim,
        role="query",
    )
    if len(dense_vec) != dim:
        msg = f"Query embedding length {len(dense_vec)} != configured {dim}"
        raise ValueError(msg)

    def _run_hybrid() -> list[zvec.Doc]:
        _, bm25_q = ensure_bm25_ready(coll)
        sparse_vec = bm25_q.embed(t)
        return coll.query(
            vectors=[
                zvec.VectorQuery("text_dense", vector=dense_vec),
                zvec.VectorQuery("text_sparse", vector=sparse_vec),
            ],
            reranker=zvec.RrfReRanker(topn=top_k),
            filter="product_text IS NOT NULL",
            topk=top_k,
            output_fields=[
                "product_id",
                "image_path",
                "source_url",
                "product_name",
                "modality",
            ],
        )

    raw = await asyncio.to_thread(_run_hybrid)
    out: list[SearchHit] = []
    for doc in raw:
        fields = doc.fields or {}
        pid = fields.get("product_id")
        if pid is None:
            continue
        score = float(doc.score) if doc.score is not None else 0.0
        out.append(
            SearchHit(
                product_id=int(pid),
                score=score,
                image_path=str(fields.get("image_path") or ""),
                source_url=(str(fields["source_url"]) if fields.get("source_url") else None),
                product_name=(str(fields["product_name"]) if fields.get("product_name") else None),
                modality=(str(fields["modality"]) if fields.get("modality") else None),
            ),
        )
    return out
```

- [ ] **Step 5: Update delete to also remove text doc and invalidate BM25**

In `delete_vectors_for_product_sync`, after the existing `coll.delete_by_filter(f"product_id = {int(product_id)}")` line, add BM25 invalidation. The filter already deletes ALL docs for that product_id (including text docs), so just add:

```python
    invalidate_bm25_cache()
```

after `coll.flush()` inside the try block.

- [ ] **Step 6: Verify**

Run: `cd sidecar && uv run ruff check src/embeddings/product_vector.py`
Run: `cd sidecar && uv run ruff format --check src/embeddings/product_vector.py`
Expected: both pass

- [ ] **Step 7: Commit**

```bash
git add sidecar/src/embeddings/product_vector.py
git commit -m "feat(embeddings): extend zvec schema with text vectors, BM25 corpus, hybrid search"
```

---

### Task 4: Update schemas + route — add transcript_text to suggest request

**Files:**
- Modify: `sidecar/src/models/schemas.py`
- Modify: `sidecar/src/routes/clips.py`
- Modify: `sidecar/src/routes/products.py`

- [ ] **Step 1: Update ClipSuggestProductRequest schema**

In `sidecar/src/models/schemas.py`, add `transcript_text` to `ClipSuggestProductRequest`:

```python
class ClipSuggestProductRequest(BaseModel):
    video_path: str
    thumbnail_path: str | None = None
    transcript_text: str | None = None
```

- [ ] **Step 2: Add text search fields to ClipSuggestProductResponse**

In `sidecar/src/models/schemas.py`, add to `ClipSuggestProductResponse`:

```python
class ClipSuggestTextHit(BaseModel):
    product_id: int
    score: float
    product_name: str | None = None
```

And add these fields to `ClipSuggestProductResponse`:

```python
    text_search_hits: list[ClipSuggestTextHit] = Field(default_factory=list)
    text_search_used: bool = False
    fusion_method: str | None = None
    suggest_weight_image: float = 0.6
    suggest_weight_text: float = 0.4
```

- [ ] **Step 3: Update IndexProductEmbeddingsRequest to include description**

In `sidecar/src/models/schemas.py`, add `product_description` to `IndexProductEmbeddingsRequest`:

```python
class IndexProductEmbeddingsRequest(BaseModel):
    product_id: int = Field(ge=1)
    product_name: str = ""
    product_description: str = ""
    items: list[ProductEmbeddingMediaItem] = Field(default_factory=list)
```

- [ ] **Step 4: Update clips route to pass transcript_text**

In `sidecar/src/routes/clips.py`, update `suggest_product_for_clip_route`:

```python
@router.post(
    "/api/clips/suggest-product",
    response_model=ClipSuggestProductResponse,
)
async def suggest_product_for_clip_route(body: ClipSuggestProductRequest):
    video = body.video_path.strip()
    if not video:
        raise HTTPException(status_code=400, detail="video_path is required")
    thumb_raw = body.thumbnail_path
    thumb_s = thumb_raw.strip() if thumb_raw else ""
    transcript = body.transcript_text
    transcript_s = transcript.strip() if transcript else ""
    logger.debug(
        "suggest-product start video=%s thumb=%s transcript_len=%s",
        video[:120],
        (thumb_s[:120] if thumb_s else ""),
        len(transcript_s) if transcript_s else 0,
    )
    async with httpx.AsyncClient() as client:
        result = await suggest_product_for_clip(
            video_path=video,
            thumbnail_path=(thumb_s if thumb_s else None),
            transcript_text=(transcript_s if transcript_s else None),
            http=client,
        )
    logger.debug(
        "suggest-product done matched=%s product_id=%s score=%s frames=%s text_used=%s skip=%r",
        result.matched,
        result.product_id,
        result.best_score,
        result.frames_used,
        result.text_search_used,
        result.skipped_reason,
    )
    return result
```

- [ ] **Step 5: Update products route to index text after media**

In `sidecar/src/routes/products.py`, add import and update `index_product_embeddings`:

Add to imports:
```python
from embeddings.product_vector import (
    MediaIndexItem,
    delete_vectors_for_product_sync,
    index_product_media,
    index_product_text,
    search_by_media_path,
    search_by_text,
)
```

Update the route handler:
```python
@router.post(
    "/api/products/embeddings/index",
    response_model=IndexProductEmbeddingsResponse,
)
async def index_product_embeddings(body: IndexProductEmbeddingsRequest):
    logger.debug(
        "embeddings/index product_id=%s items=%s desc_len=%s",
        body.product_id,
        len(body.items),
        len(body.product_description) if body.product_description else 0,
    )
    items = [MediaIndexItem.model_validate(x.model_dump()) for x in body.items]
    async with httpx.AsyncClient() as client:
        summary = await index_product_media(
            product_id=body.product_id,
            product_name=body.product_name,
            items=items,
            http=client,
        )
        if body.product_name or body.product_description:
            text_summary = await index_product_text(
                product_id=body.product_id,
                product_name=body.product_name,
                product_description=body.product_description,
                http=client,
            )
            summary.indexed += text_summary.indexed
            summary.errors.extend(text_summary.errors)
            if text_summary.message and not summary.message:
                summary.message = text_summary.message
    logger.debug(
        "embeddings/index done product_id=%s indexed=%s skipped=%s errors=%s msg=%r",
        body.product_id,
        summary.indexed,
        summary.skipped,
        len(summary.errors),
        summary.message,
    )
    return IndexProductEmbeddingsResponse(
        indexed=summary.indexed,
        skipped=summary.skipped,
        errors=summary.errors,
        message=summary.message,
    )
```

- [ ] **Step 6: Verify**

Run: `cd sidecar && uv run ruff check src/models/schemas.py src/routes/clips.py src/routes/products.py`
Run: `cd sidecar && uv run ruff format --check src/models/schemas.py src/routes/clips.py src/routes/products.py`
Expected: both pass

- [ ] **Step 7: Commit**

```bash
git add sidecar/src/models/schemas.py sidecar/src/routes/clips.py sidecar/src/routes/products.py
git commit -m "feat(routes): pass transcript_text to suggest, index product description as text"
```

---

### Task 5: Update `clip_product_suggest.py` — text hybrid search + weighted fusion

**Files:**
- Modify: `sidecar/src/embeddings/clip_product_suggest.py`

- [ ] **Step 1: Add text search + fusion logic**

Replace the full content of `sidecar/src/embeddings/clip_product_suggest.py`:

```python
"""Match a new clip to a catalog product via frame embeddings + text hybrid search + weighted fusion."""

from __future__ import annotations

import logging
from collections import Counter
from pathlib import Path
from uuid import uuid4

import httpx

from config import settings
from embeddings.product_vector import (
    SearchHit,
    resolve_storage_media_path,
    search_by_media_path,
    search_by_transcript,
)
from embeddings.video_frames import cleanup_work_dir, extract_frames_evenly
from models.schemas import (
    ClipSuggestFrameRow,
    ClipSuggestProductResponse,
    ClipSuggestTextHit,
    ClipSuggestVoteRow,
)

logger = logging.getLogger(__name__)


def _config_fields() -> dict:
    return {
        "config_target_extracted_frames": max(1, min(12, settings.auto_tag_clip_frame_count)),
        "config_max_score_threshold": float(settings.auto_tag_clip_max_score),
        "suggest_weight_image": float(settings.suggest_weight_image),
        "suggest_weight_text": float(settings.suggest_weight_text),
    }


def _storage_relative(path: Path) -> str:
    root = settings.storage_path.resolve()
    try:
        return str(path.resolve().relative_to(root))
    except ValueError:
        return path.name


def _enabled() -> tuple[bool, str | None]:
    if not settings.auto_tag_clip_product_enabled:
        return False, "auto_tag_clip_product_enabled is off"
    if not settings.product_vector_enabled:
        return False, "product_vector_enabled is off"
    if not settings.gemini_api_key:
        return False, "Gemini API key is not configured"
    return True, None


async def _run_text_search(
    transcript: str,
    http: httpx.AsyncClient,
) -> list[SearchHit]:
    try:
        return await search_by_transcript(transcript=transcript, top_k=5, http=http)
    except (RuntimeError, ValueError) as exc:
        logger.debug("text hybrid search skipped: %s", exc)
        return []


async def _run_image_search(
    video: Path,
    video_rel: str,
    thumbnail_path: str | None,
    http: httpx.AsyncClient,
) -> tuple[
    list[ClipSuggestFrameRow],
    list[tuple[int, float, str | None]],
    int,
    int,
    bool,
    list[Path],
    Path | None,
]:
    """Run image-based search on extracted frames. Returns frame_rows, top1, frames_searched, extracted_count, thumb_included, frame_paths, work_dir."""
    n = max(1, min(12, settings.auto_tag_clip_frame_count))
    frame_paths: list[Path] = []
    work_dir: Path | None = None
    thumb_included = False

    if thumbnail_path:
        try:
            thumb = resolve_storage_media_path(thumbnail_path)
            if thumb.is_file():
                frame_paths.append(thumb)
                thumb_included = True
        except (OSError, ValueError):
            pass

    work_dir = settings.storage_path / "tmp" / "clip_frames" / str(uuid4())
    extracted = extract_frames_evenly(video, n, work_dir)
    frame_paths.extend(extracted)

    frame_rows: list[ClipSuggestFrameRow] = []
    top1: list[tuple[int, float, str | None]] = []
    frames_searched = 0

    for i, fp in enumerate(frame_paths):
        is_thumb = thumb_included and i == 0
        src: str = "thumbnail" if is_thumb else "extracted"
        rel = _storage_relative(fp)
        try:
            hits = await search_by_media_path(
                media_path=str(fp), kind="image", top_k=1, http=http,
            )
        except (OSError, ValueError, FileNotFoundError) as exc:
            logger.debug("frame search skip %s: %s", fp, exc)
            frame_rows.append(ClipSuggestFrameRow(
                index=i, source=src, media_relative_path=rel, outcome="error", error=str(exc),
            ))
            continue

        frames_searched += 1
        if not hits:
            frame_rows.append(ClipSuggestFrameRow(
                index=i, source=src, media_relative_path=rel, outcome="no_hit",
            ))
            continue

        h = hits[0]
        top1.append((h.product_id, h.score, h.product_name))
        frame_rows.append(ClipSuggestFrameRow(
            index=i, source=src, media_relative_path=rel, outcome="hit",
            top_product_id=h.product_id, top_score=h.score, top_product_name=h.product_name,
        ))

    return frame_rows, top1, frames_searched, len(extracted), thumb_included, frame_paths, work_dir


def _image_vote_results(
    top1: list[tuple[int, float, str | None]],
) -> dict[int, float]:
    """Aggregate image search results by product. Returns {product_id: best_score}."""
    if not top1:
        return {}
    scores_by_pid: dict[int, list[float]] = {}
    for pid, score, _ in top1:
        scores_by_pid.setdefault(pid, []).append(score)
    return {pid: min(scores) for pid, scores in scores_by_pid.items()}


def _fuse_scores(
    image_scores: dict[int, float],
    text_hits: list[SearchHit],
    w_image: float,
    w_text: float,
) -> list[tuple[int, float, str | None]]:
    """Weighted score fusion. Returns sorted [(product_id, final_score, product_name)]."""
    norm_image: dict[int, float] = {}
    if image_scores:
        max_img = max(image_scores.values()) or 1.0
        norm_image = {pid: 1.0 - (s / max_img) for pid, s in image_scores.items()}

    norm_text: dict[int, float] = {}
    text_names: dict[int, str | None] = {}
    if text_hits:
        max_txt = max(h.score for h in text_hits) or 1.0
        for h in text_hits:
            norm_text[h.product_id] = h.score / max_txt
            text_names[h.product_id] = h.product_name

    all_pids = set(norm_image.keys()) | set(norm_text.keys())
    if not all_pids:
        return []

    results: list[tuple[int, float, str | None]] = []
    for pid in all_pids:
        img = norm_image.get(pid, 0.0)
        txt = norm_text.get(pid, 0.0)
        final = w_image * img + w_text * txt
        name = text_names.get(pid)
        results.append((pid, final, name))

    results.sort(key=lambda x: x[1], reverse=True)
    return results


async def suggest_product_for_clip(
    *,
    video_path: str,
    thumbnail_path: str | None,
    transcript_text: str | None = None,
    http: httpx.AsyncClient,
) -> ClipSuggestProductResponse:
    base = _config_fields()
    ok, reason = _enabled()
    if not ok:
        logger.debug("suggest_product_for_clip skip: %s", reason)
        return ClipSuggestProductResponse(skipped_reason=reason, **base)

    try:
        video = resolve_storage_media_path(video_path)
    except (OSError, ValueError) as exc:
        logger.debug("suggest_product_for_clip skip resolve video: %s", exc)
        return ClipSuggestProductResponse(skipped_reason=str(exc), **base)

    video_rel = _storage_relative(video)
    if not video.is_file():
        return ClipSuggestProductResponse(
            skipped_reason="clip video file not found", video_relative_path=video_rel, **base,
        )

    work_dir: Path | None = None
    try:
        # --- Step 1: Text hybrid search (if transcript available) ---
        text_hits: list[SearchHit] = []
        text_search_used = False
        transcript_s = (transcript_text or "").strip()
        if transcript_s:
            text_hits = await _run_text_search(transcript_s, http)
            text_search_used = len(text_hits) > 0

        # --- Step 2: Image search (existing flow) ---
        (
            frame_rows, top1, frames_searched, extracted_count,
            thumb_included, _frame_paths, work_dir,
        ) = await _run_image_search(video, video_rel, thumbnail_path, http)

        text_hit_rows = [
            ClipSuggestTextHit(
                product_id=h.product_id, score=h.score, product_name=h.product_name,
            )
            for h in text_hits
        ]

        if not top1 and not text_hits:
            return ClipSuggestProductResponse(
                frames_used=len(_frame_paths),
                frames_searched=frames_searched,
                skipped_reason="no vector hits from frames or text",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=extracted_count,
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=text_search_used,
                **base,
            )

        # --- Step 3: Weighted score fusion ---
        w_image = settings.suggest_weight_image
        w_text = settings.suggest_weight_text

        image_scores = _image_vote_results(top1)
        fused = _fuse_scores(image_scores, text_hits, w_image, w_text)

        if not fused:
            return ClipSuggestProductResponse(
                frames_used=len(_frame_paths),
                frames_searched=frames_searched,
                skipped_reason="fusion produced no candidates",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=extracted_count,
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=text_search_used,
                **base,
            )

        winner_pid, win_score, win_name = fused[0]

        # Build vote rows from image results for backward compat
        counts = Counter(pid for pid, _, _ in top1)
        votes_by_product = [
            ClipSuggestVoteRow(product_id=pid, vote_count=cnt) for pid, cnt in counts.most_common()
        ]

        pick = "weighted_fusion" if text_search_used else (
            "majority_vote" if counts and counts.most_common(1)[0][1] >= (len(top1) + 1) // 2
            else "min_distance_tiebreak"
        )

        # If no text used, apply old threshold logic on raw image score
        if not text_search_used and image_scores:
            raw_best = min(image_scores.values())
            max_dist = settings.auto_tag_clip_max_score
            if raw_best > max_dist:
                return ClipSuggestProductResponse(
                    frames_used=len(_frame_paths),
                    frames_searched=frames_searched,
                    skipped_reason=f"best match distance {raw_best:.4f} above threshold {max_dist:.4f}",
                    video_relative_path=video_rel,
                    thumbnail_used=thumb_included,
                    extracted_frame_count=extracted_count,
                    pick_method=pick,
                    votes_by_product=votes_by_product,
                    candidate_product_id=winner_pid,
                    candidate_product_name=win_name,
                    candidate_score=raw_best,
                    frame_rows=frame_rows,
                    text_search_hits=text_hit_rows,
                    text_search_used=text_search_used,
                    **base,
                )

        logger.debug(
            "suggest_product_for_clip match pid=%s score=%.4f pick=%s text_used=%s",
            winner_pid, win_score, pick, text_search_used,
        )
        return ClipSuggestProductResponse(
            matched=True,
            product_id=winner_pid,
            product_name=win_name,
            best_score=win_score,
            frames_used=len(_frame_paths),
            frames_searched=frames_searched,
            video_relative_path=video_rel,
            thumbnail_used=thumb_included,
            extracted_frame_count=extracted_count,
            pick_method=pick,
            votes_by_product=votes_by_product,
            frame_rows=frame_rows,
            text_search_hits=text_hit_rows,
            text_search_used=text_search_used,
            fusion_method="weighted_score" if text_search_used else None,
            **base,
        )
    finally:
        if work_dir is not None:
            cleanup_work_dir(work_dir)
```

- [ ] **Step 2: Verify**

Run: `cd sidecar && uv run ruff check src/embeddings/clip_product_suggest.py`
Run: `cd sidecar && uv run ruff format --check src/embeddings/clip_product_suggest.py`
Expected: both pass

- [ ] **Step 3: Commit**

```bash
git add sidecar/src/embeddings/clip_product_suggest.py
git commit -m "feat(suggest): hybrid text+image search with weighted score fusion"
```

---

### Task 6: Update frontend — pass transcript_text + product_description

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/components/layout/app-shell.tsx`
- Modify: `src/components/products/product-form.tsx`

- [ ] **Step 1: Update suggestProductForClip to accept transcript_text**

In `src/lib/api.ts`, update the function signature and body:

```typescript
export async function suggestProductForClip(body: {
  video_path: string;
  thumbnail_path?: string | null;
  transcript_text?: string | null;
}): Promise<ClipSuggestProductResult> {
  return sidecarJson<ClipSuggestProductResult>("/api/clips/suggest-product", {
    method: "POST",
    body: JSON.stringify({
      video_path: body.video_path,
      thumbnail_path: body.thumbnail_path ?? null,
      transcript_text: body.transcript_text ?? null,
    }),
  });
}
```

- [ ] **Step 2: Update indexProductEmbeddings to send description**

In `src/lib/api.ts`, update the function:

```typescript
export async function indexProductEmbeddings(
  productId: number,
  body: { product_name: string; product_description?: string; items: ProductEmbeddingMediaItem[] },
): Promise<IndexProductEmbeddingsResult> {
  return sidecarJson<IndexProductEmbeddingsResult>("/api/products/embeddings/index", {
    method: "POST",
    body: JSON.stringify({
      product_id: productId,
      product_name: body.product_name,
      product_description: body.product_description ?? "",
      items: body.items.map((x) => ({
        kind: x.kind,
        path: x.path,
        source_url: x.source_url ?? "",
      })),
    }),
  });
}
```

- [ ] **Step 3: Update app-shell to pass transcript_text**

In `src/components/layout/app-shell.tsx`, update `maybeAutoTagClipAfterInsert`:

```typescript
    const transcriptText =
      typeof data.transcript_text === "string" && data.transcript_text.trim() !== ""
        ? data.transcript_text
        : null;
    const res = await api.suggestProductForClip({
      video_path: videoPath,
      thumbnail_path: thumbnailPath,
      transcript_text: transcriptText,
    });
```

- [ ] **Step 4: Update product-form to send description**

In `src/components/products/product-form.tsx`, find the `indexProductEmbeddings` call and add `product_description`:

The `form.description` is already available in scope. Update the call:

```typescript
void indexProductEmbeddings(savedId, {
  product_name: name,
  product_description: form.description ?? "",
  items,
}).catch(() => {
  /* optional: indexing disabled or sidecar error */
});
```

- [ ] **Step 5: Verify**

Run: `npm run lint:js` (from repo root)
Expected: pass

- [ ] **Step 6: Commit**

```bash
git add src/lib/api.ts src/components/layout/app-shell.tsx src/components/products/product-form.tsx
git commit -m "feat(frontend): pass transcript_text to suggest, send product description for text indexing"
```

---

### Task 7: Install dashtext dependency for BM25

**Files:**
- Modify: `sidecar/pyproject.toml`

- [ ] **Step 1: Add dashtext dependency**

zvec `BM25EmbeddingFunction` requires `dashtext`. Add it:

```bash
cd sidecar && uv add dashtext
```

- [ ] **Step 2: Verify import works**

```bash
cd sidecar && uv run python -c "import dashtext; print('dashtext OK')"
```

Expected: `dashtext OK`

- [ ] **Step 3: Commit**

```bash
git add sidecar/pyproject.toml sidecar/uv.lock
git commit -m "deps(sidecar): add dashtext for zvec BM25EmbeddingFunction"
```

---

### Task 8: Full integration verify

**Files:** None (verification only)

- [ ] **Step 1: Sidecar lint + format + type check**

```bash
cd sidecar && uv run ruff check src tests
cd sidecar && uv run ruff format --check src tests
cd sidecar && uv run ty check .
```

Expected: all pass

- [ ] **Step 2: Sidecar tests**

```bash
cd sidecar && uv run pytest tests/ -q
```

Expected: all pass (existing tests should not break; new code paths are additive with fallback)

- [ ] **Step 3: Frontend lint + build**

```bash
npm run lint:js
```

Expected: pass

- [ ] **Step 4: Rust check**

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Expected: pass

- [ ] **Step 5: Final commit (if any fixups needed)**

```bash
git add -A && git commit -m "fix: lint/format fixups for hybrid product suggest"
```
