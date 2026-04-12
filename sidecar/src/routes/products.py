import logging

from fastapi import APIRouter

from models.schemas import (
    FetchedProductData,
    FetchProductRequest,
    FetchProductResponse,
)
from tiktok.product_scraper import fetch_product_from_url

logger = logging.getLogger(__name__)
router = APIRouter()


@router.post("/api/products/fetch-from-url", response_model=FetchProductResponse)
async def fetch_product(body: FetchProductRequest):
    url = body.url.strip()
    if not url:
        return FetchProductResponse(success=False, error="URL is required")

    try:
        result = await fetch_product_from_url(url, cookies_json=body.cookies_json)
    except Exception as exc:
        logger.exception("Product fetch failed for %s", url)
        return FetchProductResponse(success=False, error=str(exc))

    data = FetchedProductData(
        name=result.name,
        description=result.description,
        price=result.price,
        image_url=result.image_url,
        category=result.category,
        tiktok_shop_id=result.tiktok_shop_id,
    )

    return FetchProductResponse(
        success=result.name is not None,
        incomplete=result.incomplete,
        data=data,
    )
