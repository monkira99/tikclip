import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import {
  Boxes,
  Network,
  LayoutDashboard,
  Settings,
} from "lucide-react";

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "flows", label: "Flows", icon: Network },
  { id: "products", label: "Products", icon: Boxes },
] as const;

/** Collapsed rail — same horizontal padding as expanded so content does not shift on width tween. */
const RAIL_W_CLASS = "w-16";

export type SidebarPageId = (typeof navItems)[number]["id"] | "settings";

interface SidebarProps {
  currentPage: SidebarPageId;
  onNavigate: (page: SidebarPageId) => void;
  activeRecordings: number;
}

const easeStandard = "duration-300 ease-[cubic-bezier(0.4,0,0.2,1)]";

const expandedLabel =
  "flex min-h-0 min-w-0 max-w-0 flex-1 items-center overflow-hidden text-left whitespace-nowrap opacity-0 transition-[max-width,opacity] " +
  easeStandard +
  " group-hover:max-w-[11rem] group-hover:opacity-100 group-focus-within:max-w-[11rem] group-focus-within:opacity-100";

/** Icon sits in a fixed w-10 track (matches collapsed rail) so it stays centered; label uses flex-1 when expanded. */
const navButtonBase =
  "relative flex h-10 min-h-10 w-full shrink-0 items-stretch justify-start gap-0 rounded-lg border border-transparent text-[var(--color-text-muted)] outline-none " +
  "group-hover:gap-2 group-focus-within:gap-2 " +
  "transition-[min-height,border-color,background-color,color,box-shadow,border-radius,padding,gap] " +
  easeStandard +
  " group-hover:min-h-[44px] group-hover:rounded-xl group-hover:py-2.5 group-focus-within:min-h-[44px] group-focus-within:rounded-xl group-focus-within:py-2.5";

const navIconSlot =
  "flex w-10 shrink-0 items-center justify-center self-stretch [&>svg]:size-[18px] [&>svg]:shrink-0";

const navButtonActive =
  "border-[color-mix(in_oklab,var(--color-accent)_22%,rgba(255,255,255,0.08))] bg-[color-mix(in_oklab,var(--color-accent)_12%,transparent)] text-[var(--color-accent)] shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]";

const navButtonIdle =
  "hover:border-white/[0.08] hover:bg-white/[0.04] hover:text-[var(--color-text)]";

export function Sidebar({
  currentPage,
  onNavigate,
  activeRecordings,
}: SidebarProps) {
  return (
    <div className={cn("relative z-30 h-full shrink-0", RAIL_W_CLASS)}>
      <aside
        tabIndex={-1}
        aria-label="Workspace navigation"
        className={cn(
          "group absolute inset-y-0 left-0 z-30 flex w-16 flex-col overflow-y-auto overflow-x-visible border-r border-white/6 bg-[var(--sidebar)] py-4 pl-3 pr-3 shadow-[inset_-1px_0_0_rgba(255,255,255,0.04)] backdrop-blur-xl outline-none",
          "transition-[width,box-shadow] " + easeStandard,
          "hover:w-[248px] hover:shadow-[6px_0_32px_rgba(0,0,0,0.35)]",
          "focus-within:w-[248px] focus-within:shadow-[6px_0_32px_rgba(0,0,0,0.35)]",
          "focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--color-accent)_40%,transparent)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--sidebar)]",
        )}
      >
        {/* Brand: stable row; gap only when expanded so 40px rail is not overflowed by flex gap. */}
        <div
          className={cn(
            "flex flex-row items-center justify-start gap-0 transition-[gap] " + easeStandard,
            "group-hover:gap-3 group-focus-within:gap-3",
          )}
        >
          <div className="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-white/[0.08] bg-[linear-gradient(180deg,rgba(255,255,255,0.07),rgba(255,255,255,0.02))] shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]">
            <span className="relative block size-6">
              <span className="absolute left-0 top-1 h-4 w-1 rotate-[24deg] rounded-full bg-[var(--color-primary)]" />
              <span className="absolute left-2 top-1 h-4 w-1 rotate-[24deg] rounded-full bg-[var(--color-primary)]" />
              <span className="absolute left-4 top-1 h-4 w-1 rotate-[24deg] rounded-full bg-[var(--color-primary)]/85" />
            </span>
          </div>
          <div
            className={cn(
              "flex min-w-0 flex-col justify-center",
              "max-w-0 overflow-hidden opacity-0 transition-[max-width,opacity] " + easeStandard,
              "group-hover:max-w-[10rem] group-hover:opacity-100 group-focus-within:max-w-[10rem] group-focus-within:opacity-100",
            )}
          >
            <div className="whitespace-nowrap text-sm font-semibold tracking-[0.01em] text-white">
              TikClip
            </div>
            <div className="whitespace-nowrap text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--color-text-muted)]">
              Reup Livestream
            </div>
          </div>
        </div>

        <nav className="flex flex-1 flex-col space-y-1.5 pt-5">
          <div
            className={cn(
              "grid min-h-0 transition-[grid-template-rows] " + easeStandard,
              "grid-rows-[0fr] group-hover:grid-rows-[1fr] group-focus-within:grid-rows-[1fr]",
            )}
          >
            <div className="min-h-0 overflow-hidden">
              <div className="whitespace-nowrap pb-1 text-left text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
                Workspace
              </div>
            </div>
          </div>
          {navItems.map((item) => {
            const active = currentPage === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onNavigate(item.id)}
                className={cn(navButtonBase, active ? navButtonActive : navButtonIdle)}
              >
                <span className={navIconSlot}>
                  <item.icon aria-hidden />
                </span>
                <span className={cn("font-medium", expandedLabel)}>{item.label}</span>
                {item.id === "flows" && activeRecordings > 0 ? (
                  <>
                    <span
                      className="absolute right-1 top-1 size-2 rounded-full bg-[var(--color-primary)] ring-2 ring-[var(--sidebar)] group-hover:hidden group-focus-within:hidden"
                      aria-hidden
                    />
                    <Badge
                      variant="destructive"
                      className="ml-auto mr-2 hidden min-w-6 shrink-0 self-center justify-center px-2 group-hover:inline-flex group-focus-within:inline-flex"
                    >
                      {activeRecordings}
                    </Badge>
                  </>
                ) : null}
              </button>
            );
          })}

          <div className="mt-3 border-t border-white/[0.06] pt-3">
            <button
              type="button"
              onClick={() => onNavigate("settings")}
              className={cn(
                navButtonBase,
                currentPage === "settings" ? navButtonActive : navButtonIdle,
              )}
            >
              <span className={navIconSlot}>
                <Settings aria-hidden />
              </span>
              <span className={cn("font-medium", expandedLabel)}>Settings</span>
            </button>
          </div>
        </nav>

      </aside>
    </div>
  );
}
