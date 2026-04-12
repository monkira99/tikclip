"""Fetch product info from a TikTok Shop URL via OG tags and JSON-LD."""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass

import httpx

logger = logging.getLogger(__name__)

_OG_PATTERN = re.compile(
    r'<meta\s+(?:property|name)=["\']og:(\w+)["\']\s+content=["\']([^"\']*)["\']',
    re.IGNORECASE,
)
_JSON_LD_PATTERN = re.compile(
    r'<script\s+type=["\']application/ld\+json["\']\s*>(.*?)</script>',
    re.DOTALL | re.IGNORECASE,
)


@dataclass
class ScrapedProduct:
    name: str | None = None
    description: str | None = None
    price: float | None = None
    image_url: str | None = None
    category: str | None = None
    tiktok_shop_id: str | None = None
    incomplete: bool = True


async def fetch_product_from_url(url: str, cookies_json: str | None = None) -> ScrapedProduct:
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        ),
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9,vi;q=0.8",
    }

    cookies: dict[str, str] = {}
    if cookies_json:
        try:
            parsed = json.loads(cookies_json)
            if isinstance(parsed, dict):
                cookies = {k: str(v) for k, v in parsed.items()}
            elif isinstance(parsed, list):
                for c in parsed:
                    if isinstance(c, dict) and "name" in c and "value" in c:
                        cookies[c["name"]] = str(c["value"])
        except (json.JSONDecodeError, TypeError):
            pass

    product = ScrapedProduct()

    try:
        async with httpx.AsyncClient(
            follow_redirects=True, timeout=15.0, cookies=cookies
        ) as client:
            resp = await client.get(url, headers=headers)
            resp.raise_for_status()
            html = resp.text
    except Exception as exc:
        logger.warning("Failed to fetch product URL %s: %s", url, exc)
        product.incomplete = True
        return product

    og_tags: dict[str, str] = {}
    for m in _OG_PATTERN.finditer(html):
        og_tags[m.group(1).lower()] = m.group(2)

    product.name = og_tags.get("title")
    product.description = og_tags.get("description")
    product.image_url = og_tags.get("image")

    for m in _JSON_LD_PATTERN.finditer(html):
        try:
            data = json.loads(m.group(1))
            if isinstance(data, dict) and data.get("@type") == "Product":
                product.name = product.name or data.get("name")
                product.description = product.description or data.get("description")
                img = data.get("image")
                if isinstance(img, str):
                    product.image_url = product.image_url or img
                elif isinstance(img, list) and img:
                    product.image_url = product.image_url or str(img[0])
                product.category = data.get("category")
                offers = data.get("offers", {})
                if isinstance(offers, dict) and "price" in offers:
                    try:
                        product.price = float(offers["price"])
                    except (ValueError, TypeError):
                        pass
                sku = data.get("sku")
                if sku:
                    product.tiktok_shop_id = str(sku)
                break
        except (json.JSONDecodeError, TypeError):
            continue

    has_required = product.name is not None
    product.incomplete = not has_required

    return product
