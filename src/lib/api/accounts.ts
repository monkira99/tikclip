import { invoke } from "@tauri-apps/api/core";

import type { Account, AccountType, AutoRecordSchedule, CreateAccountInput } from "@/types";

/** Raw row from SQLite: schedule stored as JSON string. */
type AccountInvokeRow = Omit<Account, "auto_record_schedule" | "type"> & {
  type: AccountType;
  auto_record_schedule: string | AutoRecordSchedule | null;
};

function normalizeAccount(row: AccountInvokeRow): Account {
  const raw = row.auto_record_schedule;
  let auto_record_schedule: AutoRecordSchedule | null = null;
  if (typeof raw === "string" && raw.length > 0) {
    try {
      auto_record_schedule = JSON.parse(raw) as AutoRecordSchedule;
    } catch {
      auto_record_schedule = null;
    }
  } else if (raw && typeof raw === "object") {
    auto_record_schedule = raw;
  }
  return {
    ...row,
    auto_record_schedule,
  };
}

export async function listAccounts(): Promise<Account[]> {
  const rows = await invoke<AccountInvokeRow[]>("list_accounts");
  return rows.map(normalizeAccount);
}

export async function createAccount(input: CreateAccountInput): Promise<number> {
  return invoke<number>("create_account", {
    input: {
      username: input.username,
      display_name: input.display_name,
      type: input.type,
      cookies_json: input.cookies_json ?? null,
      proxy_url: input.proxy_url ?? null,
      auto_record: input.auto_record,
      priority: input.priority,
      notes: input.notes ?? null,
    },
  });
}

export async function deleteAccount(id: number): Promise<void> {
  await invoke("delete_account", { id });
}
