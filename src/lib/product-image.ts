import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

/** Resolve product cover for <img src>: remote URLs as-is; local paths via Tauri asset protocol. */
export function productImageSrc(url: string | null | undefined): string | null {
  const u = url?.trim();
  if (!u) {
    return null;
  }
  if (u.startsWith("http://") || u.startsWith("https://")) {
    return u;
  }
  if (isTauri()) {
    try {
      return convertFileSrc(u);
    } catch {
      return null;
    }
  }
  return u;
}
