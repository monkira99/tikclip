import { invoke } from "@tauri-apps/api/core";

export type AppDataPaths = {
  storage_root: string;
  data_dir: string;
  clips_dir: string;
  records_dir: string;
};

export async function getAppDataPaths(): Promise<AppDataPaths> {
  return invoke<AppDataPaths>("get_app_data_paths");
}

/** Open a folder or file in Finder / Explorer / file manager (native backend). */
export async function openPathInSystem(path: string): Promise<void> {
  await invoke("open_path", { path });
}

export async function storageRootIsCustom(): Promise<boolean> {
  return invoke<boolean>("storage_root_is_custom");
}

/** Native folder picker; requires a WebviewWindow (invoked from the app UI). */
export async function pickStorageRootFolder(): Promise<string | null> {
  return invoke<string | null>("pick_storage_root_folder");
}

/** Persists root and restarts the app (invoke may not return). */
export async function applyStorageRoot(path: string): Promise<void> {
  await invoke("apply_storage_root", { path });
}

/** Removes custom root config and restarts (back to ~/.tikclip rules). */
export async function resetStorageRootDefault(): Promise<void> {
  await invoke("reset_storage_root_default");
}
