import { create } from "zustand";
import type { Account, CreateAccountInput } from "@/types";
import * as api from "@/lib/api";

/**
 * Global account registry for Rust runtime watching, dashboard, and clip filters.
 * Primary account UX for automation is the Start node on each flow (no dedicated Accounts page).
 *
 * Ignores stale list responses when several fetchAccounts overlap (WS + pages + connect).
 */
let listAccountsFetchToken = 0;
let accountLiveRevision = 0;

interface AccountState {
  accounts: Account[];
  loading: boolean;
  error: string | null;
  fetchAccounts: () => Promise<void>;
  /** Bumps list generation so in-flight list_accounts cannot overwrite runtime live flags. */
  patchAccountLive: (id: number, isLive: boolean) => boolean;
  /** Batch update is_live from runtime-derived live flags (single gen bump). */
  applyLiveFlagsFromRuntime: (rows: { id: number; isLive: boolean }[]) => void;
  addAccount: (input: CreateAccountInput) => Promise<void>;
  removeAccount: (id: number) => Promise<void>;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  loading: false,
  error: null,

  fetchAccounts: async () => {
    const token = ++listAccountsFetchToken;
    const liveRevision = accountLiveRevision;
    set({ loading: true, error: null });
    try {
      const accounts = await api.listAccounts();
      set((state) => {
        if (token !== listAccountsFetchToken) {
          return state;
        }
        if (liveRevision === accountLiveRevision) {
          return { accounts, loading: false };
        }
        const liveMap = new Map(state.accounts.map((account) => [Number(account.id), account.is_live]));
        return {
          accounts: accounts.map((account) => {
            const isLive = liveMap.get(Number(account.id));
            return isLive === undefined ? account : { ...account, is_live: isLive };
          }),
          loading: false,
        };
      });
    } catch (e) {
      set((state) => {
        if (token !== listAccountsFetchToken) {
          return state;
        }
        return { error: String(e), loading: false };
      });
    }
  },

  patchAccountLive: (id, isLive) => {
    accountLiveRevision += 1;
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

  applyLiveFlagsFromRuntime: (rows) => {
    if (rows.length === 0) {
      return;
    }
    accountLiveRevision += 1;
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
      await api.createAccount(input);
      await get().fetchAccounts();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  removeAccount: async (id) => {
    set({ error: null });
    try {
      await api.deleteAccount(id);
      await get().fetchAccounts();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },
}));
