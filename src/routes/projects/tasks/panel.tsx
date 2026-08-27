import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { confirmDialog } from "@/lib/confirm";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { formatCNY } from "@/lib/money";
import { useTasksStore } from "@/stores/tasks";
import { useTimelogsStore } from "@/stores/timelogs";
import { useMembersStore } from "@/stores/members";
import { useModulesStore } from "@/stores/modules";
import { useModuleStatsStore } from "@/stores/moduleStats";
import { Badge } from "@/components/ui/badge";
import { Play, Pause, PlayCircle, CheckCircle, Archive, Clock, ChevronRight, ChevronDown } from "lucide-react";
import { FormDialogContent, useFormDialog } from "@/components/ui/form-dialog";
import ZentaoImportDialog from "@/components/zentao-import/ZentaoImportDialog";
import { StatusTransitionDialog, TASK_STATUS_BADGE_CLASS } from "@/components/tasks/StatusTransitionDialog";
import type { Member, Module, ModuleLaborStat, Task, TaskInput, TimeLog, TimeLogUpdateInput } from "@/types";

// ─── Tasks + TimeLogs ───────────────────────────────────────────────────────

export default function TasksPanel({ projectId, companyId }: { projectId: number; companyId: number }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { byProject, loadFor, create, update, setStatus } = useTasksStore();
  const { create: createTimelog } = useTimelogsStore();
  const {
    byProject: modulesByProject,
    loadedForProject: modulesLoadedFor,
    loadFor: loadModules,
    create: createModule,
    update: updateModule,
    moveUp: moveModuleUp,
    moveDown: moveModuleDown,
    softDelete: softDeleteModule,
  } = useModulesStore();
  const modules = modulesByProject[projectId] ?? [];
  const [moduleFilter, setModuleFilter] = useState<string>("__all"); // __all | __unassigned | <id>
  // __active hides closed tasks (default); __all shows everything; else exact status.
  const [statusFilter, setStatusFilter] = useState<string>("__active");
  const [openManageModules, setOpenManageModules] = useState(false);
  const [openZentaoImport, setOpenZentaoImport] = useState(false);
  const [moduleStatsOpen, setModuleStatsOpen] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();
  const focusTaskId = Number(searchParams.get("task")) || null;
  const [highlightId, setHighlightId] = useState<number | null>(null);
  const rowRefs = useRef<Record<number, HTMLTableRowElement | null>>({});
  // Handed from phase 1 to phase 2 of the focus effect below. A ref, not state,
  // so setting it does not itself trigger a render.
  const pendingFocusRef = useRef<number | null>(null);
  // Memoized because the `?? []` fallback would otherwise hand out a fresh
  // array every render, re-firing the focus effects below on each one.
  const tasks = useMemo(() => byProject[projectId] ?? [], [byProject, projectId]);
  const visibleTasks = tasks.filter((tk) => {
    if (statusFilter === "__active") { if (tk.status === "closed") return false; }
    else if (statusFilter !== "__all") { if (tk.status !== statusFilter) return false; }
    if (moduleFilter === "__all") return true;
    if (moduleFilter === "__unassigned") return tk.module_id == null;
    return tk.module_id === Number(moduleFilter);
  });
  const { list: members, loadedForCompany: membersLoadedFor, loadFor: loadMembers } = useMembersStore();
  const [openNew, setOpenNew] = useState(false);
  const [startingTask, setStartingTask] = useState<Task | null>(null);
  const [completingTask, setCompletingTask] = useState<Task | null>(null);
  const [pausingTask, setPausingTask] = useState<Task | null>(null);
  const [resumingTask, setResumingTask] = useState<Task | null>(null);
  const PAGE_SIZE = 20;
  const [page, setPage] = useState(1);
  const totalPages = Math.max(1, Math.ceil(visibleTasks.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages);
  const pagedTasks = visibleTasks.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);
  useEffect(() => { setPage(1); }, [statusFilter, moduleFilter]);

  // Phase 1 — arrived from the command palette. Clear the filters that could
  // hide the target (the default drops closed tasks) and park the id in a ref.
  // The URL param is dropped right away so a later re-render cannot re-trigger.
  useEffect(() => {
    if (focusTaskId == null) return;
    if (tasks.length === 0) return; // wait for the task list to load

    // Gone, or belongs to another project: drop the param and stop.
    if (!tasks.some((tk) => tk.id === focusTaskId)) {
      setSearchParams({}, { replace: true });
      return;
    }

    pendingFocusRef.current = focusTaskId;
    // Only clear the filters when they would actually hide the target. A deep
    // link should not silently throw away the filter the user had set.
    if (!visibleTasks.some((tk) => tk.id === focusTaskId)) {
      setStatusFilter("__all");
      setModuleFilter("__all");
    }
    setSearchParams({}, { replace: true });
  }, [focusTaskId, tasks, visibleTasks, setSearchParams]);

  // Phase 2 — filters have settled, so visibleTasks now contains the target.
  // Runs after the effect above resets the page to 1, which is exactly why this
  // cannot be folded into phase 1: that reset would clobber the page we set.
  useEffect(() => {
    const id = pendingFocusRef.current;
    if (id == null) return;

    // Not here yet: phase 1 just relaxed the filters and visibleTasks has not
    // caught up. Wait for the next pass rather than guessing a page.
    const index = visibleTasks.findIndex((tk) => tk.id === id);
    if (index < 0) return;

    pendingFocusRef.current = null;
    setPage(Math.floor(index / PAGE_SIZE) + 1);
    setHighlightId(id);
  }, [visibleTasks]);

  useEffect(() => {
    if (highlightId == null) return;
    rowRefs.current[highlightId]?.scrollIntoView({ block: "center" });
    const timer = setTimeout(() => setHighlightId(null), 2000);
    return () => clearTimeout(timer);
  }, [highlightId]);

  const moduleStats: ModuleLaborStat[] = useModuleStatsStore((s) => s.byProject[projectId] ?? []);
  const refreshModuleStats = useModuleStatsStore((s) => s.refresh);

  useEffect(() => { loadFor(projectId, null); }, [projectId, loadFor]);
  useEffect(() => {
    if (!modulesLoadedFor[projectId]) loadModules(projectId);
  }, [projectId, modulesLoadedFor, loadModules]);
  useEffect(() => {
    if (membersLoadedFor !== companyId) loadMembers(companyId);
  }, [companyId, membersLoadedFor, loadMembers]);
  useEffect(() => { refreshModuleStats(projectId); }, [projectId, refreshModuleStats]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="w-40"><SelectValue placeholder={t("task.filterByStatus")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__active">{t("task.activeStatuses")}</SelectItem>
              <SelectItem value="todo">{t("taskStatus.todo")}</SelectItem>
              <SelectItem value="in_progress">{t("taskStatus.in_progress")}</SelectItem>
              <SelectItem value="paused">{t("taskStatus.paused")}</SelectItem>
              <SelectItem value="done">{t("taskStatus.done")}</SelectItem>
              <SelectItem value="closed">{t("taskStatus.closed")}</SelectItem>
              <SelectItem value="__all">{t("task.allStatuses")}</SelectItem>
            </SelectContent>
          </Select>
          <Select value={moduleFilter} onValueChange={setModuleFilter}>
            <SelectTrigger className="w-40"><SelectValue placeholder={t("module.filterByModule")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("module.allModules")}</SelectItem>
              <SelectItem value="__unassigned">{t("module.unassigned")}</SelectItem>
              {modules.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="outline" onClick={() => setOpenManageModules(true)}>
            {t("module.manage")}
          </Button>
          <Button variant="outline" onClick={() => setOpenZentaoImport(true)}>
            {t("zentaoImport.title")}
          </Button>
        </div>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("task.create")}</Button></DialogTrigger>
          <FormDialogContent>
            <DialogHeader><DialogTitle>{t("task.create")}</DialogTitle></DialogHeader>
            <TaskForm
              members={members}
              modules={modules}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </FormDialogContent>
        </Dialog>
      </div>

      {moduleStats.length > 0 && (
        <Card>
          <button
            type="button"
            className="flex w-full items-center justify-between px-4 py-3 text-left"
            onClick={() => setModuleStatsOpen((o) => !o)}
          >
            <div className="flex items-center gap-1.5">
              {moduleStatsOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <span className="text-sm font-semibold">{t("financial.laborByModule")}</span>
            </div>
            <span className="text-sm text-muted-foreground">
              {t("financial.laborByModuleSummary", {
                hours: moduleStats.reduce((sum, s) => sum + s.hours, 0),
                cost: formatCNY(moduleStats.reduce((sum, s) => sum + s.cost_cents, 0)),
              })}
            </span>
          </button>
          {moduleStatsOpen && (
            <CardContent className="p-0">
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("module.title")}</TableHead>
                    <TableHead className="text-right w-24">{t("timelog.hours")}</TableHead>
                    <TableHead className="text-right w-32">人力成本</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {moduleStats.map((s) => (
                    <TableRow key={s.module_id ?? "unassigned"}>
                      <TableCell>{s.module_name ?? t("module.unassigned")}</TableCell>
                      <TableCell className="text-right">{s.hours}</TableCell>
                      <TableCell className="text-right">{formatCNY(s.cost_cents)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          )}
        </Card>
      )}

      {visibleTasks.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">
          {tasks.length === 0 ? t("task.empty") : t("task.emptyFiltered")}
        </CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-24 whitespace-nowrap">{t("task.status")}</TableHead>
                  <TableHead>{t("task.name")}</TableHead>
                  <TableHead className="w-24">{t("task.assignee")}</TableHead>
                  <TableHead className="text-right w-16">预估</TableHead>
                  <TableHead className="text-right w-16">实际</TableHead>
                  <TableHead className="w-28 whitespace-nowrap">{t("task.startedAt")}</TableHead>
                  <TableHead className="w-28 whitespace-nowrap">{t("task.completedAt")}</TableHead>
                  <TableHead className="w-40 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagedTasks.map((tk) => {
                  const assignee = members.find((m) => m.id === tk.assignee_id);
                  return (
                    <TableRow
                      key={tk.id}
                      ref={(el) => { rowRefs.current[tk.id] = el; }}
                      className={highlightId === tk.id ? "bg-amber-100 transition-colors" : undefined}
                    >
                      <TableCell>
                        <Badge
                          variant="secondary"
                          className={`whitespace-nowrap ${TASK_STATUS_BADGE_CLASS[tk.status] ?? ""}`}
                        >{t(`taskStatus.${tk.status}`)}</Badge>
                      </TableCell>
                      <TableCell className="font-medium">
                        <button
                          className="text-left hover:underline cursor-pointer"
                          onClick={() => navigate(`/projects/${projectId}/tasks/${tk.id}`)}
                        >{tk.title}</button>
                      </TableCell>
                      <TableCell className="text-muted-foreground">{assignee?.name ?? "—"}</TableCell>
                      <TableCell className="text-right">{tk.estimated_hours != null ? `${tk.estimated_hours}h` : "—"}</TableCell>
                      <TableCell className="text-right">{tk.actual_hours > 0 ? `${tk.actual_hours}h` : "—"}</TableCell>
                      <TableCell className="text-muted-foreground whitespace-nowrap">{tk.started_at ?? "—"}</TableCell>
                      <TableCell className="text-muted-foreground whitespace-nowrap">{tk.completed_at ?? "—"}</TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        {tk.status === "todo" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title="开始"
                            onClick={() => setStartingTask(tk)}
                          ><Play className="h-4 w-4" /></Button>
                        )}
                        {tk.status === "in_progress" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title={t("task.pause")}
                            onClick={() => setPausingTask(tk)}
                          ><Pause className="h-4 w-4" /></Button>
                        )}
                        {tk.status === "paused" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title={t("task.resume")}
                            onClick={() => setResumingTask(tk)}
                          ><PlayCircle className="h-4 w-4" /></Button>
                        )}
                        {tk.status !== "done" && tk.status !== "closed" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title="完成"
                            onClick={() => setCompletingTask(tk)}
                          ><CheckCircle className="h-4 w-4" /></Button>
                        )}
                        {tk.status === "done" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title="关闭任务"
                            onClick={async () => {
                              try { await setStatus(tk.id, { to: "closed" }, projectId); }
                              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                            }}
                          ><Archive className="h-4 w-4" /></Button>
                        )}
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-7 px-2"
                          title={t("timelog.title")}
                          onClick={() => navigate(`/projects/${projectId}/tasks/${tk.id}`)}
                        ><Clock className="h-4 w-4" /></Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
            {visibleTasks.length > PAGE_SIZE && (
              <div className="flex items-center justify-between border-t px-3 py-2 text-sm text-muted-foreground">
                <div>{t("pagination.info", { total: visibleTasks.length, page: currentPage, pages: totalPages })}</div>
                <div className="flex items-center gap-1">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={currentPage <= 1}
                    onClick={() => setPage((p) => Math.max(1, p - 1))}
                  >{t("pagination.prev")}</Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    disabled={currentPage >= totalPages}
                    onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                  >{t("pagination.next")}</Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      <Dialog open={openManageModules} onOpenChange={setOpenManageModules}>
        <FormDialogContent>
          <DialogHeader>
            <DialogTitle>{t("module.manage")}</DialogTitle>
          </DialogHeader>
          <ManageModulesForm
            projectId={projectId}
            modules={modules}
            onClose={() => setOpenManageModules(false)}
            createModule={createModule}
            updateModule={updateModule}
            moveModuleUp={moveModuleUp}
            moveModuleDown={moveModuleDown}
            softDeleteModule={softDeleteModule}
          />
        </FormDialogContent>
      </Dialog>

      <ZentaoImportDialog
        projectId={projectId}
        companyId={companyId}
        open={openZentaoImport}
        onOpenChange={setOpenZentaoImport}
      />

      <Dialog open={!!startingTask} onOpenChange={(o) => !o && setStartingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>开始任务</DialogTitle></DialogHeader>
          {startingTask && (
            <StatusTransitionDialog
              task={startingTask}
              mode="start"
              onSubmit={async (input, change) => {
                try { await update(startingTask.id, input, projectId, change); setStartingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setStartingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!pausingTask} onOpenChange={(o) => !o && setPausingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.pauseTitle")}</DialogTitle></DialogHeader>
          {pausingTask && (
            <StatusTransitionDialog
              task={pausingTask}
              mode="pause"
              onSubmit={async (_input, change) => {
                try { await setStatus(pausingTask.id, change, projectId); setPausingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setPausingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!resumingTask} onOpenChange={(o) => !o && setResumingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.resumeTitle")}</DialogTitle></DialogHeader>
          {resumingTask && (
            <StatusTransitionDialog
              task={resumingTask}
              mode="resume"
              onSubmit={async (_input, change) => {
                try { await setStatus(resumingTask.id, change, projectId); setResumingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setResumingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!completingTask} onOpenChange={(o) => !o && setCompletingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>完成任务</DialogTitle></DialogHeader>
          {completingTask && (
            <StatusTransitionDialog
              task={completingTask}
              mode="complete"
              existingHours={(tasks.find((t) => t.id === completingTask.id))?.actual_hours ?? 0}
              onSubmit={async (input, change) => {
                try {
                  await update(completingTask.id, input, projectId, change);
                  const h = input.hours;
                  if (typeof h === "number" && h > 0 && completingTask.assignee_id != null) {
                    await createTimelog({
                      task_id: completingTask.id,
                      member_id: completingTask.assignee_id,
                      work_date: new Date().toISOString().slice(0, 10),
                      hours: h,
                    }, projectId);
                  }
                  setCompletingTask(null);
                }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setCompletingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>
    </div>
  );
}

export function toDatetimeLocal(v: string | null | undefined): string {
  if (!v) return "";
  if (v.includes(" ")) return v.replace(" ", "T").slice(0, 16);
  if (v.length === 10) return v + "T00:00";
  return v.slice(0, 16);
}

export function fromDatetimeLocal(v: string): string | null {
  if (!v) return null;
  return v.replace("T", " ");
}

export function TaskForm({ members, modules, initial, onSubmit, onCancel }: {
  members: Member[];
  modules: Module[];
  initial?: Task;
  onSubmit: (input: TaskInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const { markDirty } = useFormDialog();
  const [title, setTitle] = useState(initial?.title ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [assigneeId, setAssigneeId] = useState<string>(
    initial?.assignee_id ? String(initial.assignee_id) : "__none"
  );
  const [status] = useState(initial?.status ?? "todo");
  const [estHours, setEstHours] = useState(
    initial?.estimated_hours != null ? String(initial.estimated_hours) : ""
  );
  const [dueDate, setDueDate] = useState(initial?.due_date ?? "");
  const [startedAt, setStartedAt] = useState(toDatetimeLocal(initial?.started_at));
  const [completedAt, setCompletedAt] = useState(toDatetimeLocal(initial?.completed_at));
  const [moduleId, setModuleId] = useState<string>(
    initial?.module_id ? String(initial.module_id) : "__none"
  );
  const [busy, setBusy] = useState(false);

  const currentAssignee = initial?.assignee_id
    ? members.find((m) => m.id === initial.assignee_id)
    : null;
  const active = members.filter((m) => m.is_active);
  // Include the current assignee even if archived, so the Select value has a matching item.
  const options = currentAssignee && !currentAssignee.is_active
    ? [currentAssignee, ...active]
    : active;

  const submit = async () => {
    if (!title.trim()) return toast.error(t("task.titleRequired"));
    // Estimated hours is mandatory when creating a task (but not when editing an
    // existing one, so tasks predating this rule stay editable).
    if (!initial && estHours.trim() === "") return toast.error(t("task.estimatedHoursRequired"));
    setBusy(true);
    try {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || null,
        assignee_id: assigneeId === "__none" ? null : Number(assigneeId),
        status,
        estimated_hours: estHours === "" ? null : Number(estHours),
        due_date: dueDate || null,
        started_at: fromDatetimeLocal(startedAt),
        completed_at: fromDatetimeLocal(completedAt),
        module_id: moduleId === "__none" ? null : Number(moduleId),
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("task.name")}</Label>
        <Input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.assignee")}</Label>
          <Select value={assigneeId} onValueChange={(v) => { markDirty(); setAssigneeId(v); }}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__none">{t("task.unassigned")}</SelectItem>
              {options.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>
                  {m.is_active ? m.name : `${m.name}（已归档）`}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("task.status")}</Label>
          <Input value={t(`taskStatus.${status}`)} disabled />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.estimatedHours")}</Label>
          <Input type="number" min="0" step="0.5" value={estHours} onChange={(e) => setEstHours(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("task.dueDate")}</Label>
          <Input type="date" value={dueDate ?? ""} onChange={(e) => setDueDate(e.target.value)} />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("task.startedAt")}</Label>
          <Input type="datetime-local" value={startedAt} onChange={(e) => setStartedAt(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("task.completedAt")}</Label>
          <Input type="datetime-local" value={completedAt} onChange={(e) => setCompletedAt(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("task.module")}</Label>
        <Select value={moduleId} onValueChange={(v) => { markDirty(); setModuleId(v); }}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="__none">{t("module.unassigned")}</SelectItem>
            {modules.map((m) => (
              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-1">
        <Label>{t("task.description")}</Label>
        <Textarea value={description ?? ""} onChange={(e) => setDescription(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel} disabled={busy}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("task.save")}</Button>
      </DialogFooter>
    </div>
  );
}

// Exported so the task detail page can reuse this exact edit form: unlike
// TimeLogForm (create-only, needs a member picker), an edit never changes who
// logged the hours, so its shape matches TimeLogUpdateInput one-to-one.
export function TimeLogEditForm({ initial, onSubmit, onCancel }: {
  initial: TimeLog;
  onSubmit: (input: TimeLogUpdateInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [date, setDate] = useState(initial.work_date);
  const [hours, setHours] = useState(initial.hours);
  const [notes, setNotes] = useState(initial.notes ?? "");
  const [busy, setBusy] = useState(false);

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("timelog.workDate")}</Label>
        <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.hours")}</Label>
        <Input
          type="number"
          inputMode="decimal"
          min="0"
          max="24"
          step="0.25"
          value={hours}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
          }}
        />
      </div>
      <div className="space-y-1">
        <Label>{t("timelog.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!date) return toast.error(t("timelog.dateRequired"));
            if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
            setBusy(true);
            try { await onSubmit({ work_date: date, hours, notes: notes.trim() || null }); }
            finally { setBusy(false); }
          }}
        >{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}

export function ManageModulesForm({
  projectId,
  modules,
  onClose,
  createModule,
  updateModule,
  moveModuleUp,
  moveModuleDown,
  softDeleteModule,
}: {
  projectId: number;
  modules: Module[];
  onClose: () => void;
  createModule: (projectId: number, input: { name: string }) => Promise<Module>;
  updateModule: (id: number, input: { name: string }, projectId: number) => Promise<Module>;
  moveModuleUp: (id: number, projectId: number) => Promise<void>;
  moveModuleDown: (id: number, projectId: number) => Promise<void>;
  softDeleteModule: (id: number, projectId: number) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState("");

  return (
    <div className="space-y-3">
      <div className="space-y-2 max-h-64 overflow-y-auto">
        {modules.length === 0 ? (
          <div className="text-sm text-muted-foreground">{t("task.empty")}</div>
        ) : (
          modules.map((m, idx) => (
            <div key={m.id} className="flex items-center gap-2">
              {editingId === m.id ? (
                <>
                  <Input
                    value={editingName}
                    onChange={(e) => setEditingName(e.target.value)}
                    className="flex-1"
                  />
                  <Button
                    size="sm"
                    onClick={async () => {
                      if (!editingName.trim()) return toast.error(t("module.nameRequired"));
                      try {
                        await updateModule(m.id, { name: editingName.trim() }, projectId);
                        setEditingId(null);
                      } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("common.save")}</Button>
                  <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>
                    {t("common.cancel")}
                  </Button>
                </>
              ) : (
                <>
                  <div className="flex-1">{m.name}</div>
                  <Button size="sm" variant="ghost" disabled={idx === 0}
                    onClick={async () => {
                      try { await moveModuleUp(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveUp")}</Button>
                  <Button size="sm" variant="ghost" disabled={idx === modules.length - 1}
                    onClick={async () => {
                      try { await moveModuleDown(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveDown")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={() => { setEditingId(m.id); setEditingName(m.name); }}
                  >{t("module.rename")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={async () => {
                      if (!(await confirmDialog(t("module.deleteConfirm", { name: m.name }), { title: t("common.delete"), okLabel: t("common.delete") }))) return;
                      try { await softDeleteModule(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.delete")}</Button>
                </>
              )}
            </div>
          ))
        )}
      </div>
      <div className="flex items-center gap-2">
        <Input
          placeholder={t("module.new")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <Button
          onClick={async () => {
            if (!newName.trim()) return toast.error(t("module.nameRequired"));
            try { await createModule(projectId, { name: newName.trim() }); setNewName(""); }
            catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
          }}
        >{t("module.new")}</Button>
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>{t("common.close")}</Button>
      </DialogFooter>
    </div>
  );
}
