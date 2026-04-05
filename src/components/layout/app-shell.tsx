import { useState, type ComponentType } from "react";
import { AccountsPage } from "@/pages/accounts";
import { ClipsPage } from "@/pages/clips";
import { DashboardPage } from "@/pages/dashboard";
import { RecordingsPage } from "@/pages/recordings";
import { SettingsPage } from "@/pages/settings";
import { Sidebar } from "./sidebar";
import { TopBar } from "./top-bar";

type PageId =
  | "dashboard"
  | "accounts"
  | "recordings"
  | "clips"
  | "statistics"
  | "settings";

const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  accounts: { title: "Accounts", subtitle: "Manage TikTok accounts" },
  recordings: { title: "Recordings", subtitle: "Active and completed recordings" },
  clips: { title: "Clips", subtitle: "Generated video clips" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

const pageComponents: Record<PageId, ComponentType> = {
  dashboard: DashboardPage,
  accounts: AccountsPage,
  recordings: RecordingsPage,
  clips: ClipsPage,
  statistics: () => (
    <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>
  ),
  settings: SettingsPage,
};

export function AppShell() {
  const [currentPage, setCurrentPage] = useState<PageId>("dashboard");

  const meta = pageMeta[currentPage];
  const PageComponent = pageComponents[currentPage];

  return (
    <div className="flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <Sidebar
        currentPage={currentPage}
        onNavigate={setCurrentPage}
        sidecarConnected={false}
        activeRecordings={0}
      />
      <div className="flex flex-1 flex-col overflow-hidden">
        <TopBar title={meta.title} subtitle={meta.subtitle} />
        <main className="flex-1 overflow-y-auto p-6">
          <PageComponent />
        </main>
      </div>
    </div>
  );
}
