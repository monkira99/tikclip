"""Fetch product info from a TikTok Shop URL via OG tags, JSON-LD, and optional media download."""

from __future__ import annotations

import json
import logging
import re
import secrets
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from config import settings
from tiktok.cookies import normalize_tiktok_cookies
from tiktok.http_transport import TikTokHttpStatusError, create_tiktok_transport

logger = logging.getLogger(__name__)

_OG_META_PROPERTY_FIRST = re.compile(
    r'<meta\b[^>]*\bproperty=["\']og:(\w+)["\'][^>]*\bcontent=["\']([^"\']*)["\']',
    re.IGNORECASE,
)
_OG_META_CONTENT_FIRST = re.compile(
    r'<meta\b[^>]*\bcontent=["\']([^"\']*)["\'][^>]*\bproperty=["\']og:(\w+)["\']',
    re.IGNORECASE,
)
_JSON_LD_PATTERN = re.compile(
    r'<script\s+type=["\']application/ld\+json["\']\s*>(.*?)</script>',
    re.DOTALL | re.IGNORECASE,
)
_LOADER_JSON_SCRIPT_RE = re.compile(
    r'<script[^>]*type=["\']application/json["\'][^>]*>(.*?)</script>',
    re.DOTALL | re.IGNORECASE,
)
_PRODUCT_ID_PATH = re.compile(r"/view/product/(\d+)", re.IGNORECASE)
_TITLE_TAG = re.compile(r"<title[^>]*>([^<]{1,800})</title>", re.IGNORECASE)

_MAX_PRODUCT_VIDEOS = 8
_MAX_IMAGE_BYTES = 25 * 1024 * 1024
_MAX_VIDEO_BYTES = 80 * 1024 * 1024


@dataclass
class ScrapedProduct:
    name: str | None = None
    description: str | None = None
    price: float | None = None
    image_url: str | None = None
    category: str | None = None
    tiktok_shop_id: str | None = None
    incomplete: bool = True
    error: str | None = None
    image_urls: list[str] = field(default_factory=list)
    video_urls: list[str] = field(default_factory=list)
    media_files: list[dict[str, str]] = field(default_factory=list)


def _cookies_from_json(cookies_json: str | None) -> dict[str, str]:
    if not cookies_json or not cookies_json.strip():
        return {}
    try:
        parsed = json.loads(cookies_json)
    except (json.JSONDecodeError, TypeError):
        return {}
    raw: dict[str, str] = {}
    if isinstance(parsed, dict):
        raw = {k: str(v) for k, v in parsed.items()}
    elif isinstance(parsed, list):
        for c in parsed:
            if isinstance(c, dict) and "name" in c and "value" in c:
                raw[c["name"]] = str(c["value"])
    return normalize_tiktok_cookies(raw) if raw else {}


def _parse_og_tags(html: str) -> dict[str, str]:
    og: dict[str, str] = {}
    for m in _OG_META_PROPERTY_FIRST.finditer(html):
        og.setdefault(m.group(1).lower(), m.group(2))
    for m in _OG_META_CONTENT_FIRST.finditer(html):
        og.setdefault(m.group(2).lower(), m.group(1))
    return og


def _html_suggests_security_challenge(html: str) -> bool:
    h = html.lower()
    return (
        "security check" in h
        or "captcha_container" in h
        or "wafchallenge" in h
        or "please wait" in h
    )


def _title_tag_product_name(html: str) -> str | None:
    m = _TITLE_TAG.search(html)
    if not m:
        return None
    t = re.sub(r"\s+", " ", m.group(1)).strip()
    if not t:
        return None
    for suffix in (
        " - TikTok Shop Vietnam",
        " - TikTok Shop",
        " | TikTok Shop",
        " - TikTok",
    ):
        if t.endswith(suffix):
            t = t[: -len(suffix)].strip()
    return t or None


def _product_id_from_final_url(final_url: str) -> str | None:
    m = _PRODUCT_ID_PATH.search(final_url)
    return m.group(1) if m else None


def _find_product_model_in_loader(data: dict[str, Any]) -> dict[str, Any] | None:
    """Resolve `product_model` from loaderData → page_config.components_map → product_info."""
    ld = data.get("loaderData")
    if not isinstance(ld, dict):
        return None
    for page in ld.values():
        if not isinstance(page, dict):
            continue
        pc = page.get("page_config")
        if not isinstance(pc, dict):
            continue
        cm = pc.get("components_map")
        if not isinstance(cm, list):
            continue
        for comp in cm:
            if not isinstance(comp, dict):
                continue
            cd = comp.get("component_data")
            if not isinstance(cd, dict):
                continue
            pi = cd.get("product_info")
            if not isinstance(pi, dict):
                continue
            pm = pi.get("product_model")
            if isinstance(pm, dict) and pm.get("product_id") is not None:
                return pm
    return None


def _gallery_image_urls_from_product_model(pm: dict[str, Any]) -> list[str]:
    """Main seller carousel only: `product_model.images` (excludes sku map, feed, reviews)."""
    out: list[str] = []
    seen: set[str] = set()
    for item in pm.get("images") or []:
        if not isinstance(item, dict):
            continue
        urls = item.get("url_list")
        if isinstance(urls, list):
            for u in urls:
                if isinstance(u, str) and u.startswith("http") and u not in seen:
                    seen.add(u)
                    out.append(u)
                    break
    return out


def _video_urls_from_product_model_videos(videos: Any, limit: int) -> list[str]:
    """`.mp4` / `.m3u8` URLs only under `product_model.videos`."""
    out: list[str] = []
    seen: set[str] = set()

    def walk(x: Any) -> None:
        if len(out) >= limit:
            return
        if isinstance(x, dict):
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)
        elif isinstance(x, str):
            s = x.strip()
            if not s.startswith("http"):
                return
            low = s.lower()
            if (".mp4" in low or ".m3u8" in low) and s not in seen:
                seen.add(s)
                out.append(s)

    if videos not in (None, {}, []):
        walk(videos)
    return out[:limit]


def _parse_product_model_from_html(html: str) -> dict[str, Any] | None:
    best: dict[str, Any] | None = None
    best_len = 0
    for m in _LOADER_JSON_SCRIPT_RE.finditer(html):
        chunk = m.group(1).strip()
        if "loaderData" not in chunk:
            continue
        try:
            root = json.loads(chunk)
        except json.JSONDecodeError:
            continue
        if not isinstance(root, dict):
            continue
        pm = _find_product_model_in_loader(root)
        if pm is None:
            continue
        nimg = len(pm.get("images") or [])
        if nimg >= best_len:
            best_len = nimg
            best = pm
    return best


def _flatten_description_blocks(blocks: list[Any]) -> str:
    """Turn TikTok `product_model.description` JSON blocks into plain text (PDP copy)."""
    chunks: list[str] = []
    for b in blocks:
        if not isinstance(b, dict):
            continue
        btype = b.get("type")
        if btype == "text":
            t = b.get("text")
            if isinstance(t, str) and t:
                chunks.append(t)
        elif btype == "image":
            chunks.append("\n")

    out: list[str] = []
    for ch in chunks:
        if ch == "\n":
            if out and not out[-1].endswith("\n"):
                out.append("\n")
            continue
        st = ch.lstrip()
        if out and (st.startswith("\\-") or st.startswith("*")):
            joined = "".join(out)
            if joined and not joined.endswith("\n"):
                out.append("\n")
        elif out and out[-1] != "\n":
            prev = out[-1]
            if prev and ch and prev[-1].isalnum() and ch[0].isalnum():
                out.append(" ")
        out.append(ch)
    return "".join(out).strip()


def _description_from_product_model(pm: dict[str, Any]) -> str | None:
    """Full product copy from `product_model.description` (rich JSON), not og:description."""
    raw = pm.get("description")
    if raw is None:
        return None
    if isinstance(raw, list):
        flat = _flatten_description_blocks(raw)
        return flat or None
    if not isinstance(raw, str):
        return None
    raw_st = raw.strip()
    if not raw_st:
        return None
    if not raw_st.startswith("["):
        return raw_st
    try:
        blocks = json.loads(raw_st)
    except json.JSONDecodeError:
        return raw_st
    if isinstance(blocks, list):
        flat = _flatten_description_blocks(blocks)
        return flat or None
    return raw_st


def _file_extension(url: str, content_type: str | None, kind: str) -> str:
    path = urlparse(url).path
    ext = Path(path.split("?")[0]).suffix.lower()
    if ext and 1 < len(ext) <= 8 and all(c.isalnum() or c == "." for c in ext):
        return ext
    if content_type:
        if "jpeg" in content_type or "jpg" in content_type:
            return ".jpg"
        if "png" in content_type:
            return ".png"
        if "webp" in content_type:
            return ".webp"
        if "mp4" in content_type:
            return ".mp4"
        if "mpegurl" in content_type or "m3u8" in content_type:
            return ".m3u8"
    return ".mp4" if kind == "video" else ".bin"


async def _download_media_items(
    transport,
    out_dir: Path,
    items: list[tuple[str, str]],
) -> list[dict[str, str]]:
    """items: (kind, url). Returns dicts with kind, path (absolute), source_url."""
    saved: list[dict[str, str]] = []
    for i, (kind, url) in enumerate(items):
        try:
            status, body, _final, ct = await transport.get_bytes(url)
        except Exception as exc:
            logger.warning("media download failed url=%s err=%s", url[:80], exc)
            continue
        if not (200 <= status < 300):
            logger.warning("media download HTTP %s for %s", status, url[:80])
            continue
        max_b = _MAX_VIDEO_BYTES if kind == "video" else _MAX_IMAGE_BYTES
        if len(body) > max_b:
            logger.warning("media download too large (%s bytes) %s", len(body), url[:80])
            continue
        ext = _file_extension(url, ct, kind)
        name = f"{kind}_{i:03d}{ext}"
        path = out_dir / name
        try:
            path.write_bytes(body)
        except OSError as exc:
            logger.warning("media write failed %s: %s", path, exc)
            continue
        saved.append(
            {
                "kind": kind,
                "path": str(path.resolve()),
                "source_url": url,
            }
        )
    return saved


async def fetch_product_from_url(
    url: str,
    cookies_json: str | None = None,
    *,
    download_media: bool = True,
) -> ScrapedProduct:
    cookies = _cookies_from_json(cookies_json)
    product = ScrapedProduct()
    url_s = url.strip()
    logger.debug(
        "fetch_product_from_url start url=%s download_media=%s has_cookies=%s",
        url_s[:160],
        download_media,
        bool(cookies),
    )

    transport = create_tiktok_transport(cookies, proxy=None)
    try:
        try:
            resp = await transport.get(url_s)
            resp.raise_for_status()
        except TikTokHttpStatusError as exc:
            logger.warning(
                "Product URL HTTP error %s status=%s url=%s",
                url,
                exc.status_code,
                exc.url,
            )
            product.error = f"HTTP {exc.status_code} from TikTok"
            product.incomplete = True
            return product
        except Exception as exc:
            logger.warning("Failed to fetch product URL %s: %s", url, exc)
            product.error = "Network error while loading the page"
            product.incomplete = True
            return product

        html = resp.text
        final_url = resp.url
        logger.debug(
            "fetch_product_from_url GET ok status=%s final_url=%s html_len=%s",
            resp.status_code,
            final_url[:160],
            len(html),
        )

        if _html_suggests_security_challenge(html):
            logger.debug("fetch_product_from_url security/captcha page detected")
            product.error = (
                "TikTok returned a security or captcha page. "
                "Paste cookies from a logged-in browser (optional field) or try again later."
            )
            product.incomplete = True
            return product

        og_tags = _parse_og_tags(html)
        product.name = og_tags.get("title")
        product.description = og_tags.get("description")
        product.image_url = og_tags.get("image")
        product.name = product.name or _title_tag_product_name(html)

        pid = _product_id_from_final_url(final_url)
        if pid:
            product.tiktok_shop_id = pid

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

        pm = _parse_product_model_from_html(html)
        logger.debug(
            "fetch_product_from_url og_keys=%s product_model=%s",
            sorted(og_tags.keys()),
            pm is not None,
        )
        if pm is not None:
            rich_desc = _description_from_product_model(pm)
            if rich_desc:
                product.description = rich_desc
            product.image_urls = _gallery_image_urls_from_product_model(pm)
            product.video_urls = _video_urls_from_product_model_videos(
                pm.get("videos"),
                _MAX_PRODUCT_VIDEOS,
            )
        else:
            logger.warning(
                "Could not parse product_model from PDP JSON; falling back to OG image only"
            )
            product.image_urls = []
            product.video_urls = []
            if product.image_url and product.image_url.startswith("http"):
                product.image_urls = [product.image_url]

        logger.debug(
            "fetch_product_from_url gallery images=%s videos=%s",
            len(product.image_urls),
            len(product.video_urls),
        )

        if download_media and (product.image_urls or product.video_urls):
            batch = secrets.token_hex(8)
            out_dir = settings.storage_path.resolve() / "products" / "fetched" / batch
            try:
                out_dir.mkdir(parents=True, exist_ok=True)
            except OSError as exc:
                logger.warning("could not create media dir %s: %s", out_dir, exc)
            else:
                to_fetch: list[tuple[str, str]] = [("image", u) for u in product.image_urls]
                to_fetch.extend(("video", u) for u in product.video_urls)
                product.media_files = await _download_media_items(transport, out_dir, to_fetch)
                first_local_image = next(
                    (m["path"] for m in product.media_files if m.get("kind") == "image"),
                    None,
                )
                if first_local_image:
                    product.image_url = first_local_image
                logger.debug(
                    "fetch_product_from_url downloaded media_files=%s",
                    len(product.media_files),
                )
        elif download_media:
            logger.debug("fetch_product_from_url skip download (no gallery URLs)")

        has_required = product.name is not None
        product.incomplete = not has_required
        if not has_required:
            product.error = "Could not find product title on this page (layout may have changed)."
        logger.debug(
            "fetch_product_from_url done incomplete=%s error=%r name=%r",
            product.incomplete,
            (product.error[:120] if product.error else None),
            (product.name[:80] if product.name else None),
        )
        return product
    finally:
        await transport.aclose()
