from __future__ import annotations

import asyncio
import logging
import threading
from pathlib import Path
from typing import Any, Literal

import httpx
import zvec
from pydantic import BaseModel

from config import settings
from embeddings import gemini

logger = logging.getLogger(__name__)

VECTOR_SUBDIR = Path("vector") / "product_media"

_coll_lock = threading.Lock()
_coll_cache: dict[str, zvec.Collection] = {}

# Recommended in zvec docs: mmap for local collections; read-write for upsert/delete.
_COLLECTION_RW = zvec.CollectionOption(read_only=False, enable_mmap=True)


class MediaIndexItem(BaseModel):
    kind: Literal["image", "video"]
    path: str
    source_url: str = ""


def _vector_root() -> Path:
    return (settings.storage_path / VECTOR_SUBDIR).resolve()


def _cache_key(path_str: str, dim: int) -> str:
    return f"{path_str}\0{dim}"


def _embedding_dimension_from_schema(schema: Any) -> int:
    vecs = schema.vectors
    if isinstance(vecs, list) and vecs:
        return int(vecs[0].dimension)
    msg = "zvec schema has no vector field"
    raise RuntimeError(msg)


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
        ],
        vectors=zvec.VectorSchema(
            "embedding",
            zvec.DataType.VECTOR_FP32,
            dim,
            index_param=zvec.HnswIndexParam(metric_type=zvec.MetricType.COSINE),
        ),
    )


def resolve_storage_media_path(raw: str) -> Path:
    storage = settings.storage_path.resolve()
    p = Path(raw).expanduser()
    p = (storage / p).resolve() if not p.is_absolute() else p.resolve()
    try:
        p.relative_to(storage)
    except ValueError as exc:
        msg = "Media path must be under storage root"
        raise ValueError(msg) from exc
    return p


def get_or_open_collection_sync() -> zvec.Collection:
    if not settings.product_vector_enabled:
        msg = "Product vector indexing is disabled in settings"
        raise RuntimeError(msg)
    if not settings.gemini_api_key:
        msg = "Gemini API key is not configured"
        raise RuntimeError(msg)

    dim = settings.gemini_embedding_dimensions
    if dim < 1 or dim > 8192:
        msg = "gemini_embedding_dimensions must be between 1 and 8192"
        raise ValueError(msg)

    root = _vector_root()
    path_str = str(root)
    key = _cache_key(path_str, dim)

    with _coll_lock:
        cached = _coll_cache.get(key)
        if cached is not None:
            return cached

        root.parent.mkdir(parents=True, exist_ok=True)

        if root.exists() and any(root.iterdir()):
            coll = zvec.open(path_str, option=_COLLECTION_RW)
            got = _embedding_dimension_from_schema(coll.schema)
            if got != dim:
                msg = (
                    f"zvec collection dimension is {got} but settings request {dim}; "
                    "delete the vector folder or match gemini_embedding_dimensions."
                )
                raise ValueError(msg)
        else:
            if root.exists() and not any(root.iterdir()):
                try:
                    root.rmdir()
                except OSError:
                    pass
            coll = zvec.create_and_open(
                path_str,
                _build_schema(dim),
                option=_COLLECTION_RW,
            )

        _coll_cache[key] = coll
        return coll


def delete_vectors_for_product_sync(product_id: int) -> None:
    if not settings.product_vector_enabled:
        return
    root = _vector_root()
    if not root.exists() or not any(root.iterdir()):
        return
    path_str = str(root)
    dim = settings.gemini_embedding_dimensions
    key = _cache_key(path_str, dim)
    with _coll_lock:
        coll = _coll_cache.get(key)
    if coll is None:
        try:
            coll = zvec.open(path_str, option=_COLLECTION_RW)
        except Exception as exc:
            logger.debug("skip vector delete: could not open zvec collection: %s", exc)
            return
    try:
        coll.delete_by_filter(f"product_id = {int(product_id)}")
        coll.flush()
    except Exception as exc:
        logger.warning("vector delete failed for product_id=%s: %s", product_id, exc)


class IndexSummary(BaseModel):
    indexed: int = 0
    skipped: int = 0
    errors: list[str] = []
    message: str | None = None


async def index_product_media(
    *,
    product_id: int,
    product_name: str,
    items: list[MediaIndexItem],
    http: httpx.AsyncClient,
) -> IndexSummary:
    if not settings.product_vector_enabled:
        return IndexSummary(
            indexed=0,
            skipped=len(items),
            message="Product vector indexing is disabled in settings",
        )
    if not settings.gemini_api_key:
        return IndexSummary(
            indexed=0,
            skipped=len(items),
            message="Gemini API key is not configured",
        )
    if not items:
        return IndexSummary(indexed=0, skipped=0, message="No media items to index")

    def _open_and_clear() -> zvec.Collection:
        coll = get_or_open_collection_sync()
        coll.delete_by_filter(f"product_id = {int(product_id)}")
        return coll

    try:
        coll = await asyncio.to_thread(_open_and_clear)
    except (RuntimeError, ValueError) as exc:
        return IndexSummary(errors=[str(exc)], message=str(exc))

    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""
    indexed = 0
    skipped = 0
    errors: list[str] = []
    docs: list[zvec.Doc] = []

    for i, item in enumerate(items):
        if item.kind not in ("image", "video"):
            skipped += 1
            continue
        try:
            fs_path = resolve_storage_media_path(item.path)
        except (OSError, ValueError) as exc:
            errors.append(f"{item.path}: {exc}")
            skipped += 1
            continue
        suffix = fs_path.suffix.lower()
        if suffix == ".m3u8":
            skipped += 1
            continue
        doc_id = f"p{product_id}_{i}"
        try:
            vec = await gemini.embed_file(
                http,
                api_key=api_key,
                model=model,
                path=fs_path,
                kind=item.kind,
                output_dimensionality=dim,
                product_name=product_name,
            )
        except (OSError, ValueError, FileNotFoundError) as exc:
            errors.append(f"{item.path}: {exc}")
            skipped += 1
            continue

        if len(vec) != dim:
            errors.append(
                f"{item.path}: embedding length {len(vec)} != configured {dim}",
            )
            skipped += 1
            continue

        src = item.source_url.strip() if item.source_url else ""
        pname = product_name.strip() if product_name else ""
        docs.append(
            zvec.Doc(
                id=doc_id,
                vectors={"embedding": vec},
                fields={
                    "product_id": int(product_id),
                    "image_path": str(fs_path),
                    "source_url": src or None,
                    "product_name": pname or None,
                    "modality": item.kind,
                },
            ),
        )
        indexed += 1

    if docs:

        def _upsert() -> None:
            coll.upsert(docs)
            coll.flush()

        try:
            await asyncio.to_thread(_upsert)
        except Exception as exc:
            logger.exception("zvec upsert failed for product %s", product_id)
            return IndexSummary(
                indexed=0,
                skipped=len(items),
                errors=[str(exc)],
                message="Vector store upsert failed",
            )

    return IndexSummary(
        indexed=indexed,
        skipped=skipped,
        errors=errors,
    )


class SearchHit(BaseModel):
    product_id: int
    score: float
    image_path: str
    source_url: str | None = None
    product_name: str | None = None
    modality: str | None = None


async def search_by_text(
    *,
    query: str,
    top_k: int,
    http: httpx.AsyncClient,
) -> list[SearchHit]:
    if not settings.product_vector_enabled:
        msg = "Product vector search is disabled in settings"
        raise ValueError(msg)
    if not settings.gemini_api_key:
        msg = "Gemini API key is not configured"
        raise ValueError(msg)

    def _open() -> zvec.Collection:
        return get_or_open_collection_sync()

    coll = await asyncio.to_thread(_open)
    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    vec = await gemini.embed_text(
        http,
        api_key=api_key,
        model=model,
        text=query,
        output_dimensionality=dim,
    )
    if len(vec) != dim:
        msg = f"Query embedding length {len(vec)} != configured {dim}"
        raise ValueError(msg)

    def _run_query() -> list[zvec.Doc]:
        return coll.query(
            vectors=zvec.VectorQuery("embedding", vector=vec),
            topk=top_k,
            output_fields=[
                "product_id",
                "image_path",
                "source_url",
                "product_name",
                "modality",
            ],
        )

    raw = await asyncio.to_thread(_run_query)
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


async def search_by_media_path(
    *,
    media_path: str,
    kind: Literal["image", "video"],
    top_k: int,
    http: httpx.AsyncClient,
) -> list[SearchHit]:
    fs_path = resolve_storage_media_path(media_path)
    if not settings.product_vector_enabled:
        msg = "Product vector search is disabled in settings"
        raise ValueError(msg)
    if not settings.gemini_api_key:
        msg = "Gemini API key is not configured"
        raise ValueError(msg)

    coll = await asyncio.to_thread(get_or_open_collection_sync)
    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    vec = await gemini.embed_file(
        http,
        api_key=api_key,
        model=model,
        path=fs_path,
        kind=kind,
        output_dimensionality=dim,
    )
    if len(vec) != dim:
        msg = f"Media embedding length {len(vec)} != configured {dim}"
        raise ValueError(msg)

    def _run_query() -> list[zvec.Doc]:
        return coll.query(
            vectors=zvec.VectorQuery("embedding", vector=vec),
            topk=top_k,
            output_fields=[
                "product_id",
                "image_path",
                "source_url",
                "product_name",
                "modality",
            ],
        )

    raw = await asyncio.to_thread(_run_query)
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
