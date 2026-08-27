import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ChevronLeft, Play, Pause, PlayCircle, CheckCircle, Archive, Trash2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { FormDialogContent } from "@/components/ui/form-dialog";
import { call } from "@/lib/ipc";
import { confirmDialog } from "@/lib/confirm";
import { useMembersStore } from "@/stores/members";
import { useModulesStore } from "@/stores/modules";
import { useTasksStore } from "@/stores/tasks";
import { useTaskEventsStore } from "@/stores/taskEvents";
import { useTimelogsStore } from "@/stores/timelogs";
import { StatusTransitionDialog, TASK_STATUS_BADGE_CLASS, type TransitionMode }
  from "@/components/tasks/StatusTransitionDialog";
import { Timeline } from "@/components/tasks/Timeline";
import { NoteComposer } from "@/components/tasks/NoteComposer";
import TimeLogForm from "@/components/tasks/TimeLogForm";
import { TaskForm, TimeLogEditForm } from "@/routes/projects/tasks/panel";
import type { Project, StatusChange, Task, TaskEvent, TaskInput, TimeLog } from "@/types";

export default function TaskDetailPage() {
  const { projectId, taskId } = useParams<{ projectId: string; taskId: string }>();
  const pid = Number(projectId);
  const tid = Number(taskId);
  const navigate = useNavigate();
  const { t } = useTranslation();

  const [project, setProject] = useState<Project | null>(null);
  const [task, setTask] = useState<Task | null>(null);
  const [mode, setMode] = useState<TransitionMode | null>(null);
  const [openNewLog, setOpenNewLog] = useState(false);
  const [editingLog, setEditingLog] = useState<TimeLog | null>(null);
  const [editingNote, setEditingNote] = useState<TaskEvent | null>(null);

  const update = useTasksStore((s) => s.update);
  const setStatus = useTasksStore((s) => s.setStatus);
  const softDeleteTask = useTasksStore((s) => s.softDelete);
  const events = useTaskEventsStore((s) => s.byTask[tid] ?? []);
  const loadEvents = useTaskEventsStore((s) => s.loadFor);
  const createNote = useTaskEventsStore((s) => s.createNote);
  const updateNote = useTaskEventsStore((s) => s.updateNote);
  const deleteNote = useTaskEventsStore((s) => s.deleteNote);
  const logs = useTimelogsStore((s) => s.byTask[tid] ?? []);
  const loadLogs = useTimelogsStore((s) => s.loadFor);
  const createLog = useTimelogsStore((s) => s.create);
  const updateLog = useTimelogsStore((s) => s.update);
  const deleteLog = useTimelogsStore((s) => s.softDelete);
  const members = useMembersStore((s) => s.list);
  const loadMembers = useMembersStore((s) => s.loadFor);
  const modules = useModulesStore((s) => s.byProject[pid] ?? []);
  const loadModules = useModulesStore((s) => s.loadFor);

  // Reload the task itself after any mutation: actual_hours and status live on
  // it and the list store does not necessarily hold this project.
  const reloadTask = useMemo(
    () => async () => {
      try {
        setTask(await call<Task>("get_task", { id: tid }));
      } catch (e: unknown) {
        toast.error(t("common.error", { msg: String(e) }));
        navigate(`/projects/${pid}`);
      }
    },
    [tid, pid, navigate, t],
  );

  useEffect(() => {
    if (Number.isNaN(tid) || Number.isNaN(pid)) return;
    call<Project>("get_project", { id: pid }).then(setProject).catch(() => navigate("/projects"));
    reloadTask();
    loadEvents(tid);
    loadLogs(tid);
    loadModules(pid);
  }, [tid, pid, navigate, reloadTask, loadEvents, loadLogs, loadModules]);

  useEffect(() => {
    if (project) loadMembers(project.company_id);
  }, [project, loadMembers]);

  const pausedDays = useMemo(() => {
    if (task?.status !== "paused") return null;
    const last = [...events].reverse().find((e) => e.to_status === "paused");
    if (!last) return null;
    const since = new Date(last.occurred_at.replace(" ", "T")).getTime();
    return { days: Math.floor((Date.now() - since) / 86_400_000), reason: last.body };
  }, [task, events]);

  if (!task || !project) return null;

  const backToList = () => navigate(`/projects/${pid}?task=${tid}`);

  const runTransition = async (input: TaskInput & { hours?: number }, change: StatusChange) => {
    try {
      if (mode === "pause" || mode === "resume") {
        await setStatus(tid, change, pid);
      } else {
        await update(tid, input, pid, change);
        if (mode === "complete" && typeof input.hours === "number" && input.hours > 0
            && task.assignee_id != null) {
          await createLog({
            task_id: tid,
            member_id: task.assignee_id,
            work_date: new Date().toISOString().slice(0, 10),
            hours: input.hours,
          }, pid);
        }
      }
      setMode(null);
      await reloadTask();
      await loadEvents(tid);
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    }
  };

  return (
    <div className="space-y-4">
      <button
        className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        onClick={backToList}
      >
        <ChevronLeft className="h-4 w-4" />
        {project.name} / {t("task.title")}
      </button>

      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-2 min-w-0">
          <h1 className="text-xl font-semibold truncate">{task.title}</h1>
          <Badge variant="secondary" className={`whitespace-nowrap ${TASK_STATUS_BADGE_CLASS[task.status] ?? ""}`}>
            {t(`taskStatus.${task.status}`)}
          </Badge>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {task.status === "todo" && (
            <Button size="sm" variant="outline" onClick={() => setMode("start")}>
              <Play className="h-4 w-4 mr-1" />{t("taskStatus.in_progress")}
            </Button>
          )}
          {task.status === "in_progress" && (
            <Button size="sm" variant="outline" onClick={() => setMode("pause")}>
              <Pause className="h-4 w-4 mr-1" />{t("task.pause")}
            </Button>
          )}
          {task.status === "paused" && (
            <Button size="sm" variant="outline" onClick={() => setMode("resume")}>
              <PlayCircle className="h-4 w-4 mr-1" />{t("task.resume")}
            </Button>
          )}
          {task.status !== "done" && task.status !== "closed" && (
            <Button size="sm" variant="outline" onClick={() => setMode("complete")}>
              <CheckCircle className="h-4 w-4 mr-1" />{t("taskStatus.done")}
            </Button>
          )}
          {task.status === "done" && (
            <Button
              size="sm"
              variant="outline"
              onClick={async () => {
                try { await setStatus(tid, { to: "closed" }, pid); await reloadTask(); await loadEvents(tid); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            ><Archive className="h-4 w-4 mr-1" />{t("taskStatus.closed")}</Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            onClick={async () => {
              const ok = await confirmDialog(t("task.deleteConfirm", { title: task.title }), {
                title: t("task.delete"),
                kind: "warning",
                okLabel: t("task.delete"),
                cancelLabel: t("common.cancel"),
              });
              if (!ok) return;
              try { await softDeleteTask(tid, pid); navigate(`/projects/${pid}`); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          ><Trash2 className="h-4 w-4" /></Button>
        </div>
      </div>

      {pausedDays && (
        <div className="rounded-md border border-rose-200 bg-rose-50 px-4 py-2 text-sm text-rose-800">
          {t("task.pausedFor", { days: pausedDays.days })}
          {pausedDays.reason ? ` · ${pausedDays.reason}` : ""}
        </div>
      )}

      <Card>
        <CardContent className="p-4">
          <TaskForm
            // TaskForm seeds its started_at/completed_at state from `initial`
            // only on mount. A status transition rewrites those columns and
            // bumps updated_at server-side; keying on both forces a remount
            // so the form picks up the fresh values instead of resubmitting
            // stale ones on the next save.
            key={`${task.id}-${task.updated_at}`}
            members={members}
            modules={modules}
            initial={task}
            onCancel={backToList}
            onSubmit={async (input) => {
              try { await update(tid, input, pid); await reloadTask(); toast.success(t("task.save")); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          />
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-4 space-y-4">
          <div className="flex items-start gap-3">
            <div className="flex-1"><NoteComposer onSubmit={(body) => createNote(tid, body)} /></div>
            <Button size="sm" variant="outline" onClick={() => setOpenNewLog(true)}>
              <Plus className="h-4 w-4 mr-1" />{t("timelog.add")}
            </Button>
          </div>
          <Timeline
            events={events}
            logs={logs}
            members={members}
            onEditNote={setEditingNote}
            onDeleteNote={async (e) => {
              if (!(await confirmDialog(t("timeline.noteDeleteConfirm"), {
                title: t("common.delete"), kind: "warning",
                okLabel: t("common.delete"), cancelLabel: t("common.cancel"),
              }))) return;
              try { await deleteNote(e.id, tid); }
              catch (err: unknown) { toast.error(t("common.error", { msg: String(err) })); }
            }}
            onEditLog={setEditingLog}
            onDeleteLog={async (l) => {
              if (!(await confirmDialog(t("timelog.deleteConfirm"), {
                title: t("common.delete"), kind: "warning",
                okLabel: t("common.delete"), cancelLabel: t("common.cancel"),
              }))) return;
              try { await deleteLog(l.id, tid, pid); await reloadTask(); }
              catch (err: unknown) { toast.error(t("common.error", { msg: String(err) })); }
            }}
          />
        </CardContent>
      </Card>

      <Dialog open={mode !== null} onOpenChange={(o) => !o && setMode(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.title")}</DialogTitle></DialogHeader>
          {mode && (
            <StatusTransitionDialog
              task={task}
              mode={mode}
              existingHours={task.actual_hours}
              onSubmit={runTransition}
              onCancel={() => setMode(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={openNewLog} onOpenChange={setOpenNewLog}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("timelog.add")}</DialogTitle></DialogHeader>
          <TimeLogForm
            taskId={tid}
            members={members.filter((m) => m.is_active)}
            onCancel={() => setOpenNewLog(false)}
            onSubmit={async (input) => {
              try { await createLog(input, pid); setOpenNewLog(false); await reloadTask(); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          />
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!editingLog} onOpenChange={(o) => !o && setEditingLog(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("timelog.edit")}</DialogTitle></DialogHeader>
          {editingLog && (
            <TimeLogEditForm
              initial={editingLog}
              onCancel={() => setEditingLog(null)}
              onSubmit={async (input) => {
                try {
                  await updateLog(editingLog.id, input, tid, pid);
                  setEditingLog(null);
                  await reloadTask();
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!editingNote} onOpenChange={(o) => !o && setEditingNote(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.notes")}</DialogTitle></DialogHeader>
          {editingNote && (
            <NoteComposer
              key={editingNote.id}
              initial={editingNote.body ?? ""}
              submitLabel={t("task.save")}
              onSubmit={async (body) => { await updateNote(editingNote.id, body, tid); setEditingNote(null); }}
            />
          )}
        </FormDialogContent>
      </Dialog>
    </div>
  );
}
