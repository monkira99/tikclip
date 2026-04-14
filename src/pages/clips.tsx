import { useEffect } from "react";
import { ClipDetail } from "@/components/clips/clip-detail";
import { ClipGrid } from "@/components/clips/clip-grid";
import { ClipList } from "@/components/clips/clip-list";
import { ClipToolbar } from "@/components/clips/clip-toolbar";
import { useAccountStore } from "@/stores/account-store";
import { useClipStore } from "@/stores/clip-store";

export function ClipsPage() {
  const fetchClips = useClipStore((s) => s.fetchClips);
  const clipsRevision = useClipStore((s) => s.clipsRevision);
  const activeClipId = useClipStore((s) => s.activeClipId);
  const viewMode = useClipStore((s) => s.viewMode);
  const fetchAccounts = useAccountStore((s) => s.fetchAccounts);

  useEffect(() => {
    void fetchClips();
  }, [fetchClips, clipsRevision]);

  useEffect(() => {
    void fetchAccounts();
  }, [fetchAccounts]);

  if (activeClipId != null) {
    return (
      <div className="space-y-6">
        <ClipDetail clipId={activeClipId} />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <ClipToolbar />
      {viewMode === "grid" ? <ClipGrid /> : <ClipList />}
    </div>
  );
}
