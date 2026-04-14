import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import {
  BarChart3,
  Boxes,
  Network,
  LayoutDashboard,
  Settings,
  Users,
} from "lucide-react";

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "accounts", label: "Accounts", icon: Users },
  { id: "flows", label: "Flows", icon: Network },
  { id: "products", label: "Products", icon: Boxes },
  { id: "statistics", label: "Statistics", icon: BarChart3 },
] as const;

export type SidebarPageId = (typeof navItems)[number]["id"] | "settings";

interface SidebarProps {
  currentPage: SidebarPageId;
  onNavigate: (page: SidebarPageId) => void;
  sidecarConnected: boolean;
  activeRecordings: number;
}

export function Sidebar({
  currentPage,
  onNavigate,
  sidecarConnected,
  activeRecordings,
}: SidebarProps) {
  return (
    <aside className="flex w-[248px] flex-col border-r border-white/6 bg-[var(--sidebar)] px-4 py-5 shadow-[inset_-1px_0_0_rgba(255,255,255,0.03)] backdrop-blur-xl">
      <div className="app-panel-subtle flex items-center gap-3 rounded-2xl px-4 py-4">
        <div className="relative flex h-11 w-11 items-center justify-center overflow-hidden rounded-2xl border border-white/8 bg-[linear-gradient(180deg,rgba(255,255,255,0.08),rgba(255,255,255,0.02))] shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]">
          <span className="absolute -left-1 top-1 h-8 w-2 rotate-[28deg] rounded-full bg-[var(--color-primary)]" />
          <span className="absolute left-3 top-1 h-8 w-2 rotate-[28deg] rounded-full bg-[var(--color-primary)]" />
          <span className="absolute left-7 top-1 h-8 w-2 rotate-[28deg] rounded-full bg-[var(--color-primary)]/80" />
        </div>
        <div>
          <div className="text-sm font-semibold tracking-[0.01em] text-white">TikClip</div>
          <div className="text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--color-text-muted)]">
            Reup Livestream
          </div>
        </div>
      </div>

      <nav className="flex-1 space-y-2 pt-6">
        <div className="px-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
          Workspace
        </div>
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => onNavigate(item.id)}
            className={cn(
              "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-sm transition-[border-color,background-color,opacity]",
              currentPage === item.id
                ? "border-[color-mix(in_oklab,var(--color-accent)_28%,var(--color-border))] bg-[color-mix(in_oklab,var(--color-accent)_10%,transparent)] text-[var(--color-text)] shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]"
                : "border-transparent text-[var(--color-text-muted)] hover:border-white/8 hover:bg-white/[0.03] hover:text-[var(--color-text)]",
            )}
          >
            <span
              className={cn(
                "flex size-8 items-center justify-center rounded-lg border border-white/8 bg-white/[0.03]",
                currentPage === item.id &&
                  "bg-[color-mix(in_oklab,var(--color-accent)_12%,transparent)] text-[var(--color-accent)]",
              )}
            >
              <item.icon className="size-4" aria-hidden />
            </span>
            <span className="flex-1 text-left font-medium">{item.label}</span>
            {item.id === "flows" && activeRecordings > 0 && (
              <Badge variant="destructive" className="ml-auto min-w-6 justify-center px-2">
                {activeRecordings}
              </Badge>
            )}
          </button>
        ))}

        <div className="mt-4 border-t border-white/6 pt-4">
          <button
            type="button"
            onClick={() => onNavigate("settings")}
            className={cn(
              "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-sm transition-[border-color,background-color,opacity]",
              currentPage === "settings"
                ? "border-[color-mix(in_oklab,var(--color-accent)_28%,var(--color-border))] bg-[color-mix(in_oklab,var(--color-accent)_10%,transparent)] text-[var(--color-text)] shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]"
                : "border-transparent text-[var(--color-text-muted)] hover:border-white/8 hover:bg-white/[0.03] hover:text-[var(--color-text)]",
            )}
          >
            <span
              className={cn(
                "flex size-8 items-center justify-center rounded-lg border border-white/8 bg-white/[0.03]",
                currentPage === "settings" &&
                  "bg-[color-mix(in_oklab,var(--color-accent)_12%,transparent)] text-[var(--color-accent)]",
              )}
            >
              <Settings className="size-4" aria-hidden />
            </span>
            <span className="font-medium">Settings</span>
          </button>
        </div>
      </nav>

      <div className="app-panel-subtle rounded-2xl p-4 text-xs">
        <div className="mb-2 flex items-center gap-2">
          <div
            className={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full shadow-[0_0_10px_currentColor]",
              sidecarConnected
                ? "bg-[var(--color-success)] text-[var(--color-success)]"
                : "bg-[var(--color-primary)] text-[var(--color-primary)]",
            )}
          />
          <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--color-text-muted)]">
            Sidecar
          </span>
          <Badge
            variant="outline"
            className={cn(
              "ml-auto",
              sidecarConnected
                ? "border-[rgba(95,201,146,0.18)] bg-[rgba(95,201,146,0.12)] text-[var(--color-success)]"
                : "border-[rgba(255,99,99,0.18)] bg-[rgba(255,99,99,0.12)] text-[var(--color-primary)]",
            )}
          >
            {sidecarConnected ? "Connected" : "Disconnected"}
          </Badge>
        </div>
        <p className="text-[13px] leading-relaxed text-[var(--color-text-soft)]">
          {sidecarConnected
            ? "Live polling, recording control, and clip processing are available."
            : "Realtime features pause until the Python sidecar reconnects."}
        </p>
        {activeRecordings > 0 && (
          <div className="mt-2 text-[var(--color-text-muted)]">
            {activeRecordings} active recordings
          </div>
        )}
      </div>
    </aside>
  );
}
