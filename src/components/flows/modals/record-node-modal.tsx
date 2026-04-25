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
  parseRecordNodeDraft,
  serializeRecordNodeDraft,
  type RecordNodeForm,
} from "@/lib/flow-node-forms";

type RecordNodeModalProps = {
  flowId: number;
  rawDraft: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAutoSave: (draftJson: string) => Promise<void>;
};

export function RecordNodeModal({
  flowId,
  rawDraft,
  open,
  onOpenChange,
  onAutoSave,
}: RecordNodeModalProps) {
  const [form, setForm] = useState<RecordNodeForm>(() => parseRecordNodeDraft(rawDraft));
  const [saving, setSaving] = useState(false);
  const wasOpen = useRef(false);
  const dirtyRef = useRef(false);
  const savedDraftRef = useRef(serializeRecordNodeDraft(parseRecordNodeDraft(rawDraft)));

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      dirtyRef.current = false;
      return;
    }
    if (!wasOpen.current) {
      const nextForm = parseRecordNodeDraft(rawDraft);
      setForm(nextForm);
      savedDraftRef.current = serializeRecordNodeDraft(nextForm);
      dirtyRef.current = false;
      wasOpen.current = true;
    }
  }, [open, rawDraft]);

  const flush = useCallback(async () => {
    if (!dirtyRef.current) {
      return;
    }
    const nextDraft = serializeRecordNodeDraft(form);
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

  const patch = (partial: Partial<RecordNodeForm>) => {
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
        className="max-h-[min(90vh,640px)] w-full max-w-[min(36rem,calc(100vw-2rem))] gap-0 overflow-hidden p-0 sm:max-w-[min(36rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="max-h-[min(90vh,640px)] overflow-y-auto p-4">
          <DialogHeader>
            <DialogTitle>Record node</DialogTitle>
            <DialogDescription>Maximum duration for one live recording.</DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-2">
            <Label htmlFor={`rec-dur-${flowId}`}>Max duration (min)</Label>
            <Input
              id={`rec-dur-${flowId}`}
              type="number"
              min={1}
              value={form.max_duration_minutes}
              onChange={(e) =>
                patch({ max_duration_minutes: Math.max(1, Math.floor(Number(e.target.value) || 1)) })
              }
            />
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
