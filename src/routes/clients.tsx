import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Pencil, Trash2 } from "lucide-react";
import { useCompanyStore } from "@/stores/company";
import { useClientsStore } from "@/stores/clients";
import type { Client, ClientInput } from "@/types";

export default function ClientsPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, loadFor, create, update, softDelete } = useClientsStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Client | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("client.title")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild>
            <Button>{t("client.create")}</Button>
          </DialogTrigger>
          <DialogContent className="max-w-lg">
            <DialogHeader>
              <DialogTitle>{t("client.create")}</DialogTitle>
            </DialogHeader>
            <ClientForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try {
                  await create(currentId, input);
                  setOpenNew(false);
                } catch (e: unknown) {
                  toast.error(t("common.error", { msg: String(e) }));
                }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card>
          <CardContent className="p-6 text-sm text-muted-foreground">
            {t("client.empty")}
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("client.name")}</TableHead>
                  <TableHead className="w-40">{t("client.contactName")}</TableHead>
                  <TableHead className="w-48">{t("client.contactInfo")}</TableHead>
                  <TableHead className="w-48">{t("client.legalName")}</TableHead>
                  <TableHead className="w-40">{t("client.taxId")}</TableHead>
                  <TableHead className="w-24 text-right">{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {list.map((c) => (
                  <TableRow key={c.id}>
                    <TableCell className="font-medium">{c.name}</TableCell>
                    <TableCell className="text-muted-foreground">{c.contact_name ?? "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{c.contact_info ?? "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{c.legal_name ?? "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{c.tax_id ?? "—"}</TableCell>
                    <TableCell className="text-right whitespace-nowrap">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        title={t("client.edit")}
                        onClick={() => setEditing(c)}
                      ><Pencil className="h-4 w-4" /></Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        title={t("client.delete")}
                        onClick={async () => {
                          if (!confirm(t("client.deleteConfirm", { name: c.name }))) return;
                          try {
                            await softDelete(c.id);
                          } catch (e: unknown) {
                            toast.error(t("common.error", { msg: String(e) }));
                          }
                        }}
                      ><Trash2 className="h-4 w-4" /></Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("client.edit")}</DialogTitle>
          </DialogHeader>
          {editing && (
            <ClientForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try {
                  await update(editing.id, input);
                  setEditing(null);
                } catch (e: unknown) {
                  toast.error(t("common.error", { msg: String(e) }));
                }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ClientForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: Client;
  onSubmit: (input: ClientInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [contactName, setContactName] = useState(initial?.contact_name ?? "");
  const [contactInfo, setContactInfo] = useState(initial?.contact_info ?? "");
  const [taxId, setTaxId] = useState(initial?.tax_id ?? "");
  const [legalName, setLegalName] = useState(initial?.legal_name ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) {
      toast.error(t("client.nameRequired"));
      return;
    }
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        contact_name: contactName.trim() || null,
        contact_info: contactInfo.trim() || null,
        tax_id: taxId.trim() || null,
        legal_name: legalName.trim() || null,
        notes: notes.trim() || null,
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("client.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("client.contactName")}</Label>
          <Input value={contactName ?? ""} onChange={(e) => setContactName(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("client.contactInfo")}</Label>
          <Input value={contactInfo ?? ""} onChange={(e) => setContactInfo(e.target.value)} />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("client.legalName")}</Label>
          <Input value={legalName ?? ""} onChange={(e) => setLegalName(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("client.taxId")}</Label>
          <Input value={taxId ?? ""} onChange={(e) => setTaxId(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("client.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button onClick={submit} disabled={busy}>
          {t("client.save")}
        </Button>
      </DialogFooter>
    </div>
  );
}
