import { useEffect, useState } from "react";
import { LayoutGrid, List } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useAccountStore } from "@/stores/account-store";
import { useClipStore } from "@/stores/clip-store";
import type { ClipFilters, ClipStatus, SceneType } from "@/types";

const selectClass =
  "h-8 min-w-[8rem] rounded-lg border border-input bg-transparent px-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30";

const STATUS_OPTIONS: { value: ClipFilters["status"]; label: string }[] = [
  { value: "all", label: "All" },
  { value: "draft", label: "Draft" },
  { value: "ready", label: "Ready" },
  { value: "posted", label: "Posted" },
  { value: "archived", label: "Archived" },
];

const SCENE_OPTIONS: { value: ClipFilters["sceneType"]; label: string }[] = [
  { value: "all", label: "All" },
  { value: "product_intro", label: "product_intro" },
  { value: "highlight", label: "highlight" },
  { value: "general", label: "general" },
];

const SORT_OPTIONS: { value: ClipFilters["sortBy"]; label: string }[] = [
  { value: "created_at", label: "Date" },
  { value: "duration", label: "Duration" },
  { value: "file_size", label: "Size" },
  { value: "title", label: "Title" },
];

const BATCH_STATUSES: ClipStatus[] = ["draft", "ready", "posted", "archived"];

type ClipToolbarProps = {
  hideAccountFilter?: boolean;
};

export function ClipToolbar({ hideAccountFilter = false }: ClipToolbarProps = {}) {
  const filters = useClipStore((s) => s.filters);
  const setFilter = useClipStore((s) => s.setFilter);
  const viewMode = useClipStore((s) => s.viewMode);
  const setViewMode = useClipStore((s) => s.setViewMode);
  const selectedClipIds = useClipStore((s) => s.selectedClipIds);
  const batchUpdateStatus = useClipStore((s) => s.batchUpdateStatus);
  const batchDelete = useClipStore((s) => s.batchDelete);
  const clearSelection = useClipStore((s) => s.clearSelection);

  const accounts = useAccountStore((s) => s.accounts);
  const [batchStatusPick, setBatchStatusPick] = useState("");

  const [searchDraft, setSearchDraft] = useState(filters.search);

  useEffect(() => {
    setSearchDraft(filters.search);
  }, [filters.search]);

  useEffect(() => {
    const t = window.setTimeout(() => {
      if (searchDraft !== filters.search) {
        setFilter({ search: searchDraft });
      }
    }, 300);
    return () => window.clearTimeout(t);
  }, [searchDraft, filters.search, setFilter]);

  const selectedCount = selectedClipIds.size;
  const showBatch = selectedCount > 0;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex rounded-lg border border-input p-0.5">
          <Button
            type="button"
            variant={viewMode === "grid" ? "secondary" : "ghost"}
            size="sm"
            className="h-7 px-2"
            onClick={() => setViewMode("grid")}
            aria-pressed={viewMode === "grid"}
          >
            <LayoutGrid className="size-4" />
            <span className="sr-only">Grid</span>
          </Button>
          <Button
            type="button"
            variant={viewMode === "list" ? "secondary" : "ghost"}
            size="sm"
            className="h-7 px-2"
            onClick={() => setViewMode("list")}
            aria-pressed={viewMode === "list"}
          >
            <List className="size-4" />
            <span className="sr-only">List</span>
          </Button>
        </div>

        <select
          className={selectClass}
          value={filters.status}
          onChange={(e) =>
            setFilter({ status: e.target.value as ClipFilters["status"] })
          }
          aria-label="Filter by status"
        >
          {STATUS_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>

        {hideAccountFilter ? null : (
          <select
            className={selectClass}
            value={filters.accountId ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              setFilter({ accountId: v === "" ? null : Number(v) });
            }}
            aria-label="Filter by account"
          >
            <option value="">All accounts</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                @{a.username}
              </option>
            ))}
          </select>
        )}

        <select
          className={selectClass}
          value={filters.sceneType}
          onChange={(e) =>
            setFilter({
              sceneType: e.target.value as SceneType | "all",
            })
          }
          aria-label="Filter by scene type"
        >
          {SCENE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>

        <Input
          className="h-8 max-w-[220px]"
          placeholder="Search title / notes…"
          value={searchDraft}
          onChange={(e) => setSearchDraft(e.target.value)}
          aria-label="Search clips"
        />

        <select
          className={selectClass}
          value={filters.sortBy}
          onChange={(e) =>
            setFilter({ sortBy: e.target.value as ClipFilters["sortBy"] })
          }
          aria-label="Sort by"
        >
          {SORT_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>

        <select
          className={cn(selectClass, "min-w-[5.5rem]")}
          value={filters.sortOrder}
          onChange={(e) =>
            setFilter({
              sortOrder: e.target.value as ClipFilters["sortOrder"],
            })
          }
          aria-label="Sort order"
        >
          <option value="desc">Desc</option>
          <option value="asc">Asc</option>
        </select>
      </div>

      {showBatch ? (
        <div
          className="flex flex-wrap items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2"
          role="region"
          aria-label="Batch actions"
        >
          <Badge variant="outline" className="font-mono">
            {selectedCount} selected
          </Badge>
          <select
            className={selectClass}
            value={batchStatusPick}
            onChange={(e) => {
              const v = e.target.value as ClipStatus | "";
              if (!v) return;
              setBatchStatusPick("");
              void batchUpdateStatus(v);
            }}
            aria-label="Set status for selected clips"
          >
            <option value="">Set status…</option>
            {BATCH_STATUSES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
          <Button
            type="button"
            variant="destructive"
            size="sm"
            onClick={() => {
              if (
                !window.confirm(
                  `Delete ${selectedCount} clip${selectedCount === 1 ? "" : "s"}? This cannot be undone.`,
                )
              ) {
                return;
              }
              void batchDelete();
            }}
          >
            Delete
          </Button>
          <Button type="button" variant="outline" size="sm" onClick={clearSelection}>
            Clear selection
          </Button>
        </div>
      ) : null}
    </div>
  );
}
