import { NotificationMenu } from "@/components/layout/notification-menu";

interface TopBarProps {
  title: string;
  subtitle?: string;
}

export function TopBar({ title, subtitle }: TopBarProps) {
  return (
    <div className="sticky top-0 z-20 border-b border-white/6 bg-[rgb(7_8_10_/_0.82)] px-6 py-3 backdrop-blur-xl">
      <div className="mx-auto flex w-full max-w-[1280px] items-center justify-between gap-4">
        <div className="min-w-0">
          <h2 className="text-xl font-medium leading-tight text-white">{title}</h2>
          {subtitle ? (
            <p className="mt-0.5 max-w-2xl text-xs text-[var(--color-text-muted)]">{subtitle}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-3">
          <NotificationMenu />
        </div>
      </div>
    </div>
  );
}
