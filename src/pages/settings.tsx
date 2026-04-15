import { useCallback, useEffect, useId, useState } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  applyStorageRoot,
  getAppDataPaths,
  getSetting,
  getStorageStats,
  openPathInSystem,
  pickStorageRootFolder,
  resetStorageRootDefault,
  restartSidecar,
  runStorageCleanupNow,
  setSetting,
  storageRootIsCustom,
  type AppDataPaths,
  type StorageStats,
} from "@/lib/api";
import { resyncSidecarWatchers } from "@/lib/resync-sidecar-watchers";
import { cn } from "@/lib/utils";

const fieldSurface =
  "border-[var(--color-border)] bg-[var(--color-bg)] text-[var(--color-text)]";

/** Mirrors `sidecar/src/config.py` defaults when SQLite has no row. */
const DEFAULTS = {
  maxConcurrent: "5",
  pollInterval: "30",
  clipMin: "15",
  clipMax: "90",
  /** Minutes per recording when auto-record does not override (maps to TIKCLIP_MAX_DURATION_MINUTES). */
  recordingMaxMinutes: "5",
  geminiEmbeddingModel: "gemini-embedding-2-preview",
  geminiEmbeddingDim: "1536",
  autoTagClipFrames: "4",
  autoTagClipMaxScore: "0.35",
  suggestWeightImage: "0.6",
  suggestWeightText: "0.4",
  suggestMinFusedScore: "0.25",
  suggestImageEmbedFocusPrompt:
    "Focus on the main product in this image for similarity to product catalog photos.",
  speechMergeGapSec: "0.5",
  speechCutToleranceSec: "1.5",
  sttNumThreads: "4",
} as const;

const KEY_AUDIO_PROCESSING = "audio_processing_enabled";
const KEY_SPEECH_MERGE_GAP = "speech_merge_gap_sec";
const KEY_SPEECH_CUT_TOLERANCE = "speech_cut_tolerance_sec";
const KEY_STT_NUM_THREADS = "stt_num_threads";
const KEY_STT_QUANTIZE = "stt_quantize";

function valueFromDb(db: string | null, fallback: string): string {
  if (db === null) {
    return fallback;
  }
  return db;
}

const AUTO_PROCESS_AFTER_RECORD_KEY = "auto_process_after_record";

const KEY_PRODUCT_VECTOR = "product_vector_enabled";
const KEY_GEMINI_API_KEY = "gemini_api_key";
const KEY_GEMINI_EMBEDDING_MODEL = "gemini_embedding_model";
const KEY_GEMINI_EMBEDDING_DIM = "gemini_embedding_dimensions";
const KEY_AUTO_TAG_CLIP = "auto_tag_clip_product_enabled";
const KEY_AUTO_TAG_FRAMES = "auto_tag_clip_frame_count";
const KEY_AUTO_TAG_MAX_SCORE = "auto_tag_clip_max_score";
const KEY_SUGGEST_WEIGHT_IMAGE = "suggest_weight_image";
const KEY_SUGGEST_WEIGHT_TEXT = "suggest_weight_text";
const KEY_SUGGEST_MIN_FUSED_SCORE = "suggest_min_fused_score";
const KEY_DEBUG_KEEP_SUGGEST_FRAMES = "debug_keep_suggest_clip_frames";
const KEY_SUGGEST_IMAGE_EMBED_FOCUS_PROMPT = "suggest_image_embed_focus_prompt";

const KEY_RAW_RETENTION = "TIKCLIP_RAW_RETENTION_DAYS";
const KEY_ARCHIVE_RETENTION = "TIKCLIP_ARCHIVE_RETENTION_DAYS";
const KEY_STORAGE_WARN = "TIKCLIP_STORAGE_WARN_PERCENT";
const KEY_STORAGE_CLEANUP = "TIKCLIP_STORAGE_CLEANUP_PERCENT";

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) {
    return "0 B";
  }
  const gb = n / (1024 * 1024 * 1024);
  if (gb >= 1) {
    return gb >= 10 ? `${gb.toFixed(1)} GB` : `${gb.toFixed(2)} GB`;
  }
  const mb = n / (1024 * 1024);
  if (mb >= 1) {
    return mb >= 100 ? `${mb.toFixed(0)} MB` : `${mb.toFixed(1)} MB`;
  }
  return `${Math.round(n / 1024)} KB`;
}

function parseAutoProcessAfterRecord(raw: string | null): boolean {
  if (raw === null || raw.trim() === "") {
    return true;
  }
  const t = raw.trim().toLowerCase();
  return t === "1" || t === "true" || t === "yes" || t === "on";
}

function parseProductVectorEnabled(raw: string | null): boolean {
  if (raw === null || raw.trim() === "") {
    return false;
  }
  const t = raw.trim().toLowerCase();
  return t === "1" || t === "true" || t === "yes" || t === "on";
}

function parseAutoTagClipProductEnabled(raw: string | null): boolean {
  return parseProductVectorEnabled(raw);
}

function parseAudioProcessingEnabled(raw: string | null): boolean {
  if (raw === null || raw.trim() === "") {
    return true;
  }
  const t = raw.trim().toLowerCase();
  return t === "1" || t === "true" || t === "yes" || t === "on";
}

function PathRow({
  label,
  description,
  path,
  onOpen,
  opening,
}: {
  label: string;
  description?: string;
  path: string;
  onOpen: () => void;
  opening: boolean;
}) {
  return (
    <div className="space-y-2">
      <div className="flex flex-col gap-0.5 sm:flex-row sm:items-baseline sm:justify-between">
        <Label className="text-[var(--color-text)]">{label}</Label>
        {description ? (
          <span className="text-xs text-[var(--color-text-muted)]">{description}</span>
        ) : null}
      </div>
      <div className="flex flex-col gap-2 sm:flex-row sm:items-stretch">
        <div
          className={`min-h-10 flex-1 rounded-md border px-3 py-2 font-mono text-xs break-all ${fieldSurface}`}
        >
          {path}
        </div>
        <Button
          type="button"
          variant="outline"
          className="shrink-0 border-[var(--color-border)]"
          disabled={opening || !path}
          onClick={() => onOpen()}
        >
          <FolderOpen className="mr-2 size-4 opacity-80" aria-hidden />
          Mở thư mục
        </Button>
      </div>
    </div>
  );
}

export function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [paths, setPaths] = useState<AppDataPaths | null>(null);
  const [maxConcurrent, setMaxConcurrent] = useState("");
  const [pollInterval, setPollInterval] = useState("");
  const [recordingMaxMinutes, setRecordingMaxMinutes] = useState("");
  const [clipMinDuration, setClipMinDuration] = useState("");
  const [clipMaxDuration, setClipMaxDuration] = useState("");
  const [maxStorageGb, setMaxStorageGb] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [storageIsCustom, setStorageIsCustom] = useState(false);
  const [pickingRoot, setPickingRoot] = useState(false);
  const [autoProcessAfterRecord, setAutoProcessAfterRecord] = useState(true);
  const [autoProcessToggleBusy, setAutoProcessToggleBusy] = useState(false);
  const autoProcessSwitchId = useId();
  const [rawRetentionDays, setRawRetentionDays] = useState("7");
  const [archiveRetentionDays, setArchiveRetentionDays] = useState("0");
  const [storageWarnPercent, setStorageWarnPercent] = useState("80");
  const [storageCleanupPercent, setStorageCleanupPercent] = useState("95");
  const [storageStats, setStorageStats] = useState<StorageStats | null>(null);
  const [storageScanBusy, setStorageScanBusy] = useState(false);
  const [storageStatsError, setStorageStatsError] = useState<string | null>(null);
  const [storageCleanupBusy, setStorageCleanupBusy] = useState(false);
  const [productVectorEnabled, setProductVectorEnabled] = useState(false);
  const [geminiApiKey, setGeminiApiKey] = useState("");
  const [geminiEmbeddingModel, setGeminiEmbeddingModel] = useState("");
  const [geminiEmbeddingDim, setGeminiEmbeddingDim] = useState("");
  const [autoTagClipProductEnabled, setAutoTagClipProductEnabled] = useState(false);
  const [autoTagClipFrameCount, setAutoTagClipFrameCount] = useState("");
  const [autoTagClipMaxScore, setAutoTagClipMaxScore] = useState("");
  const [suggestWeightImage, setSuggestWeightImage] = useState("");
  const [suggestWeightText, setSuggestWeightText] = useState("");
  const [suggestMinFusedScore, setSuggestMinFusedScore] = useState("");
  const [debugKeepSuggestClipFrames, setDebugKeepSuggestClipFrames] = useState(false);
  const [suggestImageEmbedFocusPrompt, setSuggestImageEmbedFocusPrompt] = useState("");
  const autoTagClipSwitchId = useId();
  const debugSuggestFramesSwitchId = useId();
  const [audioProcessingEnabled, setAudioProcessingEnabled] = useState(true);
  const [speechMergeGapSec, setSpeechMergeGapSec] = useState("");
  const [speechCutToleranceSec, setSpeechCutToleranceSec] = useState("");
  const [sttNumThreads, setSttNumThreads] = useState("");
  const [sttQuantize, setSttQuantize] = useState<"auto" | "fp32" | "int8">("auto");
  const audioProcessingSwitchId = useId();

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [
          pathInfo,
          isCustom,
          mc,
          pi,
          rmin,
          rhLegacy,
          cmin,
          cmax,
          sg,
          autoProc,
          rawR,
          archR,
          sw,
          sc,
          pvEn,
          gKey,
          gModel,
          gDim,
          atEn,
          atFrames,
          atScore,
          swImg,
          swTxt,
          smf,
          dbgFrames,
          imgFocusPrompt,
          apEn,
          smg,
          sct,
          sttTh,
          sttQ,
        ] = await Promise.all([
          getAppDataPaths(),
          storageRootIsCustom(),
          getSetting("max_concurrent"),
          getSetting("poll_interval"),
          getSetting("recording_max_minutes"),
          getSetting("recording_max_hours"),
          getSetting("clip_min_duration"),
          getSetting("clip_max_duration"),
          getSetting("max_storage_gb"),
          getSetting(AUTO_PROCESS_AFTER_RECORD_KEY),
          getSetting(KEY_RAW_RETENTION),
          getSetting(KEY_ARCHIVE_RETENTION),
          getSetting(KEY_STORAGE_WARN),
          getSetting(KEY_STORAGE_CLEANUP),
          getSetting(KEY_PRODUCT_VECTOR),
          getSetting(KEY_GEMINI_API_KEY),
          getSetting(KEY_GEMINI_EMBEDDING_MODEL),
          getSetting(KEY_GEMINI_EMBEDDING_DIM),
          getSetting(KEY_AUTO_TAG_CLIP),
          getSetting(KEY_AUTO_TAG_FRAMES),
          getSetting(KEY_AUTO_TAG_MAX_SCORE),
          getSetting(KEY_SUGGEST_WEIGHT_IMAGE),
          getSetting(KEY_SUGGEST_WEIGHT_TEXT),
          getSetting(KEY_SUGGEST_MIN_FUSED_SCORE),
          getSetting(KEY_DEBUG_KEEP_SUGGEST_FRAMES),
          getSetting(KEY_SUGGEST_IMAGE_EMBED_FOCUS_PROMPT),
          getSetting(KEY_AUDIO_PROCESSING),
          getSetting(KEY_SPEECH_MERGE_GAP),
          getSetting(KEY_SPEECH_CUT_TOLERANCE),
          getSetting(KEY_STT_NUM_THREADS),
          getSetting(KEY_STT_QUANTIZE),
        ]);
        if (cancelled) return;
        setPaths(pathInfo);
        setStorageIsCustom(isCustom);
        setMaxConcurrent(valueFromDb(mc, DEFAULTS.maxConcurrent));
        setPollInterval(valueFromDb(pi, DEFAULTS.pollInterval));
        let initialMinutes = rmin;
        if (initialMinutes === null && rhLegacy !== null && rhLegacy.trim() !== "") {
          const h = Number(rhLegacy.trim());
          if (!Number.isNaN(h) && Number.isInteger(h) && h > 0) {
            initialMinutes = String(h * 60);
          }
        }
        setRecordingMaxMinutes(valueFromDb(initialMinutes, DEFAULTS.recordingMaxMinutes));
        setClipMinDuration(valueFromDb(cmin, DEFAULTS.clipMin));
        setClipMaxDuration(valueFromDb(cmax, DEFAULTS.clipMax));
        setMaxStorageGb(sg === null ? "" : sg);
        setAutoProcessAfterRecord(parseAutoProcessAfterRecord(autoProc));
        setRawRetentionDays(valueFromDb(rawR, "7"));
        setArchiveRetentionDays(valueFromDb(archR, "0"));
        setStorageWarnPercent(valueFromDb(sw, "80"));
        setStorageCleanupPercent(valueFromDb(sc, "95"));
        setProductVectorEnabled(parseProductVectorEnabled(pvEn));
        setGeminiApiKey(gKey ?? "");
        setGeminiEmbeddingModel(valueFromDb(gModel, DEFAULTS.geminiEmbeddingModel));
        setGeminiEmbeddingDim(valueFromDb(gDim, DEFAULTS.geminiEmbeddingDim));
        setAutoTagClipProductEnabled(parseAutoTagClipProductEnabled(atEn));
        setAutoTagClipFrameCount(valueFromDb(atFrames, DEFAULTS.autoTagClipFrames));
        setAutoTagClipMaxScore(valueFromDb(atScore, DEFAULTS.autoTagClipMaxScore));
        setSuggestWeightImage(valueFromDb(swImg, DEFAULTS.suggestWeightImage));
        setSuggestWeightText(valueFromDb(swTxt, DEFAULTS.suggestWeightText));
        setSuggestMinFusedScore(valueFromDb(smf, DEFAULTS.suggestMinFusedScore));
        setDebugKeepSuggestClipFrames(parseProductVectorEnabled(dbgFrames));
        setSuggestImageEmbedFocusPrompt(
          valueFromDb(imgFocusPrompt, DEFAULTS.suggestImageEmbedFocusPrompt),
        );
        setAudioProcessingEnabled(parseAudioProcessingEnabled(apEn));
        setSpeechMergeGapSec(valueFromDb(smg, DEFAULTS.speechMergeGapSec));
        setSpeechCutToleranceSec(valueFromDb(sct, DEFAULTS.speechCutToleranceSec));
        setSttNumThreads(valueFromDb(sttTh, DEFAULTS.sttNumThreads));
        const q = (sttQ ?? "").trim().toLowerCase();
        if (q === "fp32" || q === "float32") {
          setSttQuantize("fp32");
        } else if (q === "int8") {
          setSttQuantize("int8");
        } else {
          setSttQuantize("auto");
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "Failed to load settings");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const openPath = useCallback(async (dir: string) => {
    setOpeningPath(dir);
    setError(null);
    try {
      await openPathInSystem(dir);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Không mở được thư mục");
    } finally {
      setOpeningPath(null);
    }
  }, []);

  const clearFeedback = useCallback(() => {
    setMessage(null);
    setError(null);
  }, []);

  const fetchStorageStats = useCallback(
    async (opts?: {
      announce?: boolean;
      signal?: AbortSignal;
      showBusy?: boolean;
      /** When false, failed refresh does not clear existing stats (e.g. after cleanup). */
      clearStatsOnError?: boolean;
    }) => {
      const announce = opts?.announce ?? false;
      const showBusy = opts?.showBusy !== false;
      const clearStatsOnError = opts?.clearStatsOnError !== false;
      const sig = opts?.signal;
      const aborted = () => sig?.aborted ?? false;

      if (announce) clearFeedback();
      if (showBusy) setStorageScanBusy(true);
      setStorageStatsError(null);
      try {
        const s = await getStorageStats();
        if (aborted()) return;
        setStorageStats(s);
        if (announce) {
          setMessage("Đã cập nhật số liệu lưu trữ từ sidecar.");
        }
      } catch (e) {
        if (aborted()) return;
        const msg = e instanceof Error ? e.message : "Không lấy được số liệu lưu trữ.";
        if (clearStatsOnError) {
          setStorageStats(null);
        }
        setStorageStatsError(msg);
        if (announce) {
          setError(msg);
        }
      } finally {
        if (showBusy && !aborted()) {
          setStorageScanBusy(false);
        }
      }
    },
    [clearFeedback],
  );

  useEffect(() => {
    if (loading) return;
    const ac = new AbortController();
    void fetchStorageStats({ signal: ac.signal });
    return () => ac.abort();
  }, [loading, fetchStorageStats]);

  const chooseStorageRoot = useCallback(async () => {
    clearFeedback();
    setPickingRoot(true);
    try {
      const picked = await pickStorageRootFolder();
      if (!picked) return;
      const ok = window.confirm(
        "Ứng dụng sẽ khởi động lại để dùng thư mục gốc mới. CSDL và file sẽ đọc từ đường dẫn đã chọn (thư mục/data/app.db). Tiếp tục?",
      );
      if (!ok) return;
      await applyStorageRoot(picked);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Không chọn được thư mục");
    } finally {
      setPickingRoot(false);
    }
  }, [clearFeedback]);

  const restoreDefaultStorageRoot = useCallback(async () => {
    clearFeedback();
    const ok = window.confirm(
      "Xóa thư mục gốc tùy chỉnh và khởi động lại? Lần sau app dùng lại quy tắc mặc định (~/.tikclip hoặc bản đã migrate).",
    );
    if (!ok) return;
    try {
      await resetStorageRootDefault();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Không đặt lại được");
    }
  }, [clearFeedback]);

  const saveRecording = useCallback(async () => {
    clearFeedback();
    const mc = maxConcurrent.trim();
    const pi = pollInterval.trim();
    if (mc && Number.isNaN(Number(mc))) {
      setError("Max concurrent must be a number.");
      return;
    }
    if (pi && Number.isNaN(Number(pi))) {
      setError("Poll interval must be a number.");
      return;
    }
    const rmin = recordingMaxMinutes.trim();
    if (rmin && Number.isNaN(Number(rmin))) {
      setError("Thời lượng tối đa mỗi lần ghi phải là số (phút).");
      return;
    }
    if (rmin) {
      const n = Number(rmin);
      if (!Number.isInteger(n) || n < 1 || n > 10080) {
        setError("Thời lượng ghi: nhập số nguyên phút từ 1 đến 10080 (tối đa 7 ngày).");
        return;
      }
    }
    setSaving("recording");
    try {
      await setSetting("max_concurrent", mc);
      await setSetting("poll_interval", pi);
      await setSetting("recording_max_minutes", rmin);
      await setSetting("recording_max_hours", "");
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage("Recording settings saved. Sidecar restarted to apply.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [clearFeedback, maxConcurrent, pollInterval, recordingMaxMinutes]);

  const onAutoProcessAfterRecordChange = useCallback(
    async (checked: boolean) => {
      clearFeedback();
      const previous = autoProcessAfterRecord;
      setAutoProcessAfterRecord(checked);
      setAutoProcessToggleBusy(true);
      try {
        await setSetting(AUTO_PROCESS_AFTER_RECORD_KEY, checked ? "1" : "0");
        await restartSidecar();
        await resyncSidecarWatchers();
        setMessage(
          checked
            ? "Đã bật tự xử lý clip sau khi ghi. Sidecar đã khởi động lại."
            : "Đã tắt tự xử lý clip sau khi ghi. Sidecar đã khởi động lại.",
        );
      } catch (e) {
        setAutoProcessAfterRecord(previous);
        setError(e instanceof Error ? e.message : "Không lưu được cài đặt");
      } finally {
        setAutoProcessToggleBusy(false);
      }
    },
    [autoProcessAfterRecord, clearFeedback],
  );

  const saveProductVector = useCallback(async () => {
    clearFeedback();
    const dimStr = geminiEmbeddingDim.trim();
    if (dimStr) {
      const n = Number(dimStr);
      if (!Number.isInteger(n) || n < 1 || n > 8192) {
        setError("Giá trị “số chiều” phải là số nguyên từ 1 đến 8192 (thường giữ 1536).");
        return;
      }
    }
    const framesStr = autoTagClipFrameCount.trim();
    if (framesStr) {
      const fn = Number(framesStr);
      if (!Number.isInteger(fn) || fn < 1 || fn > 12) {
        setError("Số ảnh lấy từ mỗi clip phải là số nguyên từ 1 đến 12.");
        return;
      }
    }
    const scoreStr = autoTagClipMaxScore.trim();
    if (scoreStr) {
      const sn = Number(scoreStr);
      if (!Number.isFinite(sn) || sn <= 0 || sn > 2) {
        setError("Ngưỡng độ khớp phải là số dương (ví dụ 0.35, tối đa 2).");
        return;
      }
    }
    const wImgStr = suggestWeightImage.trim();
    const wTxtStr = suggestWeightText.trim();
    const minFusStr = suggestMinFusedScore.trim();
    if (wImgStr) {
      const n = Number(wImgStr);
      if (!Number.isFinite(n) || n < 0 || n > 1) {
        setError("Trọng số ảnh (fusion) phải từ 0 đến 1.");
        return;
      }
    }
    if (wTxtStr) {
      const n = Number(wTxtStr);
      if (!Number.isFinite(n) || n < 0 || n > 1) {
        setError("Trọng số chữ (fusion) phải từ 0 đến 1.");
        return;
      }
    }
    if (minFusStr) {
      const n = Number(minFusStr);
      if (!Number.isFinite(n) || n < 0 || n > 1) {
        setError("Ngưỡng điểm fusion tối thiểu phải từ 0 đến 1.");
        return;
      }
    }
    setSaving("product_vector");
    try {
      await setSetting(KEY_PRODUCT_VECTOR, productVectorEnabled ? "1" : "0");
      await setSetting(KEY_GEMINI_API_KEY, geminiApiKey.trim());
      await setSetting(KEY_GEMINI_EMBEDDING_MODEL, geminiEmbeddingModel.trim());
      await setSetting(KEY_GEMINI_EMBEDDING_DIM, dimStr);
      await setSetting(KEY_AUTO_TAG_CLIP, autoTagClipProductEnabled ? "1" : "0");
      await setSetting(KEY_AUTO_TAG_FRAMES, framesStr);
      await setSetting(KEY_AUTO_TAG_MAX_SCORE, scoreStr);
      await setSetting(KEY_SUGGEST_WEIGHT_IMAGE, wImgStr);
      await setSetting(KEY_SUGGEST_WEIGHT_TEXT, wTxtStr);
      await setSetting(KEY_SUGGEST_MIN_FUSED_SCORE, minFusStr);
      await setSetting(KEY_DEBUG_KEEP_SUGGEST_FRAMES, debugKeepSuggestClipFrames ? "1" : "0");
      await setSetting(
        KEY_SUGGEST_IMAGE_EMBED_FOCUS_PROMPT,
        suggestImageEmbedFocusPrompt.trim(),
      );
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage(
        "Đã lưu. Dịch vụ nền đã khởi động lại để áp dụng cài đặt nhận diện sản phẩm.",
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [
    clearFeedback,
    productVectorEnabled,
    geminiApiKey,
    geminiEmbeddingModel,
    geminiEmbeddingDim,
    autoTagClipProductEnabled,
    autoTagClipFrameCount,
    autoTagClipMaxScore,
    suggestWeightImage,
    suggestWeightText,
    suggestMinFusedScore,
    debugKeepSuggestClipFrames,
    suggestImageEmbedFocusPrompt,
  ]);

  const saveClips = useCallback(async () => {
    clearFeedback();
    const mn = clipMinDuration.trim();
    const mx = clipMaxDuration.trim();
    if (mn && Number.isNaN(Number(mn))) {
      setError("Min duration must be a number.");
      return;
    }
    if (mx && Number.isNaN(Number(mx))) {
      setError("Max duration must be a number.");
      return;
    }
    if (mn && mx && Number(mn) > Number(mx)) {
      setError("Min duration cannot be greater than max duration.");
      return;
    }
    const gapStr = speechMergeGapSec.trim();
    const tolStr = speechCutToleranceSec.trim();
    const thStr = sttNumThreads.trim();
    if (gapStr) {
      const g = Number(gapStr);
      if (!Number.isFinite(g) || g < 0 || g > 5) {
        setError("Khoảng gộp khoảng lặng (giây) phải từ 0 đến 5.");
        return;
      }
    }
    if (tolStr) {
      const t = Number(tolStr);
      if (!Number.isFinite(t) || t < 0.05 || t > 10) {
        setError("Dung sai cắt hybrid (giây) phải từ 0.05 đến 10 (khuyến nghị ~1.5).");
        return;
      }
    }
    if (thStr) {
      const n = Number(thStr);
      if (!Number.isInteger(n) || n < 1 || n > 32) {
        setError("Số luồng STT phải là số nguyên từ 1 đến 32.");
        return;
      }
    }
    setSaving("clips");
    try {
      await setSetting("clip_min_duration", mn);
      await setSetting("clip_max_duration", mx);
      await setSetting(KEY_AUDIO_PROCESSING, audioProcessingEnabled ? "1" : "0");
      await setSetting(KEY_SPEECH_MERGE_GAP, gapStr || DEFAULTS.speechMergeGapSec);
      await setSetting(KEY_SPEECH_CUT_TOLERANCE, tolStr || DEFAULTS.speechCutToleranceSec);
      await setSetting(KEY_STT_NUM_THREADS, thStr || DEFAULTS.sttNumThreads);
      await setSetting(KEY_STT_QUANTIZE, sttQuantize);
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage(
        "Đã lưu cài đặt xử lý clip và âm thanh (VAD/STT). Sidecar đã khởi động lại để áp dụng.",
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [
    clearFeedback,
    clipMinDuration,
    clipMaxDuration,
    audioProcessingEnabled,
    speechMergeGapSec,
    speechCutToleranceSec,
    sttNumThreads,
    sttQuantize,
  ]);

  const saveStorageCard = useCallback(async () => {
    clearFeedback();
    const sg = maxStorageGb.trim();
    if (sg && Number.isNaN(Number(sg))) {
      setError("Giới hạn dung lượng (GB) phải là số hoặc để trống.");
      return;
    }
    const raw = rawRetentionDays.trim();
    const arch = archiveRetentionDays.trim();
    const w = storageWarnPercent.trim();
    const c = storageCleanupPercent.trim();
    for (const [label, v] of [
      ["Bản ghi thô — xóa sau (ngày)", raw],
      ["Clip — xóa sau (ngày)", arch],
      ["Ngưỡng cảnh báo (%)", w],
      ["Ngưỡng nghiêm trọng (%)", c],
    ] as const) {
      if (v && Number.isNaN(Number(v))) {
        setError(`${label} phải là số.`);
        return;
      }
    }
    const wn = w ? Number(w) : 80;
    const cn = c ? Number(c) : 95;
    if (wn < 1 || wn > 100 || cn < 1 || cn > 100) {
      setError("Ngưỡng % phải từ 1 đến 100.");
      return;
    }
    if (cn < wn) {
      setError("Ngưỡng nghiêm trọng (%) nên lớn hơn hoặc bằng ngưỡng cảnh báo (%) — ví dụ 95 và 80.");
      return;
    }
    setSaving("storage_card");
    try {
      await setSetting("max_storage_gb", sg);
      await setSetting(KEY_RAW_RETENTION, raw || "7");
      await setSetting(KEY_ARCHIVE_RETENTION, arch || "0");
      await setSetting(KEY_STORAGE_WARN, w || "80");
      await setSetting(KEY_STORAGE_CLEANUP, c || "95");
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage(
        "Đã lưu giới hạn dung lượng, dọn dữ liệu và cảnh báo. Sidecar đã khởi động lại để áp dụng.",
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [
    clearFeedback,
    maxStorageGb,
    rawRetentionDays,
    archiveRetentionDays,
    storageWarnPercent,
    storageCleanupPercent,
  ]);

  const runCleanupManual = useCallback(async () => {
    clearFeedback();
    const rawStr = rawRetentionDays.trim();
    const archStr = archiveRetentionDays.trim();
    const rawN = rawStr === "" ? 7 : Number(rawStr);
    const archN = archStr === "" ? 0 : Number(archStr);
    if (!Number.isFinite(rawN) || rawN < 0 || !Number.isFinite(archN) || archN < 0) {
      setError("Số ngày xóa bản ghi thô / clip phải là số không âm.");
      return;
    }
    setStorageCleanupBusy(true);
    try {
      const summary = await runStorageCleanupNow({
        raw_retention_days: rawN,
        archive_retention_days: archN,
      });
      const mb = summary.freed_bytes / (1024 * 1024);
      setMessage(
        `Cleanup xong: ${summary.deleted_recordings} recording(s), ${summary.deleted_clips} clip(s), ~${mb.toFixed(1)} MB.`,
      );
      await fetchStorageStats({ showBusy: false, clearStatsOnError: false });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Cleanup thất bại");
    } finally {
      setStorageCleanupBusy(false);
    }
  }, [clearFeedback, fetchStorageStats, rawRetentionDays, archiveRetentionDays]);

  if (loading) {
    return (
      <p className="text-sm text-[var(--color-text-muted)]">Loading settings…</p>
    );
  }

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      {(message || error) && (
        <p
          className={`text-sm ${error ? "text-red-500" : "text-[var(--color-text-muted)]"}`}
          role="status"
        >
          {error ?? message}
        </p>
      )}
      {paths ? (
        <Card className="order-10 bg-[var(--color-bg-elevated)]">
          <CardHeader>
            <CardTitle>Thư mục gốc dữ liệu</CardTitle>
            <CardDescription>
              Nơi lưu trữ dữ liệu của ứng dụng.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <PathRow
              label="Thư mục gốc hiện tại"
              path={paths.storage_root}
              opening={openingPath === paths.storage_root}
              onOpen={() => void openPath(paths.storage_root)}
            />
          </CardContent>
          <CardFooter className="flex flex-wrap justify-end gap-2 border-t-0 bg-transparent pt-0">
            <Button
              type="button"
              variant="outline"
              className="border-[var(--color-border)]"
              disabled={pickingRoot}
              onClick={() => void chooseStorageRoot()}
            >
              {pickingRoot ? "Đang chọn…" : "Chọn thư mục gốc…"}
            </Button>
            {storageIsCustom ? (
              <Button
                type="button"
                variant="outline"
                className="border-[var(--color-border)]"
                onClick={() => void restoreDefaultStorageRoot()}
              >
                Về mặc định (~/.tikclip)
              </Button>
            ) : null}
          </CardFooter>
        </Card>
      ) : null}

      <Card className="order-40 bg-[var(--color-bg-subtle)]">
        <CardHeader>
          <CardTitle>Workflow defaults - Recording</CardTitle>
          <CardDescription>
            Mặc định ghi hình cho flow mới; có thể override theo từng flow trong workspace.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="max_concurrent">Số luồng record đồng thời tối đa</Label>
            <Input
              id="max_concurrent"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={maxConcurrent}
              onChange={(e) => setMaxConcurrent(e.target.value)}
              placeholder={DEFAULTS.maxConcurrent}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="poll_interval">Thời gian poll (giây)</Label>
            <Input
              id="poll_interval"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={pollInterval}
              onChange={(e) => setPollInterval(e.target.value)}
              placeholder={DEFAULTS.pollInterval}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="recording_max_minutes">Thời lượng tối đa mỗi lần ghi (phút)</Label>
            <Input
              id="recording_max_minutes"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={recordingMaxMinutes}
              onChange={(e) => setRecordingMaxMinutes(e.target.value)}
              placeholder={DEFAULTS.recordingMaxMinutes}
            />
          </div>
        </CardContent>
        <CardFooter className="justify-end border-t-0 bg-transparent pt-0">
          <Button
            type="button"
            disabled={saving === "recording"}
            onClick={() => void saveRecording()}
          >
            {saving === "recording" ? "Đang lưu…" : "Lưu cài đặt ghi hình"}
          </Button>
        </CardFooter>
      </Card>

      <Card className="order-50 bg-[var(--color-bg-subtle)]">
        <CardHeader>
          <CardTitle>Workflow defaults - Clip processing</CardTitle>
          <CardDescription>
            Mặc định xử lý clip cho flow mới; chỉnh sâu hơn tại Flow node config khi cần.
          </CardDescription>
          <CardAction>
            <div className="flex items-center gap-2">
              <Label
                htmlFor={autoProcessSwitchId}
                className="cursor-pointer text-xs whitespace-nowrap text-[var(--color-text-muted)]"
              >
                Tự động tạo clip sau khi ghi hình
              </Label>
              <Switch
                id={autoProcessSwitchId}
                checked={autoProcessAfterRecord}
                onCheckedChange={(v) => {
                  void onAutoProcessAfterRecordChange(v);
                }}
                disabled={loading || autoProcessToggleBusy}
                aria-label="Tự xử lý clip sau khi ghi hình"
              />
            </div>
          </CardAction>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="clip_min">Thời lượng tối thiểu (giây)</Label>
              <Input
                id="clip_min"
                type="text"
                inputMode="numeric"
                className={fieldSurface}
                value={clipMinDuration}
                onChange={(e) => setClipMinDuration(e.target.value)}
                placeholder={DEFAULTS.clipMin}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="clip_max">Thời lượng tối đa (giây)</Label>
              <Input
                id="clip_max"
                type="text"
                inputMode="numeric"
                className={fieldSurface}
                value={clipMaxDuration}
                onChange={(e) => setClipMaxDuration(e.target.value)}
                placeholder={DEFAULTS.clipMax}
              />
            </div>
          </div>
          <div className="border-t border-[var(--color-border)] pt-4">
            <div className="mb-4 flex items-center justify-between gap-4">
              <div>
                <Label
                  htmlFor={audioProcessingSwitchId}
                  className="text-sm font-medium text-[var(--color-text)]"
                >
                  Xử lý âm thanh (VAD + STT)
                </Label>
                <p className="mt-1 text-xs text-[var(--color-text-muted)]">
                  Tắt nếu chỉ cần cắt clip theo cảnh, không tải model và không ghi transcript.
                </p>
              </div>
              <Switch
                id={audioProcessingSwitchId}
                checked={audioProcessingEnabled}
                onCheckedChange={setAudioProcessingEnabled}
                disabled={loading}
                aria-label="Bật xử lý âm thanh VAD và STT"
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="speech_merge_gap">Gộp đoạn nói nếu im lặng ngắn hơn (giây)</Label>
                <Input
                  id="speech_merge_gap"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={speechMergeGapSec}
                  onChange={(e) => setSpeechMergeGapSec(e.target.value)}
                  placeholder={DEFAULTS.speechMergeGapSec}
                  disabled={!audioProcessingEnabled}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="speech_cut_tol">Dung sai cắt hybrid (giây)</Label>
                <Input
                  id="speech_cut_tol"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={speechCutToleranceSec}
                  onChange={(e) => setSpeechCutToleranceSec(e.target.value)}
                  placeholder={DEFAULTS.speechCutToleranceSec}
                  disabled={!audioProcessingEnabled}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="stt_threads">Luồng STT</Label>
                <Input
                  id="stt_threads"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={sttNumThreads}
                  onChange={(e) => setSttNumThreads(e.target.value)}
                  placeholder={DEFAULTS.sttNumThreads}
                  disabled={!audioProcessingEnabled}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="stt_quantize">Chất lượng mô hình</Label>
                <select
                  id="stt_quantize"
                  className={cn(
                    "h-8 w-full min-w-0 rounded-lg border px-2.5 py-1 text-sm outline-none transition-colors",
                    "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
                    "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
                    fieldSurface,
                  )}
                  value={sttQuantize}
                  onChange={(e) =>
                    setSttQuantize(e.target.value as "auto" | "fp32" | "int8")
                  }
                  disabled={!audioProcessingEnabled}
                >
                  <option value="auto">Tự động (CUDA → fp32, còn lại → int8)</option>
                  <option value="fp32">fp32 (nặng hơn, chính xác hơn)</option>
                  <option value="int8">int8 (nhẹ, nhanh hơn)</option>
                </select>
              </div>
            </div>
          </div>
        </CardContent>
        <CardFooter className="justify-end border-t-0 bg-transparent pt-0">
          <Button type="button" disabled={saving === "clips"} onClick={() => void saveClips()}>
            {saving === "clips" ? "Đang lưu…" : "Lưu cài đặt xử lý clip"}
          </Button>
        </CardFooter>
      </Card>

      <Card className="order-30 bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>API & AI integration</CardTitle>
          <CardDescription>
            Quản lý Gemini API key và các mặc định nhận diện sản phẩm cho hệ thống.
          </CardDescription>
          <CardAction>
            <div className="flex items-center gap-2">
              <Label
                htmlFor="product_vector_switch"
                className="cursor-pointer text-xs whitespace-nowrap text-[var(--color-text-muted)]"
              >
                Bật tìm kiếm theo ảnh, video và chữ
              </Label>
              <Switch
                id="product_vector_switch"
                checked={productVectorEnabled}
                onCheckedChange={setProductVectorEnabled}
                aria-label="Bật tìm kiếm sản phẩm theo ảnh, video và chữ"
              />
            </div>
          </CardAction>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="space-y-2">
            <Label htmlFor="gemini_api_key">Khóa API Google AI (Gemini)</Label>
            <Input
              id="gemini_api_key"
              type="password"
              autoComplete="off"
              className={fieldSurface}
              value={geminiApiKey}
              onChange={(e) => setGeminiApiKey(e.target.value)}
              placeholder="AIza…"
            />
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="gemini_embed_model">Mô hình nhận diện (tùy chọn)</Label>
              <Input
                id="gemini_embed_model"
                type="text"
                className={fieldSurface}
                value={geminiEmbeddingModel}
                onChange={(e) => setGeminiEmbeddingModel(e.target.value)}
                placeholder={DEFAULTS.geminiEmbeddingModel}
              />
              <p className="text-xs text-[var(--color-text-muted)]">Chỉ đổi khi Google AI yêu cầu tên khác.</p>
            </div>
            <div className="space-y-2">
              <Label htmlFor="gemini_embed_dim">Tham số nâng cao: số chiều</Label>
              <Input
                id="gemini_embed_dim"
                type="text"
                inputMode="numeric"
                className={fieldSurface}
                value={geminiEmbeddingDim}
                onChange={(e) => setGeminiEmbeddingDim(e.target.value)}
                placeholder={DEFAULTS.geminiEmbeddingDim}
              />
              <p className="text-xs text-[var(--color-text-muted)]">Thường giữ mặc định; đổi khi mô hình yêu cầu.</p>
            </div>
          </div>

          <div className="space-y-3 border-t border-[var(--color-border)] pt-4">
            <div className="flex items-center justify-between gap-3 rounded-md border border-[var(--color-border)] px-3 py-2">
              <div className="space-y-0.5">
                <Label htmlFor={autoTagClipSwitchId} className="text-[var(--color-text)]">
                  Tự gắn sản phẩm cho clip mới
                </Label>
              </div>
              <Switch
                id={autoTagClipSwitchId}
                checked={autoTagClipProductEnabled}
                onCheckedChange={setAutoTagClipProductEnabled}
                aria-label="Tự gắn sản phẩm cho clip mới"
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="auto_tag_frames">Số ảnh lấy từ mỗi clip (1–12)</Label>
                <Input
                  id="auto_tag_frames"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={autoTagClipFrameCount}
                  onChange={(e) => setAutoTagClipFrameCount(e.target.value)}
                  placeholder={DEFAULTS.autoTagClipFrames}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="auto_tag_score">Độ chặt khi gắn</Label>
                <Input
                  id="auto_tag_score"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={autoTagClipMaxScore}
                  onChange={(e) => setAutoTagClipMaxScore(e.target.value)}
                  placeholder={DEFAULTS.autoTagClipMaxScore}
                />
              </div>
            </div>
            <p className="text-xs text-[var(--color-text-muted)]">
              Khi có transcript (STT): kết hợp ảnh + chữ. Trọng số 0–1; tổng không bắt buộc = 1. Đặt{" "}
              <span className="font-medium text-[var(--color-text)]">0</span> để tắt hẳn nhánh ảnh hoặc
              transcript (không gọi embed frame / không tìm theo chữ).
            </p>
            <div className="flex items-center justify-between gap-3 rounded-md border border-[var(--color-border)] px-3 py-2">
              <div className="space-y-0.5">
                <Label htmlFor={debugSuggestFramesSwitchId} className="text-[var(--color-text)]">
                  Giữ ảnh frame debug (suggest-product)
                </Label>
                <p className="text-xs text-[var(--color-text-muted)]">
                  Lưu JPEG tách từ clip dưới{" "}
                  <code className="rounded bg-[var(--color-bg-subtle)] px-1">debug/suggest_clip_frames/</code>{" "}
                  trong thư mục dữ liệu; API trả về{" "}
                  <code className="rounded bg-[var(--color-bg-subtle)] px-1">debug_extracted_frames_dir</code>.
                </p>
              </div>
              <Switch
                id={debugSuggestFramesSwitchId}
                checked={debugKeepSuggestClipFrames}
                onCheckedChange={setDebugKeepSuggestClipFrames}
                aria-label="Giữ ảnh frame debug cho suggest-product"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="suggest_image_focus_prompt">Prompt kèm ảnh khi embed frame (Gemini)</Label>
              <textarea
                id="suggest_image_focus_prompt"
                rows={3}
                className={cn(
                  "min-h-[4.5rem] w-full resize-y rounded-lg border px-2.5 py-2 text-sm outline-none transition-colors",
                  "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
                  "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
                  fieldSurface,
                )}
                value={suggestImageEmbedFocusPrompt}
                onChange={(e) => setSuggestImageEmbedFocusPrompt(e.target.value)}
                placeholder={DEFAULTS.suggestImageEmbedFocusPrompt}
                spellCheck={false}
              />
              <p className="text-xs text-[var(--color-text-muted)]">
                Gửi cùng bytes ảnh tới Gemini để hướng embedding vào sản phẩm (không dùng transcript).
                Để trống = chỉ ảnh.
              </p>
            </div>
            <div className="grid gap-4 sm:grid-cols-3">
              <div className="space-y-2">
                <Label htmlFor="suggest_w_img">Trọng số ảnh</Label>
                <Input
                  id="suggest_w_img"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={suggestWeightImage}
                  onChange={(e) => setSuggestWeightImage(e.target.value)}
                  placeholder={DEFAULTS.suggestWeightImage}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="suggest_w_txt">Trọng số chữ (STT)</Label>
                <Input
                  id="suggest_w_txt"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={suggestWeightText}
                  onChange={(e) => setSuggestWeightText(e.target.value)}
                  placeholder={DEFAULTS.suggestWeightText}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="suggest_min_fused">Ngưỡng fusion tối thiểu</Label>
                <Input
                  id="suggest_min_fused"
                  type="text"
                  inputMode="decimal"
                  className={fieldSurface}
                  value={suggestMinFusedScore}
                  onChange={(e) => setSuggestMinFusedScore(e.target.value)}
                  placeholder={DEFAULTS.suggestMinFusedScore}
                />
              </div>
            </div>
          </div>
        </CardContent>
        <CardFooter className="justify-end border-t-0 bg-transparent pt-0">
          <Button
            type="button"
            disabled={saving === "product_vector"}
            onClick={() => void saveProductVector()}
          >
            {saving === "product_vector" ? "Đang lưu…" : "Lưu cài đặt nhận diện"}
          </Button>
        </CardFooter>
      </Card>

      <Card className="order-20 bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Storage</CardTitle>
          <CardDescription>
            Giới hạn quota, xem dung lượng đang dùng và chính sách xóa bản ghi thô.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          <div className="space-y-2 sm:max-w-xs">
            <Label htmlFor="max_storage_gb">Max storage (GB)</Label>
            <Input
              id="max_storage_gb"
              type="text"
              inputMode="decimal"
              className={fieldSurface}
              value={maxStorageGb}
              onChange={(e) => setMaxStorageGb(e.target.value)}
              placeholder="Để trống nếu không dùng quota"
            />
          </div>

          <div className="space-y-3 border-t border-[var(--color-border)] pt-4">
            <Label className="text-[var(--color-text)]">Dung lượng thư mục dữ liệu</Label>
            {storageScanBusy && !storageStats ? (
              <p className="text-xs text-[var(--color-text-muted)]">Đang tải số liệu lưu trữ…</p>
            ) : null}
            {storageStats ? (
              <div className="space-y-2 text-sm text-[var(--color-text-muted)]">
                <p>
                  Tổng:{" "}
                  <span className="font-medium text-[var(--color-text)]">
                    {formatBytes(storageStats.total_bytes)}
                  </span>
                  {storageStats.quota_bytes != null && storageStats.quota_bytes > 0 ? (
                    <span className="tabular-nums">
                      {" "}
                      (~{storageStats.usage_percent}% quota)
                    </span>
                  ) : null}
                </p>
                <ul className="list-inside list-disc space-y-1">
                  <li>
                    Recordings: {formatBytes(storageStats.recordings_bytes)} (
                    {storageStats.recordings_count} files)
                  </li>
                  <li>
                    Clips: {formatBytes(storageStats.clips_bytes)} ({storageStats.clips_count} files)
                  </li>
                  <li>Products: {formatBytes(storageStats.products_bytes)}</li>
                </ul>
                {storageStats.quota_bytes != null && storageStats.quota_bytes > 0 ? (
                  <div className="pt-1">
                    <div className="h-2 w-full overflow-hidden rounded-full bg-[var(--color-border)]">
                      <div
                        className={`h-full rounded-full transition-all ${
                          storageStats.usage_percent > 95
                            ? "bg-red-500"
                            : storageStats.usage_percent >= 80
                              ? "bg-amber-500"
                              : "bg-emerald-500"
                        }`}
                        style={{
                          width: `${Math.min(100, Math.max(0, storageStats.usage_percent))}%`,
                        }}
                      />
                    </div>
                  </div>
                ) : null}
              </div>
            ) : storageStatsError && !storageScanBusy ? (
              <p className="text-xs text-red-500" role="alert">
                {storageStatsError}
              </p>
            ) : null}
          </div>

          <div className="space-y-4 border-t border-[var(--color-border)] pt-4">
            <Label className="text-[var(--color-text)]">Dọn dữ liệu &amp; cảnh báo dung lượng</Label>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="raw_ret">Bản ghi thô — xóa sau (ngày), 0 = tắt</Label>
                <Input
                  id="raw_ret"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={rawRetentionDays}
                  onChange={(e) => setRawRetentionDays(e.target.value)}
                  placeholder="7"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="arch_ret">Clip — xóa sau (ngày), 0 = tắt</Label>
                <Input
                  id="arch_ret"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={archiveRetentionDays}
                  onChange={(e) => setArchiveRetentionDays(e.target.value)}
                  placeholder="0 = tắt"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="st_warn">Cảnh báo khi dùng quá (% quota)</Label>
                <Input
                  id="st_warn"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={storageWarnPercent}
                  onChange={(e) => setStorageWarnPercent(e.target.value)}
                  placeholder="80"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="st_clean">Mức nghiêm trọng / ưu tiên dọn (% quota)</Label>
                <Input
                  id="st_clean"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={storageCleanupPercent}
                  onChange={(e) => setStorageCleanupPercent(e.target.value)}
                  placeholder="95"
                />
              </div>
            </div>
          </div>
        </CardContent>
        <CardFooter className="flex flex-col gap-3 border-t-0 bg-transparent pt-0 sm:flex-row sm:flex-wrap sm:justify-end">
          <Button
            type="button"
            variant="outline"
            className="w-full border-[var(--color-border)] sm:w-auto"
            disabled={storageCleanupBusy}
            onClick={() => void runCleanupManual()}
          >
            {storageCleanupBusy ? "Đang chạy…" : "Chạy cleanup ngay"}
          </Button>
          <Button
            type="button"
            disabled={saving === "storage_card"}
            className="w-full sm:w-auto"
            onClick={() => void saveStorageCard()}
          >
            {saving === "storage_card" ? "Đang lưu…" : "Lưu cài đặt lưu trữ"}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
