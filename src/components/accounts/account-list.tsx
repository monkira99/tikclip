import { useEffect, useState } from "react";
import { AccountForm } from "@/components/accounts/account-form";
import { AccountBadge } from "@/components/accounts/account-badge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useAccountStore } from "@/stores/account-store";
import type { AccountStatus } from "@/types";

function accountStatus(account: { is_live: boolean }): AccountStatus {
  if (account.is_live) return "live";
  return "offline";
}

export function AccountList() {
  const { accounts, loading, error, fetchAccounts, addAccount, removeAccount } =
    useAccountStore();
  const [formOpen, setFormOpen] = useState(false);

  useEffect(() => {
    void fetchAccounts();
  }, [fetchAccounts]);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-[var(--color-text-muted)]">
          {loading ? "Loading…" : `${accounts.length} accounts`}
        </p>
        <Button type="button" onClick={() => setFormOpen(true)}>
          + Add account
        </Button>
      </div>

      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}

      <div className="overflow-hidden rounded-lg border border-[var(--color-border)]">
        <Table>
          <TableHeader>
            <TableRow className="border-[var(--color-border)] hover:bg-transparent">
              <TableHead className="text-[var(--color-text-muted)]">Username</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Type</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Status</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Auto-record</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Priority</TableHead>
              <TableHead className="text-[var(--color-text-muted)]">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {accounts.map((account) => (
              <TableRow key={account.id} className="border-[var(--color-border)]">
                <TableCell className="font-medium text-[var(--color-text)]">
                  @{account.username}
                </TableCell>
                <TableCell>
                  {account.type === "own" ? (
                    <Badge variant="secondary">My account</Badge>
                  ) : (
                    <Badge variant="outline">Monitored</Badge>
                  )}
                </TableCell>
                <TableCell>
                  <AccountBadge status={accountStatus(account)} />
                </TableCell>
                <TableCell>{account.auto_record ? "On" : "Off"}</TableCell>
                <TableCell>{account.priority}</TableCell>
                <TableCell>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="text-destructive hover:text-destructive"
                    onClick={() => void removeAccount(account.id)}
                  >
                    Delete
                  </Button>
                </TableCell>
              </TableRow>
            ))}
            {accounts.length === 0 && !loading ? (
              <TableRow className="hover:bg-transparent">
                <TableCell
                  colSpan={6}
                  className="bg-[var(--color-bg)] py-10 text-center text-base text-[var(--color-text)]"
                >
                  No accounts yet. Click &quot;Add account&quot; to create one.
                </TableCell>
              </TableRow>
            ) : null}
          </TableBody>
        </Table>
      </div>

      <AccountForm
        open={formOpen}
        onOpenChange={setFormOpen}
        onSubmit={addAccount}
      />
    </div>
  );
}
