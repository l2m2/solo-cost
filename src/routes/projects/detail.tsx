import { useEffect, useState } from "react";
import { useParams, useNavigate, useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { confirmDialog } from "@/lib/confirm";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
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
import { useCompanyStore } from "@/stores/company";
import { useCategoriesStore } from "@/stores/categories";
import { useCostsStore } from "@/stores/costs";
import { usePaymentsStore } from "@/stores/payments";
import { useProjectsStore } from "@/stores/projects";
import { useFinancialStore } from "@/stores/financial";
import { FormDialogContent, useFormDialog } from "@/components/ui/form-dialog";
import TasksPanel from "@/routes/projects/tasks/panel";
import type { CostEntry, CostEntryInput, ContractPayment, PaymentInput, Project, ProjectFinancialSummary } from "@/types";

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

  // A ?task= deep link targets the tasks tab. Radix unmounts inactive tab
  // content, so without this the panel that consumes the param never mounts
  // and the link silently does nothing but open the project.
  const [detailSearchParams] = useSearchParams();
  const [tab, setTab] = useState(() =>
    detailSearchParams.get("task") ? "tasks" : "overview"
  );
  // The initializer covers a fresh mount; this covers arriving at the same
  // project that is already open, where the route does not remount.
  useEffect(() => {
    if (detailSearchParams.get("task")) setTab("tasks");
  }, [detailSearchParams]);

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

      <Tabs value={tab} onValueChange={setTab}>
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
          <FormDialogContent>
            <DialogHeader><DialogTitle>{t("cost.add")}</DialogTitle></DialogHeader>
            <CostForm
              cats={cats}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </FormDialogContent>
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
                          if (!(await confirmDialog(t("cost.deleteConfirm"), { title: t("common.delete"), okLabel: t("common.delete") }))) return;
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
        <FormDialogContent>
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
        </FormDialogContent>
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
  const { markDirty } = useFormDialog();
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
          <Select value={String(categoryId)} onValueChange={(v) => { markDirty(); setCategoryId(Number(v)); }}>
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
          <FormDialogContent>
            <DialogHeader><DialogTitle>{t("payment.create")}</DialogTitle></DialogHeader>
            <PaymentForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </FormDialogContent>
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
                        if (!(await confirmDialog("确认删除该收款节点？", { title: t("common.delete"), okLabel: t("common.delete") }))) return;
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
        <FormDialogContent>
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
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!marking} onOpenChange={(o) => !o && setMarking(null)}>
        <FormDialogContent>
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
        </FormDialogContent>
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

