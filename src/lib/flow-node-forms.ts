function num(value: unknown, fallback: number): number {
  const n = typeof value === "number" ? value : Number(value);
  return Number.isFinite(n) ? n : fallback;
}

function normalizeUsername(value: unknown): string {
  if (typeof value !== "string") {
    return "";
  }

  return value.trim().replace(/^@/, "").trim();
}

function stringValue(...values: unknown[]): string {
  for (const value of values) {
    if (typeof value === "string") {
      return value;
    }
  }
  return "";
}

function firstDefined(...values: unknown[]): unknown {
  for (const value of values) {
    if (value !== undefined && value !== null) {
      return value;
    }
  }
  return undefined;
}

function secondsToRoundedUpMinutes(value: unknown, fallbackSeconds: number): number {
  const seconds = Math.max(1, Math.floor(num(value, fallbackSeconds)));
  return Math.max(1, Math.ceil(seconds / 60));
}

export type StartNodeForm = {
  username: string;
  cookies_json: string;
  proxy_url: string;
  poll_interval_seconds: number;
  retry_limit: number;
};

export function parseStartNodeDraft(raw: string): StartNodeForm {
  let value: Record<string, unknown> = {};
  try {
    value = JSON.parse(raw || "{}") as Record<string, unknown>;
  } catch {
    value = {};
  }
  return {
    username: normalizeUsername(value.username),
    cookies_json: stringValue(value.cookies_json, value.cookiesJson),
    proxy_url: stringValue(value.proxy_url, value.proxyUrl),
    poll_interval_seconds: Math.max(
      5,
      Math.floor(num(firstDefined(value.poll_interval_seconds, value.pollIntervalSeconds), 60)),
    ),
    retry_limit: Math.max(0, Math.floor(num(firstDefined(value.retry_limit, value.retryLimit), 3))),
  };
}

export function serializeStartNodeDraft(form: StartNodeForm): string {
  return JSON.stringify({
    username: normalizeUsername(form.username),
    cookies_json: form.cookies_json,
    proxy_url: form.proxy_url,
    poll_interval_seconds: form.poll_interval_seconds,
    retry_limit: form.retry_limit,
  });
}

export type RecordNodeForm = {
  max_duration_minutes: number;
  speech_merge_gap_sec: number;
  stt_num_threads: number;
  stt_quantize: "auto" | "fp32" | "int8";
};

function sttQuantize(value: unknown): "auto" | "fp32" | "int8" {
  if (typeof value !== "string") {
    return "auto";
  }
  const normalized = value.trim().toLowerCase();
  if (normalized === "fp32" || normalized === "float32") {
    return "fp32";
  }
  if (normalized === "int8") {
    return "int8";
  }
  return "auto";
}

export function parseRecordNodeDraft(raw: string): RecordNodeForm {
  let value: Record<string, unknown> = {};
  try {
    value = JSON.parse(raw || "{}") as Record<string, unknown>;
  } catch {
    value = {};
  }

  const minutesValue = firstDefined(
    value.max_duration_minutes,
    value.maxDurationMinutes,
    value.maxDuration,
  );
  const secondsValue = firstDefined(value.max_duration_seconds, value.maxDurationSeconds, value.durationSeconds);

  return {
    max_duration_minutes:
      minutesValue !== undefined
        ? Math.max(1, Math.floor(num(minutesValue, 5)))
        : secondsToRoundedUpMinutes(secondsValue, 300),
    speech_merge_gap_sec: Math.max(0, num(value.speech_merge_gap_sec, 0.5)),
    stt_num_threads: Math.max(1, Math.floor(num(value.stt_num_threads, 4))),
    stt_quantize: sttQuantize(value.stt_quantize),
  };
}

export function serializeRecordNodeDraft(form: RecordNodeForm): string {
  return JSON.stringify({
    max_duration_minutes: form.max_duration_minutes,
    speech_merge_gap_sec: form.speech_merge_gap_sec,
    stt_num_threads: form.stt_num_threads,
    stt_quantize: form.stt_quantize,
  });
}

export type ClipNodeForm = {
  clip_min_duration: number;
  clip_max_duration: number;
  speech_cut_tolerance_sec: number;
};

export function parseClipNodeDraft(raw: string): ClipNodeForm {
  let value: Record<string, unknown> = {};
  try {
    value = JSON.parse(raw || "{}") as Record<string, unknown>;
  } catch {
    value = {};
  }
  return {
    clip_min_duration: Math.max(1, Math.floor(num(value.clip_min_duration, 15))),
    clip_max_duration: Math.max(1, Math.floor(num(value.clip_max_duration, 120))),
    speech_cut_tolerance_sec: Math.max(0, num(value.speech_cut_tolerance_sec, 0.4)),
  };
}

export function serializeClipNodeDraft(form: ClipNodeForm): string {
  return JSON.stringify({
    clip_min_duration: form.clip_min_duration,
    clip_max_duration: form.clip_max_duration,
    speech_cut_tolerance_sec: form.speech_cut_tolerance_sec,
  });
}

export type CaptionNodeForm = {
  inherit_global_defaults: boolean;
  model: string;
};

export function parseCaptionNodeDraft(raw: string): CaptionNodeForm {
  let value: Record<string, unknown> = {};
  try {
    value = JSON.parse(raw || "{}") as Record<string, unknown>;
  } catch {
    value = {};
  }
  return {
    inherit_global_defaults: value.inherit_global_defaults !== false,
    model: typeof value.model === "string" ? value.model : "",
  };
}

export function serializeCaptionNodeDraft(form: CaptionNodeForm): string {
  const o: Record<string, unknown> = {
    inherit_global_defaults: form.inherit_global_defaults,
  };
  if (form.model.trim()) {
    o.model = form.model.trim();
  }
  return JSON.stringify(o);
}
