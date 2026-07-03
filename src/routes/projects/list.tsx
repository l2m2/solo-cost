import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { STATUS_OPTIONS, statusBadgeClass, statusLabel } from "@/lib/status";
import { useCompanyStore } from "@/stores/company";
import { useProjectsStore } from "@/stores/projects";
import { useClientsStore } from "@/stores/clients";
import type { Client, Project, ProjectInput } from "@/types";

export default function ProjectsListPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, statusFilter, loadFor, create, update, softDelete } =
    useProjectsStore();
  const {
    loadedForCompany: clientsLoadedFor,
    loadFor: loadClients,
  } = useClientsStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Project | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) {
      loadFor(currentId, null);
    }
  }, [currentId, loadedForCompany, loadFor]);
  useEffect(() => {
    if (currentId != null && clientsLoadedFor !== currentId) {
      loadClients(currentId);
    }
  }, [currentId, clientsLoadedFor, loadClients]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  // "全部" 语义 = 不含已归档；用户想看已归档需显式选择该选项
  const visible = statusFilter == null
    ? list.filter((p) => p.status !== "archived")
    : list;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("project.title")}</h1>
        <div className="flex items-center gap-2">
          <Select
            value={statusFilter ?? "__all"}
            onValueChange={(v) => loadFor(currentId, v === "__all" ? null : v)}
          >
            <SelectTrigger className="w-56"><SelectValue placeholder={t("project.filterByStatus")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("project.allStatuses")}</SelectItem>
              {STATUS_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Dialog open={openNew} onOpenChange={setOpenNew}>
            <DialogTrigger asChild><Button>{t("project.create")}</Button></DialogTrigger>
            <DialogContent className="max-w-lg">
              <DialogHeader><DialogTitle>{t("project.create")}</DialogTitle></DialogHeader>
              <ProjectForm
                onCancel={() => setOpenNew(false)}
                onSubmit={async (input) => {
                  try {
                    await create(currentId, input);
                    setOpenNew(false);
                  } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                }}
              />
            </DialogContent>
          </Dialog>
        </div>
      </div>

      {visible.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("project.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {visible.map((p) => (
            <Card key={p.id}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0">
                <div className="space-y-1">
                  <CardTitle className="text-base flex items-center gap-2">
                    <Link to={`/projects/${p.id}`} className="hover:underline">{p.name}</Link>
                    <span className={`text-xs px-2 py-0.5 rounded ${statusBadgeClass(p.status)}`}>
                      {statusLabel(p.status)}
                    </span>
                  </CardTitle>
                  {p.client_name && (
                    <div className="text-xs text-muted-foreground">{t("project.client")}：{p.client_name}</div>
                  )}
                </div>
                <div className="flex items-center gap-3">
                  <div className="text-right">
                    <div className="text-sm font-medium">{formatCNY(p.contract_amount_cents)}</div>
                    <div className="text-xs text-muted-foreground">
                      {p.contract_amount_is_tax_inclusive ? "含税" : "不含税"} · 税率 {(p.tax_rate * 100).toFixed(2)}%
                    </div>
                  </div>
                  <div className="flex gap-1">
                    <Button asChild size="sm" variant="ghost"><Link to={`/projects/${p.id}`}>{t("project.openDetail")}</Link></Button>
                    <Button size="sm" variant="ghost" onClick={() => setEditing(p)}>编辑</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={async () => {
                        if (!confirm(t("project.deleteConfirm", { name: p.name }))) return;
                        try { await softDelete(p.id); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >
                      {t("project.delete")}
                    </Button>
                  </div>
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader><DialogTitle>{t("project.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <ProjectForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try {
                  await update(editing.id, input);
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

function ProjectForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: Project;
  onSubmit: (input: ProjectInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list: clients, create: createClient } = useClientsStore();
  const [name, setName] = useState(initial?.name ?? "");
  const [clientId, setClientId] = useState<string>(
    initial?.client_id != null ? String(initial.client_id) : ""
  );
  const [quickCreate, setQuickCreate] = useState(false);
  const [quickName, setQuickName] = useState("");
  const [status, setStatus] = useState(initial?.status ?? "pending");
  const [amount, setAmount] = useState(initial?.contract_amount_cents ?? 0);
  const [inclusive, setInclusive] = useState(initial?.contract_amount_is_tax_inclusive ?? true);
  const [taxRate, setTaxRate] = useState(String(initial?.tax_rate ?? 0.06));
  const [startDate, setStartDate] = useState(initial?.start_date ?? "");
  const [endDate, setEndDate] = useState(initial?.end_date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [commissionMode, setCommissionMode] = useState(initial?.commission_mode ?? "none");
  const [commissionRate, setCommissionRate] = useState(
    initial?.commission_rate != null ? String(initial.commission_rate) : ""
  );
  const [commissionAmount, setCommissionAmount] = useState(initial?.commission_amount_cents ?? 0);
  const [commissionSettled, setCommissionSettled] = useState(initial?.commission_settled ?? false);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("project.nameRequired"));
    if (!clientId) return toast.error(t("project.clientRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        client_id: Number(clientId),
        status,
        contract_amount_cents: amount,
        contract_amount_is_tax_inclusive: inclusive,
        tax_rate: Number(taxRate),
        start_date: startDate || null,
        end_date: endDate || null,
        notes: notes.trim() || null,
        commission_mode: commissionMode,
        commission_rate:
          commissionMode === "rate"
            ? (commissionRate === "" ? 0 : Number(commissionRate))
            : null,
        commission_amount_cents:
          commissionMode === "fixed" ? commissionAmount : null,
        commission_settled:
          commissionMode === "fixed" ? commissionSettled : false,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("project.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("project.client")}</Label>
          <div className="flex gap-2">
            <Select value={clientId} onValueChange={setClientId}>
              <SelectTrigger className="flex-1">
                <SelectValue placeholder={t("project.selectClient")} />
              </SelectTrigger>
              <SelectContent>
                {clients.map((c: Client) => (
                  <SelectItem key={c.id} value={String(c.id)}>{c.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => { setQuickName(""); setQuickCreate(true); }}
            >{t("client.create")}</Button>
          </div>
        </div>
        <div className="space-y-1">
          <Label>状态</Label>
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {STATUS_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <Dialog open={quickCreate} onOpenChange={setQuickCreate}>
        <DialogContent className="max-w-sm">
          <DialogHeader><DialogTitle>{t("client.create")}</DialogTitle></DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>{t("client.name")}</Label>
              <Input
                value={quickName}
                onChange={(e) => setQuickName(e.target.value)}
                autoFocus
              />
            </div>
            <p className="text-xs text-muted-foreground">{t("client.quickCreateHint")}</p>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setQuickCreate(false)}>{t("common.cancel")}</Button>
            <Button onClick={async () => {
              if (currentId == null) return;
              if (!quickName.trim()) return toast.error(t("client.nameRequired"));
              try {
                const c = await createClient(currentId, { name: quickName.trim() });
                setClientId(String(c.id));
                setQuickCreate(false);
              } catch (e: unknown) {
                toast.error(t("common.error", { msg: String(e) }));
              }
            }}>{t("client.save")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("project.contractAmount")}</Label>
          <MoneyInput value={amount} onChange={setAmount} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.taxInclusive")}</Label>
          <Select value={inclusive ? "1" : "0"} onValueChange={(v) => setInclusive(v === "1")}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="1">含税</SelectItem>
              <SelectItem value="0">不含税</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1">
          <Label>{t("project.taxRate")}</Label>
          <Input type="number" step="0.01" min="0" max="0.99" value={taxRate} onChange={(e) => setTaxRate(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.startDate")}</Label>
          <Input type="date" value={startDate ?? ""} onChange={(e) => setStartDate(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.endDate")}</Label>
          <Input type="date" value={endDate ?? ""} onChange={(e) => setEndDate(e.target.value)} />
        </div>
      </div>
      <div className="space-y-2 rounded border p-3">
        <div className="text-sm font-medium">{t("project.commissionSection")}</div>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label>{t("project.commissionMode")}</Label>
            <Select value={commissionMode} onValueChange={setCommissionMode}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">{t("commissionMode.none")}</SelectItem>
                <SelectItem value="rate">{t("commissionMode.rate")}</SelectItem>
                <SelectItem value="fixed">{t("commissionMode.fixed")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {commissionMode === "rate" && (
            <div className="space-y-1">
              <Label>{t("project.commissionRate")}</Label>
              <Input
                type="number"
                step="0.01"
                min="0"
                max="1"
                value={commissionRate}
                onChange={(e) => setCommissionRate(e.target.value)}
                placeholder="0.05"
              />
            </div>
          )}
          {commissionMode === "fixed" && (
            <div className="space-y-1">
              <Label>{t("project.commissionAmount")}</Label>
              <MoneyInput value={commissionAmount} onChange={setCommissionAmount} />
            </div>
          )}
        </div>
        {commissionMode === "fixed" && (
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={commissionSettled}
              onCheckedChange={(v) => setCommissionSettled(!!v)}
            />
            {t("project.commissionSettled")}
          </label>
        )}
      </div>
      <div className="space-y-1">
        <Label>{t("project.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("project.save")}</Button>
      </DialogFooter>
    </div>
  );
}
