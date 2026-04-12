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
} as const;

function valueFromDb(db: string | null, fallback: string): string {
  if (db === null) {
    return fallback;
  }
  return db;
}

const AUTO_PROCESS_AFTER_RECORD_KEY = "auto_process_after_record";

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
  const [storageCleanupBusy, setStorageCleanupBusy] = useState(false);

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
    setSaving("clips");
    try {
      await setSetting("clip_min_duration", mn);
      await setSetting("clip_max_duration", mx);
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage("Clip processing settings saved. Sidecar restarted to apply.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [clearFeedback, clipMinDuration, clipMaxDuration]);

  const scanStorageStats = useCallback(async () => {
    clearFeedback();
    setStorageScanBusy(true);
    try {
      const s = await getStorageStats();
      setStorageStats(s);
      setMessage("Đã cập nhật số liệu lưu trữ từ sidecar.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Không lấy được stats");
      setStorageStats(null);
    } finally {
      setStorageScanBusy(false);
    }
  }, [clearFeedback]);

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
      ["Số ngày giữ bản ghi thô", raw],
      ["Số ngày với clip lưu trữ", arch],
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
      setError("Số ngày giữ bản ghi / clip phải là số không âm.");
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
      await scanStorageStats();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Cleanup thất bại");
    } finally {
      setStorageCleanupBusy(false);
    }
  }, [
    clearFeedback,
    scanStorageStats,
    rawRetentionDays,
    archiveRetentionDays,
  ]);

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
        <Card className="bg-[var(--color-bg-elevated)]">
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

      <Card className="bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Recording</CardTitle>
          <CardDescription>
            Thông tin cấu hình quá trình quay video.
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

      <Card className="bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Clip processing</CardTitle>
          <CardDescription>
            Cấu hình xử lý clip sau khi ghi hình.
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
        <CardContent className="grid gap-4 sm:grid-cols-2">
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
        </CardContent>
        <CardFooter className="justify-end border-t-0 bg-transparent pt-0">
          <Button type="button" disabled={saving === "clips"} onClick={() => void saveClips()}>
            {saving === "clips" ? "Đang lưu…" : "Lưu cài đặt xử lý clip"}
          </Button>
        </CardFooter>
      </Card>

      <Card className="bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Storage</CardTitle>
          <CardDescription>
            Cấu hình giới hạn lưu trữ video và dữ liệu. Lưu ý: nếu bạn đã thiết lập thư mục gốc tùy chỉnh, hãy đảm bảo rằng thư mục đó có đủ dung lượng cho giới hạn mới. Nếu không, ứng dụng có thể gặp lỗi khi quay video mới hoặc xử lý clip.
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
            <div className="flex flex-wrap items-center justify-between gap-2">
              <Label className="text-[var(--color-text)]">Tổng quan (sidecar)</Label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="border-[var(--color-border)]"
                disabled={storageScanBusy}
                onClick={() => void scanStorageStats()}
              >
                {storageScanBusy ? "Đang quét…" : "Quét ngay"}
              </Button>
            </div>
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
            ) : (
              <p className="text-xs text-[var(--color-text-muted)]">
                Bấm &quot;Quét ngay&quot; khi sidecar đang chạy để xem dung lượng theo thư mục.
              </p>
            )}
          </div>

          <div className="space-y-3 border-t border-[var(--color-border)] pt-4">
            <div>
              <p className="text-sm font-medium text-[var(--color-text)]">
                Dọn dữ liệu &amp; cảnh báo dung lượng
              </p>
              <p className="mt-1 text-xs text-[var(--color-text-muted)]">
                Sidecar dùng các giá trị này khi quét và dọn file. Không cần nhập mã cấu hình kỹ thuật. Nút
                &quot;Chạy cleanup ngay&quot; áp dụng đúng số ngày đang nhập; &quot;Lưu cài đặt lưu trữ&quot;
                ghi quota GB, retention và ngưỡng cảnh báo vào cơ sở dữ liệu và khởi động lại sidecar một lần.
              </p>
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="raw_ret">Xóa bản ghi thô sau (ngày)</Label>
                <Input
                  id="raw_ret"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={rawRetentionDays}
                  onChange={(e) => setRawRetentionDays(e.target.value)}
                  placeholder="7"
                />
                <p className="text-xs text-[var(--color-text-muted)]">
                  File ghi gốc trong <code className="text-[var(--color-text)]">records/</code> (và bản cũ{" "}
                  <code className="text-[var(--color-text)]">recordings/</code> nếu còn) cũ hơn số ngày
                  này có thể bị xóa khi sidecar chạy dọn dẹp.
                </p>
              </div>
              <div className="space-y-2">
                <Label htmlFor="arch_ret">Clip lưu trữ — tuổi tối đa (ngày)</Label>
                <Input
                  id="arch_ret"
                  type="text"
                  inputMode="numeric"
                  className={fieldSurface}
                  value={archiveRetentionDays}
                  onChange={(e) => setArchiveRetentionDays(e.target.value)}
                  placeholder="0"
                />
                <p className="text-xs text-[var(--color-text-muted)]">
                  Đặt <span className="tabular-nums">0</span> để không tự xóa clip theo tuổi file (cần
                  khớp trạng thái trong app). Giá trị &gt; 0 chỉ có hiệu lực khi sidecar hỗ trợ đồng bộ với
                  dữ liệu clip.
                </p>
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
                <p className="text-xs text-[var(--color-text-muted)]">
                  Khi dung lượng đạt mức này so với giới hạn (GB) ở trên, app có thể hiện cảnh báo.
                </p>
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
                <p className="text-xs text-[var(--color-text-muted)]">
                  Nên cao hơn mức cảnh báo (ví dụ 95 so với 80). Dùng cho thông báo và chính sách dọn dẹp
                  tự động của sidecar.
                </p>
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
