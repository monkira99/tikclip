import { Input } from "@/components/ui/input";
import { NotificationMenu } from "@/components/layout/notification-menu";
import { cn } from "@/lib/utils";

interface TopBarProps {
  title: string;
  subtitle?: string;
}

export function TopBar({ title, subtitle }: TopBarProps) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
      <div>
        <h2 className="text-lg font-semibold text-white">{title}</h2>
        {subtitle ? (
          <p className="mt-0.5 text-xs text-[var(--color-text-muted)]">{subtitle}</p>
        ) : null}
      </div>
      <div className="flex items-center gap-2">
        <Input
          readOnly
          placeholder="Search..."
          className={cn(
            "h-8 w-[200px] cursor-default border-[var(--color-border)] bg-[var(--color-surface)] text-xs",
            "text-[var(--color-text-muted)] placeholder:text-[var(--color-text-muted)]",
          )}
          aria-label="Search (coming soon)"
        />
        <NotificationMenu />
      </div>
    </div>
  );
}
