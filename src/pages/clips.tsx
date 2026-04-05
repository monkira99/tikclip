import { ClipGrid } from "@/components/clips/clip-grid";

export function ClipsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Clips</h2>
        <p className="text-sm text-[var(--color-text-muted)]">
          Clips stored in the local database. Thumbnails load in the Tauri app via{" "}
          <code className="rounded bg-white/5 px-1 text-xs">convertFileSrc</code>.
        </p>
      </div>
      <ClipGrid />
    </div>
  );
}
