import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  parseClipNodeDraft,
  serializeClipNodeDraft,
  type ClipNodeForm,
} from "@/lib/flow-node-forms";

type ClipNodeModalProps = {
  flowId: number;
  rawDraft: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAutoSave: (draftJson: string) => Promise<void>;
};

export function ClipNodeModal({
  flowId,
  rawDraft,
  open,
  onOpenChange,
  onAutoSave,
}: ClipNodeModalProps) {
  const [form, setForm] = useState<ClipNodeForm>(() => parseClipNodeDraft(rawDraft));
  const [saving, setSaving] = useState(false);
  const wasOpen = useRef(false);

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      return;
    }
    if (!wasOpen.current) {
      setForm(parseClipNodeDraft(rawDraft));
      wasOpen.current = true;
    }
  }, [open, rawDraft]);

  const flush = useCallback(async () => {
    await onAutoSave(serializeClipNodeDraft(form));
  }, [form, onAutoSave]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const t = window.setTimeout(() => {
      void flush().catch(() => {});
    }, 300);
    return () => window.clearTimeout(t);
  }, [flush, open, form]);

  const patch = (partial: Partial<ClipNodeForm>) => {
    setForm((f) => ({ ...f, ...partial }));
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await flush();
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-[min(90vh,760px)] w-full max-w-[min(42rem,calc(100vw-2rem))] gap-0 overflow-hidden p-0 sm:max-w-[min(42rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="max-h-[min(90vh,760px)] overflow-y-auto p-4">
          <DialogHeader>
            <DialogTitle>Clip node</DialogTitle>
            <DialogDescription>Segment length, speech detection, and post-processing defaults.</DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2">
              <div>
                <p className="text-sm font-medium text-[var(--color-text)]">Auto process after record</p>
              </div>
              <Switch
                checked={form.auto_process_after_record}
                onCheckedChange={(v) => patch({ auto_process_after_record: Boolean(v) })}
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor={`clip-min-${flowId}`}>Clip min duration (s)</Label>
                <Input
                  id={`clip-min-${flowId}`}
                  type="number"
                  min={1}
                  value={form.clip_min_duration}
                  onChange={(e) => patch({ clip_min_duration: Math.max(1, Math.floor(Number(e.target.value) || 1)) })}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor={`clip-max-${flowId}`}>Clip max duration (s)</Label>
                <Input
                  id={`clip-max-${flowId}`}
                  type="number"
                  min={1}
                  value={form.clip_max_duration}
                  onChange={(e) => patch({ clip_max_duration: Math.max(1, Math.floor(Number(e.target.value) || 1)) })}
                />
              </div>
            </div>
            <div className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2">
              <div>
                <p className="text-sm font-medium text-[var(--color-text)]">Audio processing</p>
              </div>
              <Switch
                checked={form.audio_processing_enabled}
                onCheckedChange={(v) => patch({ audio_processing_enabled: Boolean(v) })}
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor={`clip-gap-${flowId}`}>Speech merge gap (s)</Label>
                <Input
                  id={`clip-gap-${flowId}`}
                  type="number"
                  step="0.1"
                  min={0}
                  value={form.speech_merge_gap_sec}
                  onChange={(e) => patch({ speech_merge_gap_sec: Math.max(0, Number(e.target.value) || 0) })}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor={`clip-tol-${flowId}`}>Speech cut tolerance (s)</Label>
                <Input
                  id={`clip-tol-${flowId}`}
                  type="number"
                  step="0.1"
                  min={0}
                  value={form.speech_cut_tolerance_sec}
                  onChange={(e) =>
                    patch({ speech_cut_tolerance_sec: Math.max(0, Number(e.target.value) || 0) })
                  }
                />
              </div>
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor={`clip-threads-${flowId}`}>STT threads</Label>
                <Input
                  id={`clip-threads-${flowId}`}
                  type="number"
                  min={1}
                  value={form.stt_num_threads}
                  onChange={(e) => patch({ stt_num_threads: Math.max(1, Math.floor(Number(e.target.value) || 1)) })}
                />
              </div>
              <div className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2 sm:col-span-1">
                <div>
                  <p className="text-sm font-medium text-[var(--color-text)]">STT quantize</p>
                </div>
                <Switch checked={form.stt_quantize} onCheckedChange={(v) => patch({ stt_quantize: Boolean(v) })} />
              </div>
            </div>
          </div>
        </div>

        <DialogFooter className="mx-0 mb-0 border-t border-white/8 bg-[rgb(7_8_10_/_0.96)] px-4 py-3 sm:justify-end">
          <Button type="button" size="sm" disabled={saving} onClick={() => void handleSave()}>
            {saving ? "Saving…" : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
