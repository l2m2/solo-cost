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
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { STATUS_OPTIONS, statusBadgeClass, statusLabel } from "@/lib/status";
import { useCompanyStore } from "@/stores/company";
import { useProjectsStore } from "@/stores/projects";
import type { Project, ProjectInput } from "@/types";

export default function ProjectsListPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, statusFilter, loadFor, create, update, softDelete } =
    useProjectsStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Project | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) {
      loadFor(currentId, null);
    }
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("project.title")}</h1>
        <div className="flex items-center gap-2">
          <Select
            value={statusFilter ?? "__all"}
            onValueChange={(v) => loadFor(currentId, v === "__all" ? null : v)}
          >
            <SelectTrigger className="w-40"><SelectValue placeholder={t("project.filterByStatus")} /></SelectTrigger>
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

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("project.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {list.map((p) => (
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
  const [name, setName] = useState(initial?.name ?? "");
  const [client, setClient] = useState(initial?.client_name ?? "");
  const [status, setStatus] = useState(initial?.status ?? "pending");
  const [amount, setAmount] = useState(initial?.contract_amount_cents ?? 0);
  const [inclusive, setInclusive] = useState(initial?.contract_amount_is_tax_inclusive ?? true);
  const [taxRate, setTaxRate] = useState(String(initial?.tax_rate ?? 0.06));
  const [startDate, setStartDate] = useState(initial?.start_date ?? "");
  const [endDate, setEndDate] = useState(initial?.end_date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("project.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        client_name: client.trim() || null,
        status,
        contract_amount_cents: amount,
        contract_amount_is_tax_inclusive: inclusive,
        tax_rate: Number(taxRate),
        start_date: startDate || null,
        end_date: endDate || null,
        notes: notes.trim() || null,
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
          <Input value={client} onChange={(e) => setClient(e.target.value)} />
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
