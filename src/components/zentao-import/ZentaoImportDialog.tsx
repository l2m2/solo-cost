import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { call } from "@/lib/ipc";
import { useMembersStore } from "@/stores/members";
import { useModulesStore } from "@/stores/modules";
import { useModuleStatsStore } from "@/stores/moduleStats";
import { useTasksStore } from "@/stores/tasks";
import { useFinancialStore } from "@/stores/financial";
import type {
  ImportPreview, ImportReport, MemberChoice, ModuleChoice,
} from "@/types";

type Step = 1 | 2 | 3 | 4 | 5;

export default function ZentaoImportDialog({
  projectId,
  companyId,
  open,
  onOpenChange,
}: {
  projectId: number;
  companyId: number;
  open: boolean;
  onOpenChange: (v: boolean) => void;
}) {
  const { t } = useTranslation();
  const { list: members, loadedForCompany: membersLoadedFor, loadFor: loadMembers } = useMembersStore();
  const { byProject: modulesByProject, loadedForProject: modulesLoadedFor, loadFor: loadModules } = useModulesStore();
  const modules = modulesByProject[projectId] ?? [];

  const [step, setStep] = useState<Step>(1);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [memberMapping, setMemberMapping] = useState<Record<string, MemberChoice>>({});
  const [moduleMapping, setModuleMapping] = useState<Record<string, ModuleChoice>>({});
  const [report, setReport] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);

  // Reset on open
  useEffect(() => {
    if (open) {
      setStep(1);
      setFilePath(null);
      setPreview(null);
      setMemberMapping({});
      setModuleMapping({});
      setReport(null);
      setBusy(false);
    }
  }, [open]);

  // Ensure members + modules are loaded for the mapping steps
  useEffect(() => {
    if (open && membersLoadedFor !== companyId) loadMembers(companyId);
  }, [open, companyId, membersLoadedFor, loadMembers]);
  useEffect(() => {
    if (open && !modulesLoadedFor[projectId]) loadModules(projectId);
  }, [open, projectId, modulesLoadedFor, loadModules]);

  const activeMembers = members.filter((m) => m.is_active);

  const goPreview = async () => {
    try {
      const p = await openFilePicker({
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (typeof p !== "string") return; // user cancelled
      setFilePath(p);
      setBusy(true);
      const res = await call<ImportPreview>("preview_zentao_csv", {
        projectId, filePath: p,
      });
      setPreview(res);
      // Pre-fill mappings with defaults
      const mm: Record<string, MemberChoice> = {};
      for (const name of res.member_names) {
        const match = activeMembers.find((m) => m.name === name);
        mm[name] = match
          ? { kind: "use_member", member_id: match.id }
          : { kind: "unassigned" };
      }
      const modmap: Record<string, ModuleChoice> = {};
      for (const name of res.module_names) {
        const match = modules.find((m) => m.name === name);
        modmap[name] = match
          ? { kind: "use_module", module_id: match.id }
          : { kind: "create_with_name", name };
      }
      setMemberMapping(mm);
      setModuleMapping(modmap);
      // Skip empty mapping steps
      if (res.member_names.length === 0 && res.module_names.length === 0) setStep(4);
      else if (res.member_names.length === 0) setStep(3);
      else setStep(2);
    } catch (e: unknown) {
      toast.error(t("zentaoImport.error.parseFailed", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const execute = async () => {
    if (!filePath) return;
    setBusy(true);
    try {
      const res = await call<ImportReport>("execute_zentao_import", {
        projectId,
        filePath,
        memberMapping,
        moduleMapping,
      });
      setReport(res);
      setStep(5);
    } catch (e: unknown) {
      toast.error(t("zentaoImport.error.executeFailed", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const finish = async () => {
    // Refresh downstream views
    await useTasksStore.getState().loadFor(projectId, null);
    await useModulesStore.getState().loadFor(projectId);
    await useModuleStatsStore.getState().refresh(projectId);
    await useFinancialStore.getState().refresh(projectId);
    onOpenChange(false);
  };

  const willImportCount = (() => {
    if (!preview) return 0;
    const memberSkipped = preview.member_names.filter(
      (n) => memberMapping[n]?.kind === "skip_row",
    ).length;
    // approximate: preview rows minus cancelled minus already_imported minus rows with SkipRow assignee
    // Note: this is a coarse estimate; report will be authoritative.
    return preview.total_rows
      - preview.pre_skip.cancelled
      - preview.pre_skip.already_imported
      - memberSkipped;
  })();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>{t("zentaoImport.title")}</DialogTitle>
        </DialogHeader>
        <div className="text-xs text-muted-foreground">
          {t("zentaoImport.step.file")} → {t("zentaoImport.step.members")} → {t("zentaoImport.step.modules")} → {t("zentaoImport.step.confirm")} → {t("zentaoImport.step.report")}
        </div>

        {step === 1 && (
          <div className="space-y-3">
            <Button onClick={goPreview} disabled={busy}>{t("zentaoImport.chooseFile")}</Button>
            {preview && filePath && (
              <div className="text-sm text-muted-foreground">
                {filePath}<br />
                {t("zentaoImport.preview.summary", {
                  total: preview.total_rows,
                  already: preview.pre_skip.already_imported,
                  cancelled: preview.pre_skip.cancelled,
                })}
              </div>
            )}
          </div>
        )}

        {step === 2 && preview && (
          <div className="space-y-3">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("zentaoImport.member.column")}</TableHead>
                  <TableHead>{t("zentaoImport.member.mapTo")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.member_names.map((name) => {
                  const cur = memberMapping[name];
                  const value =
                    cur?.kind === "use_member" ? String(cur.member_id)
                    : cur?.kind === "skip_row" ? "__skip"
                    : "__unassigned";
                  return (
                    <TableRow key={name}>
                      <TableCell>{name}</TableCell>
                      <TableCell>
                        <Select
                          value={value}
                          onValueChange={(v) => {
                            let choice: MemberChoice;
                            if (v === "__unassigned") choice = { kind: "unassigned" };
                            else if (v === "__skip") choice = { kind: "skip_row" };
                            else choice = { kind: "use_member", member_id: Number(v) };
                            setMemberMapping({ ...memberMapping, [name]: choice });
                          }}
                        >
                          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="__unassigned">{t("zentaoImport.member.unassigned")}</SelectItem>
                            <SelectItem value="__skip">{t("zentaoImport.member.skipRow")}</SelectItem>
                            {activeMembers.map((m) => (
                              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}

        {step === 3 && preview && (
          <div className="space-y-3">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("zentaoImport.module.column")}</TableHead>
                  <TableHead>{t("zentaoImport.module.mapTo")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.module_names.map((name) => {
                  const cur = moduleMapping[name];
                  const value =
                    cur?.kind === "use_module" ? String(cur.module_id)
                    : cur?.kind === "create_with_name" ? "__create"
                    : "__unassigned";
                  return (
                    <TableRow key={name}>
                      <TableCell>{name}</TableCell>
                      <TableCell>
                        <Select
                          value={value}
                          onValueChange={(v) => {
                            let choice: ModuleChoice;
                            if (v === "__unassigned") choice = { kind: "unassigned" };
                            else if (v === "__create") choice = { kind: "create_with_name", name };
                            else choice = { kind: "use_module", module_id: Number(v) };
                            setModuleMapping({ ...moduleMapping, [name]: choice });
                          }}
                        >
                          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="__unassigned">{t("zentaoImport.module.unassigned")}</SelectItem>
                            <SelectItem value="__create">{t("zentaoImport.module.createWith", { name })}</SelectItem>
                            {modules.map((m) => (
                              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}

        {step === 4 && preview && (
          <Card>
            <CardContent className="p-6 space-y-2 text-sm">
              <div>{t("zentaoImport.preview.willImport", { n: willImportCount })}</div>
              <div className="text-muted-foreground">
                {t("zentaoImport.preview.willSkip", {
                  n: preview.pre_skip.cancelled + preview.pre_skip.already_imported
                     + preview.member_names.filter((n) => memberMapping[n]?.kind === "skip_row").length,
                  already: preview.pre_skip.already_imported,
                  cancelled: preview.pre_skip.cancelled,
                  member: preview.member_names.filter((n) => memberMapping[n]?.kind === "skip_row").length,
                })}
              </div>
            </CardContent>
          </Card>
        )}

        {step === 5 && report && (
          <div className="space-y-3">
            <div className="font-medium">{t("zentaoImport.report.title")}</div>
            <div className="text-sm">
              {t("zentaoImport.report.imported", { tasks: report.imported_tasks, logs: report.imported_timelogs })}
            </div>
            <div className="text-sm text-muted-foreground">
              {t("zentaoImport.report.skipped", {
                n: report.skipped.cancelled + report.skipped.already_imported + report.skipped.member_skipped,
                already: report.skipped.already_imported,
                cancelled: report.skipped.cancelled,
                member: report.skipped.member_skipped,
              })}
            </div>
            {report.failed.length > 0 && (
              <div className="rounded border border-destructive/40 p-3 space-y-1">
                <div className="text-sm text-destructive">
                  {t("zentaoImport.report.failedTitle", { n: report.failed.length })}
                </div>
                <div className="max-h-40 overflow-y-auto space-y-1">
                  {report.failed.map((f) => (
                    <div key={`${f.row_no}-${f.zentao_id}`} className="text-xs">
                      {t("zentaoImport.report.failedItem", { row: f.row_no, ref: f.zentao_id, err: f.error })}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        <DialogFooter>
          {step > 1 && step < 5 && (
            <Button variant="outline" onClick={() => setStep((s) => (s - 1) as Step)} disabled={busy}>
              {t("common.back")}
            </Button>
          )}
          {step === 1 && preview && (
            <Button
              onClick={() => {
                if (preview.member_names.length === 0 && preview.module_names.length === 0) setStep(4);
                else if (preview.member_names.length === 0) setStep(3);
                else setStep(2);
              }}
              disabled={busy}
            >
              {t("common.next")}
            </Button>
          )}
          {step === 2 && (
            <Button onClick={() => setStep(preview!.module_names.length === 0 ? 4 : 3)} disabled={busy}>
              {t("common.next")}
            </Button>
          )}
          {step === 3 && (
            <Button onClick={() => setStep(4)} disabled={busy}>
              {t("common.next")}
            </Button>
          )}
          {step === 4 && (
            <Button onClick={execute} disabled={busy}>
              {busy ? t("zentaoImport.action.importing") : t("zentaoImport.action.start")}
            </Button>
          )}
          {step === 5 && (
            <Button onClick={finish}>{t("common.done")}</Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
