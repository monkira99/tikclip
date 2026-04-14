import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: "📊" },
  { id: "accounts", label: "Accounts", icon: "👤" },
  { id: "recordings", label: "Recordings", icon: "🔴" },
  { id: "clips", label: "Clips", icon: "✂️" },
  { id: "products", label: "Products", icon: "📦" },
  { id: "statistics", label: "Statistics", icon: "📈" },
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
    <aside className="flex w-[220px] flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)]">
      <div className="flex items-center gap-3 border-b border-[var(--color-border)] p-5">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-[var(--color-primary)] to-[var(--color-accent)] text-base">
          🎬
        </div>
        <div>
          <div className="text-sm font-bold text-white">TikClip</div>
          <div className="text-[10px] text-[var(--color-text-muted)]">Reup Livestream</div>
        </div>
      </div>

      <nav className="flex-1 space-y-1 p-2">
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => onNavigate(item.id)}
            className={cn(
              "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors",
              currentPage === item.id
                ? "bg-[var(--color-primary)]/20 font-semibold text-[var(--color-primary)]"
                : "text-[var(--color-text-muted)] hover:bg-white/5",
            )}
          >
            <span>{item.icon}</span>
            <span>{item.label}</span>
            {item.id === "recordings" && activeRecordings > 0 && (
              <Badge variant="destructive" className="ml-auto px-2 py-0 text-[10px]">
                {activeRecordings}
              </Badge>
            )}
          </button>
        ))}

        <div className="mt-4 border-t border-[var(--color-border)] pt-4">
          <button
            type="button"
            onClick={() => onNavigate("settings")}
            className={cn(
              "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors",
              currentPage === "settings"
                ? "bg-[var(--color-primary)]/20 font-semibold text-[var(--color-primary)]"
                : "text-[var(--color-text-muted)] hover:bg-white/5",
            )}
          >
            <span>⚙️</span>
            <span>Settings</span>
          </button>
        </div>
      </nav>

      <div className="border-t border-[var(--color-border)] p-4 text-xs">
        <div className="mb-1 flex items-center gap-2">
          <div
            className={cn(
              "h-2 w-2 shrink-0 rounded-full",
              sidecarConnected ? "bg-green-500" : "bg-red-500",
            )}
          />
          <span className="text-[var(--color-text-muted)]">Sidecar</span>
          <Badge
            variant="outline"
            className={cn(
              "ml-auto border-0 text-[10px]",
              sidecarConnected
                ? "bg-green-500/15 text-green-500"
                : "bg-red-500/15 text-red-500",
            )}
          >
            {sidecarConnected ? "Connected" : "Disconnected"}
          </Badge>
        </div>
        {activeRecordings > 0 && (
          <div className="text-[var(--color-text-muted)]">
            {activeRecordings} active recordings
          </div>
        )}
      </div>
    </aside>
  );
}
