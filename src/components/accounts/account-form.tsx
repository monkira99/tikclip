import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { AccountType, CreateAccountInput } from "@/types";

const fieldSurface =
  "border-[var(--color-border)] bg-[var(--color-bg)] text-[var(--color-text)]";

interface AccountFormProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (data: CreateAccountInput) => Promise<void>;
}

export function AccountForm({ open, onOpenChange, onSubmit }: AccountFormProps) {
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [accountType, setAccountType] = useState<AccountType>("monitored");
  const [autoRecord, setAutoRecord] = useState(false);
  const [priority, setPriority] = useState(0);
  const [proxyUrl, setProxyUrl] = useState("");
  const [cookiesJson, setCookiesJson] = useState("");
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const reset = () => {
    setUsername("");
    setDisplayName("");
    setAccountType("monitored");
    setAutoRecord(false);
    setPriority(0);
    setProxyUrl("");
    setCookiesJson("");
    setNotes("");
  };

  const handleSubmit = async () => {
    const u = username.trim().replace(/^@/, "");
    if (!u) return;
    setSubmitting(true);
    try {
      await onSubmit({
        username: u,
        display_name: displayName.trim() || u,
        type: accountType,
        cookies_json: cookiesJson.trim() || null,
        proxy_url: proxyUrl.trim() || null,
        auto_record: autoRecord,
        priority: Number.isFinite(priority) ? priority : 0,
        notes: notes.trim() || null,
      });
      reset();
      onOpenChange(false);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "max-h-[min(90vh,640px)] overflow-y-auto sm:max-w-md",
          "border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)]",
        )}
      >
        <DialogHeader>
          <DialogTitle className="text-[var(--color-text)]">Add account</DialogTitle>
        </DialogHeader>
        <div className="grid gap-4">
          <div className="grid gap-2">
            <Label htmlFor="acc-username">TikTok username</Label>
            <Input
              id="acc-username"
              placeholder="e.g. beauty_store_vn"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className={fieldSurface}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="acc-display">Display name</Label>
            <Input
              id="acc-display"
              placeholder="Optional display name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              className={fieldSurface}
            />
          </div>
          <div className="grid gap-2">
            <span className="text-sm font-medium text-[var(--color-text)]" id="acc-type-label">
              Type
            </span>
            <div
              className="flex flex-wrap gap-2"
              role="group"
              aria-labelledby="acc-type-label"
            >
              <button
                type="button"
                aria-pressed={accountType === "own"}
                className={cn(
                  "min-w-[7.5rem] rounded-lg border-2 px-3 py-1.5 text-sm font-medium transition-colors outline-none",
                  "focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-surface)]",
                  accountType === "own"
                    ? "border-[var(--color-primary)] bg-[var(--color-primary)] text-white shadow-sm"
                    : "border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] hover:border-[var(--color-text-muted)] hover:bg-[var(--color-bg)]",
                )}
                onClick={() => setAccountType("own")}
              >
                My account
              </button>
              <button
                type="button"
                aria-pressed={accountType === "monitored"}
                className={cn(
                  "min-w-[7.5rem] rounded-lg border-2 px-3 py-1.5 text-sm font-medium transition-colors outline-none",
                  "focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-surface)]",
                  accountType === "monitored"
                    ? "border-[var(--color-primary)] bg-[var(--color-primary)] text-white shadow-sm"
                    : "border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] hover:border-[var(--color-text-muted)] hover:bg-[var(--color-bg)]",
                )}
                onClick={() => setAccountType("monitored")}
              >
                Monitored
              </button>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <input
              id="acc-autorecord"
              type="checkbox"
              checked={autoRecord}
              onChange={(e) => setAutoRecord(e.target.checked)}
              className="size-4 rounded border-input"
            />
            <Label htmlFor="acc-autorecord">Auto-record when live</Label>
          </div>
          <div className="grid gap-2">
            <Label htmlFor="acc-priority">Priority</Label>
            <Input
              id="acc-priority"
              type="number"
              value={priority}
              onChange={(e) => setPriority(Number(e.target.value))}
              className={fieldSurface}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="acc-proxy">Proxy URL (optional)</Label>
            <Input
              id="acc-proxy"
              placeholder="http://proxy:port"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              className={fieldSurface}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="acc-cookies">Cookies JSON (optional)</Label>
            <textarea
              id="acc-cookies"
              placeholder="Paste cookie export JSON"
              value={cookiesJson}
              onChange={(e) => setCookiesJson(e.target.value)}
              rows={4}
              className={cn(
                "min-h-20 w-full resize-y rounded-lg border px-2.5 py-1.5 text-sm outline-none",
                "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
                fieldSurface,
              )}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="acc-notes">Notes (optional)</Label>
            <textarea
              id="acc-notes"
              placeholder="Internal notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={2}
              className={cn(
                "min-h-14 w-full resize-y rounded-lg border px-2.5 py-1.5 text-sm outline-none",
                "focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
                fieldSurface,
              )}
            />
          </div>
        </div>
        <DialogFooter className="gap-2 sm:gap-0">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            Cancel
          </Button>
          <Button
            type="button"
            onClick={() => void handleSubmit()}
            disabled={!username.trim() || submitting}
          >
            {submitting ? "Adding…" : "Add account"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
