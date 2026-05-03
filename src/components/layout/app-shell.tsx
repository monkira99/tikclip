import { useState } from "react";

import {
  useActiveRecordingCountSync,
  useNavigationTargetSync,
  useNotificationBootstrap,
  useStorageRuntimeEvents,
  useTauriRuntimeEvents,
} from "@/components/layout/app-shell-effects";
import { pageComponents, pageMeta, type PageId } from "@/components/layout/app-shell-pages";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";
import { Sidebar } from "./sidebar";
import { TopBar } from "./top-bar";

export function AppShell() {
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");
  const activeRecordings = useAppStore((s) => s.activeRecordings);

  useActiveRecordingCountSync();
  useNavigationTargetSync(setCurrentPage);
  useNotificationBootstrap();
  useTauriRuntimeEvents();
  useStorageRuntimeEvents();

  const meta = pageMeta[currentPage];
  const PageComponent = pageComponents[currentPage];

  return (
    <div className="relative flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute left-[-10%] top-[-8%] h-72 w-72 rounded-full bg-[rgba(85,179,255,0.08)] blur-3xl" />
        <div className="absolute right-[-6%] top-[8%] h-60 w-60 rounded-full bg-[rgba(255,99,99,0.08)] blur-3xl" />
        <div className="absolute bottom-[-12%] left-[28%] h-80 w-80 rounded-full bg-[var(--color-warm-glow)] blur-3xl" />
      </div>
      <Sidebar
        currentPage={currentPage}
        onNavigate={setCurrentPage}
        activeRecordings={activeRecordings}
      />
      <div className="relative flex flex-1 flex-col overflow-hidden">
        <TopBar
          title={meta.title}
          subtitle={meta.subtitle}
        />
        <main
          className={cn(
            "flex-1 overflow-y-auto px-6 pt-6 sm:px-8",
            currentPage === "flows" ? "pb-3" : "pb-8",
          )}
        >
          <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-8">
            <PageComponent />
          </div>
        </main>
      </div>
    </div>
  );
}
