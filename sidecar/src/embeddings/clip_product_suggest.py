"""Match a clip to a product: image frames, optional STT text hybrid, weighted fusion."""

from __future__ import annotations

import asyncio
import logging
import re
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal, cast
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
    ClipSuggestImageEvidenceHit,
    ClipSuggestProductRankRow,
    ClipSuggestProductResponse,
    ClipSuggestTextHit,
    ClipSuggestTranscriptSegmentRow,
    ClipSuggestVoteRow,
)

logger = logging.getLogger(__name__)


def _config_fields() -> dict:
    focus = (settings.suggest_image_embed_focus_prompt or "").strip()
    return {
        "config_target_extracted_frames": max(1, min(12, settings.auto_tag_clip_frame_count)),
        "config_max_score_threshold": float(settings.auto_tag_clip_max_score),
        "suggest_weight_image": float(settings.suggest_weight_image),
        "suggest_weight_text": float(settings.suggest_weight_text),
        "suggest_min_fused_score": float(settings.suggest_min_fused_score),
        "suggest_image_embed_focus_prompt": focus,
    }


def _storage_relative(path: Path) -> str:
    root = settings.storage_path.resolve()
    try:
        return str(path.resolve().relative_to(root))
    except ValueError:
        return path.name


def _catalog_media_rel(image_path: str) -> str | None:
    raw = (image_path or "").strip()
    if not raw or raw == "__text__":
        return None
    try:
        return _storage_relative(Path(raw))
    except OSError:
        return None


def _catalog_modality(raw: str | None) -> Literal["image", "video"] | None:
    if raw == "image" or raw == "video":
        return cast(Literal["image", "video"], raw)
    return None


def _enabled() -> tuple[bool, str | None]:
    if not settings.auto_tag_clip_product_enabled:
        return False, "auto_tag_clip_product_enabled is off"
    if not settings.product_vector_enabled:
        return False, "product_vector_enabled is off"
    if not settings.gemini_api_key:
        return False, "Gemini API key is not configured"
    return True, None


def _build_rankings(
    frame_rows: list[ClipSuggestFrameRow],
    text_hits: list[SearchHit],
    w_img: float,
    w_txt: float,
) -> list[ClipSuggestProductRankRow]:
    """Build product rankings using **absolute** scores on a [0,1] scale.

    Image: similarity = 1 - mean_best_distance  (best per-frame, then average)
    Text:  similarity = raw zvec score (already [0,1] range)
    Final: w_img * image_score + w_txt * text_score
    """
    # Per product, per frame: keep best (lowest) distance
    frame_ev: dict[int, dict[int, float]] = {}
    names: dict[int, str | None] = {}
    for row in frame_rows:
        if row.outcome != "hit":
            continue
        for ev in row.image_evidence_hits:
            pid = ev.product_id
            if ev.product_name:
                names[pid] = ev.product_name
            bests = frame_ev.setdefault(pid, {})
            cur = bests.get(row.index)
            if cur is None or ev.score < cur:
                bests[row.index] = ev.score

    img_scores: dict[int, float] = {}
    img_counts: dict[int, int] = {}
    img_dists: dict[int, float] = {}
    for pid, fd in frame_ev.items():
        dists = list(fd.values())
        m = sum(dists) / len(dists)
        img_scores[pid] = max(0.0, 1.0 - m)
        img_counts[pid] = len(dists)
        img_dists[pid] = m

    txt_scores: dict[int, float] = {}
    for h in text_hits:
        if h.product_id not in txt_scores:
            txt_scores[h.product_id] = h.score
        if h.product_name:
            names.setdefault(h.product_id, h.product_name)

    all_pids = set(img_scores) | set(txt_scores)
    rows: list[ClipSuggestProductRankRow] = []
    for pid in all_pids:
        i_s = img_scores.get(pid, 0.0)
        t_s = txt_scores.get(pid, 0.0)
        rows.append(
            ClipSuggestProductRankRow(
                product_id=pid,
                product_name=names.get(pid),
                frame_hit_count=img_counts.get(pid, 0),
                avg_frame_distance=img_dists.get(pid),
                image_score=i_s,
                transcript_text_score=txt_scores.get(pid),
                text_score=t_s,
                final_score=w_img * i_s + w_txt * t_s,
            ),
        )
    rows.sort(key=lambda r: r.final_score, reverse=True)
    return rows


def _split_transcript_segments(text: str, *, max_segments: int = 8) -> list[str]:
    t = text.strip()
    if not t:
        return []
    parts = [p.strip() for p in re.split(r"\n\s*\n+", t) if p.strip()]
    if len(parts) <= 1:
        parts = [p.strip() for p in re.split(r"(?<=[.!?])\s+", t) if p.strip()]
    if not parts:
        return [t[:2000]]
    out: list[str] = []
    for p in parts:
        if len(p) < 6:
            continue
        out.append(p[:2000])
        if len(out) >= max_segments:
            break
    return out if out else [t[:2000]]


async def _transcript_segment_evidence(
    transcript_s: str,
    http: httpx.AsyncClient,
    *,
    text_hits: list[SearchHit],
) -> list[ClipSuggestTranscriptSegmentRow]:
    segments = _split_transcript_segments(transcript_s)
    if not segments:
        return []
    if len(segments) == 1:
        seg = segments[0]
        if text_hits:
            h = text_hits[0]
            return [
                ClipSuggestTranscriptSegmentRow(
                    segment_index=0,
                    segment_text=seg,
                    outcome="hit",
                    best_product_id=h.product_id,
                    best_score=h.score,
                    best_product_name=h.product_name,
                    matched_product_description=h.product_description,
                ),
            ]
        return [
            ClipSuggestTranscriptSegmentRow(
                segment_index=0,
                segment_text=seg,
                outcome="no_hit",
            ),
        ]
    tasks = [search_by_transcript(transcript=seg, top_k=1, http=http) for seg in segments]
    results = await asyncio.gather(*tasks, return_exceptions=True)
    rows: list[ClipSuggestTranscriptSegmentRow] = []
    for i, (seg, res) in enumerate(zip(segments, results, strict=True)):
        if isinstance(res, BaseException):
            rows.append(
                ClipSuggestTranscriptSegmentRow(
                    segment_index=i,
                    segment_text=seg,
                    outcome="error",
                    error=str(res)[:500],
                ),
            )
            continue
        if not res:
            rows.append(
                ClipSuggestTranscriptSegmentRow(
                    segment_index=i,
                    segment_text=seg,
                    outcome="no_hit",
                ),
            )
            continue
        h = res[0]
        rows.append(
            ClipSuggestTranscriptSegmentRow(
                segment_index=i,
                segment_text=seg,
                outcome="hit",
                best_product_id=h.product_id,
                best_score=h.score,
                best_product_name=h.product_name,
                matched_product_description=h.product_description,
            ),
        )
    return rows


async def suggest_product_for_clip(
    *,
    video_path: str,
    thumbnail_path: str | None,
    transcript_text: str | None,
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
        logger.debug("suggest_product_for_clip skip: clip video not found %s", video)
        return ClipSuggestProductResponse(
            skipped_reason="clip video file not found",
            video_relative_path=video_rel,
            **base,
        )

    n = settings.auto_tag_clip_frame_count
    n = max(1, min(12, n))
    transcript_s = (transcript_text or "").strip()
    w_img = float(settings.suggest_weight_image)
    w_txt = float(settings.suggest_weight_text)
    if w_img <= 0 and w_txt <= 0:
        return ClipSuggestProductResponse(
            skipped_reason=(
                "suggest_weight_image and suggest_weight_text are both zero (nothing to score)"
            ),
            video_relative_path=video_rel,
            **base,
        )

    frame_paths: list[Path] = []
    work_dir: Path | None = None
    thumb_included = False
    extracted: list[Path] = []
    try:
        if w_img > 0:
            if thumbnail_path:
                try:
                    thumb = resolve_storage_media_path(thumbnail_path)
                    if thumb.is_file():
                        frame_paths.append(thumb)
                        thumb_included = True
                except (OSError, ValueError):
                    pass

            run_id = uuid4().hex[:8]
            if settings.debug_keep_suggest_clip_frames:
                ts = datetime.now(UTC).strftime("%Y%m%dT%H%M%S")
                sub = f"{ts}_{run_id}"
                work_dir = settings.storage_path / "debug" / "suggest_clip_frames" / sub
            else:
                work_dir = settings.storage_path / "tmp" / "clip_frames" / str(uuid4())
            extracted = extract_frames_evenly(video, n, work_dir)
            frame_paths.extend(extracted)

        def _resp(**kwargs: Any) -> ClipSuggestProductResponse:
            merged: dict[str, Any] = {**base, **kwargs}
            if settings.debug_keep_suggest_clip_frames and work_dir is not None:
                merged["debug_extracted_frames_dir"] = _storage_relative(work_dir)
            return ClipSuggestProductResponse(**merged)

        if settings.debug_keep_suggest_clip_frames and work_dir is not None:
            try:
                (work_dir / "README.txt").write_text(
                    f"video_relative_path={video_rel}\n"
                    f"thumbnail_included={thumb_included}\n"
                    f"extracted_jpeg_count={len(extracted)}\n",
                    encoding="utf-8",
                )
            except OSError:
                pass
            logger.info(
                "debug_keep_suggest_clip_frames: extracted JPEGs under %s",
                work_dir,
            )

        text_hits: list[SearchHit] = []
        text_search_used = False
        text_hit_rows: list[ClipSuggestTextHit] = []
        if w_txt > 0 and transcript_s:
            try:
                text_hits = await search_by_transcript(
                    transcript=transcript_s,
                    top_k=5,
                    http=http,
                )
            except (RuntimeError, ValueError) as exc:
                logger.debug("transcript search skipped: %s", exc)
                text_hits = []
            text_search_used = len(text_hits) > 0
            text_hit_rows = [
                ClipSuggestTextHit(
                    product_id=h.product_id,
                    score=h.score,
                    product_name=h.product_name,
                    product_description=h.product_description,
                )
                for h in text_hits
            ]

        segment_rows: list[ClipSuggestTranscriptSegmentRow] = []
        if w_txt > 0 and transcript_s:
            segment_rows = await _transcript_segment_evidence(
                transcript_s,
                http,
                text_hits=text_hits,
            )

        if w_img > 0 and not frame_paths:
            logger.debug("suggest_product_for_clip skip: no frames (thumb+extract empty)")
            return _resp(
                skipped_reason="could not extract any frames",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=0,
                transcript_segment_evidence=segment_rows,
                product_ranks=_build_rankings([], text_hits, w_img, w_txt),
            )

        frame_rows: list[ClipSuggestFrameRow] = []
        frames_searched = 0
        focus_prompt = (settings.suggest_image_embed_focus_prompt or "").strip()
        image_companion: str | None = focus_prompt if focus_prompt else None

        for i, fp in enumerate(frame_paths):
            is_thumb = thumb_included and i == 0
            src: str = "thumbnail" if is_thumb else "extracted"
            rel = _storage_relative(fp)
            try:
                hits = await search_by_media_path(
                    media_path=str(fp),
                    kind="image",
                    top_k=3,
                    http=http,
                    companion_text=image_companion,
                )
            except (OSError, ValueError, FileNotFoundError) as exc:
                logger.debug("frame search skip %s: %s", fp, exc)
                frame_rows.append(
                    ClipSuggestFrameRow(
                        index=i,
                        source=src,
                        media_relative_path=rel,
                        outcome="error",
                        error=str(exc),
                    ),
                )
                continue

            frames_searched += 1
            if not hits:
                frame_rows.append(
                    ClipSuggestFrameRow(
                        index=i,
                        source=src,
                        media_relative_path=rel,
                        outcome="no_hit",
                    ),
                )
                continue

            h = hits[0]
            ev = [
                ClipSuggestImageEvidenceHit(
                    product_id=x.product_id,
                    score=x.score,
                    product_name=x.product_name,
                    product_description=x.product_description,
                    catalog_media_relative_path=_catalog_media_rel(x.image_path),
                    catalog_source_url=x.source_url,
                    catalog_modality=_catalog_modality(x.modality),
                )
                for x in hits[:3]
            ]
            frame_rows.append(
                ClipSuggestFrameRow(
                    index=i,
                    source=src,
                    media_relative_path=rel,
                    outcome="hit",
                    top_product_id=h.product_id,
                    top_score=h.score,
                    top_product_name=h.product_name,
                    matched_product_description=h.product_description,
                    image_evidence_hits=ev,
                ),
            )

        # --- Unified scoring (absolute [0,1] scale, no relative normalization) ---
        product_ranks = _build_rankings(frame_rows, text_hits, w_img, w_txt)

        counts = Counter(
            row.top_product_id
            for row in frame_rows
            if row.outcome == "hit" and row.top_product_id is not None
        )
        votes_by_product = [
            ClipSuggestVoteRow(product_id=pid, vote_count=cnt) for pid, cnt in counts.most_common()
        ]

        common = dict(
            frames_used=len(frame_paths),
            frames_searched=frames_searched,
            video_relative_path=video_rel,
            thumbnail_used=thumb_included,
            extracted_frame_count=len(extracted),
            pick_method="unified_score",
            votes_by_product=votes_by_product,
            frame_rows=frame_rows,
            text_search_hits=text_hit_rows,
            text_search_used=text_search_used,
            transcript_segment_evidence=segment_rows,
            product_ranks=product_ranks,
        )

        if not product_ranks:
            return _resp(
                skipped_reason="no vector hits from frames or transcript",
                **common,
            )

        winner = product_ranks[0]
        max_dist = settings.auto_tag_clip_max_score
        min_fused = settings.suggest_min_fused_score

        if winner.final_score < min_fused:
            return _resp(
                skipped_reason=(
                    f"final score {winner.final_score:.4f} below minimum {min_fused:.4f}"
                ),
                candidate_product_id=winner.product_id,
                candidate_product_name=winner.product_name,
                candidate_score=winner.final_score,
                **common,
            )

        if (
            w_img > 0
            and winner.avg_frame_distance is not None
            and winner.avg_frame_distance > max_dist
        ):
            return _resp(
                skipped_reason=(
                    f"winner image distance {winner.avg_frame_distance:.4f} "
                    f"above threshold {max_dist:.4f}"
                ),
                candidate_product_id=winner.product_id,
                candidate_product_name=winner.product_name,
                candidate_score=winner.final_score,
                **common,
            )

        logger.debug(
            "suggest_product_for_clip matched pid=%s final=%.4f (img=%.4f txt=%.4f)",
            winner.product_id,
            winner.final_score,
            winner.image_score,
            winner.text_score,
        )
        return _resp(
            matched=True,
            product_id=winner.product_id,
            product_name=winner.product_name,
            best_score=winner.final_score,
            **common,
        )
    finally:
        if work_dir is not None and not settings.debug_keep_suggest_clip_frames:
            cleanup_work_dir(work_dir)
