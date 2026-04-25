import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";

type PublishFlowDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** True when at least one `flow_runs` row is `running`. */
  hasActiveRun: boolean;
  pending: boolean;
  onPublishKeepRun: () => Promise<void>;
  onPublishRestart: () => Promise<void>;
};

export function PublishFlowDialog({
  open,
  onOpenChange,
  hasActiveRun,
  pending,
  onPublishKeepRun,
  onPublishRestart,
}: PublishFlowDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-full max-w-[min(28rem,calc(100vw-2rem))] gap-0 p-0 sm:max-w-[min(28rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="p-4">
          <DialogHeader className="space-y-2 text-left">
            <DialogTitle className="text-base font-semibold tracking-[0.01em] text-[var(--color-text)]">
              Publish flow changes
            </DialogTitle>
            <DialogDescription className="text-left text-sm leading-relaxed text-[var(--color-text-muted)]">
              {hasActiveRun
                ? "If this flow is running, the current execution will keep using the previous published definition unless you stop and restart after publish."
                : "Publish copies your draft nodes to the live pipeline. New runs will use this definition."}
            </DialogDescription>
          </DialogHeader>
        </div>

        <Separator className="bg-[var(--color-border)]" />

        <DialogFooter className="mx-0 mb-0 flex-col gap-3 border-0 bg-transparent px-4 py-4 sm:flex-col">
          {hasActiveRun ? (
            <div className="flex w-full flex-col gap-2 sm:flex-row sm:justify-end">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="w-full sm:w-auto"
                disabled={pending}
                onClick={() => void onPublishKeepRun()}
              >
                {pending ? "Publishing…" : "Keep current run"}
              </Button>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                className="w-full sm:w-auto"
                disabled={pending}
                onClick={() => void onPublishRestart()}
              >
                {pending ? "Publishing…" : "Stop and restart"}
              </Button>
            </div>
          ) : (
            <div className="flex w-full justify-end">
              <Button
                type="button"
                variant="secondary"
                size="sm"
                disabled={pending}
                onClick={() => void onPublishKeepRun()}
              >
                {pending ? "Publishing…" : "Publish"}
              </Button>
            </div>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
