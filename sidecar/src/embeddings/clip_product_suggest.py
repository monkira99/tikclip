"""Match a clip to a product: image frames, optional STT text hybrid, weighted fusion."""

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
        "suggest_min_fused_score": float(settings.suggest_min_fused_score),
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


def _norm_image_scores(distances: dict[int, float]) -> dict[int, float]:
    """Lower distance is better → higher norm in [0, 1]."""
    if not distances:
        return {}
    max_d = max(distances.values())
    if max_d <= 0:
        return {pid: 1.0 for pid in distances}
    return {pid: max(0.0, 1.0 - (d / max_d)) for pid, d in distances.items()}


def _norm_text_scores(hits: list[SearchHit]) -> dict[int, float]:
    if not hits:
        return {}
    max_s = max(h.score for h in hits)
    if max_s <= 0:
        return {h.product_id: 0.0 for h in hits}
    return {h.product_id: h.score / max_s for h in hits}


def _fuse(
    image_norm: dict[int, float],
    text_norm: dict[int, float],
    w_img: float,
    w_txt: float,
) -> list[tuple[int, float]]:
    pids = set(image_norm) | set(text_norm)
    scored = [
        (pid, w_img * image_norm.get(pid, 0.0) + w_txt * text_norm.get(pid, 0.0)) for pid in pids
    ]
    scored.sort(key=lambda x: x[1], reverse=True)
    return scored


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

    frame_paths: list[Path] = []
    work_dir: Path | None = None
    thumb_included = False
    try:
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
        if not frame_paths:
            logger.debug("suggest_product_for_clip skip: no frames (thumb+extract empty)")
            return ClipSuggestProductResponse(
                skipped_reason="could not extract any frames",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=0,
                **base,
            )

        text_hits: list[SearchHit] = []
        text_search_used = False
        text_hit_rows: list[ClipSuggestTextHit] = []
        if transcript_s:
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
                )
                for h in text_hits
            ]

        frame_rows: list[ClipSuggestFrameRow] = []
        top1: list[tuple[int, float, str | None]] = []
        frames_searched = 0

        for i, fp in enumerate(frame_paths):
            is_thumb = thumb_included and i == 0
            src: str = "thumbnail" if is_thumb else "extracted"
            rel = _storage_relative(fp)
            try:
                hits = await search_by_media_path(
                    media_path=str(fp),
                    kind="image",
                    top_k=1,
                    http=http,
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
            top1.append((h.product_id, h.score, h.product_name))
            frame_rows.append(
                ClipSuggestFrameRow(
                    index=i,
                    source=src,
                    media_relative_path=rel,
                    outcome="hit",
                    top_product_id=h.product_id,
                    top_score=h.score,
                    top_product_name=h.product_name,
                ),
            )

        if not top1 and not text_hits:
            logger.debug(
                "suggest_product_for_clip skip: no hits (frames=%s transcript=%s)",
                len(frame_paths),
                bool(transcript_s),
            )
            return ClipSuggestProductResponse(
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                skipped_reason="no vector hits from frames or transcript",
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=text_search_used,
                **base,
            )

        max_dist = settings.auto_tag_clip_max_score
        w_img = settings.suggest_weight_image
        w_txt = settings.suggest_weight_text
        min_fused = settings.suggest_min_fused_score

        # --- Image-only path (no usable text hits) ---
        if not text_hits:
            counts = Counter(pid for pid, _, _ in top1)
            winner_pid, win_count = counts.most_common(1)[0]
            half = (len(top1) + 1) // 2
            if win_count < half:
                best = min(top1, key=lambda t: t[1])
                winner_pid, win_score, win_name = best[0], best[1], best[2]
                pick = "min_distance_tiebreak"
            else:
                scores = [s for pid, s, _ in top1 if pid == winner_pid]
                win_score = min(scores)
                win_name = next((nm for pid, _, nm in top1 if pid == winner_pid), None)
                pick = "majority_vote"

            votes_by_product = [
                ClipSuggestVoteRow(product_id=pid, vote_count=cnt)
                for pid, cnt in counts.most_common()
            ]

            if win_score > max_dist:
                return ClipSuggestProductResponse(
                    frames_used=len(frame_paths),
                    frames_searched=frames_searched,
                    skipped_reason=(
                        f"best match distance {win_score:.4f} above threshold {max_dist:.4f}"
                    ),
                    video_relative_path=video_rel,
                    thumbnail_used=thumb_included,
                    extracted_frame_count=len(extracted),
                    pick_method=pick,
                    votes_by_product=votes_by_product,
                    candidate_product_id=winner_pid,
                    candidate_product_name=win_name,
                    candidate_score=win_score,
                    frame_rows=frame_rows,
                    text_search_hits=text_hit_rows,
                    text_search_used=False,
                    **base,
                )

            return ClipSuggestProductResponse(
                matched=True,
                product_id=winner_pid,
                product_name=win_name,
                best_score=win_score,
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                pick_method=pick,
                votes_by_product=votes_by_product,
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=False,
                **base,
            )

        # --- Hybrid: text hits present; fuse with image when available ---
        img_dist: dict[int, float] = {}
        for pid, score, _ in top1:
            img_dist[pid] = min(img_dist.get(pid, score), score)

        img_norm = _norm_image_scores(img_dist)
        txt_norm = _norm_text_scores(text_hits)
        fused = _fuse(img_norm, txt_norm, w_img, w_txt)
        winner_pid, fused_score = fused[0]
        win_name = next((h.product_name for h in text_hits if h.product_id == winner_pid), None)
        if win_name is None:
            win_name = next((nm for pid, _, nm in top1 if pid == winner_pid), None)

        counts = Counter(pid for pid, _, _ in top1)
        votes_by_product = [
            ClipSuggestVoteRow(product_id=pid, vote_count=cnt) for pid, cnt in counts.most_common()
        ]

        if fused_score < min_fused:
            return ClipSuggestProductResponse(
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                skipped_reason=(f"fused score {fused_score:.4f} below minimum {min_fused:.4f}"),
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                pick_method="weighted_fusion",
                votes_by_product=votes_by_product,
                candidate_product_id=winner_pid,
                candidate_product_name=win_name,
                candidate_score=fused_score,
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=True,
                fusion_method="weighted_score",
                **base,
            )

        if winner_pid in img_dist and img_dist[winner_pid] > max_dist:
            return ClipSuggestProductResponse(
                frames_used=len(frame_paths),
                frames_searched=frames_searched,
                skipped_reason=(
                    f"hybrid winner image distance {img_dist[winner_pid]:.4f} "
                    f"above threshold {max_dist:.4f}"
                ),
                video_relative_path=video_rel,
                thumbnail_used=thumb_included,
                extracted_frame_count=len(extracted),
                pick_method="weighted_fusion",
                votes_by_product=votes_by_product,
                candidate_product_id=winner_pid,
                candidate_product_name=win_name,
                candidate_score=fused_score,
                frame_rows=frame_rows,
                text_search_hits=text_hit_rows,
                text_search_used=True,
                fusion_method="weighted_score",
                **base,
            )

        logger.debug(
            "suggest_product_for_clip hybrid match pid=%s fused=%.4f",
            winner_pid,
            fused_score,
        )
        return ClipSuggestProductResponse(
            matched=True,
            product_id=winner_pid,
            product_name=win_name,
            best_score=fused_score,
            frames_used=len(frame_paths),
            frames_searched=frames_searched,
            video_relative_path=video_rel,
            thumbnail_used=thumb_included,
            extracted_frame_count=len(extracted),
            pick_method="weighted_fusion",
            votes_by_product=votes_by_product,
            frame_rows=frame_rows,
            text_search_hits=text_hit_rows,
            text_search_used=True,
            fusion_method="weighted_score",
            **base,
        )
    finally:
        if work_dir is not None:
            cleanup_work_dir(work_dir)
