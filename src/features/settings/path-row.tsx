import { FolderOpen } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";

type PathRowProps = {
  label: string;
  description?: string;
  path: string;
  onOpen: () => void;
  opening: boolean;
  fieldSurface: string;
};

export function PathRow({
  label,
  description,
  path,
  onOpen,
  opening,
  fieldSurface,
}: PathRowProps) {
  return (
    <div className="space-y-2">
      <div className="flex flex-col gap-0.5 sm:flex-row sm:items-baseline sm:justify-between">
        <Label className="text-[var(--color-text)]">{label}</Label>
        {description ? (
          <span className="text-xs text-[var(--color-text-muted)]">{description}</span>
        ) : null}
      </div>
      <div className="flex flex-col gap-2 sm:flex-row sm:items-stretch">
        <div
          className={`min-h-10 flex-1 rounded-md border px-3 py-2 font-mono text-xs break-all ${fieldSurface}`}
        >
          {path}
        </div>
        <Button
          type="button"
          variant="outline"
          className="shrink-0 border-[var(--color-border)]"
          disabled={opening || !path}
          onClick={() => onOpen()}
        >
          <FolderOpen className="mr-2 size-4 opacity-80" aria-hidden />
          Mở thư mục
        </Button>
      </div>
    </div>
  );
}
