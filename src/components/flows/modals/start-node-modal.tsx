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
import { Textarea } from "@/components/ui/textarea";
import {
  parseStartNodeDraft,
  serializeStartNodeDraft,
  type StartNodeForm,
} from "@/lib/flow-node-forms";

type StartNodeModalProps = {
  flowId: number;
  rawDraft: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAutoSave: (draftJson: string) => Promise<void>;
};

export function StartNodeModal({
  flowId,
  rawDraft,
  open,
  onOpenChange,
  onAutoSave,
}: StartNodeModalProps) {
  const [form, setForm] = useState<StartNodeForm>(() => parseStartNodeDraft(rawDraft));
  const [saving, setSaving] = useState(false);
  const wasOpen = useRef(false);
  const dirtyRef = useRef(false);
  const savedDraftRef = useRef(serializeStartNodeDraft(parseStartNodeDraft(rawDraft)));

  useEffect(() => {
    if (!open) {
      wasOpen.current = false;
      dirtyRef.current = false;
      return;
    }
    if (!wasOpen.current) {
      const nextForm = parseStartNodeDraft(rawDraft);
      setForm(nextForm);
      savedDraftRef.current = serializeStartNodeDraft(nextForm);
      dirtyRef.current = false;
      wasOpen.current = true;
    }
  }, [open, rawDraft]);

  const flush = useCallback(async () => {
    if (!dirtyRef.current) {
      return;
    }
    const nextDraft = serializeStartNodeDraft(form);
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

  const patch = (partial: Partial<StartNodeForm>) => {
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
        className="max-h-[min(90vh,720px)] w-full max-w-[min(40rem,calc(100vw-2rem))] gap-0 overflow-hidden p-0 sm:max-w-[min(40rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="max-h-[min(90vh,720px)] overflow-y-auto p-4">
          <DialogHeader>
            <DialogTitle>Start node</DialogTitle>
            <DialogDescription className="sr-only">
              Account, cookies, proxy, poll interval, and retry settings for this flow Start node.
            </DialogDescription>
          </DialogHeader>

          <div className="mt-4 space-y-4">
            <div className="space-y-2">
              <Label htmlFor={`start-user-${flowId}`}>Username</Label>
              <Input
                id={`start-user-${flowId}`}
                value={form.username}
                onChange={(e) => patch({ username: e.target.value })}
                placeholder="tiktok_username"
                autoComplete="off"
              />
              <p className="text-xs font-medium tracking-[0.02em] text-[var(--color-text-muted)]">
                Leading <code>@</code> is optional and will be removed before saving.
              </p>
            </div>
            <div className="space-y-2">
              <Label htmlFor={`start-cookies-${flowId}`}>Cookies JSON</Label>
              <Textarea
                id={`start-cookies-${flowId}`}
                value={form.cookies_json}
                onChange={(e) => patch({ cookies_json: e.target.value })}
                rows={4}
                placeholder="{}"
                className="font-mono text-xs"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor={`start-proxy-${flowId}`}>Proxy URL</Label>
              <Input
                id={`start-proxy-${flowId}`}
                value={form.proxy_url}
                onChange={(e) => patch({ proxy_url: e.target.value })}
                placeholder="Optional"
              />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor={`start-poll-${flowId}`}>Poll interval (seconds)</Label>
                <Input
                  id={`start-poll-${flowId}`}
                  type="number"
                  min={5}
                  value={form.poll_interval_seconds}
                  onChange={(e) => patch({ poll_interval_seconds: Math.max(5, Number(e.target.value) || 5) })}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor={`start-retry-${flowId}`}>Retry limit</Label>
                <Input
                  id={`start-retry-${flowId}`}
                  type="number"
                  min={0}
                  value={form.retry_limit}
                  onChange={(e) => patch({ retry_limit: Math.max(0, Math.floor(Number(e.target.value) || 0)) })}
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
