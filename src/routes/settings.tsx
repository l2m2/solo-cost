import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { save } from "@tauri-apps/plugin-dialog";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useBackupStore } from "@/stores/backup";

export default function SettingsPage() {
  const { t } = useTranslation();
  const { status, list, loadStatus, loadList, createNow, exportPlaintext } =
    useBackupStore();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    loadStatus();
    loadList();
  }, [loadStatus, loadList]);

  const doCreate = async () => {
    setBusy(true);
    try {
      const info = await createNow();
      toast.success(t("settings.backup.success", { path: info.absolute_path }));
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const doExport = async () => {
    if (!confirm(t("settings.backup.exportWarning"))) return;
    const picked = await save({
      defaultPath: "solo-cost-plaintext.db",
      filters: [{ name: "SQLite db", extensions: ["db"] }],
    });
    if (!picked) return;
    setBusy(true);
    try {
      await exportPlaintext(picked);
      toast.success(t("settings.backup.success", { path: picked }));
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("settings.title")}</h1>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">
            {t("settings.backup.sectionTitle")}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="text-sm">
            {t("settings.backup.statusLabel")}：
            <span className="ml-2 font-medium">
              {status?.last_backup_at ?? t("settings.backup.never")}
            </span>
          </div>
          <div className="text-sm text-muted-foreground">
            {t("settings.backup.count", { n: status?.auto_count ?? 0 })}
          </div>
          <div className="flex gap-2 pt-2">
            <Button onClick={doCreate} disabled={busy}>
              {busy ? t("settings.backup.backingUp") : t("settings.backup.runNow")}
            </Button>
            <Button variant="outline" onClick={doExport} disabled={busy}>
              {t("settings.backup.exportPlaintext")}
            </Button>
          </div>
        </CardContent>
      </Card>
      {list.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">备份历史</CardTitle>
          </CardHeader>
          <CardContent className="space-y-1 text-sm">
            {list.map((b) => (
              <div
                key={b.absolute_path}
                className="flex items-center justify-between border-b py-2 last:border-b-0"
              >
                <div>
                  <div>{b.created_at}</div>
                  <div className="text-xs text-muted-foreground">{b.file_name}</div>
                </div>
                <div className="text-xs text-muted-foreground">
                  {(b.size_bytes / 1024).toFixed(0)} KB
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
