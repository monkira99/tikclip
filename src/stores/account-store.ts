import { create } from "zustand";
import type { Account, CreateAccountInput } from "@/types";
import * as api from "@/lib/api";

interface AccountState {
  accounts: Account[];
  loading: boolean;
  error: string | null;
  fetchAccounts: () => Promise<void>;
  addAccount: (input: CreateAccountInput) => Promise<void>;
  removeAccount: (id: number) => Promise<void>;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  loading: false,
  error: null,

  fetchAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const accounts = await api.listAccounts();
      set({ accounts, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
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
