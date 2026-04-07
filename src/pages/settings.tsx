import { useCallback, useEffect, useState } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  applyStorageRoot,
  getAppDataPaths,
  getSetting,
  openPathInSystem,
  pickStorageRootFolder,
  resetStorageRootDefault,
  restartSidecar,
  setSetting,
  storageRootIsCustom,
  type AppDataPaths,
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

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [pathInfo, isCustom, mc, pi, rmin, rhLegacy, cmin, cmax, sg] = await Promise.all([
          getAppDataPaths(),
          storageRootIsCustom(),
          getSetting("max_concurrent"),
          getSetting("poll_interval"),
          getSetting("recording_max_minutes"),
          getSetting("recording_max_hours"),
          getSetting("clip_min_duration"),
          getSetting("clip_max_duration"),
          getSetting("max_storage_gb"),
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

  const saveStorage = useCallback(async () => {
    clearFeedback();
    const sg = maxStorageGb.trim();
    if (sg && Number.isNaN(Number(sg))) {
      setError("Max storage must be a number.");
      return;
    }
    setSaving("storage");
    try {
      await setSetting("max_storage_gb", sg);
      await restartSidecar();
      await resyncSidecarWatchers();
      const fresh = await getAppDataPaths();
      setPaths(fresh);
      setMessage("Storage settings saved. Sidecar restarted to apply.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [clearFeedback, maxStorageGb]);

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
        <CardContent className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
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
            {saving === "recording" ? "Saving…" : "Save recording settings"}
          </Button>
        </CardFooter>
      </Card>

      <Card className="bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Clip processing</CardTitle>
          <CardDescription>
            Cấu hình xử lý clip sau khi record.
          </CardDescription>
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
            {saving === "clips" ? "Saving…" : "Save clip settings"}
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
        <CardContent>
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
        </CardContent>
        <CardFooter className="justify-end border-t-0 bg-transparent pt-0">
          <Button
            type="button"
            disabled={saving === "storage"}
            onClick={() => void saveStorage()}
          >
            {saving === "storage" ? "Saving…" : "Save storage settings"}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
