import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { DialogFooter } from "@/components/ui/dialog";
import { nowDatetimeLocal } from "@/lib/time";
import type { Task } from "@/types";

export const TASK_STATUS_BADGE_CLASS: Record<string, string> = {
  todo: "bg-slate-100 text-slate-700",
  in_progress: "bg-amber-100 text-amber-700",
  done: "bg-emerald-100 text-emerald-700",
  closed: "bg-zinc-200 text-zinc-500",
};

// Shared start/complete dialog body for a task. Used by the project detail page
// and the dashboard todo-task card so both flows behave identically: pick a
// timestamp, optionally log hours (on complete), and edit the description.
export function StatusTransitionDialog({ task, label, fieldKey, existingHours, onSubmit, onCancel }: {
  task: Task;
  label: string;
  fieldKey: "started_at" | "completed_at";
  existingHours?: number;
  onSubmit: (input: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
}) {
  const [datetime, setDatetime] = useState(
    task[fieldKey] ? task[fieldKey]!.replace(" ", "T").slice(0, 16) : nowDatetimeLocal()
  );
  const [startedAt, setStartedAt] = useState(
    task.started_at ? task.started_at.replace(" ", "T").slice(0, 16) : ""
  );
  const [description, setDescription] = useState(task.description ?? "");
  const [hours, setHours] = useState(0);
  const [busy, setBusy] = useState(false);
  const showHours = fieldKey === "completed_at";

  const handleSubmit = async () => {
    setBusy(true);
    try {
      const stored = datetime ? datetime.replace("T", " ") : null;
      const storedStartedAt = showHours && startedAt
        ? startedAt.replace("T", " ")
        : null;
      const payload: Record<string, unknown> = {
        title: task.title,
        description: description.trim() || null,
        assignee_id: task.assignee_id,
        estimated_hours: task.estimated_hours,
        due_date: task.due_date,
        started_at: task.started_at,
        completed_at: task.completed_at,
        module_id: task.module_id,
        external_ref: task.external_ref,
      };
      payload[fieldKey] = stored;
      if (showHours) {
        if (storedStartedAt) payload.started_at = storedStartedAt;
        if (hours > 0) payload.hours = hours;
      }
      await onSubmit(payload);
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="text-sm text-muted-foreground">{task.title}</div>
      {showHours && (
        <div className="space-y-1">
          <Label>开始时间</Label>
          <Input
            type="datetime-local"
            value={startedAt}
            onChange={(e) => setStartedAt(e.target.value)}
          />
        </div>
      )}
      <div className="space-y-1">
        <Label>{label}</Label>
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
      <div className="space-y-1">
        <Label>描述</Label>
        <Textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>取消</Button>
        <Button onClick={handleSubmit} disabled={busy}>确定</Button>
      </DialogFooter>
    </div>
  );
}
