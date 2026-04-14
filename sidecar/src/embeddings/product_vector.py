from __future__ import annotations

import asyncio
import importlib.util
import logging
import threading
from pathlib import Path
from typing import Any, Literal, cast

import httpx
import zvec
from pydantic import BaseModel

from config import settings
from embeddings import gemini

logger = logging.getLogger(__name__)

# dashtext: cp311-cp312 wheels only; else dense-only text search.
_HAS_DASHTEXT = importlib.util.find_spec("dashtext") is not None
_BM25_EMBED_FN: Any = getattr(zvec, "BM25EmbeddingFunction", None)

VECTOR_SUBDIR = Path("vector") / "product_media"
_TEXT_PATH_PLACEHOLDER = "__text__"

_coll_lock = threading.Lock()
_coll_cache: dict[str, zvec.Collection] = {}

_bm25_lock = threading.Lock()
_bm25_doc: Any = None
_bm25_query: Any = None
_bm25_corpus_size: int = 0

_COLLECTION_RW = zvec.CollectionOption(read_only=False, enable_mmap=True)


class MediaIndexItem(BaseModel):
    kind: Literal["image", "video"]
    path: str
    source_url: str = ""


def _vector_root() -> Path:
    return (settings.storage_path / VECTOR_SUBDIR).resolve()


def _rw_cache_key(path_str: str, dim: int) -> str:
    return f"{path_str}\0{dim}\0rw"


def _embedding_dimension_from_schema(schema: Any) -> int:
    vecs = schema.vectors
    if isinstance(vecs, list) and vecs:
        return int(vecs[0].dimension)
    msg = "zvec schema has no vector field"
    raise RuntimeError(msg)


def _collection_has_hybrid_text_vectors(schema: Any) -> bool:
    vecs = schema.vectors
    if not isinstance(vecs, list):
        return False
    names = {v.name for v in vecs}
    return "text_dense" in names and "text_sparse" in names


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


def _zeros(dim: int) -> list[float]:
    return [0.0] * dim


def _empty_sparse() -> dict[int, float]:
    return {}


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


def _require_hybrid_schema(coll: zvec.Collection) -> None:
    if not _collection_has_hybrid_text_vectors(coll.schema):
        msg = (
            "Product vector store uses an older schema without text hybrid vectors. "
            f"Delete the folder {_vector_root()} and re-index products."
        )
        raise ValueError(msg)


def _load_corpus_texts(coll: zvec.Collection) -> list[str]:
    docs = coll.query(
        filter="product_text IS NOT NULL",
        topk=10000,
        output_fields=["product_text"],
    )
    out: list[str] = []
    for d in docs:
        fields = d.fields or {}
        pt = fields.get("product_text")
        if pt is not None and str(pt).strip():
            out.append(str(pt).strip())
    return out


def _rebuild_bm25(coll: zvec.Collection) -> None:
    global _bm25_doc, _bm25_query, _bm25_corpus_size
    if not _HAS_DASHTEXT:
        _bm25_doc = None
        _bm25_query = None
        _bm25_corpus_size = 0
        return
    corpus = _load_corpus_texts(coll)
    if not corpus:
        _bm25_doc = None
        _bm25_query = None
        _bm25_corpus_size = 0
        return
    _bm25_doc = _BM25_EMBED_FN(
        corpus=corpus,
        encoding_type="document",
        language="en",
    )
    _bm25_query = _BM25_EMBED_FN(
        corpus=corpus,
        encoding_type="query",
        language="en",
    )
    _bm25_corpus_size = len(corpus)


def invalidate_bm25_cache() -> None:
    global _bm25_doc, _bm25_query, _bm25_corpus_size
    with _bm25_lock:
        _bm25_doc = None
        _bm25_query = None
        _bm25_corpus_size = 0


def ensure_bm25_ready(coll: zvec.Collection) -> tuple[Any, Any]:
    _require_hybrid_schema(coll)
    if not _HAS_DASHTEXT:
        msg = "BM25 hybrid requires dashtext (CPython 3.11-3.12); use dense-only build"
        raise RuntimeError(msg)
    with _bm25_lock:
        corpus_now = len(_load_corpus_texts(coll))
        if _bm25_doc is not None and _bm25_query is not None and _bm25_corpus_size == corpus_now:
            return _bm25_doc, _bm25_query
        _rebuild_bm25(coll)
        if _bm25_doc is None or _bm25_query is None:
            msg = "No product text indexed yet; cannot run BM25"
            raise RuntimeError(msg)
        return _bm25_doc, _bm25_query


def _open_existing_rw_collection(path_str: str, dim: int) -> zvec.Collection:
    coll = zvec.open(path_str, option=_COLLECTION_RW)
    got = _embedding_dimension_from_schema(coll.schema)
    if got != dim:
        msg = (
            f"zvec collection dimension is {got} but settings request {dim}; "
            "delete the vector folder or match gemini_embedding_dimensions."
        )
        raise ValueError(msg)
    if not _collection_has_hybrid_text_vectors(coll.schema):
        msg = (
            "Product vector index was created before hybrid text search. "
            f"Delete {_vector_root()} and re-index products to upgrade."
        )
        raise ValueError(msg)
    return coll


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
    rwk = _rw_cache_key(path_str, dim)

    with _coll_lock:
        cached = _coll_cache.get(rwk)
        if cached is not None:
            return cached

        root.parent.mkdir(parents=True, exist_ok=True)

        if root.exists() and any(root.iterdir()):
            coll = _open_existing_rw_collection(path_str, dim)
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

        _coll_cache[rwk] = coll
        return coll


def get_or_open_collection_for_query_sync() -> zvec.Collection:
    """Open the product vector store for search.

    Uses the same read-write handle as indexing. A separate read-only zvec handle
    would keep a filesystem lock until GC and blocks ``get_or_open_collection_sync``
    with "Can't lock read-write collection".
    """
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
    rwk = _rw_cache_key(path_str, dim)

    with _coll_lock:
        rw = _coll_cache.get(rwk)
        if rw is not None:
            return rw

        if not root.exists() or not any(root.iterdir()):
            msg = "Product vector store is empty; index product media before searching"
            raise RuntimeError(msg)

        coll = _open_existing_rw_collection(path_str, dim)
        _coll_cache[rwk] = coll
        return coll


def delete_vectors_for_product_sync(product_id: int) -> None:
    if not settings.product_vector_enabled:
        return
    root = _vector_root()
    if not root.exists() or not any(root.iterdir()):
        return
    path_str = str(root)
    dim = settings.gemini_embedding_dimensions
    rwk = _rw_cache_key(path_str, dim)
    with _coll_lock:
        coll = _coll_cache.get(rwk)
    if coll is None:
        try:
            coll = zvec.open(path_str, option=_COLLECTION_RW)
        except Exception as exc:
            logger.debug("skip vector delete: could not open zvec collection: %s", exc)
            return
    try:
        coll.delete_by_filter(f"product_id = {int(product_id)}")
        coll.flush()
        invalidate_bm25_cache()
    except Exception as exc:
        logger.warning("vector delete failed for product_id=%s: %s", product_id, exc)


class IndexSummary(BaseModel):
    indexed: int = 0
    skipped: int = 0
    errors: list[str] = []
    message: str | None = None


def _delete_media_docs_for_product_sync(coll: zvec.Collection, product_id: int) -> None:
    coll.delete_by_filter(
        f"(product_id = {int(product_id)}) AND (modality = 'image' OR modality = 'video')",
    )


async def index_product_media(
    *,
    product_id: int,
    product_name: str,
    items: list[MediaIndexItem],
    http: httpx.AsyncClient,
    product_description: str = "",
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

    def _open_and_clear_media() -> zvec.Collection:
        coll = get_or_open_collection_sync()
        _delete_media_docs_for_product_sync(coll, product_id)
        return coll

    try:
        coll = await asyncio.to_thread(_open_and_clear_media)
    except (RuntimeError, ValueError) as exc:
        return IndexSummary(errors=[str(exc)], message=str(exc))

    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""
    zd = _zeros(dim)
    zs = _empty_sparse()
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
        pname_strip = product_name.strip() if product_name else ""
        suffix = (settings.product_media_embed_suffix or "").strip()
        catalog_caption = (
            (pname_strip + " " + suffix).strip()
            if pname_strip and suffix
            else (pname_strip or None)
        )
        try:
            vec = await gemini.embed_file(
                http,
                api_key=api_key,
                model=model,
                path=fs_path,
                kind=item.kind,
                output_dimensionality=dim,
                product_name=None,
                companion_text=catalog_caption,
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
        pdesc = (product_description or "").strip()
        pdesc_field = pdesc[:8000] if pdesc else None
        docs.append(
            zvec.Doc(
                id=doc_id,
                vectors=cast(Any, {"embedding": vec, "text_dense": zd, "text_sparse": zs}),
                fields={
                    "product_id": int(product_id),
                    "image_path": str(fs_path),
                    "source_url": src or None,
                    "product_name": pname or None,
                    "modality": item.kind,
                    "product_text": None,
                    "product_description": pdesc_field,
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
            text=raw_text,
            output_dimensionality=dim,
            role="document",
            title=name or None,
        )
    except (OSError, ValueError) as exc:
        return IndexSummary(errors=[str(exc)], message="Dense text embedding failed")

    if len(dense_vec) != dim:
        msg = f"Text embedding length {len(dense_vec)} != configured {dim}"
        return IndexSummary(errors=[msg], message=msg)

    def _sparse_upsert() -> None:
        coll.delete_by_filter(
            f"(product_id = {int(product_id)}) AND (modality = 'text')",
        )
        zd = _zeros(dim)
        zs = _empty_sparse()
        if _HAS_DASHTEXT:
            corpus_remaining = _load_corpus_texts(coll)
            full_corpus = [*corpus_remaining, raw_text]
            bm25_doc = _BM25_EMBED_FN(
                corpus=full_corpus,
                encoding_type="document",
                language="en",
            )
            sparse_vec = bm25_doc.embed(raw_text)
        else:
            sparse_vec = zs
        doc_id = f"t{product_id}"
        coll.upsert(
            [
                zvec.Doc(
                    id=doc_id,
                    vectors=cast(
                        Any,
                        {
                            "embedding": zd,
                            "text_dense": dense_vec,
                            "text_sparse": sparse_vec,
                        },
                    ),
                    fields={
                        "product_id": int(product_id),
                        "image_path": _TEXT_PATH_PLACEHOLDER,
                        "source_url": None,
                        "product_name": name or None,
                        "modality": "text",
                        "product_text": raw_text,
                        "product_description": desc or None,
                    },
                ),
            ],
        )
        coll.flush()
        invalidate_bm25_cache()

    try:
        await asyncio.to_thread(_sparse_upsert)
    except Exception as exc:
        logger.exception("zvec text upsert failed for product %s", product_id)
        return IndexSummary(errors=[str(exc)], message="Text vector upsert failed")

    return IndexSummary(indexed=1)


class SearchHit(BaseModel):
    product_id: int
    score: float
    image_path: str
    source_url: str | None = None
    product_name: str | None = None
    modality: str | None = None
    product_text: str | None = None
    product_description: str | None = None


def _hit_text_field(fields: dict, key: str, max_len: int = 4000) -> str | None:
    raw = fields.get(key)
    if raw is None:
        return None
    s = str(raw).strip()
    if not s:
        return None
    return s[:max_len]


def _docs_to_hits(raw: list[zvec.Doc]) -> list[SearchHit]:
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
                product_text=_hit_text_field(fields, "product_text"),
                product_description=_hit_text_field(fields, "product_description"),
            ),
        )
    return out


_OUTPUT_HIT_FIELDS = [
    "product_id",
    "image_path",
    "source_url",
    "product_name",
    "modality",
    "product_text",
    "product_description",
]


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

    coll = await asyncio.to_thread(get_or_open_collection_for_query_sync)
    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    q = query.strip()
    if not q:
        return []

    dense_vec = await gemini.embed_text(
        http,
        api_key=api_key,
        model=model,
        text=q,
        output_dimensionality=dim,
        role="query",
    )
    if len(dense_vec) != dim:
        msg = f"Query embedding length {len(dense_vec)} != configured {dim}"
        raise ValueError(msg)

    def _run_query() -> list[zvec.Doc]:
        if _HAS_DASHTEXT:
            _, bm25_q = ensure_bm25_ready(coll)
            sparse_vec = bm25_q.embed(q)
            return coll.query(
                vectors=[
                    zvec.VectorQuery("text_dense", vector=dense_vec),
                    zvec.VectorQuery("text_sparse", vector=sparse_vec),
                ],
                reranker=zvec.RrfReRanker(topn=top_k),
                filter="modality = 'text'",
                topk=top_k,
                output_fields=_OUTPUT_HIT_FIELDS,
            )
        return coll.query(
            vectors=zvec.VectorQuery("text_dense", vector=dense_vec),
            filter="modality = 'text'",
            topk=top_k,
            output_fields=_OUTPUT_HIT_FIELDS,
        )

    raw = await asyncio.to_thread(_run_query)
    return _docs_to_hits(raw)


async def search_by_media_path(
    *,
    media_path: str,
    kind: Literal["image", "video"],
    top_k: int,
    http: httpx.AsyncClient,
    companion_text: str | None = None,
) -> list[SearchHit]:
    fs_path = resolve_storage_media_path(media_path)
    if not settings.product_vector_enabled:
        msg = "Product vector search is disabled in settings"
        raise ValueError(msg)
    if not settings.gemini_api_key:
        msg = "Gemini API key is not configured"
        raise ValueError(msg)

    coll = await asyncio.to_thread(get_or_open_collection_for_query_sync)
    model = settings.gemini_embedding_model
    dim = settings.gemini_embedding_dimensions
    api_key = settings.gemini_api_key or ""

    ct = (companion_text or "").strip()
    vec = await gemini.embed_file(
        http,
        api_key=api_key,
        model=model,
        path=fs_path,
        kind=kind,
        output_dimensionality=dim,
        companion_text=ct if ct else None,
    )
    if len(vec) != dim:
        msg = f"Media embedding length {len(vec)} != configured {dim}"
        raise ValueError(msg)

    def _run_query() -> list[zvec.Doc]:
        return coll.query(
            vectors=zvec.VectorQuery("embedding", vector=vec),
            topk=top_k,
            filter="(modality = 'image' OR modality = 'video')",
            output_fields=_OUTPUT_HIT_FIELDS,
        )

    raw = await asyncio.to_thread(_run_query)
    return _docs_to_hits(raw)


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

    def _run_query() -> list[zvec.Doc]:
        if _HAS_DASHTEXT:
            _, bm25_q = ensure_bm25_ready(coll)
            sparse_vec = bm25_q.embed(t)
            return coll.query(
                vectors=[
                    zvec.VectorQuery("text_dense", vector=dense_vec),
                    zvec.VectorQuery("text_sparse", vector=sparse_vec),
                ],
                reranker=zvec.RrfReRanker(topn=top_k),
                filter="modality = 'text'",
                topk=top_k,
                output_fields=_OUTPUT_HIT_FIELDS,
            )
        return coll.query(
            vectors=zvec.VectorQuery("text_dense", vector=dense_vec),
            filter="modality = 'text'",
            topk=top_k,
            output_fields=_OUTPUT_HIT_FIELDS,
        )

    raw = await asyncio.to_thread(_run_query)
    return _docs_to_hits(raw)
