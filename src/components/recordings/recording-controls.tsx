import { useState } from "react";
import { Button } from "@/components/ui/button";
import { stopRecording } from "@/lib/api";

interface RecordingControlsProps {
  recordingId: string;
  disabled?: boolean;
}

export function RecordingControls({ recordingId, disabled }: RecordingControlsProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function onStop() {
    setError(null);
    setLoading(true);
    try {
      await stopRecording(recordingId);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <Button
        type="button"
        variant="destructive"
        size="sm"
        disabled={disabled || loading}
        onClick={() => void onStop()}
      >
        {loading ? "Stopping…" : "Stop"}
      </Button>
      {error && <span className="max-w-[200px] text-right text-[10px] text-red-400">{error}</span>}
    </div>
  );
}
