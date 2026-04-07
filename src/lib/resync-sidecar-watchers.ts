import * as api from "@/lib/api";
import { useAccountStore } from "@/stores/account-store";

/** Re-register watched accounts after a sidecar restart (in-memory watcher state is cleared). */
export async function resyncSidecarWatchers(): Promise<void> {
  await useAccountStore.getState().fetchAccounts();
  const accounts = useAccountStore.getState().accounts;
  await api.syncWatcherForAccounts(
    accounts.map((a) => ({
      id: a.id,
      username: a.username,
      auto_record: a.auto_record,
      cookies_json: a.cookies_json,
      proxy_url: a.proxy_url,
    })),
  );
}
