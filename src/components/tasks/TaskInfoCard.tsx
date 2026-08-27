import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { TaskForm } from "@/routes/projects/tasks/panel";
import type { Member, Module, Task, TaskInput } from "@/types";

/**
 * The task's fields, read-only until asked otherwise. Reading the timeline is
 * what this page is for; a permanently-open form would put eight inputs above
 * the thing people came to see. Editing swaps the whole block for the same
 * TaskForm the list page uses, so there is one definition of a task's fields.
 */
export function TaskInfoCard({ task, members, modules, onSave }: {
  task: Task;
  members: Member[];
  modules: Module[];
  // Resolves false when the save failed, so a rejected edit keeps the form
  // open with the user's input rather than discarding it behind a toast.
  onSave: (input: TaskInput) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
        <CardTitle className="text-sm">{t("task.basicInfo")}</CardTitle>
        {!editing && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7 px-2 -mr-2 text-muted-foreground hover:text-foreground"
            onClick={() => setEditing(true)}
          >
            {t("task.edit")}
          </Button>
        )}
      </CardHeader>
      <CardContent>
        {editing ? (
          <TaskForm
            // Remount on every server-side change so the form re-reads the
            // timestamps a status transition may have rewritten.
            key={`${task.id}-${task.updated_at}`}
            members={members}
            modules={modules}
            initial={task}
            layout="narrow"
            // The detail page gives description its own card; keeping it here
            // too would put one field behind two entry points.
            showDescription={false}
            onCancel={() => setEditing(false)}
            onSubmit={async (input) => {
              if (await onSave(input)) setEditing(false);
            }}
          />
        ) : (
          <ReadOnlyFields task={task} members={members} modules={modules} />
        )}
      </CardContent>
    </Card>
  );
}

function ReadOnlyFields({ task, members, modules }: {
  task: Task;
  members: Member[];
  modules: Module[];
}) {
  const { t } = useTranslation();
  const assignee = members.find((m) => m.id === task.assignee_id);
  const module = modules.find((m) => m.id === task.module_id);

  return (
    <div className="space-y-4">
      <HoursSummary task={task} />

      <dl className="space-y-2 text-sm">
        <Row label={t("task.assignee")} value={assignee?.name} />
        <Row label={t("task.module")} value={module?.name} />
        <Row label={t("task.dueDate")} value={task.due_date} />
        <Row label={t("task.startedAt")} value={task.started_at} />
        <Row label={t("task.completedAt")} value={task.completed_at} />
      </dl>
    </div>
  );
}

/**
 * Actual hours against the estimate — the one number this page emphasises.
 * In a cost-accounting tool the estimate/actual gap is the question every
 * other field on the card exists to explain.
 */
function HoursSummary({ task }: { task: Task }) {
  const { t } = useTranslation();
  const estimated = task.estimated_hours;
  const over = estimated != null && task.actual_hours > estimated;
  // Both sides are floats; round the gap so 8 - 7.7 reads 0.3, not 0.30000000000000004.
  const diff = estimated != null
    ? Math.round(Math.abs(task.actual_hours - estimated) * 100) / 100
    : 0;

  return (
    <div>
      <div className="text-xs text-muted-foreground">{t("task.actualHours")}</div>
      <div className="text-2xl font-semibold tabular-nums leading-tight">
        {task.actual_hours}h
      </div>
      <div className="mt-0.5 text-xs text-muted-foreground">
        {estimated == null ? (
          t("task.noEstimate")
        ) : (
          <>
            {t("task.estimatedHours")} {estimated}h
            {diff > 0 && (
              <span className={over ? "text-rose-600" : undefined}>
                {" · "}
                {over ? t("task.overBy", { hours: diff }) : t("task.underBy", { hours: diff })}
              </span>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value?: string | null }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="shrink-0 text-muted-foreground">{label}</dt>
      <dd className="min-w-0 truncate text-right" title={value || undefined}>{value || "—"}</dd>
    </div>
  );
}
