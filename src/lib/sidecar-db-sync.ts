import { invoke, isTauri } from "@tauri-apps/api/core";

function coerceFiniteNumber(v: unknown, fallback = 0): number {
  if (typeof v === "number" && Number.isFinite(v)) {
    return v;
  }
  if (typeof v === "string") {
    const n = Number(v);
    if (Number.isFinite(n)) {
      return n;
    }
  }
  return fallback;
}

/**
 * Insert a clip row from a `clip_ready` WebSocket payload.
 * Returns SQLite clip id when a row was inserted or already existed; otherwise null.
 */
export async function insertClipFromSidecarWsPayload(
  data: Record<string, unknown>,
): Promise<number | null> {
  if (!isTauri()) {
    return null;
  }
  const sidecar_recording_id =
    typeof data.recording_id === "string" ? data.recording_id : null;
  if (!sidecar_recording_id) {
    return null;
  }

  const account_id = coerceFiniteNumber(data.account_id, 0);
  if (account_id <= 0) {
    return null;
  }

  const file_path = typeof data.path === "string" ? data.path : "";
  if (!file_path) {
    return null;
  }

  const thumbnail_path =
    typeof data.thumbnail_path === "string" ? data.thumbnail_path : "";
  const duration_sec = coerceFiniteNumber(data.duration_sec, 0);
  const start_sec = coerceFiniteNumber(data.start_sec, 0);
  const end_sec = coerceFiniteNumber(data.end_sec, 0);
  const transcript_text =
    typeof data.transcript_text === "string" && data.transcript_text.trim() !== ""
      ? data.transcript_text.trim()
      : null;

  const clipId = await invoke<number>("insert_clip_from_sidecar", {
    input: {
      sidecar_recording_id,
      account_id,
      file_path,
      thumbnail_path,
      duration_sec,
      start_sec,
      end_sec,
      transcript_text,
    },
  });
  return typeof clipId === "number" && Number.isFinite(clipId) ? clipId : null;
}

/**
 * Insert a `speech_segments` row from a `speech_segment_ready` WebSocket payload.
 */
export async function insertSpeechSegmentFromWsPayload(
  data: Record<string, unknown>,
): Promise<number | null> {
  if (!isTauri()) {
    return null;
  }
  const sidecar_recording_id =
    typeof data.recording_id === "string" ? data.recording_id : null;
  if (!sidecar_recording_id) {
    return null;
  }

  const account_id = coerceFiniteNumber(data.account_id, 0);
  if (account_id <= 0) {
    return null;
  }

  const start_time = coerceFiniteNumber(data.start_sec, NaN);
  const end_time = coerceFiniteNumber(data.end_sec, NaN);
  if (!Number.isFinite(start_time) || !Number.isFinite(end_time)) {
    return null;
  }

  const text = typeof data.text === "string" ? data.text : "";
  const confidenceRaw = data.confidence;
  const confidence =
    typeof confidenceRaw === "number" && Number.isFinite(confidenceRaw)
      ? confidenceRaw
      : null;

  const id = await invoke<number>("insert_speech_segment", {
    input: {
      sidecar_recording_id,
      account_id,
      start_time,
      end_time,
      text,
      confidence,
    },
  });
  return typeof id === "number" && Number.isFinite(id) ? id : null;
}

/**
 * Update clip caption from `caption_ready` WebSocket payload.
 */
export async function syncClipCaptionFromWsPayload(
  data: Record<string, unknown>,
): Promise<boolean> {
  if (!isTauri()) {
    return false;
  }
  const clipIdRaw = data.clip_id;
  const clipId =
    typeof clipIdRaw === "number" ? clipIdRaw : typeof clipIdRaw === "string" ? Number(clipIdRaw) : NaN;
  if (!Number.isFinite(clipId) || clipId <= 0) {
    return false;
  }
  const captionTextRaw = data.caption_text;
  const captionText =
    typeof captionTextRaw === "string" && captionTextRaw.trim() !== "" ? captionTextRaw : null;
  if (!captionText) {
    return false;
  }

  await invoke("update_clip_caption", {
    clipId: Math.trunc(clipId),
    captionText,
    captionStatus: "completed",
    captionError: null,
  });
  return true;
}
