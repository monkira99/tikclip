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
  parseCaptionNodeDraft,
  serializeCaptionNodeDraft,
  type CaptionNodeForm,
} from "@/lib/flow-node-forms";

type CaptionNodeModalProps = {
  flowId: number;
  rawDraft: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAutoSave: (draftJson: string) => Promise<void>;
};

export function CaptionNodeModal({
  flowId,
  rawDraft,
  open,
  onOpenChange,
  onAutoSave,
}: CaptionNodeModalProps) {
  const [form, setForm] = useState<CaptionNodeForm>(() => parseCaptionNodeDraft(rawDraft));
  const [saving, setSaving] = useState(false);
  const wasOpen = useRef(false);
  const dirtyRef = useRef(false);
  const savedDraftRef = useRef(serializeCaptionNodeDraft(parseCaptionNodeDraft(rawDraft)));

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      dirtyRef.current = false;
      return;
    }
    if (!wasOpen.current) {
      const nextForm = parseCaptionNodeDraft(rawDraft);
      setForm(nextForm);
      savedDraftRef.current = serializeCaptionNodeDraft(nextForm);
      dirtyRef.current = false;
      wasOpen.current = true;
    }
  }, [open, rawDraft]);

  const flush = useCallback(async () => {
    if (!dirtyRef.current) {
      return;
    }
    const nextDraft = serializeCaptionNodeDraft(form);
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

  const patch = (partial: Partial<CaptionNodeForm>) => {
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
        className="max-h-[min(90vh,560px)] w-full max-w-[min(34rem,calc(100vw-2rem))] gap-0 overflow-hidden p-0 sm:max-w-[min(34rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="max-h-[min(90vh,560px)] overflow-y-auto p-4">
          <DialogHeader>
            <DialogTitle>Caption node</DialogTitle>
            <DialogDescription>Optional overrides for caption generation.</DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2">
              <div>
                <p className="text-sm font-medium text-[var(--color-text)]">Inherit global defaults</p>
                <p className="text-xs text-[var(--color-text-muted)]">Use app settings when enabled.</p>
              </div>
              <Switch
                checked={form.inherit_global_defaults}
                onCheckedChange={(v) => patch({ inherit_global_defaults: Boolean(v) })}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor={`cap-model-${flowId}`}>Model override</Label>
              <Input
                id={`cap-model-${flowId}`}
                value={form.model}
                onChange={(e) => patch({ model: e.target.value })}
                placeholder="Leave empty to use defaults"
                disabled={form.inherit_global_defaults}
              />
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
