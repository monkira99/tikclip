import { useCallback, useEffect, useState } from "react";
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
import { getSetting, setSetting } from "@/lib/api";

const fieldSurface =
  "border-[var(--color-border)] bg-[var(--color-bg)] text-[var(--color-text)]";

export function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [maxConcurrent, setMaxConcurrent] = useState("");
  const [pollInterval, setPollInterval] = useState("");
  const [clipMinDuration, setClipMinDuration] = useState("");
  const [clipMaxDuration, setClipMaxDuration] = useState("");
  const [maxStorageGb, setMaxStorageGb] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [
          mc,
          pi,
          cmin,
          cmax,
          sg,
        ] = await Promise.all([
          getSetting("max_concurrent"),
          getSetting("poll_interval"),
          getSetting("clip_min_duration"),
          getSetting("clip_max_duration"),
          getSetting("max_storage_gb"),
        ]);
        if (cancelled) return;
        setMaxConcurrent(mc ?? "");
        setPollInterval(pi ?? "");
        setClipMinDuration(cmin ?? "");
        setClipMaxDuration(cmax ?? "");
        setMaxStorageGb(sg ?? "");
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

  const clearFeedback = useCallback(() => {
    setMessage(null);
    setError(null);
  }, []);

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
    setSaving("recording");
    try {
      await setSetting("max_concurrent", mc);
      await setSetting("poll_interval", pi);
      setMessage("Recording settings saved.");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(null);
    }
  }, [clearFeedback, maxConcurrent, pollInterval]);

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
      setMessage("Clip processing settings saved.");
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
      setMessage("Storage settings saved.");
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

      <Card className="bg-[var(--color-bg-elevated)]">
        <CardHeader>
          <CardTitle>Recording</CardTitle>
          <CardDescription>
            Concurrency and how often the app polls for live status (sidecar / workers).
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="max_concurrent">Max concurrent recordings</Label>
            <Input
              id="max_concurrent"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={maxConcurrent}
              onChange={(e) => setMaxConcurrent(e.target.value)}
              placeholder="e.g. 3"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="poll_interval">Poll interval (seconds)</Label>
            <Input
              id="poll_interval"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={pollInterval}
              onChange={(e) => setPollInterval(e.target.value)}
              placeholder="e.g. 60"
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
          <CardDescription>Minimum and maximum clip length (seconds).</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="clip_min">Min duration (seconds)</Label>
            <Input
              id="clip_min"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={clipMinDuration}
              onChange={(e) => setClipMinDuration(e.target.value)}
              placeholder="e.g. 15"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="clip_max">Max duration (seconds)</Label>
            <Input
              id="clip_max"
              type="text"
              inputMode="numeric"
              className={fieldSurface}
              value={clipMaxDuration}
              onChange={(e) => setClipMaxDuration(e.target.value)}
              placeholder="e.g. 300"
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
          <CardDescription>Upper bound for local clip / recording storage.</CardDescription>
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
              placeholder="e.g. 100"
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
