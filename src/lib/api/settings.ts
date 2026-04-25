import { invoke } from "@tauri-apps/api/core";

export async function getSetting(key: string): Promise<string | null> {
  const value = await invoke<string | null>("get_setting", { key });
  return value ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  await invoke("set_setting", { key, value });
}

/** Reload Python sidecar so it picks up SQLite-backed `TIKCLIP_*` settings. */
export async function restartSidecar(): Promise<number> {
  return invoke<number>("restart_sidecar");
}
