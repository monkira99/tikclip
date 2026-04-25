import type { ComponentType } from "react";

import { DashboardPage } from "@/pages/dashboard";
import { FlowsPage } from "@/pages/flows";
import { ProductsPage } from "@/pages/products";
import { SettingsPage } from "@/pages/settings";

export type PageId = "dashboard" | "flows" | "products" | "statistics" | "settings";

export const pageMeta: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "Dashboard", subtitle: "Overview of all activities" },
  flows: { title: "Flows", subtitle: "Monitor and control account automation flows" },
  products: { title: "Products", subtitle: "Product catalog and tagging" },
  statistics: { title: "Statistics", subtitle: "Analytics and reports" },
  settings: { title: "Settings", subtitle: "App configuration" },
};

export const pageComponents: Record<PageId, ComponentType> = {
  dashboard: DashboardPage,
  flows: FlowsPage,
  products: ProductsPage,
  statistics: () => (
    <p className="text-[var(--color-text-muted)]">Statistics coming in Phase 3.</p>
  ),
  settings: SettingsPage,
};
