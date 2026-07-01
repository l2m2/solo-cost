import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { useCompanyStore } from "@/stores/company";
import { useMembersStore } from "@/stores/members";
import type { Member, MemberInput } from "@/types";

export default function MembersPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, loadFor, create, update, setActive, softDelete } =
    useMembersStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Member | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("member.title")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild>
            <Button>{t("member.create")}</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{t("member.create")}</DialogTitle>
            </DialogHeader>
            <MemberForm
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
            {t("member.empty")}
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-2">
          {list.map((m) => (
            <Card key={m.id} className={m.is_active ? undefined : "opacity-60"}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 py-3">
                <div className="space-y-1">
                  <CardTitle className="text-base flex items-center gap-2">
                    <span>{m.name}</span>
                    <Badge variant={m.is_active ? "secondary" : "outline"}>
                      {m.is_active ? t("member.active") : t("member.inactive")}
                    </Badge>
                  </CardTitle>
                  <div className="text-xs text-muted-foreground">
                    {m.role ?? "—"} · {formatCNY(m.daily_cost_cents)}/天
                    {m.effective_from &&
                      ` · ${t("member.effectiveFrom")} ${m.effective_from}`}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => setEditing(m)}>
                    {t("member.edit")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      try {
                        await setActive(m.id, !m.is_active);
                      } catch (e: unknown) {
                        toast.error(t("common.error", { msg: String(e) }));
                      }
                    }}
                  >
                    {m.is_active ? t("member.archive") : t("member.unarchive")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      if (!confirm(t("member.deleteConfirm", { name: m.name }))) return;
                      try {
                        await softDelete(m.id);
                      } catch (e: unknown) {
                        toast.error(t("common.error", { msg: String(e) }));
                      }
                    }}
                  >
                    {t("member.delete")}
                  </Button>
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("member.edit")}</DialogTitle>
          </DialogHeader>
          {editing && (
            <MemberForm
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

function MemberForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: Member;
  onSubmit: (input: MemberInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [role, setRole] = useState(initial?.role ?? "");
  const [dailyCost, setDailyCost] = useState(initial?.daily_cost_cents ?? 0);
  const [effectiveFrom, setEffectiveFrom] = useState(initial?.effective_from ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) {
      toast.error(t("member.nameRequired"));
      return;
    }
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        role: role.trim() || null,
        daily_cost_cents: dailyCost,
        effective_from: effectiveFrom || null,
        notes: notes.trim() || null,
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("member.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="space-y-1">
        <Label>{t("member.role")}</Label>
        <Input value={role ?? ""} onChange={(e) => setRole(e.target.value)} />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("member.dailyCost")}</Label>
          <MoneyInput value={dailyCost} onChange={setDailyCost} />
        </div>
        <div className="space-y-1">
          <Label>{t("member.effectiveFrom")}</Label>
          <Input
            type="date"
            value={effectiveFrom ?? ""}
            onChange={(e) => setEffectiveFrom(e.target.value)}
          />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("member.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button onClick={submit} disabled={busy}>
          {t("member.save")}
        </Button>
      </DialogFooter>
    </div>
  );
}
