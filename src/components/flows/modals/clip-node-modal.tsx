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
  const dirtyRef = useRef(false);
  const savedDraftRef = useRef(serializeClipNodeDraft(parseClipNodeDraft(rawDraft)));

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      dirtyRef.current = false;
      return;
    }
    if (!wasOpen.current) {
      const nextForm = parseClipNodeDraft(rawDraft);
      setForm(nextForm);
      savedDraftRef.current = serializeClipNodeDraft(nextForm);
      dirtyRef.current = false;
      wasOpen.current = true;
    }
  }, [open, rawDraft]);

  const flush = useCallback(async () => {
    if (!dirtyRef.current) {
      return;
    }
    const nextDraft = serializeClipNodeDraft(form);
    if (nextDraft === savedDraftRef.current) {
      dirtyRef.current = false;
      return;
    }
    await onAutoSave(nextDraft);
    savedDraftRef.current = nextDraft;
    dirtyRef.current = false;
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
    dirtyRef.current = true;
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
            <DialogDescription>Scene segmentation and clip extraction defaults.</DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-4">
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
            <div className="grid gap-4 sm:grid-cols-2">
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
