import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useBackupStore } from "@/stores/backup";
import { useAuthStore } from "@/stores/auth";

export function IntegrityFailedDialog() {
  const { t } = useTranslation();
  const { list, loadList, restoreFromBackup } = useBackupStore();
  const refresh = useAuthStore((s) => s.refresh);
  const [selected, setSelected] = useState<string>("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    loadList();
  }, [loadList]);

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
    setBusy(true);
    try {
      await restoreFromBackup(selected, password);
      toast.success(t("settings.backup.restoreSuccess"));
      // Reset to "locked" so login page renders and pulls fresh stores.
      useAuthStore.setState({ status: "locked" });
      await refresh();
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.backup.corruptedTitle")}</DialogTitle>
          <DialogDescription>{t("settings.backup.corruptedBody")}</DialogDescription>
        </DialogHeader>
        {list.length === 0 ? (
          <div className="text-sm text-muted-foreground">
            {t("settings.backup.corruptedNoBackup")}
          </div>
        ) : (
          <div className="space-y-2 text-sm">
            <div className="text-muted-foreground">系统内已有的自动备份：</div>
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
          <Label>{t("settings.backup.chooseFile")}（可选自定义）</Label>
          <div className="flex gap-2">
            <Input
              readOnly
              value={selected}
              placeholder="…/backups/auto_YYYYMMDD_HHmmss.db"
            />
            <Button variant="outline" onClick={chooseFile}>
              浏览…
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
          <Button onClick={restore} disabled={busy || !selected || !password}>
            {t("settings.backup.restore")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
