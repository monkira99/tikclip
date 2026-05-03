import { useCallback, useEffect, useId, useState } from "react";
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
import { PathRow } from "@/features/settings/path-row";
import {
  DEFAULTS,
  KEY_ARCHIVE_RETENTION,
  KEY_AUTO_TAG_CLIP,
  KEY_AUTO_TAG_FRAMES,
  KEY_AUTO_TAG_MAX_SCORE,
  KEY_DEBUG_KEEP_SUGGEST_FRAMES,
  KEY_GEMINI_API_KEY,
  KEY_GEMINI_EMBEDDING_DIM,
  KEY_GEMINI_EMBEDDING_MODEL,
  KEY_PRODUCT_VECTOR,
  KEY_RAW_RETENTION,
  KEY_STORAGE_CLEANUP,
  KEY_STORAGE_WARN,
  KEY_SUGGEST_IMAGE_EMBED_FOCUS_PROMPT,
  KEY_SUGGEST_MIN_FUSED_SCORE,
  KEY_SUGGEST_WEIGHT_IMAGE,
  KEY_SUGGEST_WEIGHT_TEXT,
} from "@/features/settings/settings-config";
import {
  applyStorageRoot,
  getAppDataPaths,
  getSetting,
  getStorageStats,
  openPathInSystem,
  pickStorageRootFolder,
  resetStorageRootDefault,
  runStorageCleanupNow,
  setSetting,
  storageRootIsCustom,
  type AppDataPaths,
  type StorageStats,
} from "@/lib/api";
import { formatBytes } from "@/lib/format";
import { parseBooleanSetting, valueFromDb } from "@/lib/settings-value";
import { cn } from "@/lib/utils";

const fieldSurface =
  "border-[var(--color-border)] bg-[var(--color-bg)] text-[var(--color-text)]";

function parseProductVectorEnabled(raw: string | null): boolean {
  return parseBooleanSetting(raw, false);
}

function parseAutoTagClipProductEnabled(raw: string | null): boolean {
  return parseProductVectorEnabled(raw);
}

export function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [paths, setPaths] = useState<AppDataPaths | null>(null);
  const [maxStorageGb, setMaxStorageGb] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [storageIsCustom, setStorageIsCustom] = useState(false);
  const [pickingRoot, setPickingRoot] = useState(false);
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

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [
          pathInfo,
          isCustom,
          sg,
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
        ] = await Promise.all([
          getAppDataPaths(),
          storageRootIsCustom(),
          getSetting("max_storage_gb"),
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
        ]);
        if (cancelled) return;
        setPaths(pathInfo);
        setStorageIsCustom(isCustom);
        setMaxStorageGb(sg === null ? "" : sg);
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
          setMessage("Đã cập nhật số liệu lưu trữ từ Rust.");
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
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage(
        "Đã lưu cài đặt nhận diện sản phẩm. Rust sẽ áp dụng cho lần index/auto-tag kế tiếp.",
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
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage(
        "Đã lưu giới hạn dung lượng, dọn dữ liệu và cảnh báo. Rust cleanup sẽ áp dụng ở lần chạy kế tiếp.",
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
              fieldSurface={fieldSurface}
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
                  <li>
                    Products: {formatBytes(storageStats.products_bytes)} (
                    {storageStats.products_count} files, không tính vào quota)
                  </li>
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
