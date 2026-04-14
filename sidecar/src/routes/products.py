import asyncio
import logging

import httpx
from fastapi import APIRouter, HTTPException, Query
from pydantic import ValidationError

from embeddings.product_vector import (
    MediaIndexItem,
    delete_vectors_for_product_sync,
    index_product_media,
    index_product_text,
    search_by_media_path,
    search_by_text,
    summarize_indexed_products_sync,
)
from models.schemas import (
    DeleteProductEmbeddingsRequest,
    DeleteProductEmbeddingsResponse,
    FetchedProductData,
    FetchedProductMediaFile,
    FetchProductRequest,
    FetchProductResponse,
    IndexProductEmbeddingsRequest,
    IndexProductEmbeddingsResponse,
    ProductEmbeddingSearchByMediaRequest,
    ProductEmbeddingSearchHit,
    ProductEmbeddingSearchRequest,
    ProductEmbeddingSearchResponse,
    ProductEmbeddingsIndexedSummaryResponse,
)
from tiktok.product_scraper import fetch_product_from_url

logger = logging.getLogger(__name__)
router = APIRouter()


@router.post("/api/products/fetch-from-url", response_model=FetchProductResponse)
async def fetch_product(body: FetchProductRequest):
    url = body.url.strip()
    if not url:
        return FetchProductResponse(success=False, error="URL is required")

    logger.debug(
        "fetch-from-url start url=%s download_media=%s cookies=%s",
        url[:200],
        body.download_media,
        "yes" if (body.cookies_json and body.cookies_json.strip()) else "no",
    )
    try:
        result = await fetch_product_from_url(
            url,
            cookies_json=body.cookies_json,
            download_media=body.download_media,
        )
    except Exception as exc:
        logger.exception("Product fetch failed for %s", url)
        return FetchProductResponse(success=False, error=str(exc))

    media_out: list[FetchedProductMediaFile] = []
    for m in result.media_files:
        try:
            media_out.append(FetchedProductMediaFile.model_validate(m))
        except ValidationError:
            logger.debug("skip invalid media entry %s", m)

    data = FetchedProductData(
        name=result.name,
        description=result.description,
        price=result.price,
        image_url=result.image_url,
        category=result.category,
        tiktok_shop_id=result.tiktok_shop_id,
        image_urls=result.image_urls,
        video_urls=result.video_urls,
        media_files=media_out,
    )

    ok = result.name is not None
    logger.debug(
        "fetch-from-url done success=%s incomplete=%s name=%r "
        "media_files=%s images=%s videos=%s err=%r",
        ok,
        result.incomplete,
        (result.name or "")[:80],
        len(media_out),
        len(result.image_urls),
        len(result.video_urls),
        result.error,
    )
    return FetchProductResponse(
        success=ok,
        incomplete=result.incomplete,
        data=data,
        error=None if ok else (result.error or "Could not read product from this page."),
    )


@router.get(
    "/api/products/embeddings/indexed-summary",
    response_model=ProductEmbeddingsIndexedSummaryResponse,
)
async def get_embeddings_indexed_summary(
    max_docs: int = Query(
        100,
        ge=1,
        le=500_000,
        description="Max zvec documents to scan (higher = slower; cap avoids huge scans).",
    ),
):
    """List product IDs present in the local zvec index with per-modality document counts."""
    logger.debug("embeddings/indexed-summary max_docs=%s", max_docs)

    def _run() -> dict:
        return summarize_indexed_products_sync(max_docs=max_docs)

    raw = await asyncio.to_thread(_run)
    return ProductEmbeddingsIndexedSummaryResponse.model_validate(raw)


@router.post(
    "/api/products/embeddings/index",
    response_model=IndexProductEmbeddingsResponse,
)
async def index_product_embeddings(body: IndexProductEmbeddingsRequest):
    logger.debug(
        "embeddings/index product_id=%s items=%s desc_len=%s",
        body.product_id,
        len(body.items),
        len(body.product_description.strip()) if body.product_description else 0,
    )
    items = [MediaIndexItem.model_validate(x.model_dump()) for x in body.items]
    async with httpx.AsyncClient() as client:
        summary = await index_product_media(
            product_id=body.product_id,
            product_name=body.product_name,
            items=items,
            http=client,
            product_description=body.product_description,
        )
        if body.product_name.strip() or body.product_description.strip():
            tsum = await index_product_text(
                product_id=body.product_id,
                product_name=body.product_name,
                product_description=body.product_description,
                http=client,
            )
            summary.indexed += tsum.indexed
            summary.errors.extend(tsum.errors)
            if tsum.message and not summary.message:
                summary.message = tsum.message
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


@router.post(
    "/api/products/embeddings/delete",
    response_model=DeleteProductEmbeddingsResponse,
)
async def delete_product_embeddings(body: DeleteProductEmbeddingsRequest):
    logger.debug("embeddings/delete product_id=%s", body.product_id)
    await asyncio.to_thread(delete_vectors_for_product_sync, body.product_id)
    return DeleteProductEmbeddingsResponse(ok=True)


@router.post(
    "/api/products/embeddings/search",
    response_model=ProductEmbeddingSearchResponse,
)
async def search_product_embeddings_text(body: ProductEmbeddingSearchRequest):
    if not body.query:
        raise HTTPException(status_code=400, detail="query is required")
    logger.debug(
        "embeddings/search text top_k=%s query=%r",
        body.top_k,
        body.query[:200],
    )
    try:
        async with httpx.AsyncClient() as client:
            hits = await search_by_text(query=body.query, top_k=body.top_k, http=client)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    logger.debug("embeddings/search text hits=%s", len(hits))
    return ProductEmbeddingSearchResponse(
        hits=[
            ProductEmbeddingSearchHit(
                product_id=h.product_id,
                score=h.score,
                image_path=h.image_path,
                source_url=h.source_url,
                product_name=h.product_name,
                modality=h.modality,
                product_text=h.product_text,
                product_description=h.product_description,
            )
            for h in hits
        ],
    )


@router.post(
    "/api/products/embeddings/search-media",
    response_model=ProductEmbeddingSearchResponse,
)
async def search_product_embeddings_media(body: ProductEmbeddingSearchByMediaRequest):
    logger.debug(
        "embeddings/search-media kind=%s top_k=%s path=%s",
        body.kind,
        body.top_k,
        body.path[:120],
    )
    try:
        async with httpx.AsyncClient() as client:
            hits = await search_by_media_path(
                media_path=body.path,
                kind=body.kind,
                top_k=body.top_k,
                http=client,
                companion_text=body.companion_text,
            )
    except (ValueError, FileNotFoundError) as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    logger.debug("embeddings/search-media hits=%s", len(hits))
    return ProductEmbeddingSearchResponse(
        hits=[
            ProductEmbeddingSearchHit(
                product_id=h.product_id,
                score=h.score,
                image_path=h.image_path,
                source_url=h.source_url,
                product_name=h.product_name,
                modality=h.modality,
                product_text=h.product_text,
                product_description=h.product_description,
            )
            for h in hits
        ],
    )
