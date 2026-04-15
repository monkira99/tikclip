import { create } from "zustand";
import type { Account, CreateAccountInput } from "@/types";
import * as api from "@/lib/api";

/**
 * Global account registry for sidecar watching, dashboard, and clip filters.
 * Primary account UX for automation is the Start node on each flow (no dedicated Accounts page).
 *
 * Ignores stale list responses when several fetchAccounts overlap (WS + pages + connect).
 */
let listAccountsGeneration = 0;

interface AccountState {
  accounts: Account[];
  loading: boolean;
  error: string | null;
  fetchAccounts: () => Promise<void>;
  /**
   * After SQLite update from sidecar. Bumps list generation so in-flight list_accounts cannot
   * overwrite this row with a stale snapshot (common when fetchAccounts overlaps WS).
   */
  patchAccountLive: (id: number, isLive: boolean) => boolean;
  /** Batch update is_live from HTTP live-overview (single gen bump). */
  applyLiveFlagsFromSidecar: (rows: { id: number; isLive: boolean }[]) => void;
  addAccount: (input: CreateAccountInput) => Promise<void>;
  removeAccount: (id: number) => Promise<void>;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  loading: false,
  error: null,

  fetchAccounts: async () => {
    const gen = ++listAccountsGeneration;
    set({ loading: true, error: null });
    try {
      const accounts = await api.listAccounts();
      set((state) => {
        if (gen !== listAccountsGeneration) {
          return state;
        }
        return { accounts, loading: false };
      });
    } catch (e) {
      set((state) => {
        if (gen !== listAccountsGeneration) {
          return state;
        }
        return { error: String(e), loading: false };
      });
    }
  },

  patchAccountLive: (id, isLive) => {
    listAccountsGeneration += 1;
    const prev = get().accounts;
    const matched = prev.some((a) => Number(a.id) === Number(id));
    set({
      loading: false,
      accounts: matched
        ? prev.map((a) =>
            Number(a.id) === Number(id) ? { ...a, is_live: isLive } : a,
          )
        : prev,
    });
    return matched;
  },

  applyLiveFlagsFromSidecar: (rows) => {
    if (rows.length === 0) {
      return;
    }
    listAccountsGeneration += 1;
    const map = new Map(rows.map((r) => [Number(r.id), r.isLive]));
    set((s) => ({
      loading: false,
      accounts: s.accounts.map((a) => {
        const live = map.get(Number(a.id));
        return live === undefined ? a : { ...a, is_live: live };
      }),
    }));
  },

  addAccount: async (input) => {
    set({ error: null });
    try {
      const id = await api.createAccount(input);
      const username = input.username.trim().replace(/^@/, "");
      try {
        await api.watchAccount({
          account_id: id,
          username,
          auto_record: input.auto_record,
          cookies_json: input.cookies_json,
          proxy_url: input.proxy_url,
        });
        const status = await api.checkAccountStatus({
          username,
          cookies_json: input.cookies_json,
          proxy_url: input.proxy_url,
        });
        await api.updateAccountLiveStatus(id, status.is_live);
      } catch (e) {
        console.warn("[TikClip] sidecar watch or live check failed:", e);
      }
      await get().fetchAccounts();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  removeAccount: async (id) => {
    set({ error: null });
    try {
      await api.unwatchAccount(id);
      await api.deleteAccount(id);
      await get().fetchAccounts();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));
