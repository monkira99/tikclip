import { Clapperboard, HardDrive, Radio, Users } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

type StatCardsProps = {
  activeRecordings: number;
  accountCount: number;
  clipsToday: number;
  storageUsedGb: number;
  storageTotalGb: number;
};

export function StatCards({
  activeRecordings,
  accountCount,
  clipsToday,
  storageUsedGb,
  storageTotalGb,
}: StatCardsProps) {
  const cards = [
    {
      title: "Active recordings",
      value: String(activeRecordings),
      icon: Radio,
    },
    {
      title: "Accounts",
      value: String(accountCount),
      icon: Users,
    },
    {
      title: "Clips today",
      value: String(clipsToday),
      icon: Clapperboard,
    },
    {
      title: "Storage",
      value: `${storageUsedGb} / ${storageTotalGb} GB`,
      icon: HardDrive,
    },
  ] as const;

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {cards.map(({ title, value, icon: Icon }) => (
        <Card key={title} size="sm" className="bg-[var(--color-bg-elevated)]">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-[var(--color-text-muted)]">
              {title}
            </CardTitle>
            <Icon className="size-4 text-[var(--color-text-muted)]" aria-hidden />
          </CardHeader>
          <CardContent>
            <p className="font-heading text-2xl font-semibold tabular-nums text-[var(--color-text)]">
              {value}
            </p>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
