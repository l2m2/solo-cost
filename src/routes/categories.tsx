import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { useCompanyStore } from "@/stores/company";
import { useCategoriesStore } from "@/stores/categories";
import type { CostCategory } from "@/types";

export default function CategoriesPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, loadFor, create, update, remove } =
    useCategoriesStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<CostCategory | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("category.title")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("category.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("category.create")}</DialogTitle></DialogHeader>
            <NameForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (name) => {
                try {
                  await create(currentId, name);
                  setOpenNew(false);
                } catch (e: any) {
                  toast.error(t("common.error", { msg: String(e) }));
                }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("category.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-2">
          {list.map((c) => (
            <Card key={c.id}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 py-3">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <span>{c.name}</span>
                  <Badge variant={c.is_system ? "secondary" : "outline"}>
                    {c.is_system ? t("category.preset") : t("category.custom")}
                  </Badge>
                </CardTitle>
                <div className="flex gap-2">
                  {!c.is_system && (
                    <>
                      <Button size="sm" variant="ghost" onClick={() => setEditing(c)}>
                        {t("category.edit")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={async () => {
                          try { await remove(c.id); }
                          catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("category.delete")}
                      </Button>
                    </>
                  )}
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("category.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <NameForm
              initial={editing.name}
              onCancel={() => setEditing(null)}
              onSubmit={async (name) => {
                try {
                  await update(editing.id, name);
                  setEditing(null);
                } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function NameForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  onSubmit: (name: string) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial ?? "");
  const [busy, setBusy] = useState(false);
  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("category.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!name.trim()) return;
            setBusy(true);
            try { await onSubmit(name.trim()); } finally { setBusy(false); }
          }}
        >
          {t("common.confirm")}
        </Button>
      </DialogFooter>
    </div>
  );
}
