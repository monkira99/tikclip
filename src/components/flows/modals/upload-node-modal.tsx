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

type UploadNodeModalProps = {
  flowId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function UploadNodeModal({ flowId, open, onOpenChange }: UploadNodeModalProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-full max-w-[min(32rem,calc(100vw-2rem))] sm:max-w-[min(32rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <DialogHeader>
          <DialogTitle>Upload node</DialogTitle>
          <DialogDescription>
            Upload automation is not wired in this build. Settings here are placeholders only.
          </DialogDescription>
        </DialogHeader>

        <div className="mt-4 space-y-4 opacity-60">
          <div className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-white/[0.02] px-3 py-2">
            <div>
              <p className="text-sm font-medium text-[var(--color-text)]">Inherit global defaults</p>
            </div>
            <Switch checked disabled />
          </div>
          <div className="space-y-2">
            <Label htmlFor={`up-target-${flowId}`}>Upload target</Label>
            <Input id={`up-target-${flowId}`} disabled placeholder="Coming later" />
          </div>
          <p className="text-sm text-[var(--color-text-muted)]">Coming later — publish and drafts are unchanged.</p>
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" size="sm" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
