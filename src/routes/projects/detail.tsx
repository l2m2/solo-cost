import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { STATUS_OPTIONS, statusBadgeClass, statusLabel } from "@/lib/status";
import { call } from "@/lib/ipc";
import { nowDatetimeLocal, todayIso } from "@/lib/time";
import { useCompanyStore } from "@/stores/company";
import { useCategoriesStore } from "@/stores/categories";
import { useCostsStore } from "@/stores/costs";
import { usePaymentsStore } from "@/stores/payments";
import { useTasksStore } from "@/stores/tasks";
import { useTimelogsStore } from "@/stores/timelogs";
import { useMembersStore } from "@/stores/members";
import { useProjectsStore } from "@/stores/projects";
import { useFinancialStore } from "@/stores/financial";
import { useModulesStore } from "@/stores/modules";
import { useModuleStatsStore } from "@/stores/moduleStats";
import { Badge } from "@/components/ui/badge";
import { Play, CheckCircle, Clock, ChevronRight, ChevronDown } from "lucide-react";
import ZentaoImportDialog from "@/components/zentao-import/ZentaoImportDialog";
import type { CostEntry, CostEntryInput, ContractPayment, PaymentInput, Project, Member, Module, ModuleLaborStat, Task, TaskInput, TimeLog, TimeLogInput, TimeLogUpdateInput, ProjectFinancialSummary } from "@/types";

const TASK_STATUS_BADGE_CLASS: Record<string, string> = {
  todo: "bg-slate-100 text-slate-700",
  in_progress: "bg-amber-100 text-amber-700",
  done: "bg-emerald-100 text-emerald-700",
  closed: "bg-zinc-200 text-zinc-500",
};

export default function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const pid = id ? Number(id) : NaN;
  const currentCompanyId = useCompanyStore((s) => s.currentId);
  const { loadedForCompany, loadFor: loadCats } = useCategoriesStore();
  const { loadFor: loadCosts } = useCostsStore();
  const setProjectStatus = useProjectsStore((s) => s.setStatus);
  const [project, setProject] = useState<Project | null>(null);

  useEffect(() => {
    if (Number.isNaN(pid)) return;
    call<Project>("get_project", { id: pid })
      .then(setProject)
      .catch((e: unknown) => {
        toast.error(t("common.error", { msg: String(e) }));
        navigate("/projects");
      });
  }, [pid, navigate, t]);

  useEffect(() => {
    if (project && currentCompanyId === project.company_id && loadedForCompany !== currentCompanyId) {
      loadCats(currentCompanyId);
    }
  }, [project, currentCompanyId, loadedForCompany, loadCats]);

  useEffect(() => {
    if (!Number.isNaN(pid)) loadCosts(pid);
  }, [pid, loadCosts]);

  const financial = useFinancialStore((s) => s.byProject[pid] ?? null);
  const refreshFinancial = useFinancialStore((s) => s.refresh);

  useEffect(() => {
    if (!Number.isNaN(pid)) refreshFinancial(pid);
  }, [pid, refreshFinancial]);

  // I-2 fix: navigate away when current company no longer matches the open project
  useEffect(() => {
    if (project && currentCompanyId != null && project.company_id !== currentCompanyId) {
      navigate("/projects", { replace: true });
    }
  }, [project, currentCompanyId, navigate]);

  if (!project) return null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h1 className="text-xl font-semibold">{project.name}</h1>
          <span className={`text-xs px-2 py-0.5 rounded ${statusBadgeClass(project.status)}`}>
            {statusLabel(project.status)}
          </span>
        </div>
        <Select
          value={project.status}
          onValueChange={async (v) => {
            try {
              const p = await setProjectStatus(project.id, v);
              setProject(p);
            } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
          }}
        >
          <SelectTrigger className="w-40"><SelectValue /></SelectTrigger>
          <SelectContent>
            {STATUS_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">概览</TabsTrigger>
          <TabsTrigger value="costs">成本</TabsTrigger>
          <TabsTrigger value="payments">收款</TabsTrigger>
          <TabsTrigger value="tasks">任务+工时</TabsTrigger>
          <TabsTrigger value="attachments" disabled>附件（M4）</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="mt-4">
          <FinancialPanel project={project} financial={financial} />
        </TabsContent>

        <TabsContent value="costs" className="mt-4">
          <CostsPanel projectId={project.id} />
        </TabsContent>

        <TabsContent value="payments" className="mt-4">
          <PaymentsPanel projectId={project.id} />
        </TabsContent>

        <TabsContent value="tasks" className="mt-4">
          <TasksPanel projectId={project.id} companyId={project.company_id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function FinancialPanel({
  project,
  financial,
}: {
  project: Project;
  financial: ProjectFinancialSummary | null;
}) {
  const { t } = useTranslation();
  const formatRate = (r: number) => `${(r * 100).toFixed(2)}%`;
  return (
    <div className="space-y-4">
      {/* basic project info */}
      <div className="grid grid-cols-2 gap-3">
        <Card>
          <CardHeader><CardTitle className="text-sm">客户</CardTitle></CardHeader>
          <CardContent>{project.client_name ?? "—"}</CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">起止日期</CardTitle></CardHeader>
          <CardContent>
            {project.start_date ?? "—"} ~ {project.end_date ?? "—"}
          </CardContent>
        </Card>
      </div>

      {/* revenue / tax */}
      <div className="grid grid-cols-3 gap-3">
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.revenueInclusive")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.revenue_tax_inclusive_cents) : "—"}
            <div className="text-xs text-muted-foreground mt-1">
              税率 {(project.tax_rate * 100).toFixed(2)}% · {project.contract_amount_is_tax_inclusive ? "含税合同" : "不含税合同"}
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.revenueExclusive")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.revenue_tax_exclusive_cents) : "—"}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.tax")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.tax_amount_cents) : "—"}
          </CardContent>
        </Card>
      </div>

      {/* costs */}
      <div
        className={`grid gap-3 ${project.commission_mode !== "none" ? "grid-cols-4" : "grid-cols-3"}`}
      >
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.generalCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.general_cost_cents) : "—"}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.laborCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.labor_cost_cents) : "—"}
          </CardContent>
        </Card>
        {project.commission_mode !== "none" && (
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("financial.commission")}</CardTitle></CardHeader>
            <CardContent className="text-xl font-semibold">
              {financial ? formatCNY(financial.commission_cents) : "—"}
              <div className="text-xs text-muted-foreground mt-1">
                {project.commission_mode === "rate" &&
                  t("financial.commissionRateFootnote", {
                    rate: `${((project.commission_rate ?? 0) * 100).toFixed(2)}%`,
                  })}
                {project.commission_mode === "fixed" &&
                  (project.commission_settled
                    ? t("financial.commissionFixedSettled", {
                        amount: formatCNY(project.commission_amount_cents ?? 0),
                      })
                    : t("financial.commissionFixedUnsettled", {
                        amount: formatCNY(project.commission_amount_cents ?? 0),
                      }))}
              </div>
            </CardContent>
          </Card>
        )}
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.totalCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.total_cost_cents) : "—"}
          </CardContent>
        </Card>
      </div>

      {/* profit & collection */}
      <div className="grid grid-cols-2 gap-3">
        <Card className="border-primary">
          <CardHeader><CardTitle className="text-sm">{t("financial.grossProfit")}</CardTitle></CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {financial ? formatCNY(financial.gross_profit_cents) : "—"}
            </div>
            <div className="text-sm text-muted-foreground mt-1">
              {t("financial.profitRate")}：{financial ? formatRate(financial.profit_rate) : "—"}
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.collectionRate")}</CardTitle></CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {financial ? formatRate(financial.collection_rate) : "—"}
            </div>
            <div className="text-sm text-muted-foreground mt-1">
              {t("financial.actualPayment")}：{financial ? formatCNY(financial.actual_payment_cents) : "—"} /
              {" "}
              {t("financial.expectedPayment")}：{financial ? formatCNY(financial.expected_payment_cents) : "—"}
            </div>
          </CardContent>
        </Card>
      </div>

      {project.notes && (
        <Card>
          <CardHeader><CardTitle className="text-sm">备注</CardTitle></CardHeader>
          <CardContent className="whitespace-pre-wrap text-sm">{project.notes}</CardContent>
        </Card>
      )}
    </div>
  );
}

function CostsPanel({ projectId }: { projectId: number }) {
  const { t } = useTranslation();
  const { list: cats } = useCategoriesStore();
  const { entriesByProject, summaryByProject, create, update, remove } = useCostsStore();
  const entries = entriesByProject[projectId] ?? [];
  const summary = summaryByProject[projectId];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<CostEntry | null>(null);

  const findCatName = (cid: number) => cats.find((c) => c.id === cid)?.name ?? `#${cid}`;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="text-sm">
          {t("cost.totalLabel")}：<span className="font-semibold">{formatCNY(summary?.total_cents ?? 0)}</span>
        </div>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("cost.add")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("cost.add")}</DialogTitle></DialogHeader>
            <CostForm
              cats={cats}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {summary && summary.by_category.length > 0 && (
        <Card>
          <CardHeader><CardTitle className="text-sm">按科目汇总</CardTitle></CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-2">
              {summary.by_category.map((b) => (
                <div key={b.category_id} className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{b.category_name}</span>
                  <span className="font-medium">{formatCNY(b.total_cents)}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {entries.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("cost.empty")}</CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-28">{t("cost.incurredAt")}</TableHead>
                  <TableHead className="w-32">{t("cost.category")}</TableHead>
                  <TableHead className="text-right w-32">{t("cost.amount")}</TableHead>
                  <TableHead>{t("cost.description")}</TableHead>
                  <TableHead className="w-40 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((e) => (
                  <TableRow key={e.id}>
                    <TableCell>{e.incurred_at}</TableCell>
                    <TableCell>{findCatName(e.category_id)}</TableCell>
                    <TableCell className="text-right">{formatCNY(e.amount_cents)}</TableCell>
                    <TableCell className="text-muted-foreground">{e.description ?? ""}</TableCell>
                    <TableCell className="text-right">
                      <Button size="sm" variant="ghost" className="h-7 px-2" onClick={() => setEditing(e)}>{t("cost.edit")}</Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        onClick={async () => {
                          if (!confirm(t("cost.deleteConfirm"))) return;
                          try { await remove(e.id, projectId); }
                          catch (err: unknown) { toast.error(t("common.error", { msg: String(err) })); }
                        }}
                      >
                        {t("cost.delete")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("cost.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <CostForm
              cats={cats}
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try {
                  await update(editing.id, input, projectId);
                  setEditing(null);
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function CostForm({
  cats,
  initial,
  onSubmit,
  onCancel,
}: {
  cats: { id: number; name: string }[];
  initial?: CostEntry;
  onSubmit: (input: CostEntryInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [categoryId, setCategoryId] = useState(initial?.category_id ?? cats[0]?.id ?? 0);
  const [date, setDate] = useState(initial?.incurred_at ?? new Date().toISOString().slice(0, 10));
  const [amount, setAmount] = useState(initial?.amount_cents ?? 0);
  const [desc, setDesc] = useState(initial?.description ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!categoryId) return toast.error(t("cost.categoryRequired"));
    if (!date) return toast.error(t("cost.incurredAtRequired"));
    if (amount < 0) return toast.error(t("cost.amountInvalid"));
    setBusy(true);
    try {
      await onSubmit({
        category_id: categoryId,
        incurred_at: date,
        amount_cents: amount,
        description: desc.trim() || null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("cost.category")}</Label>
          <Select value={String(categoryId)} onValueChange={(v) => setCategoryId(Number(v))}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {cats.map((c) => (
                <SelectItem key={c.id} value={String(c.id)}>{c.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("cost.incurredAt")}</Label>
          <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("cost.amount")}</Label>
        <MoneyInput value={amount} onChange={setAmount} />
      </div>
      <div className="space-y-1">
        <Label>{t("cost.description")}</Label>
        <Input value={desc} onChange={(e) => setDesc(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("cost.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("cost.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function PaymentsPanel({ projectId }: { projectId: number }) {
  const { t } = useTranslation();
  const { byProject, loadFor, create, update, markReceived, softDelete } = usePaymentsStore();
  const list = byProject[projectId] ?? [];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<ContractPayment | null>(null);
  const [marking, setMarking] = useState<ContractPayment | null>(null);

  useEffect(() => { loadFor(projectId); }, [projectId, loadFor]);

  const expectedTotal = list.reduce((s, p) => s + p.expected_amount_cents, 0);
  const actualTotal = list.reduce(
    (s, p) => s + (p.actual_received_at && p.actual_amount_cents != null ? p.actual_amount_cents : 0),
    0,
  );
  const rate = expectedTotal === 0 ? 0 : actualTotal / expectedTotal;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-3 gap-3">
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.expectedLabel")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{formatCNY(expectedTotal)}</CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.actualLabel")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{formatCNY(actualTotal)}</CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.collectionRate")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{(rate * 100).toFixed(2)}%</CardContent></Card>
      </div>

      <div className="flex justify-end">
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("payment.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("payment.create")}</DialogTitle></DialogHeader>
            <PaymentForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("payment.empty")}</CardContent></Card>
      ) : (
        <Card><CardContent className="p-0">
          <Table compact>
            <TableHeader>
              <TableRow>
                <TableHead>{t("payment.name")}</TableHead>
                <TableHead className="w-28">{t("payment.expectedDate")}</TableHead>
                <TableHead className="text-right w-32">{t("payment.expectedAmount")}</TableHead>
                <TableHead className="w-28">{t("payment.actualReceivedAt")}</TableHead>
                <TableHead className="text-right w-32">{t("payment.actualAmount")}</TableHead>
                <TableHead className="w-56 text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.map((p) => (
                <TableRow key={p.id}>
                  <TableCell>{p.name}</TableCell>
                  <TableCell>{p.expected_date ?? "—"}</TableCell>
                  <TableCell className="text-right">{formatCNY(p.expected_amount_cents)}</TableCell>
                  <TableCell>{p.actual_received_at ?? "—"}</TableCell>
                  <TableCell className="text-right">
                    {p.actual_amount_cents != null ? formatCNY(p.actual_amount_cents) : "—"}
                  </TableCell>
                  <TableCell className="text-right">
                    {!p.actual_received_at && (
                      <Button size="sm" variant="ghost" className="h-7 px-2" onClick={() => setMarking(p)}>
                        {t("payment.markReceived")}
                      </Button>
                    )}
                    <Button size="sm" variant="ghost" className="h-7 px-2" onClick={() => setEditing(p)}>{t("payment.edit")}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 px-2"
                      onClick={async () => {
                        if (!confirm("确认删除该收款节点？")) return;
                        try { await softDelete(p.id, projectId); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >
                      {t("payment.delete")}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent></Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("payment.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <PaymentForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!marking} onOpenChange={(o) => !o && setMarking(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("payment.markReceived")}</DialogTitle></DialogHeader>
          {marking && (
            <MarkReceivedForm
              initial={marking}
              onCancel={() => setMarking(null)}
              onSubmit={async (amount, date) => {
                try {
                  await markReceived(marking.id, amount, date, projectId);
                  setMarking(null);
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function PaymentForm({ initial, onSubmit, onCancel }: {
  initial?: ContractPayment;
  onSubmit: (input: PaymentInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [expected, setExpected] = useState(initial?.expected_amount_cents ?? 0);
  const [expectedDate, setExpectedDate] = useState(initial?.expected_date ?? "");
  const [actualAmount, setActualAmount] = useState(
    initial?.actual_amount_cents ?? initial?.expected_amount_cents ?? 0
  );
  const [actualReceivedAt, setActualReceivedAt] = useState(initial?.actual_received_at ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("payment.nameRequired"));
    setBusy(true);
    try {
      // A receipt exists only when a received date is set; clearing the date un-receives.
      const received = !!actualReceivedAt;
      await onSubmit({
        name: name.trim(),
        expected_amount_cents: expected,
        expected_date: expectedDate || null,
        actual_amount_cents: received ? actualAmount : null,
        actual_received_at: received ? actualReceivedAt : null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("payment.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus /></div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("payment.expectedAmount")}</Label>
          <MoneyInput value={expected} onChange={setExpected} /></div>
        <div className="space-y-1"><Label>{t("payment.expectedDate")}</Label>
          <Input type="date" value={expectedDate ?? ""} onChange={(e) => setExpectedDate(e.target.value)} /></div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("payment.actualAmount")}</Label>
          <MoneyInput value={actualAmount} onChange={setActualAmount} /></div>
        <div className="space-y-1"><Label>{t("payment.actualReceivedAt")}</Label>
          <Input type="date" value={actualReceivedAt ?? ""} onChange={(e) => setActualReceivedAt(e.target.value)} /></div>
      </div>
      <div className="space-y-1"><Label>{t("payment.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("payment.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function MarkReceivedForm({ initial, onSubmit, onCancel }: {
  initial: ContractPayment;
  onSubmit: (actualAmountCents: number, actualReceivedAt: string) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [amount, setAmount] = useState(initial.actual_amount_cents ?? initial.expected_amount_cents);
  const [date, setDate] = useState(initial.actual_received_at ?? "");
  const [busy, setBusy] = useState(false);

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("payment.actualAmount")}</Label>
        <MoneyInput value={amount} onChange={setAmount} /></div>
      <div className="space-y-1"><Label>{t("payment.actualReceivedAt")}</Label>
        <Input type="date" value={date ?? ""} onChange={(e) => setDate(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!date) return toast.error("实收日期必填");
            setBusy(true);
            try { await onSubmit(amount, date); }
            finally { setBusy(false); }
          }}
        >确认</Button>
      </DialogFooter>
    </div>
  );
}

// ─── Tasks + TimeLogs ───────────────────────────────────────────────────────

function TasksPanel({ projectId, companyId }: { projectId: number; companyId: number }) {
  const { t } = useTranslation();
  const { byProject, loadFor, create, update, setStatus, softDelete } = useTasksStore();
  const { create: createTimelog } = useTimelogsStore();
  const {
    byProject: modulesByProject,
    loadedForProject: modulesLoadedFor,
    loadFor: loadModules,
    create: createModule,
    update: updateModule,
    moveUp: moveModuleUp,
    moveDown: moveModuleDown,
    softDelete: softDeleteModule,
  } = useModulesStore();
  const modules = modulesByProject[projectId] ?? [];
  const [moduleFilter, setModuleFilter] = useState<string>("__all"); // __all | __unassigned | <id>
  // __active hides closed tasks (default); __all shows everything; else exact status.
  const [statusFilter, setStatusFilter] = useState<string>("__active");
  const [openManageModules, setOpenManageModules] = useState(false);
  const [openZentaoImport, setOpenZentaoImport] = useState(false);
  const [moduleStatsOpen, setModuleStatsOpen] = useState(false);
  const tasks = byProject[projectId] ?? [];
  const visibleTasks = tasks.filter((tk) => {
    if (statusFilter === "__active") { if (tk.status === "closed") return false; }
    else if (statusFilter !== "__all") { if (tk.status !== statusFilter) return false; }
    if (moduleFilter === "__all") return true;
    if (moduleFilter === "__unassigned") return tk.module_id == null;
    return tk.module_id === Number(moduleFilter);
  });
  const { list: members, loadedForCompany: membersLoadedFor, loadFor: loadMembers } = useMembersStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Task | null>(null);
  const [openLogs, setOpenLogs] = useState<Task | null>(null);
  const [startingTask, setStartingTask] = useState<Task | null>(null);
  const [completingTask, setCompletingTask] = useState<Task | null>(null);
  const PAGE_SIZE = 20;
  const [page, setPage] = useState(1);
  const totalPages = Math.max(1, Math.ceil(visibleTasks.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages);
  const pagedTasks = visibleTasks.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);
  useEffect(() => { setPage(1); }, [statusFilter, moduleFilter]);

  const moduleStats: ModuleLaborStat[] = useModuleStatsStore((s) => s.byProject[projectId] ?? []);
  const refreshModuleStats = useModuleStatsStore((s) => s.refresh);

  useEffect(() => { loadFor(projectId, null); }, [projectId, loadFor]);
  useEffect(() => {
    if (!modulesLoadedFor[projectId]) loadModules(projectId);
  }, [projectId, modulesLoadedFor, loadModules]);
  useEffect(() => {
    if (membersLoadedFor !== companyId) loadMembers(companyId);
  }, [companyId, membersLoadedFor, loadMembers]);
  useEffect(() => { refreshModuleStats(projectId); }, [projectId, refreshModuleStats]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="w-40"><SelectValue placeholder={t("task.filterByStatus")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__active">{t("task.activeStatuses")}</SelectItem>
              <SelectItem value="todo">{t("taskStatus.todo")}</SelectItem>
              <SelectItem value="in_progress">{t("taskStatus.in_progress")}</SelectItem>
              <SelectItem value="done">{t("taskStatus.done")}</SelectItem>
              <SelectItem value="closed">{t("taskStatus.closed")}</SelectItem>
              <SelectItem value="__all">{t("task.allStatuses")}</SelectItem>
            </SelectContent>
          </Select>
          <Select value={moduleFilter} onValueChange={setModuleFilter}>
            <SelectTrigger className="w-40"><SelectValue placeholder={t("module.filterByModule")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("module.allModules")}</SelectItem>
              <SelectItem value="__unassigned">{t("module.unassigned")}</SelectItem>
              {modules.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="outline" onClick={() => setOpenManageModules(true)}>
            {t("module.manage")}
          </Button>
          <Button variant="outline" onClick={() => setOpenZentaoImport(true)}>
            {t("zentaoImport.title")}
          </Button>
        </div>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("task.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("task.create")}</DialogTitle></DialogHeader>
            <TaskForm
              members={members}
              modules={modules}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {moduleStats.length > 0 && (
        <Card>
          <button
            type="button"
            className="flex w-full items-center justify-between px-4 py-3 text-left"
            onClick={() => setModuleStatsOpen((o) => !o)}
          >
            <div className="flex items-center gap-1.5">
              {moduleStatsOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <span className="text-sm font-semibold">{t("financial.laborByModule")}</span>
            </div>
            <span className="text-sm text-muted-foreground">
              {t("financial.laborByModuleSummary", {
                hours: moduleStats.reduce((sum, s) => sum + s.hours, 0),
                cost: formatCNY(moduleStats.reduce((sum, s) => sum + s.cost_cents, 0)),
              })}
            </span>
          </button>
          {moduleStatsOpen && (
            <CardContent className="p-0">
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("module.title")}</TableHead>
                    <TableHead className="text-right w-24">{t("timelog.hours")}</TableHead>
                    <TableHead className="text-right w-32">人力成本</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {moduleStats.map((s) => (
                    <TableRow key={s.module_id ?? "unassigned"}>
                      <TableCell>{s.module_name ?? t("module.unassigned")}</TableCell>
                      <TableCell className="text-right">{s.hours}</TableCell>
                      <TableCell className="text-right">{formatCNY(s.cost_cents)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          )}
        </Card>
      )}

      {visibleTasks.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">
          {tasks.length === 0 ? t("task.empty") : t("task.emptyFiltered")}
        </CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-24 whitespace-nowrap">{t("task.status")}</TableHead>
                  <TableHead>{t("task.name")}</TableHead>
                  <TableHead className="w-24">{t("task.assignee")}</TableHead>
                  <TableHead className="text-right w-16">预估</TableHead>
                  <TableHead className="text-right w-16">实际</TableHead>
                  <TableHead className="w-28 whitespace-nowrap">{t("task.startedAt")}</TableHead>
                  <TableHead className="w-28 whitespace-nowrap">{t("task.completedAt")}</TableHead>
                  <TableHead className="w-40 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagedTasks.map((tk) => {
                  const assignee = members.find((m) => m.id === tk.assignee_id);
                  return (
                    <TableRow key={tk.id}>
                      <TableCell>
                        <Badge
                          variant="secondary"
                          className={`whitespace-nowrap ${TASK_STATUS_BADGE_CLASS[tk.status] ?? ""}`}
                        >{t(`taskStatus.${tk.status}`)}</Badge>
                      </TableCell>
                      <TableCell className="font-medium">
                        <button
                          className="text-left hover:underline cursor-pointer"
                          onClick={() => setEditing(tk)}
                        >{tk.title}</button>
                      </TableCell>
                      <TableCell className="text-muted-foreground">{assignee?.name ?? "—"}</TableCell>
                      <TableCell className="text-right">{tk.estimated_hours != null ? `${tk.estimated_hours}h` : "—"}</TableCell>
                      <TableCell className="text-right">{tk.actual_hours > 0 ? `${tk.actual_hours}h` : "—"}</TableCell>
                      <TableCell className="text-muted-foreground whitespace-nowrap">{tk.started_at ?? "—"}</TableCell>
                      <TableCell className="text-muted-foreground whitespace-nowrap">{tk.completed_at ?? "—"}</TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        {tk.status === "todo" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title="开始"
                            onClick={() => setStartingTask(tk)}
                          ><Play className="h-4 w-4" /></Button>
                        )}
                        {tk.status !== "done" && tk.status !== "closed" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title="完成"
                            onClick={() => setCompletingTask(tk)}
                          ><CheckCircle className="h-4 w-4" /></Button>
                        )}
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-7 px-2"
                          title={t("timelog.title")}
                          onClick={() => setOpenLogs(tk)}
                        ><Clock className="h-4 w-4" /></Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
            {visibleTasks.length > PAGE_SIZE && (
              <div className="flex items-center justify-between border-t px-3 py-2 text-sm text-muted-foreground">
                <div>{t("pagination.info", { total: visibleTasks.length, page: currentPage, pages: totalPages })}</div>
                <div className="flex items-center gap-1">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={currentPage <= 1}
                    onClick={() => setPage((p) => Math.max(1, p - 1))}
                  >{t("pagination.prev")}</Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={currentPage >= totalPages}
                    onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                  >{t("pagination.next")}</Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("task.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <TaskForm
              members={members}
              modules={modules}
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onClose={async () => {
                try { await setStatus(editing.id, "closed", projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onDelete={async () => {
                if (!confirm(t("task.deleteConfirm", { title: editing.title }))) return;
                try { await softDelete(editing.id, projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!openLogs} onOpenChange={(o) => !o && setOpenLogs(null)}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{openLogs?.title ? `${openLogs.title} - ${t("timelog.title")}` : t("timelog.title")}</DialogTitle>
          </DialogHeader>
          {openLogs && <TimeLogsSection task={openLogs} members={members} />}
        </DialogContent>
      </Dialog>

      <Dialog open={openManageModules} onOpenChange={setOpenManageModules}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("module.manage")}</DialogTitle>
          </DialogHeader>
          <ManageModulesForm
            projectId={projectId}
            modules={modules}
            onClose={() => setOpenManageModules(false)}
            createModule={createModule}
            updateModule={updateModule}
            moveModuleUp={moveModuleUp}
            moveModuleDown={moveModuleDown}
            softDeleteModule={softDeleteModule}
          />
        </DialogContent>
      </Dialog>

      <ZentaoImportDialog
        projectId={projectId}
        companyId={companyId}
        open={openZentaoImport}
        onOpenChange={setOpenZentaoImport}
      />

      <Dialog open={!!startingTask} onOpenChange={(o) => !o && setStartingTask(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>开始任务</DialogTitle></DialogHeader>
          {startingTask && (
            <StatusTransitionDialog
              task={startingTask}
              label="开始时间"
              fieldKey="started_at"
              onSubmit={async (input) => {
                try { await update(startingTask.id, { ...input, status: "in_progress" } as TaskInput, projectId); setStartingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
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
              existingHours={(tasks.find((t) => t.id === completingTask.id))?.actual_hours ?? 0}
              onSubmit={async (input) => {
                try {
                  await update(completingTask.id, { ...input, status: "done" } as TaskInput, projectId);
                  const h = (input as Record<string, unknown>).hours;
                  if (typeof h === "number" && h > 0 && completingTask.assignee_id != null) {
                    await createTimelog({
                      task_id: completingTask.id,
                      member_id: completingTask.assignee_id,
                      work_date: new Date().toISOString().slice(0, 10),
                      hours: h,
                    }, projectId);
                  }
                  setCompletingTask(null);
                }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setCompletingTask(null)}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function StatusTransitionDialog({ task, label, fieldKey, existingHours, onSubmit, onCancel }: {
  task: Task;
  label: string;
  fieldKey: "started_at" | "completed_at";
  existingHours?: number;
  onSubmit: (input: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
}) {
  const [datetime, setDatetime] = useState(
    task[fieldKey] ? task[fieldKey]!.replace(" ", "T").slice(0, 16) : nowDatetimeLocal()
  );
  const [startedAt, setStartedAt] = useState(
    task.started_at ? task.started_at.replace(" ", "T").slice(0, 16) : ""
  );
  const [description, setDescription] = useState(task.description ?? "");
  const [hours, setHours] = useState(0);
  const [busy, setBusy] = useState(false);
  const showHours = fieldKey === "completed_at";

  const handleSubmit = async () => {
    setBusy(true);
    try {
      const stored = datetime ? datetime.replace("T", " ") : null;
      const storedStartedAt = showHours && startedAt
        ? startedAt.replace("T", " ")
        : null;
      const payload: Record<string, unknown> = {
        title: task.title,
        description: description.trim() || null,
        assignee_id: task.assignee_id,
        estimated_hours: task.estimated_hours,
        due_date: task.due_date,
        started_at: task.started_at,
        completed_at: task.completed_at,
        module_id: task.module_id,
        external_ref: task.external_ref,
      };
      payload[fieldKey] = stored;
      if (showHours) {
        if (storedStartedAt) payload.started_at = storedStartedAt;
        if (hours > 0) payload.hours = hours;
      }
      await onSubmit(payload);
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="text-sm text-muted-foreground">{task.title}</div>
      {showHours && (
        <div className="space-y-1">
          <Label>开始时间</Label>
          <Input
            type="datetime-local"
            value={startedAt}
            onChange={(e) => setStartedAt(e.target.value)}
          />
        </div>
      )}
      <div className="space-y-1">
        <Label>{label}</Label>
        <Input
          type="datetime-local"
          value={datetime}
          onChange={(e) => setDatetime(e.target.value)}
        />
      </div>
      {showHours && (
        <div className="space-y-1">
          <Label>本次工时 (h)<span className="text-muted-foreground font-normal"> — 已有工时 {(existingHours ?? 0)}h</span></Label>
          <Input
            autoFocus
            type="number"
            inputMode="decimal"
            min="0"
            max="24"
            step="0.25"
            value={hours === 0 ? "" : String(hours)}
            placeholder="0"
            onChange={(e) => {
              const n = Number(e.target.value);
              if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
            }}
          />
        </div>
      )}
      <div className="space-y-1">
        <Label>描述</Label>
        <Textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>取消</Button>
        <Button onClick={handleSubmit} disabled={busy}>确定</Button>
      </DialogFooter>
    </div>
  );
}

function toDatetimeLocal(v: string | null | undefined): string {
  if (!v) return "";
  if (v.includes(" ")) return v.replace(" ", "T").slice(0, 16);
  if (v.length === 10) return v + "T00:00";
  return v.slice(0, 16);
}

function fromDatetimeLocal(v: string): string | null {
  if (!v) return null;
  return v.replace("T", " ");
}

function TaskForm({ members, modules, initial, onSubmit, onCancel, onClose, onDelete }: {
  members: Member[];
  modules: Module[];
  initial?: Task;
  onSubmit: (input: TaskInput) => Promise<void>;
  onCancel: () => void;
  onClose?: () => Promise<void>;
  onDelete?: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [title, setTitle] = useState(initial?.title ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [assigneeId, setAssigneeId] = useState<string>(
    initial?.assignee_id ? String(initial.assignee_id) : "__none"
  );
  const [status] = useState(initial?.status ?? "todo");
  const [estHours, setEstHours] = useState(
    initial?.estimated_hours != null ? String(initial.estimated_hours) : ""
  );
  const [dueDate, setDueDate] = useState(initial?.due_date ?? "");
  const [startedAt, setStartedAt] = useState(toDatetimeLocal(initial?.started_at));
  const [completedAt, setCompletedAt] = useState(toDatetimeLocal(initial?.completed_at));
  const [moduleId, setModuleId] = useState<string>(
    initial?.module_id ? String(initial.module_id) : "__none"
  );
  const [busy, setBusy] = useState(false);

  const currentAssignee = initial?.assignee_id
    ? members.find((m) => m.id === initial.assignee_id)
    : null;
  const active = members.filter((m) => m.is_active);
  // Include the current assignee even if archived, so the Select value has a matching item.
  const options = currentAssignee && !currentAssignee.is_active
    ? [currentAssignee, ...active]
    : active;

  const submit = async () => {
    if (!title.trim()) return toast.error(t("task.titleRequired"));
    setBusy(true);
    try {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || null,
        assignee_id: assigneeId === "__none" ? null : Number(assigneeId),
        status,
        estimated_hours: estHours === "" ? null : Number(estHours),
        due_date: dueDate || null,
        started_at: fromDatetimeLocal(startedAt),
        completed_at: fromDatetimeLocal(completedAt),
        module_id: moduleId === "__none" ? null : Number(moduleId),
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("task.name")}</Label>
        <Input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.assignee")}</Label>
          <Select value={assigneeId} onValueChange={setAssigneeId}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__none">{t("task.unassigned")}</SelectItem>
              {options.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>
                  {m.is_active ? m.name : `${m.name}（已归档）`}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("task.status")}</Label>
          <Input value={t(`taskStatus.${status}`)} disabled />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.estimatedHours")}</Label>
          <Input type="number" min="0" step="0.5" value={estHours} onChange={(e) => setEstHours(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("task.dueDate")}</Label>
          <Input type="date" value={dueDate ?? ""} onChange={(e) => setDueDate(e.target.value)} />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.startedAt")}</Label>
          <Input type="datetime-local" value={startedAt} onChange={(e) => setStartedAt(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("task.completedAt")}</Label>
          <Input type="datetime-local" value={completedAt} onChange={(e) => setCompletedAt(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("task.module")}</Label>
        <Select value={moduleId} onValueChange={setModuleId}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="__none">{t("module.unassigned")}</SelectItem>
            {modules.map((m) => (
              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-1">
        <Label>{t("task.description")}</Label>
        <Textarea value={description ?? ""} onChange={(e) => setDescription(e.target.value)} />
      </div>
      <DialogFooter>
        {initial && onClose && initial.status === "done" && (
          <Button
            variant="outline"
            onClick={async () => { setBusy(true); try { await onClose(); } finally { setBusy(false); } }}
            disabled={busy}
          >关闭任务</Button>
        )}
        {initial && onDelete && (
          <Button
            variant="destructive"
            onClick={async () => { setBusy(true); try { await onDelete(); } finally { setBusy(false); } }}
            disabled={busy}
          >删除</Button>
        )}
        <div className="flex-1" />
        <Button variant="outline" onClick={onCancel} disabled={busy}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("task.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function TimeLogsSection({ task, members }: { task: Task; members: Member[] }) {
  const { t } = useTranslation();
  const { byTask, loadFor, create, update, softDelete } = useTimelogsStore();
  const logs = byTask[task.id] ?? [];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<TimeLog | null>(null);

  useEffect(() => { loadFor(task.id); }, [task.id, loadFor]);

  const activeMembers = members.filter((m) => m.is_active);
  const findMemberName = (mid: number) =>
    members.find((m) => m.id === mid)?.name ?? `#${mid}`;

  return (
    <div className="space-y-3">
      <div className="text-xs text-muted-foreground">{t("timelog.snapshotHint")}</div>
      <div className="flex justify-end">
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button size="sm">{t("timelog.add")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("timelog.add")}</DialogTitle></DialogHeader>
            <TimeLogForm
              taskId={task.id}
              members={activeMembers}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(input, task.project_id); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {logs.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground text-center">{t("timelog.empty")}</div>
      ) : (
        <Table compact>
          <TableHeader>
            <TableRow>
              <TableHead className="w-28">{t("timelog.workDate")}</TableHead>
              <TableHead className="w-28">{t("timelog.member")}</TableHead>
              <TableHead className="text-right w-20">{t("timelog.hours")}</TableHead>
              <TableHead className="text-right w-28">人力成本</TableHead>
              <TableHead>{t("timelog.notes")}</TableHead>
              <TableHead className="w-28 text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {logs.map((l) => {
              // hours / 8 * snapshot cost, rounded to nearest cent
              const cost = Math.round((l.hours / 8) * l.daily_cost_snapshot_cents);
              return (
                <TableRow key={l.id}>
                  <TableCell>{l.work_date}</TableCell>
                  <TableCell>{findMemberName(l.member_id)}</TableCell>
                  <TableCell className="text-right">{l.hours}</TableCell>
                  <TableCell className="text-right">{formatCNY(cost)}</TableCell>
                  <TableCell className="text-muted-foreground">{l.notes ?? ""}</TableCell>
                  <TableCell className="text-right">
                    <Button size="sm" variant="ghost" className="h-7 px-2" onClick={() => setEditing(l)}>{t("timelog.edit")}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 px-2"
                      onClick={async () => {
                        if (!confirm(t("timelog.deleteConfirm"))) return;
                        try { await softDelete(l.id, task.id, task.project_id); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >{t("timelog.delete")}</Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("timelog.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <TimeLogEditForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, task.id, task.project_id); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function TimeLogForm({ taskId, members, onSubmit, onCancel }: {
  taskId: number;
  members: Member[];
  onSubmit: (input: TimeLogInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [memberId, setMemberId] = useState(members[0]?.id ?? 0);
  const [date, setDate] = useState(todayIso());
  const [hours, setHours] = useState(8);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!memberId) return toast.error(t("timelog.memberRequired"));
    if (!date) return toast.error(t("timelog.dateRequired"));
    if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
    setBusy(true);
    try {
      await onSubmit({
        task_id: taskId,
        member_id: memberId,
        work_date: date,
        hours,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("timelog.member")}</Label>
          <Select value={String(memberId)} onValueChange={(v) => setMemberId(Number(v))}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {members.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("timelog.workDate")}</Label>
          <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.hours")}</Label>
        <Input
          type="number"
          inputMode="decimal"
          min="0"
          max="24"
          step="0.25"
          value={hours}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
          }}
        />
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function TimeLogEditForm({ initial, onSubmit, onCancel }: {
  initial: TimeLog;
  onSubmit: (input: TimeLogUpdateInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [date, setDate] = useState(initial.work_date);
  const [hours, setHours] = useState(initial.hours);
  const [notes, setNotes] = useState(initial.notes ?? "");
  const [busy, setBusy] = useState(false);

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("timelog.workDate")}</Label>
        <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.hours")}</Label>
        <Input
          type="number"
          inputMode="decimal"
          min="0"
          max="24"
          step="0.25"
          value={hours}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
          }}
        />
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!date) return toast.error(t("timelog.dateRequired"));
            if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
            setBusy(true);
            try { await onSubmit({ work_date: date, hours, notes: notes.trim() || null }); }
            finally { setBusy(false); }
          }}
        >{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function ManageModulesForm({
  projectId,
  modules,
  onClose,
  createModule,
  updateModule,
  moveModuleUp,
  moveModuleDown,
  softDeleteModule,
}: {
  projectId: number;
  modules: Module[];
  onClose: () => void;
  createModule: (projectId: number, input: { name: string }) => Promise<Module>;
  updateModule: (id: number, input: { name: string }, projectId: number) => Promise<Module>;
  moveModuleUp: (id: number, projectId: number) => Promise<void>;
  moveModuleDown: (id: number, projectId: number) => Promise<void>;
  softDeleteModule: (id: number, projectId: number) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState("");

  return (
    <div className="space-y-3">
      <div className="space-y-2 max-h-64 overflow-y-auto">
        {modules.length === 0 ? (
          <div className="text-sm text-muted-foreground">{t("task.empty")}</div>
        ) : (
          modules.map((m, idx) => (
            <div key={m.id} className="flex items-center gap-2">
              {editingId === m.id ? (
                <>
                  <Input
                    value={editingName}
                    onChange={(e) => setEditingName(e.target.value)}
                    className="flex-1"
                  />
                  <Button
                    size="sm"
                    onClick={async () => {
                      if (!editingName.trim()) return toast.error(t("module.nameRequired"));
                      try {
                        await updateModule(m.id, { name: editingName.trim() }, projectId);
                        setEditingId(null);
                      } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("common.save")}</Button>
                  <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>
                    {t("common.cancel")}
                  </Button>
                </>
              ) : (
                <>
                  <div className="flex-1">{m.name}</div>
                  <Button size="sm" variant="ghost" disabled={idx === 0}
                    onClick={async () => {
                      try { await moveModuleUp(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveUp")}</Button>
                  <Button size="sm" variant="ghost" disabled={idx === modules.length - 1}
                    onClick={async () => {
                      try { await moveModuleDown(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveDown")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={() => { setEditingId(m.id); setEditingName(m.name); }}
                  >{t("module.rename")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={async () => {
                      if (!confirm(t("module.deleteConfirm", { name: m.name }))) return;
                      try { await softDeleteModule(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.delete")}</Button>
                </>
              )}
            </div>
          ))
        )}
      </div>
      <div className="flex items-center gap-2">
        <Input
          placeholder={t("module.new")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <Button
          onClick={async () => {
            if (!newName.trim()) return toast.error(t("module.nameRequired"));
            try { await createModule(projectId, { name: newName.trim() }); setNewName(""); }
            catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
          }}
        >{t("module.new")}</Button>
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>{t("common.close")}</Button>
      </DialogFooter>
    </div>
  );
}
