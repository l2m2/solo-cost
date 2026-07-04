import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { save, open } from "@tauri-apps/plugin-dialog";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useBackupStore } from "@/stores/backup";
import { useAuthStore } from "@/stores/auth";
import CompaniesPage from "@/routes/companies";
import CategoriesPage from "@/routes/categories";

function RestoreDialog({ open: isOpen, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { list, loadList, restoreFromBackup } = useBackupStore();
  const [selected, setSelected] = useState<string>("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (isOpen) {
      loadList();
      setSelected("");
      setPassword("");
    }
  }, [isOpen, loadList]);

  const chooseFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "SQLite db", extensions: ["db"] }],
    });
    if (typeof picked === "string") setSelected(picked);
  };

  const restore = async () => {
    if (!selected) return toast.error(t("settings.backup.chooseFile"));
    if (!password) return toast.error(t("login.password"));
    if (!confirm(t("settings.backup.restoreConfirm"))) return;
    setBusy(true);
    try {
      await restoreFromBackup(selected, password);
      toast.success(t("settings.backup.restoreSuccess"));
      // Reset to "locked" so login page renders and pulls fresh stores.
      useAuthStore.setState({ status: "locked" });
      navigate("/login", { replace: true });
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.backup.restore")}</DialogTitle>
        </DialogHeader>
        {list.length > 0 && (
          <div className="space-y-2 text-sm">
            <div className="text-muted-foreground">{t("settings.backup.availableBackups")}</div>
            {list.map((b) => (
              <div
                key={b.absolute_path}
                className="cursor-pointer hover:bg-accent p-2 rounded flex items-center justify-between"
                onClick={() => setSelected(b.absolute_path)}
              >
                <span className={selected === b.absolute_path ? "font-medium" : undefined}>
                  {b.created_at}
                </span>
                <span className="text-xs text-muted-foreground">
                  {(b.size_bytes / 1024).toFixed(0)} KB
                </span>
              </div>
            ))}
          </div>
        )}
        <div className="space-y-2">
          <Label>{t("settings.backup.chooseFile")}{t("settings.backup.chooseFileOptional")}</Label>
          <div className="flex gap-2">
            <Input
              readOnly
              value={selected}
              placeholder="…/backups/auto_YYYYMMDD_HHmmss.db"
            />
            <Button variant="outline" onClick={chooseFile}>
              {t("settings.backup.browse")}
            </Button>
          </div>
        </div>
        <div className="space-y-2">
          <Label>{t("settings.backup.restorePasswordPrompt")}</Label>
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button onClick={restore} disabled={busy || !selected || !password}>
            {t("settings.backup.restore")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default function SettingsPage() {
  const { t } = useTranslation();
  const { status, list, loadStatus, loadList, createNow, exportPlaintext } =
    useBackupStore();
  const [busy, setBusy] = useState(false);
  const [openRestore, setOpenRestore] = useState(false);

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
            <Button variant="outline" onClick={() => setOpenRestore(true)} disabled={busy}>
              {t("settings.backup.restore")}
            </Button>
          </div>
        </CardContent>
      </Card>
      {list.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("settings.backup.backupHistory")}</CardTitle>
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
      <CompaniesPage />
      <CategoriesPage />
      <RestoreDialog open={openRestore} onClose={() => setOpenRestore(false)} />
    </div>
  );
}
