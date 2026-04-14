import { useState } from "react";
import { AccountForm } from "@/components/accounts/account-form";
import { AccountBadge } from "@/components/accounts/account-badge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
  const { accounts, loading, error, addAccount, removeAccount } = useAccountStore();
  const [formOpen, setFormOpen] = useState(false);

  return (
    <div className="space-y-5">
      <div className="app-panel-subtle flex flex-wrap items-center justify-between gap-3 rounded-2xl px-4 py-4">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--color-text-muted)]">
            Coverage
          </p>
          <p className="mt-1 text-sm text-[var(--color-text-soft)]">
            {loading ? "Loading account registry…" : `${accounts.length} accounts available for monitoring`}
          </p>
        </div>
        <Button type="button" onClick={() => setFormOpen(true)}>
          + Add account
        </Button>
      </div>

      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}

      <Card>
        <CardContent className="px-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Username</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Auto-record</TableHead>
                <TableHead>Priority</TableHead>
                <TableHead>Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {accounts.map((account) => (
                <TableRow key={account.id}>
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
                    className="py-14 text-center text-base text-[var(--color-text)]"
                  >
                    No accounts yet. Click &quot;Add account&quot; to create one.
                  </TableCell>
                </TableRow>
              ) : null}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <AccountForm
        open={formOpen}
        onOpenChange={setFormOpen}
        onSubmit={addAccount}
      />
    </div>
  );
}
