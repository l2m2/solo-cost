import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { useCompanyStore } from "@/stores/company";
import { useTrashStore } from "@/stores/trash";

const TYPE_LABEL: Record<string, string> = {
  project: "项目",
  cost_entry: "成本",
  task: "任务",
  contract_payment: "收款",
  time_log: "工时",
};

export default function TrashPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { items, loadedForCompany, loadFor, restore, purge } = useTrashStore();

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("trash.title")}</h1>
      {items.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("trash.empty")}</CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-20">{t("trash.type")}</TableHead>
                  <TableHead>{t("trash.name")}</TableHead>
                  <TableHead className="w-44">{t("trash.deletedAt")}</TableHead>
                  <TableHead className="w-44 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.map((it) => (
                  <TableRow key={`${it.entity_type}-${it.id}`}>
                    <TableCell>
                      <Badge variant="outline">{TYPE_LABEL[it.entity_type] ?? it.entity_type}</Badge>
                    </TableCell>
                    <TableCell>{it.name}</TableCell>
                    <TableCell className="text-muted-foreground">{it.deleted_at}</TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        onClick={async () => {
                          try { await restore(it.entity_type, it.id, currentId); }
                          catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("trash.restore")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2"
                        onClick={async () => {
                          if (!confirm(t("trash.purgeConfirm"))) return;
                          try { await purge(it.entity_type, it.id, currentId); }
                          catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("trash.purge")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
