import { RuntimeLogsPanel } from "@/components/flows/runtime/runtime-logs-panel";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { FlowContext, FlowRuntimeLogEntry } from "@/types";

type FlowRuntimeDiagnosticsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  flow: FlowContext;
  logs: FlowRuntimeLogEntry[];
  username?: string | null;
  activeFlowRunId?: number | null;
};

export function FlowRuntimeDiagnosticsDialog({
  open,
  onOpenChange,
  flow,
  logs,
  username,
  activeFlowRunId,
}: FlowRuntimeDiagnosticsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-full max-w-[min(72rem,calc(100vw-2rem))] gap-0 p-0 sm:max-w-[min(72rem,calc(100vw-2rem))]"
        showCloseButton
      >
        <div className="p-4 pb-0">
          <DialogHeader className="space-y-2 text-left">
            <DialogTitle className="text-base font-semibold tracking-[0.01em] text-[var(--color-text)]">
              Runtime diagnostics
            </DialogTitle>
            <DialogDescription className="text-left text-sm leading-relaxed text-[var(--color-text-muted)]">
              Review recent runtime logs and copy a support-ready diagnostic bundle for this flow.
            </DialogDescription>
          </DialogHeader>
        </div>

        <div className="px-4 pb-4">
          <RuntimeLogsPanel
            flow={flow}
            logs={logs}
            username={username}
            activeFlowRunId={activeFlowRunId}
          />
        </div>
      </DialogContent>
    </Dialog>
  );
}
