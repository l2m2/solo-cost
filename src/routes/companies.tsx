import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { useCompanyStore } from "@/stores/company";
import type { Company, CompanyInput } from "@/types";

function CompanyForm({ initial, onSubmit, onCancel }: {
  initial?: Company;
  onSubmit: (input: CompanyInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [legalName, setLegalName] = useState(initial?.legal_name ?? "");
  const [taxId, setTaxId] = useState(initial?.tax_id ?? "");
  const [taxRate, setTaxRate] = useState(String(initial?.default_tax_rate ?? 0.06));
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("company.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        legal_name: legalName.trim() || null,
        tax_id: taxId.trim() || null,
        default_tax_rate: Number(taxRate),
        currency_code: "CNY",
        notes: notes.trim() || null,
      });
    } catch (e: any) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("company.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="space-y-1">
        <Label>{t("company.legalName")}</Label>
        <Input value={legalName} onChange={(e) => setLegalName(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.taxId")}</Label>
        <Input value={taxId} onChange={(e) => setTaxId(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.defaultTaxRate")}</Label>
        <Input type="number" step="0.01" min="0" max="0.99" value={taxRate} onChange={(e) => setTaxRate(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("company.notes")}</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("company.save")}</Button>
      </DialogFooter>
    </div>
  );
}

export default function CompaniesPage() {
  const { t } = useTranslation();
  const { list, loaded, loadAll, create, update, setCurrent, currentId } =
    useCompanyStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Company | null>(null);

  useEffect(() => { if (!loaded) loadAll(); }, [loaded, loadAll]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("nav.companies")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("company.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("company.create")}</DialogTitle></DialogHeader>
            <CompanyForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => { await create(input); setOpenNew(false); }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("company.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {list.map((c) => (
            <Card key={c.id} className={c.id === currentId ? "border-primary" : undefined}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0">
                <CardTitle className="text-base">{c.name}</CardTitle>
                <div className="flex gap-2">
                  {c.id !== currentId && (
                    <Button size="sm" variant="outline" onClick={() => setCurrent(c.id)}>切换为当前</Button>
                  )}
                  <Button size="sm" variant="ghost" onClick={() => setEditing(c)}>{t("company.edit")}</Button>
                </div>
              </CardHeader>
              <CardContent className="text-sm text-muted-foreground space-y-1">
                {c.legal_name && <div>工商名：{c.legal_name}</div>}
                {c.tax_id && <div>税号：{c.tax_id}</div>}
                <div>默认税率：{(c.default_tax_rate * 100).toFixed(2)}%</div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("company.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <CompanyForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => { await update(editing.id, input); setEditing(null); }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
