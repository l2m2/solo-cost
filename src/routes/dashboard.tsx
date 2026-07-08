import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { toast } from "sonner";
import { useCompanyStore } from "@/stores/company";
import { useDashboardStore } from "@/stores/dashboard";
import { useTasksStore } from "@/stores/tasks";
import { useTimelogsStore } from "@/stores/timelogs";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { RefreshCw, Play, CheckCircle } from "lucide-react";
import { StatusTransitionDialog } from "@/components/tasks/StatusTransitionDialog";
import { LedgerOverview } from "@/components/dashboard/LedgerOverview";
import { LedgerPanel, LedgerBar } from "@/components/dashboard/ledgerParts";
import { PAPER, INK, INK_SOFT, VERMILION, INDIGO, RULE, SERIF } from "@/components/dashboard/ledgerTokens";
import { call } from "@/lib/ipc";
import { formatCNY } from "@/lib/money";
import { statusLabel } from "@/lib/status";
import type { RankRow, DashYearRow, DashTaskRow, Task, TaskInput } from "@/types";

// Ledger dot colours for receivable buckets (overdue reads as red ink).
const LEDGER_BUCKET_DOT: Record<string, string> = {
  overdue: VERMILION,
  soon: INDIGO,
  future: "#B0A794",
};

function RankCard({ title, rows, t }: { title: string; rows: RankRow[]; t: TFunction }) {
  return (
    <LedgerPanel title={title}>
      <Table compact>
        <TableHeader>
          <TableRow style={{ borderColor: RULE }}>
            <TableHead className="w-8 text-center" style={{ color: INK_SOFT }}>#</TableHead>
            <TableHead style={{ color: INK_SOFT }}>{t("dashboard.name")}</TableHead>
            <TableHead className="text-right w-28" style={{ color: INK_SOFT }}>{t("dashboard.netLabel")}</TableHead>
            <TableHead className="text-right w-28" style={{ color: INK_SOFT }}>{t("dashboard.receivedLabel")}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.length === 0 ? (
            <TableRow style={{ borderColor: RULE }}><TableCell colSpan={4} className="p-4 text-sm" style={{ color: INK_SOFT }}>{t("dashboard.empty")}</TableCell></TableRow>
          ) : rows.map((r, i) => (
            <TableRow key={r.id} className="hover:bg-black/[0.03]" style={{ borderColor: RULE }}>
              <TableCell className="text-center">
                <span
                  className="text-sm font-medium tabular-nums"
                  style={{ ...SERIF, color: i < 3 ? VERMILION : INK_SOFT }}
                >
                  {i + 1}
                </span>
              </TableCell>
              <TableCell className="font-medium">{r.name}</TableCell>
              <TableCell className="text-right tabular-nums" style={SERIF}>{formatCNY(r.net_cents)}</TableCell>
              <TableCell className="text-right tabular-nums" style={{ color: INK_SOFT }}>{formatCNY(r.received_inclusive_cents)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </LedgerPanel>
  );
}

// Ledger status dots — a restrained substitute for filled badges on paper.
const LEDGER_STATUS_DOT: Record<string, string> = {
  todo: "#B0A794",
  in_progress: INDIGO,
  done: INK_SOFT,
};

// Todo list styled as a page of the account book, to match LedgerOverview.
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
    <div className="overflow-hidden rounded-lg" style={{ background: PAPER, color: INK, border: `1px solid ${RULE}` }}>
      <div className="px-5 py-3" style={{ borderBottom: `1px solid ${RULE}` }}>
        <h3 className="text-sm font-medium" style={SERIF}>{t("dashboard.todoTasks")} ({count})</h3>
      </div>
      <Table compact>
        <TableHeader>
          <TableRow style={{ borderColor: RULE }}>
            <TableHead className="w-32" style={{ color: INK_SOFT }}>{t("dashboard.project")}</TableHead>
            <TableHead className="min-w-56" style={{ color: INK_SOFT }}>{t("dashboard.taskTitle")}</TableHead>
            <TableHead className="min-w-20" style={{ color: INK_SOFT }}>{t("dashboard.assignee")}</TableHead>
            <TableHead className="w-20" style={{ color: INK_SOFT }}>{t("dashboard.status")}</TableHead>
            <TableHead className="w-28" style={{ color: INK_SOFT }}>{t("dashboard.taskDue")}</TableHead>
            <TableHead className="w-36" style={{ color: INK_SOFT }}>{t("dashboard.startedAt")}</TableHead>
            <TableHead className="w-36" style={{ color: INK_SOFT }}>{t("dashboard.completedAt")}</TableHead>
            <TableHead className="w-20 text-right" style={{ color: INK_SOFT }}>{t("common.actions")}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.length === 0 ? (
            <TableRow style={{ borderColor: RULE }}><TableCell colSpan={8} className="p-4 text-sm" style={{ color: INK_SOFT }}>{t("dashboard.noTodoTasks")}</TableCell></TableRow>
          ) : rows.map((r) => (
            <TableRow key={r.task_id} className="hover:bg-black/[0.03]" style={{ borderColor: RULE }}>
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
              <TableCell style={{ color: INK_SOFT }}>{r.assignee_name ?? "—"}</TableCell>
              <TableCell>
                <span className="inline-flex items-center gap-1.5 whitespace-nowrap text-xs" style={{ color: INK_SOFT }}>
                  <span className="h-1.5 w-1.5 rounded-full" style={{ background: LEDGER_STATUS_DOT[r.status] ?? INK_SOFT }} />
                  {t(`taskStatus.${r.status}`)}
                </span>
              </TableCell>
              <TableCell className="whitespace-nowrap tabular-nums" style={r.overdue ? { color: VERMILION } : undefined}>{r.due_date ?? "—"}</TableCell>
              <TableCell className="whitespace-nowrap tabular-nums" style={{ color: INK_SOFT }}>{r.started_at ?? "—"}</TableCell>
              <TableCell className="whitespace-nowrap tabular-nums" style={{ color: INK_SOFT }}>{r.completed_at ?? "—"}</TableCell>
              <TableCell className="text-right whitespace-nowrap">
                {r.status === "todo" && (
                  <Button size="sm" variant="ghost" className="h-7 px-2" title="开始" style={{ color: INK_SOFT }} onClick={() => onStart(r)}>
                    <Play className="h-4 w-4" />
                  </Button>
                )}
                {r.status !== "done" && (
                  <Button size="sm" variant="ghost" className="h-7 px-2" title="完成" style={{ color: INK_SOFT }} onClick={() => onComplete(r)}>
                    <CheckCircle className="h-4 w-4" />
                  </Button>
                )}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {hidden > 0 && (
        <div className="px-5 py-2 text-sm" style={{ color: INK_SOFT, borderTop: `1px solid ${RULE}` }}>
          {t("dashboard.taskMore", { count: hidden })}
        </div>
      )}
    </div>
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

  const maxStatus = Math.max(1, ...data.by_status.map((s) => s.contract_inclusive_cents));

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between pb-3" style={{ borderBottom: `2px solid ${INK}` }}>
        <div>
          <h1 className="text-xl font-semibold tracking-tight" style={SERIF}>{t("nav.dashboard")}</h1>
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
          <LedgerOverview data={data} t={t} onOpenYear={setOpenYear} />
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
          <LedgerPanel title={t("dashboard.statusDist")} bodyClassName="space-y-2 p-5">
            {data.by_status.length === 0 ? (
              <div className="text-sm" style={{ color: INK_SOFT }}>{t("dashboard.empty")}</div>
            ) : data.by_status.map((s) => (
              <div key={s.status} className="space-y-1">
                <div className="flex justify-between text-xs">
                  <span>
                    {statusLabel(s.status)}
                    <span className="ml-1.5 tabular-nums" style={{ color: INK_SOFT }}>{s.count}</span>
                  </span>
                  <span className="tabular-nums" style={{ color: INK_SOFT }}>{formatCNY(s.contract_inclusive_cents)}</span>
                </div>
                <LedgerBar value={s.contract_inclusive_cents} max={maxStatus} color={INDIGO} />
              </div>
            ))}
          </LedgerPanel>
          <div className="grid grid-cols-2 gap-4">
            <RankCard title={t("dashboard.topClients")} rows={data.top_clients} t={t} />
            <RankCard title={t("dashboard.topProjects")} rows={data.top_projects} t={t} />
          </div>
        </TabsContent>

        <TabsContent value="receivables" className="space-y-4">
          <LedgerPanel
            title={t("dashboard.tabReceivables")}
            right={
              <span className="flex items-baseline gap-2">
                <span className="text-xs" style={{ color: INK_SOFT }}>{t("dashboard.receivablesOutstanding")}</span>
                <span className="text-lg font-semibold tabular-nums" style={SERIF}>{formatCNY(data.receivables_outstanding_cents)}</span>
              </span>
            }
          >
            <Table compact>
              <TableHeader>
                <TableRow style={{ borderColor: RULE }}>
                  <TableHead className="w-28" style={{ color: INK_SOFT }}>{t("dashboard.dueDate")}</TableHead>
                  <TableHead className="min-w-32" style={{ color: INK_SOFT }}>{t("dashboard.project")}</TableHead>
                  <TableHead className="min-w-20" style={{ color: INK_SOFT }}>{t("dashboard.client")}</TableHead>
                  <TableHead className="min-w-24" style={{ color: INK_SOFT }}>{t("payment.name")}</TableHead>
                  <TableHead className="text-right w-36 whitespace-nowrap" style={{ color: INK_SOFT }}>{t("payment.expectedAmount")}</TableHead>
                  <TableHead className="w-24" style={{ color: INK_SOFT }}>{t("dashboard.status")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.receivables.length === 0 ? (
                  <TableRow style={{ borderColor: RULE }}><TableCell colSpan={6} className="p-4 text-sm" style={{ color: INK_SOFT }}>{t("dashboard.noReceivables")}</TableCell></TableRow>
                ) : data.receivables.map((r, i) => (
                  <TableRow key={i} className="hover:bg-black/[0.03]" style={{ borderColor: RULE }}>
                    <TableCell className="whitespace-nowrap tabular-nums" style={r.bucket === "overdue" ? { color: VERMILION } : undefined}>{r.expected_date}</TableCell>
                    <TableCell className="font-medium">{r.project_name}</TableCell>
                    <TableCell style={{ color: INK_SOFT }}>{r.client_name || "—"}</TableCell>
                    <TableCell style={{ color: INK_SOFT }}>{r.name}</TableCell>
                    <TableCell className="text-right tabular-nums" style={SERIF}>{formatCNY(r.expected_amount_cents)}</TableCell>
                    <TableCell>
                      <span className="inline-flex items-center gap-1.5 whitespace-nowrap text-xs" style={{ color: INK_SOFT }}>
                        <span className="h-1.5 w-1.5 rounded-full" style={{ background: LEDGER_BUCKET_DOT[r.bucket] ?? INK_SOFT }} />
                        {t(`dashboard.bucket.${r.bucket}`)}
                      </span>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </LedgerPanel>
        </TabsContent>
      </Tabs>

      <Dialog open={!!openYear} onOpenChange={(o) => !o && setOpenYear(null)}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>{openYear ? t("dashboard.yearDetailTitle", { year: openYear.year }) : ""}</DialogTitle>
          </DialogHeader>
          {openYear && (
            <div className="space-y-3">
              <div className="grid grid-cols-4 gap-2 rounded-lg p-3 text-sm" style={{ background: PAPER, border: `1px solid ${RULE}`, color: INK }}>
                <div className="space-y-0.5">
                  <div className="text-xs" style={{ color: INK_SOFT }}>{t("dashboard.received")}</div>
                  <div className="font-medium tabular-nums" style={SERIF}>{formatCNY(openYear.received_inclusive_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs" style={{ color: INK_SOFT }}>{t("financial.generalCost")}</div>
                  <div className="font-medium tabular-nums" style={{ ...SERIF, color: VERMILION }}>−{formatCNY(openYear.general_cost_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs" style={{ color: INK_SOFT }}>{t("financial.commission")}</div>
                  <div className="font-medium tabular-nums" style={{ ...SERIF, color: VERMILION }}>−{formatCNY(openYear.commission_cents)}</div>
                </div>
                <div className="space-y-0.5">
                  <div className="text-xs" style={{ color: INK_SOFT }}>{t("dashboard.netLabel")}</div>
                  <div className="text-lg font-semibold tabular-nums" style={{ ...SERIF, color: INDIGO }}>{formatCNY(openYear.net_cents)}</div>
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
                        <TableCell className="text-right tabular-nums" style={SERIF}>{formatCNY(r.amount_inclusive_cents)}</TableCell>
                        <TableCell className="whitespace-nowrap tabular-nums">{r.received_at}</TableCell>
                        <TableCell className="text-right tabular-nums text-muted-foreground">{proj ? formatCNY(proj.commission_cents) : "—"}</TableCell>
                        <TableCell className="text-right font-medium tabular-nums" style={{ ...SERIF, color: INDIGO }}>{proj ? formatCNY(proj.net_cents) : "—"}</TableCell>
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
