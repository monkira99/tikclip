import {
  Bell,
  BellOff,
  CheckCheck,
  Clapperboard,
  Clock,
  Film,
  Info,
  Radio,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { isTauri } from "@tauri-apps/api/core";
import { markAllNotificationsReadDb, markNotificationReadDb } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import {
  useNotificationStore,
  useUnreadNotificationCount,
  type AppNotification,
  type NotificationKind,
} from "@/stores/notification-store";

function formatWhen(ts: number): string {
  try {
    return new Intl.DateTimeFormat("vi-VN", {
      dateStyle: "short",
      timeStyle: "short",
    }).format(ts);
  } catch {
    return "";
  }
}

function kindMeta(kind: NotificationKind): {
  Icon: LucideIcon;
  label: string;
  iconWrap: string;
  iconClass: string;
} {
  switch (kind) {
    case "account_live":
      return {
        Icon: Radio,
        label: "Đang live",
        iconWrap:
          "bg-[color-mix(in_oklab,var(--color-primary)_22%,transparent)] shadow-[0_0_12px_color-mix(in_oklab,var(--color-primary)_35%,transparent)]",
        iconClass: "text-[var(--color-primary)]",
      };
    case "recording_finished":
      return {
        Icon: Clapperboard,
        label: "Ghi hình",
        iconWrap: "bg-[color-mix(in_oklab,var(--color-accent)_18%,transparent)]",
        iconClass: "text-[var(--color-accent)]",
      };
    case "clip_ready":
      return {
        Icon: Film,
        label: "Clip",
        iconWrap: "bg-[color-mix(in_oklab,var(--color-accent)_18%,transparent)]",
        iconClass: "text-[var(--color-accent)]",
      };
    default:
      return {
        Icon: Info,
        label: "Thông tin",
        iconWrap: "bg-foreground/8",
        iconClass: "text-[var(--color-text-muted)]",
      };
  }
}

function NotificationRow({
  n,
  onRead,
}: {
  n: AppNotification;
  onRead: () => void;
}) {
  const { Icon, label, iconWrap, iconClass } = kindMeta(n.kind);
  const when = formatWhen(n.createdAt);

  return (
    <DropdownMenuItem
      className={cn(
        "group cursor-pointer rounded-xl p-0",
        "focus:bg-transparent data-[highlighted]:bg-transparent",
      )}
      onSelect={() => onRead()}
    >
      <div
        className={cn(
          "relative flex w-full gap-3 rounded-xl border px-3 py-2.5 text-left transition-colors",
          "border-[var(--color-border)]/80 bg-[var(--color-bg)]/40",
          "hover:border-[var(--color-border)] hover:bg-[var(--color-surface)]",
          "group-data-[highlighted]:border-[var(--color-border)] group-data-[highlighted]:bg-[var(--color-surface)]",
          "group-data-[highlighted]:ring-1 group-data-[highlighted]:ring-[var(--color-accent)]/20",
          !n.read &&
            "border-[color-mix(in_oklab,var(--color-primary)_25%,var(--color-border))] bg-[color-mix(in_oklab,var(--color-primary)_6%,var(--color-bg))]",
        )}
      >
        {!n.read ? (
          <span
            className="absolute left-0 top-1/2 h-8 w-[3px] -translate-y-1/2 rounded-r-full bg-[var(--color-primary)]"
            aria-hidden
          />
        ) : null}
        <div
          className={cn(
            "flex size-10 shrink-0 items-center justify-center rounded-xl",
            iconWrap,
          )}
        >
          <Icon className={cn("size-[1.125rem]", iconClass)} strokeWidth={2} aria-hidden />
        </div>
        <div className="min-w-0 flex-1 pt-0.5">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <p className="text-[10px] font-medium uppercase tracking-wider text-[var(--color-text-muted)]">
                {label}
              </p>
              <p className="mt-0.5 font-semibold leading-tight text-[var(--color-text)]">{n.title}</p>
            </div>
            {!n.read ? (
              <span
                className="mt-1 size-2 shrink-0 rounded-full bg-[var(--color-primary)] shadow-[0_0_8px_var(--color-primary)]"
                title="Chưa đọc"
              />
            ) : null}
          </div>
          {n.body ? (
            <p className="mt-1 text-xs leading-relaxed text-[var(--color-text-muted)]">{n.body}</p>
          ) : null}
          {when ? (
            <p className="mt-2 flex items-center gap-1 text-[10px] tabular-nums text-[var(--color-text-muted)]/90">
              <Clock className="size-3 opacity-70" aria-hidden />
              {when}
            </p>
          ) : null}
        </div>
      </div>
    </DropdownMenuItem>
  );
}

async function persistMarkRead(id: string, markRead: (id: string) => void) {
  const num = Number(id);
  if (isTauri() && Number.isSafeInteger(num) && num > 0) {
    try {
      await markNotificationReadDb(num);
    } catch {
      /* ignore */
    }
  }
  markRead(id);
}

export function NotificationMenu() {
  const items = useNotificationStore((s) => s.items);
  const markRead = useNotificationStore((s) => s.markRead);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const unread = useUnreadNotificationCount();

  const onMarkAll = () => {
    void (async () => {
      if (isTauri()) {
        try {
          await markAllNotificationsReadDb();
        } catch {
          /* ignore */
        }
      }
      markAllRead();
    })();
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="relative h-8 w-8 border-[var(--color-border)] bg-[var(--color-surface)]"
          aria-label={`Thông báo${unread > 0 ? `, ${unread} chưa đọc` : ""}`}
        >
          <Bell className="size-4 text-[var(--color-text-muted)]" aria-hidden />
          {unread > 0 ? (
            <Badge
              variant="destructive"
              className="absolute -right-1 -top-1 flex h-4 min-w-4 justify-center px-1 text-[10px] leading-none"
            >
              {unread > 99 ? "99+" : unread}
            </Badge>
          ) : null}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="w-[min(100vw-2rem,24rem)] overflow-x-hidden border-[var(--color-border)] bg-[var(--color-surface)] p-0 shadow-xl shadow-black/40"
      >
        <div className="flex items-center justify-between gap-2 border-b border-[var(--color-border)]/80 bg-[var(--color-bg)]/30 px-3 py-2.5">
          <div className="flex items-center gap-2">
            <DropdownMenuLabel className="p-0 text-base font-semibold tracking-tight text-[var(--color-text)]">
              Thông báo
            </DropdownMenuLabel>
            {items.length > 0 ? (
              <span className="rounded-md bg-foreground/10 px-2 py-0.5 text-[11px] font-medium tabular-nums text-[var(--color-text-muted)]">
                {items.length}
              </span>
            ) : null}
          </div>
          {items.length > 0 ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 gap-1.5 px-2 text-xs text-[var(--color-accent)] hover:bg-[color-mix(in_oklab,var(--color-accent)_12%,transparent)] hover:text-[var(--color-accent)]"
              onClick={onMarkAll}
            >
              <CheckCheck className="size-3.5" aria-hidden />
              Đã đọc hết
            </Button>
          ) : null}
        </div>
        {items.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-4 py-10 text-center">
            <div className="flex size-12 items-center justify-center rounded-2xl bg-foreground/5 ring-1 ring-[var(--color-border)]/60">
              <BellOff className="size-6 text-[var(--color-text-muted)]" strokeWidth={1.5} aria-hidden />
            </div>
            <p className="text-sm font-medium text-[var(--color-text)]">Chưa có thông báo</p>
            <p className="max-w-[14rem] text-xs leading-relaxed text-[var(--color-text-muted)]">
              Khi có tài khoản live, ghi hình xong hoặc clip mới, bạn sẽ thấy ở đây.
            </p>
          </div>
        ) : (
          <div
            className={cn(
              "max-h-80 min-h-0 overflow-y-auto overflow-x-hidden overscroll-y-contain",
              "[-webkit-overflow-scrolling:touch]",
            )}
          >
            <div className="flex flex-col gap-2 p-2">
              {items.map((n) => (
                <NotificationRow
                  key={n.id}
                  n={n}
                  onRead={() => void persistMarkRead(n.id, markRead)}
                />
              ))}
            </div>
          </div>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
