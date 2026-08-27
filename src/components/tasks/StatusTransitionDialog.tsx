import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { DialogFooter } from "@/components/ui/dialog";
import { nowDatetimeLocal } from "@/lib/time";
import type { StatusChange, Task, TaskInput } from "@/types";

export const TASK_STATUS_BADGE_CLASS: Record<string, string> = {
  todo: "bg-slate-100 text-slate-700",
  in_progress: "bg-amber-100 text-amber-700",
  paused: "bg-rose-100 text-rose-700",
  done: "bg-emerald-100 text-emerald-700",
  closed: "bg-zinc-200 text-zinc-500",
};

export type TransitionMode = "start" | "complete" | "pause" | "resume";

const TARGET_STATUS: Record<TransitionMode, string> = {
  start: "in_progress",
  complete: "done",
  pause: "paused",
  resume: "in_progress",
};

// Shared transition dialog for a task, used by the project task panel, the task
// detail page and the dashboard todo card so all three behave identically:
// pick a timestamp, optionally log hours (on complete), attach a note.
export function StatusTransitionDialog({
  task, mode, existingHours, onSubmit, onCancel,
}: {
  task: Task;
  mode: TransitionMode;
  existingHours?: number;
  onSubmit: (input: TaskInput & { hours?: number }, change: StatusChange) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const showHours = mode === "complete";
  // Pause and resume do not own a column on tasks; only start/complete write
  // back to started_at / completed_at.
  const dateField = mode === "start" ? "started_at" : mode === "complete" ? "completed_at" : null;

  const [datetime, setDatetime] = useState(
    dateField && task[dateField]
      ? task[dateField]!.replace(" ", "T").slice(0, 16)
      : nowDatetimeLocal()
  );
  const [startedAt, setStartedAt] = useState(
    task.started_at ? task.started_at.replace(" ", "T").slice(0, 16) : ""
  );
  // Not editable here — description has its own card on the detail page. It
  // still has to be resent because update_task overwrites every field it is
  // given, and omitting it would clear the description as a side effect of
  // starting or completing the task.
  const description = task.description?.trim() || null;
  const [note, setNote] = useState("");
  const [hours, setHours] = useState(0);
  const [busy, setBusy] = useState(false);

  const dateLabel =
    mode === "start" ? t("task.startedAt")
    : mode === "complete" ? t("task.completedAt")
    : mode === "pause" ? t("task.pausedAt")
    : t("task.resumedAt");

  const handleSubmit = async () => {
    if (mode === "pause" && !note.trim()) {
      toast.error(t("task.pauseReasonRequired"));
      return;
    }
    if (showHours) {
      // Completing a task must attribute its hours to someone, and the task's
      // total actual hours (already logged + this session) must end up > 0.
      if (task.assignee_id == null) {
        toast.error("请先为任务指定负责人，再完成任务");
        return;
      }
      if ((existingHours ?? 0) + hours <= 0) {
        toast.error("完成任务时总工时须大于 0（已有工时 + 本次）");
        return;
      }
    }
    setBusy(true);
    try {
      const stored = datetime ? datetime.replace("T", " ") : null;
      const storedStartedAt = showHours && startedAt ? startedAt.replace("T", " ") : null;
      const input: TaskInput & { hours?: number } = {
        title: task.title,
        description,
        assignee_id: task.assignee_id,
        estimated_hours: task.estimated_hours,
        due_date: task.due_date,
        started_at: storedStartedAt ?? task.started_at,
        completed_at: task.completed_at,
        module_id: task.module_id,
        external_ref: task.external_ref,
      };
      if (dateField) input[dateField] = stored;
      if (showHours && hours > 0) input.hours = hours;
      const change: StatusChange = {
        to: TARGET_STATUS[mode],
        occurred_at: stored,
        body: note.trim() || null,
      };
      await onSubmit(input, change);
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="text-sm text-muted-foreground">{task.title}</div>
      {showHours && (
        <div className="space-y-1">
          <Label>{t("task.startedAt")}</Label>
          <Input
            type="datetime-local"
            value={startedAt}
            onChange={(e) => setStartedAt(e.target.value)}
          />
        </div>
      )}
      <div className="space-y-1">
        <Label>{dateLabel}</Label>
        <Input
          type="datetime-local"
          value={datetime}
          onChange={(e) => setDatetime(e.target.value)}
        />
      </div>
      {showHours && (
        <div className="space-y-1">
          <Label>本次工时 (h)<span className="text-muted-foreground font-normal"> — 已有工时 {(existingHours ?? 0)}h</span></Label>
          <Input
            autoFocus
            type="number"
            inputMode="decimal"
            min="0"
            max="24"
            step="0.25"
            value={hours === 0 ? "" : String(hours)}
            placeholder="0"
            onChange={(e) => {
              const n = Number(e.target.value);
              if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
            }}
          />
        </div>
      )}
      {mode === "pause" && (
        <div className="space-y-1">
          <Label>{t("task.pauseReason")}</Label>
          <Textarea
            autoFocus
            rows={3}
            value={note}
            placeholder={t("task.pauseReasonPlaceholder")}
            onChange={(e) => setNote(e.target.value)}
          />
        </div>
      )}
      {mode === "resume" && (
        <div className="space-y-1">
          <Label>{t("task.resumeNote")}</Label>
          <Textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
        </div>
      )}
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={handleSubmit} disabled={busy}>确定</Button>
      </DialogFooter>
    </div>
  );
}
