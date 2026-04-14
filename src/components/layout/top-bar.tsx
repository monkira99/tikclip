import { Search } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { NotificationMenu } from "@/components/layout/notification-menu";

interface TopBarProps {
  title: string;
  subtitle?: string;
  sidecarConnected: boolean;
}

export function TopBar({ title, subtitle, sidecarConnected }: TopBarProps) {
  return (
    <div className="sticky top-0 z-20 border-b border-white/6 bg-[rgb(7_8_10_/_0.82)] px-6 py-5 backdrop-blur-xl">
      <div className="mx-auto flex w-full max-w-[1280px] items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
            Control Surface
          </p>
          <h2 className="mt-1 text-2xl font-medium leading-tight text-white">{title}</h2>
          {subtitle ? (
            <p className="mt-1 max-w-2xl text-sm text-[var(--color-text-muted)]">{subtitle}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-3">
          <div className="app-shell-surface hidden items-center gap-3 rounded-full px-4 py-2.5 lg:flex">
            <Search className="size-4 text-[var(--color-text-muted)]" aria-hidden />
            <span className="text-sm font-medium text-[var(--color-text-muted)]">Search coming soon</span>
            <span className="app-keycap rounded-md px-2 py-0.5 text-[11px] font-medium">⌘K</span>
          </div>
          <Badge
            variant="outline"
            className={
              sidecarConnected
                ? "hidden border-[rgba(95,201,146,0.18)] bg-[rgba(95,201,146,0.12)] text-[var(--color-success)] md:inline-flex"
                : "hidden border-[rgba(255,99,99,0.18)] bg-[rgba(255,99,99,0.12)] text-[var(--color-primary)] md:inline-flex"
            }
          >
            {sidecarConnected ? "Sidecar Online" : "Sidecar Offline"}
          </Badge>
          <NotificationMenu />
        </div>
      </div>
    </div>
  );
}
