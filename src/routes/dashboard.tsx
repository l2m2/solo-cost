import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { useCompanyStore } from "@/stores/company";
import { useDashboardStore } from "@/stores/dashboard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { formatCNY } from "@/lib/money";
import { statusLabel } from "@/lib/status";
import type { RankRow } from "@/types";

const BUCKET_CLASS: Record<string, string> = {
  overdue: "bg-red-100 text-red-700",
  soon: "bg-amber-100 text-amber-700",
  future: "bg-slate-100 text-slate-600",
};

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <Card>
      <CardContent className="p-4 space-y-1">
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-lg font-semibold">{value}</div>
        {sub && <div className="text-xs text-muted-foreground">{sub}</div>}
      </CardContent>
    </Card>
  );
}

function RankCard({ title, rows, t }: { title: string; rows: RankRow[]; t: TFunction }) {
  return (
    <Card>
      <CardHeader><CardTitle className="text-sm">{title}</CardTitle></CardHeader>
      <CardContent className="p-0">
        <Table compact>
          <TableHeader>
            <TableRow>
              <TableHead>{t("dashboard.name")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.netLabel")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.receivedLabel")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow><TableCell colSpan={3} className="p-4 text-sm text-muted-foreground">{t("dashboard.empty")}</TableCell></TableRow>
            ) : rows.map((r) => (
              <TableRow key={r.id}>
                <TableCell>{r.name}</TableCell>
                <TableCell className="text-right">{formatCNY(r.net_cents)}</TableCell>
                <TableCell className="text-right text-muted-foreground">{formatCNY(r.received_inclusive_cents)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { data, loadedForCompany, loadFor } = useDashboardStore();

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">{t("dashboard.selectCompany")}</div>;
  }
  if (!data) {
    return <div className="text-sm text-muted-foreground">{t("dashboard.loading")}</div>;
  }

  const maxYear = Math.max(1, ...data.by_year.map((y) => Math.max(y.net_cents, y.received_exclusive_cents)));
  const maxStatus = Math.max(1, ...data.by_status.map((s) => s.contract_inclusive_cents));
  const pct = (v: number, max: number) => `${Math.max(0, Math.min(100, (v / max) * 100))}%`;

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("nav.dashboard")}</h1>
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">{t("dashboard.tabOverview")}</TabsTrigger>
          <TabsTrigger value="ranking">{t("dashboard.tabRanking")}</TabsTrigger>
          <TabsTrigger value="receivables">{t("dashboard.tabReceivables")}</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-4">
          <div>
            <div className="mb-2 text-sm font-medium">{t("dashboard.contractScope")}</div>
            <div className="grid grid-cols-3 gap-3">
              <Kpi label={t("dashboard.contractTotal")} value={formatCNY(data.contract_total_inclusive_cents)} />
              <Kpi label={t("dashboard.revenueExclusive")} value={formatCNY(data.revenue_exclusive_cents)} />
              <Kpi label={t("dashboard.netPotential")} value={formatCNY(data.net_potential_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <div>
            <div className="mb-2 text-sm font-medium">{t("dashboard.receivedScope")}</div>
            <div className="grid grid-cols-3 gap-3">
              <Kpi label={t("dashboard.received")} value={formatCNY(data.received_inclusive_cents)} />
              <Kpi label={t("dashboard.outstanding")} value={formatCNY(data.outstanding_cents)} />
              <Kpi label={t("dashboard.netRealized")} value={formatCNY(data.net_realized_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("dashboard.netByYear")}</CardTitle></CardHeader>
            <CardContent className="space-y-3">
              {data.by_year.length === 0 ? (
                <div className="text-sm text-muted-foreground">{t("dashboard.empty")}</div>
              ) : data.by_year.map((y) => (
                <div key={y.year} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{y.year}</span>
                    <span className="text-muted-foreground">
                      {t("dashboard.netLabel")} {formatCNY(y.net_cents)} · {t("dashboard.receivedLabel")} {formatCNY(y.received_exclusive_cents)}
                    </span>
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-slate-300" style={{ width: pct(y.received_exclusive_cents, maxYear) }} />
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-emerald-500" style={{ width: pct(y.net_cents, maxYear) }} />
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="ranking" className="space-y-4">
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("dashboard.statusDist")}</CardTitle></CardHeader>
            <CardContent className="space-y-2">
              {data.by_status.length === 0 ? (
                <div className="text-sm text-muted-foreground">{t("dashboard.empty")}</div>
              ) : data.by_status.map((s) => (
                <div key={s.status} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{statusLabel(s.status)} · {s.count}</span>
                    <span className="text-muted-foreground">{formatCNY(s.contract_inclusive_cents)}</span>
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-sky-400" style={{ width: pct(s.contract_inclusive_cents, maxStatus) }} />
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
          <div className="grid grid-cols-2 gap-4">
            <RankCard title={t("dashboard.topClients")} rows={data.top_clients} t={t} />
            <RankCard title={t("dashboard.topProjects")} rows={data.top_projects} t={t} />
          </div>
        </TabsContent>

        <TabsContent value="receivables" className="space-y-4">
          <Kpi label={t("dashboard.receivablesOutstanding")} value={formatCNY(data.receivables_outstanding_cents)} />
          <Card>
            <CardContent className="p-0">
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-28">{t("dashboard.dueDate")}</TableHead>
                    <TableHead>{t("dashboard.project")}</TableHead>
                    <TableHead>{t("payment.name")}</TableHead>
                    <TableHead className="text-right w-32">{t("payment.expectedAmount")}</TableHead>
                    <TableHead className="w-20">{t("dashboard.status")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.receivables.length === 0 ? (
                    <TableRow><TableCell colSpan={5} className="p-4 text-sm text-muted-foreground">{t("dashboard.noReceivables")}</TableCell></TableRow>
                  ) : data.receivables.map((r, i) => (
                    <TableRow key={i}>
                      <TableCell className="whitespace-nowrap">{r.expected_date}</TableCell>
                      <TableCell>{r.project_name}</TableCell>
                      <TableCell className="text-muted-foreground">{r.name}</TableCell>
                      <TableCell className="text-right">{formatCNY(r.expected_amount_cents)}</TableCell>
                      <TableCell>
                        <Badge variant="secondary" className={BUCKET_CLASS[r.bucket] ?? ""}>
                          {t(`dashboard.bucket.${r.bucket}`)}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
