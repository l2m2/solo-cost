import { useEffect, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { toast } from "sonner";
import { useCompanyStore } from "@/stores/company";
import { useDashboardStore } from "@/stores/dashboard";
import { useTasksStore } from "@/stores/tasks";
import { useTimelogsStore } from "@/stores/timelogs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  RefreshCw, Play, CheckCircle,
  FileText, Receipt, TrendingUp, Banknote, Clock, PiggyBank, Coins,
  type LucideIcon,
} from "lucide-react";
import { StatusTransitionDialog, TASK_STATUS_BADGE_CLASS } from "@/components/tasks/StatusTransitionDialog";
import { call } from "@/lib/ipc";
import { formatCNY } from "@/lib/money";
import { statusLabel } from "@/lib/status";
import type { RankRow, DashYearRow, DashTaskRow, Task, TaskInput } from "@/types";

const BUCKET_CLASS: Record<string, string> = {
  overdue: "bg-red-100 text-red-700",
  soon: "bg-amber-100 text-amber-700",
  future: "bg-slate-100 text-slate-600",
};

function Kpi({ label, value, sub, icon: Icon, accent }: {
  label: string;
  value: string;
  sub?: string;
  icon?: LucideIcon;
  accent?: boolean;
}) {
  return (
    <Card className="transition-shadow hover:shadow-sm">
      <CardContent className="flex items-start gap-3 p-4">
        {Icon && (
          <div
            className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${
              accent ? "bg-emerald-50 text-emerald-600" : "bg-muted text-muted-foreground"
            }`}
          >
            <Icon className="h-5 w-5" />
          </div>
        )}
        <div className="min-w-0 space-y-1">
          <div className="text-xs text-muted-foreground">{label}</div>
          <div className={`text-2xl font-semibold tracking-tight tabular-nums ${accent ? "text-emerald-600" : ""}`}>
            {value}
          </div>
          {sub && <div className="text-xs text-muted-foreground">{sub}</div>}
        </div>
      </CardContent>
    </Card>
  );
}

// A section label with a small accent bar, used to head KPI groups.
function SectionTitle({ children }: { children: ReactNode }) {
  return (
    <div className="mb-2 flex items-center gap-2">
      <span className="h-3.5 w-1 rounded-full bg-primary/70" />
      <span className="text-sm font-medium">{children}</span>
    </div>
  );
}

function LegendDot({ className, label }: { className: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className={`h-2 w-2 rounded-full ${className}`} />
      {label}
    </span>
  );
}

// Unified rounded progress bar for the year / status breakdowns.
function Bar({ value, max, className }: { value: number; max: number; className: string }) {
  const width = `${Math.max(0, Math.min(100, (value / max) * 100))}%`;
  return (
    <div className="h-2 overflow-hidden rounded-full bg-muted">
      <div className={`h-full rounded-full ${className}`} style={{ width }} />
    </div>
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
              <TableHead className="w-8 text-center">#</TableHead>
              <TableHead>{t("dashboard.name")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.netLabel")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.receivedLabel")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow><TableCell colSpan={4} className="p-4 text-sm text-muted-foreground">{t("dashboard.empty")}</TableCell></TableRow>
            ) : rows.map((r, i) => (
              <TableRow key={r.id}>
                <TableCell className="text-center">
                  <span
                    className={`inline-flex h-5 w-5 items-center justify-center rounded-full text-xs tabular-nums ${
                      i < 3 ? "bg-primary/10 font-medium text-primary" : "text-muted-foreground"
                    }`}
                  >
                    {i + 1}
                  </span>
                </TableCell>
                <TableCell className="font-medium">{r.name}</TableCell>
                <TableCell className="text-right tabular-nums">{formatCNY(r.net_cents)}</TableCell>
                <TableCell className="text-right tabular-nums text-muted-foreground">{formatCNY(r.received_inclusive_cents)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function TodoTasksCard({
  rows, count, t, onOpen, onStart, onComplete,
}: {
  rows: DashTaskRow[];
  count: number;
  t: TFunction;
  onOpen: (projectId: number) => void;
  onStart: (row: DashTaskRow) => void;
  onComplete: (row: DashTaskRow) => void;
}) {
  const hidden = count - rows.length;
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-sm">{t("dashboard.todoTasks")} ({count})</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <Table compact>
          <TableHeader>
            <TableRow>
              <TableHead className="w-32">{t("dashboard.project")}</TableHead>
              <TableHead className="min-w-56">{t("dashboard.taskTitle")}</TableHead>
              <TableHead className="min-w-20">{t("dashboard.assignee")}</TableHead>
              <TableHead className="w-20">{t("dashboard.status")}</TableHead>
              <TableHead className="w-28">{t("dashboard.taskDue")}</TableHead>
              <TableHead className="w-36">{t("dashboard.startedAt")}</TableHead>
              <TableHead className="w-36">{t("dashboard.completedAt")}</TableHead>
              <TableHead className="w-20 text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow><TableCell colSpan={8} className="p-4 text-sm text-muted-foreground">{t("dashboard.noTodoTasks")}</TableCell></TableRow>
            ) : rows.map((r) => (
              <TableRow key={r.task_id}>
                <TableCell>
                  <button
                    className="block max-w-32 truncate text-left hover:underline cursor-pointer"
                    title={r.project_name}
                    onClick={() => onOpen(r.project_id)}
                  >
                    {r.project_name}
                  </button>
                </TableCell>
                <TableCell className="font-medium">
                  <button className="text-left hover:underline cursor-pointer" onClick={() => onOpen(r.project_id)}>
                    {r.title}
                  </button>
                </TableCell>
                <TableCell className="text-muted-foreground">{r.assignee_name ?? "—"}</TableCell>
                <TableCell>
                  <Badge variant="secondary" className={`whitespace-nowrap ${TASK_STATUS_BADGE_CLASS[r.status] ?? ""}`}>
                    {t(`taskStatus.${r.status}`)}
                  </Badge>
                </TableCell>
                <TableCell className={`whitespace-nowrap tabular-nums ${r.overdue ? "text-red-600" : ""}`}>{r.due_date ?? "—"}</TableCell>
                <TableCell className="text-muted-foreground whitespace-nowrap tabular-nums">{r.started_at ?? "—"}</TableCell>
                <TableCell className="text-muted-foreground whitespace-nowrap tabular-nums">{r.completed_at ?? "—"}</TableCell>
                <TableCell className="text-right whitespace-nowrap">
                  {r.status === "todo" && (
                    <Button size="sm" variant="ghost" className="h-7 px-2" title="开始" onClick={() => onStart(r)}>
                      <Play className="h-4 w-4" />
                    </Button>
                  )}
                  {r.status !== "done" && (
                    <Button size="sm" variant="ghost" className="h-7 px-2" title="完成" onClick={() => onComplete(r)}>
                      <CheckCircle className="h-4 w-4" />
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        {hidden > 0 && (
          <div className="border-t px-3 py-2 text-sm text-muted-foreground">
            {t("dashboard.taskMore", { count: hidden })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const currentId = useCompanyStore((s) => s.currentId);
  const { data, loadedForCompany, loadFor } = useDashboardStore();
  const updateTask = useTasksStore((s) => s.update);
  const createTimelog = useTimelogsStore((s) => s.create);
  const [openYear, setOpenYear] = useState<DashYearRow | null>(null);
  const [startingTask, setStartingTask] = useState<Task | null>(null);
  const [completingTask, setCompletingTask] = useState<Task | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // Start/complete act directly from the dashboard: fetch the full task (the
  // dialog needs every field to build its update payload), then open the shared
  // transition dialog. On success the whole dashboard reloads.
  const openTaskAction = async (row: DashTaskRow, kind: "start" | "complete") => {
    try {
      const task = await call<Task>("get_task", { id: row.task_id });
      if (kind === "start") setStartingTask(task);
      else setCompletingTask(task);
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    }
  };

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

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">{t("nav.dashboard")}</h1>
          <p className="text-sm text-muted-foreground">{t("dashboard.subtitle")}</p>
        </div>
        <Button
          variant="outline"
          size="sm"
          disabled={refreshing}
          onClick={async () => {
            setRefreshing(true);
            try { await loadFor(currentId); }
            finally { setRefreshing(false); }
          }}
        >
          <RefreshCw className={`mr-1 h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
          {t("dashboard.refresh")}
        </Button>
      </div>
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">{t("dashboard.tabOverview")}</TabsTrigger>
          <TabsTrigger value="ranking">{t("dashboard.tabRanking")}</TabsTrigger>
          <TabsTrigger value="receivables">{t("dashboard.tabReceivables")}</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-5">
          <div>
            <SectionTitle>{t("dashboard.contractScope")}</SectionTitle>
            <div className="grid grid-cols-3 gap-3">
              <Kpi icon={FileText} label={t("dashboard.contractTotal")} value={formatCNY(data.contract_total_inclusive_cents)} />
              <Kpi icon={Receipt} label={t("dashboard.revenueExclusive")} value={formatCNY(data.revenue_exclusive_cents)} />
              <Kpi icon={TrendingUp} accent label={t("dashboard.netPotential")} value={formatCNY(data.net_potential_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <div>
            <SectionTitle>{t("dashboard.receivedScope")}</SectionTitle>
            <div className="grid grid-cols-3 gap-3">
              <Kpi icon={Banknote} label={t("dashboard.received")} value={formatCNY(data.received_inclusive_cents)} />
              <Kpi icon={Clock} label={t("dashboard.outstanding")} value={formatCNY(data.outstanding_cents)} />
              <Kpi icon={PiggyBank} accent label={t("dashboard.netRealized")} value={formatCNY(data.net_realized_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <Card>
            <CardHeader className="flex-row items-center justify-between space-y-0">
              <CardTitle className="text-sm">{t("dashboard.netByYear")}</CardTitle>
              <div className="flex items-center gap-4 text-xs text-muted-foreground">
                <LegendDot className="bg-slate-300" label={t("dashboard.receivedLabel")} />
                <LegendDot className="bg-emerald-500" label={t("dashboard.netLabel")} />
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              {data.by_year.length === 0 ? (
                <div className="text-sm text-muted-foreground">{t("dashboard.empty")}</div>
              ) : data.by_year.map((y) => (
                <button
                  key={y.year}
                  type="button"
                  className="block w-full space-y-1.5 rounded-md p-1.5 text-left transition-colors hover:bg-muted/50"
                  title={t("dashboard.viewYearDetail")}
                  onClick={() => setOpenYear(y)}
                >
                  <div className="flex justify-between text-xs">
                    <span className="font-medium tabular-nums">{y.year}</span>
                    <span className="text-muted-foreground tabular-nums">
                      {t("dashboard.netLabel")} {formatCNY(y.net_cents)} · {t("dashboard.receivedLabel")} {formatCNY(y.received_exclusive_cents)}
                    </span>
                  </div>
                  <Bar value={y.received_exclusive_cents} max={maxYear} className="bg-slate-300" />
                  <Bar value={y.net_cents} max={maxYear} className="bg-emerald-500" />
                </button>
              ))}
            </CardContent>
          </Card>
          <TodoTasksCard
            rows={data.todo_tasks}
            count={data.todo_task_count}
            t={t}
            onOpen={(projectId) => navigate(`/projects/${projectId}`)}
            onStart={(row) => openTaskAction(row, "start")}
            onComplete={(row) => openTaskAction(row, "complete")}
          />
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
                    <span>
                      {statusLabel(s.status)}
                      <span className="ml-1.5 text-muted-foreground tabular-nums">{s.count}</span>
                    </span>
                    <span className="text-muted-foreground tabular-nums">{formatCNY(s.contract_inclusive_cents)}</span>
                  </div>
                  <Bar value={s.contract_inclusive_cents} max={maxStatus} className="bg-sky-400" />
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
          <div className="max-w-xs">
            <Kpi icon={Coins} label={t("dashboard.receivablesOutstanding")} value={formatCNY(data.receivables_outstanding_cents)} />
          </div>
          <Card>
            <CardContent className="p-0">
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-28">{t("dashboard.dueDate")}</TableHead>
                    <TableHead className="min-w-32">{t("dashboard.project")}</TableHead>
                    <TableHead className="min-w-20">{t("dashboard.client")}</TableHead>
                    <TableHead className="min-w-24">{t("payment.name")}</TableHead>
                    <TableHead className="text-right w-36 whitespace-nowrap">{t("payment.expectedAmount")}</TableHead>
                    <TableHead className="w-24">{t("dashboard.status")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.receivables.length === 0 ? (
                    <TableRow><TableCell colSpan={6} className="p-4 text-sm text-muted-foreground">{t("dashboard.noReceivables")}</TableCell></TableRow>
                  ) : data.receivables.map((r, i) => (
                    <TableRow key={i}>
                      <TableCell className="whitespace-nowrap tabular-nums">{r.expected_date}</TableCell>
                      <TableCell className="font-medium">{r.project_name}</TableCell>
                      <TableCell className="text-muted-foreground">{r.client_name || "—"}</TableCell>
                      <TableCell className="text-muted-foreground">{r.name}</TableCell>
                      <TableCell className="text-right tabular-nums">{formatCNY(r.expected_amount_cents)}</TableCell>
                      <TableCell>
                        <Badge variant="secondary" className={`whitespace-nowrap ${BUCKET_CLASS[r.bucket] ?? ""}`}>
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

      <Dialog open={!!openYear} onOpenChange={(o) => !o && setOpenYear(null)}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>{openYear ? t("dashboard.yearDetailTitle", { year: openYear.year }) : ""}</DialogTitle>
          </DialogHeader>
          {openYear && (
            <div className="space-y-3">
              <div className="grid grid-cols-4 gap-2 rounded-lg border bg-muted/30 p-3 text-sm">
                <div className="space-y-0.5">
                  <div className="text-xs text-muted-foreground">{t("dashboard.received")}</div>
                  <div className="font-medium tabular-nums">{formatCNY(openYear.received_inclusive_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs text-muted-foreground">{t("financial.generalCost")}</div>
                  <div className="font-medium tabular-nums">−{formatCNY(openYear.general_cost_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs text-muted-foreground">{t("financial.commission")}</div>
                  <div className="font-medium tabular-nums">−{formatCNY(openYear.commission_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs text-muted-foreground">{t("dashboard.netLabel")}</div>
                  <div className="font-semibold tabular-nums text-emerald-600">{formatCNY(openYear.net_cents)}</div>
                </div>
              </div>
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead className="min-w-32">{t("dashboard.project")}</TableHead>
                    <TableHead className="min-w-24">{t("payment.name")}</TableHead>
                    <TableHead className="text-right w-32">{t("payment.actualAmount")}</TableHead>
                    <TableHead className="w-28">{t("dashboard.dueDate")}</TableHead>
                    <TableHead className="text-right w-28">{t("financial.commission")}</TableHead>
                    <TableHead className="text-right w-28">{t("dashboard.netLabel")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {openYear.receipts.length === 0 ? (
                    <TableRow><TableCell colSpan={6} className="p-4 text-sm text-muted-foreground">{t("dashboard.empty")}</TableCell></TableRow>
                  ) : openYear.receipts.map((r, i) => {
                    const proj = openYear.projects.find((p) => p.project_id === r.project_id);
                    return (
                      <TableRow key={i}>
                        <TableCell className="font-medium">{r.project_name}</TableCell>
                        <TableCell className="text-muted-foreground">{r.name}</TableCell>
                        <TableCell className="text-right tabular-nums">{formatCNY(r.amount_inclusive_cents)}</TableCell>
                        <TableCell className="whitespace-nowrap tabular-nums">{r.received_at}</TableCell>
                        <TableCell className="text-right tabular-nums text-muted-foreground">{proj ? formatCNY(proj.commission_cents) : "—"}</TableCell>
                        <TableCell className="text-right font-medium tabular-nums">{proj ? formatCNY(proj.net_cents) : "—"}</TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!startingTask} onOpenChange={(o) => !o && setStartingTask(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>开始任务</DialogTitle></DialogHeader>
          {startingTask && (
            <StatusTransitionDialog
              task={startingTask}
              label="开始时间"
              fieldKey="started_at"
              onSubmit={async (input) => {
                try {
                  await updateTask(startingTask.id, { ...input, status: "in_progress" } as TaskInput, startingTask.project_id);
                  setStartingTask(null);
                  if (currentId != null) await loadFor(currentId);
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setStartingTask(null)}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!completingTask} onOpenChange={(o) => !o && setCompletingTask(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>完成任务</DialogTitle></DialogHeader>
          {completingTask && (
            <StatusTransitionDialog
              task={completingTask}
              label="完成时间"
              fieldKey="completed_at"
              existingHours={completingTask.actual_hours}
              onSubmit={async (input) => {
                try {
                  await updateTask(completingTask.id, { ...input, status: "done" } as TaskInput, completingTask.project_id);
                  const h = (input as Record<string, unknown>).hours;
                  if (typeof h === "number" && h > 0 && completingTask.assignee_id != null) {
                    await createTimelog({
                      task_id: completingTask.id,
                      member_id: completingTask.assignee_id,
                      work_date: new Date().toISOString().slice(0, 10),
                      hours: h,
                    }, completingTask.project_id);
                  }
                  setCompletingTask(null);
                  if (currentId != null) await loadFor(currentId);
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setCompletingTask(null)}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
