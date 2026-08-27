import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquare, GitCommitHorizontal, Clock, Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Member, TaskEvent, TimeLog } from "@/types";

type Item =
  | { at: string; sortAt: string; sortId: number; kind: "event"; event: TaskEvent }
  | { at: string; sortAt: string; sortId: number; kind: "log"; log: TimeLog };

// Timestamps arrive at two precisions: SQLite's COALESCE(?, datetime('now'))
// produces "YYYY-MM-DD HH:MM:SS", while the status-transition dialog sends
// "YYYY-MM-DD HH:MM" (no seconds) for a user-picked time. Comparing those
// two forms as strings is unsound ("...09:05" is a string-prefix of, and
// thus sorts as older than, any "...09:05:SS"). Normalize to second
// precision before comparing. A minute-precision value is read as the START
// of that minute — i.e. ":00" — since that's the only rule the data gives us
// grounds to encode; do not read anything more into it.
function toSecondPrecision(timestamp: string): string {
  return timestamp.length === 16 ? `${timestamp}:00` : timestamp;
}

// Stamps reach this column in three shapes: a time_log carries a work_date and
// no clock time at all, apply_transition writes seconds, and the transition
// dialog's picker writes minutes. Render one shape — date, plus time when the
// source actually has one — so the column reads as a column. Seconds are
// dropped rather than padded onto the rows that lack them: a timeline is read
// at minute resolution, and inventing ":00" would assert precision the picker
// never captured.
function formatStamp(value: string): string {
  return value.length > 16 ? value.slice(0, 16) : value;
}

// time_logs stays its own table — it carries a cost snapshot and has its own
// mutation paths — so the two sources merge here at render time rather than
// being double-written into task_events.
function merge(events: TaskEvent[], logs: TimeLog[]): Item[] {
  const items: Item[] = [
    ...events.map((event) => ({
      at: event.occurred_at,
      sortAt: toSecondPrecision(event.occurred_at),
      sortId: event.id,
      kind: "event" as const,
      event,
    })),
    ...logs.map((log) => ({
      at: log.work_date,
      // A work_date has no time component; pin it to end of day so a log lands
      // after the same day's status changes rather than before them.
      sortAt: `${log.work_date} 23:59:59`,
      sortId: log.id,
      kind: "log" as const,
      log,
    })),
  ];
  // Newest first: compare normalized timestamps, then break ties by id.
  // Two rows can share an identical timestamp (e.g. a task created already
  // "in_progress" writes created_at == started_at; two same-day time_logs
  // share the same end-of-day pin) — id order is the only remaining signal
  // for which one came later, and it must be compared numerically, not as
  // a string ('3' > '1' would otherwise put id 3 ahead of id 10).
  return items.sort((a, b) => {
    if (a.sortAt !== b.sortAt) return a.sortAt < b.sortAt ? 1 : -1;
    return b.sortId - a.sortId;
  });
}

export function Timeline({
  events, logs, members, onEditNote, onDeleteNote, onEditLog, onDeleteLog,
}: {
  events: TaskEvent[];
  logs: TimeLog[];
  members: Member[];
  onEditNote: (e: TaskEvent) => void;
  onDeleteNote: (e: TaskEvent) => void;
  onEditLog: (l: TimeLog) => void;
  onDeleteLog: (l: TimeLog) => void;
}) {
  const { t } = useTranslation();
  const items = useMemo(() => merge(events, logs), [events, logs]);
  const memberName = (id: number) => members.find((m) => m.id === id)?.name ?? `#${id}`;

  if (items.length === 0) {
    return <div className="p-6 text-sm text-muted-foreground text-center">{t("timeline.empty")}</div>;
  }

  return (
    <ol className="space-y-0">
      {items.map((item) => {
        const key = `${item.kind}-${item.kind === "event" ? item.event.id : item.log.id}`;
        return (
          <li key={key} className="group flex gap-3 border-l pl-4 pb-4 last:pb-0 relative">
            <span className="absolute -left-[7px] top-1 h-3 w-3 rounded-full bg-background border-2 border-muted-foreground/40" />
            <div className="flex-1 min-w-0">
              {item.kind === "log" ? (
                <LogRow
                  log={item.log}
                  who={memberName(item.log.member_id)}
                  onEdit={() => onEditLog(item.log)}
                  onDelete={() => onDeleteLog(item.log)}
                />
              ) : (
                <EventRow
                  event={item.event}
                  onEdit={() => onEditNote(item.event)}
                  onDelete={() => onDeleteNote(item.event)}
                />
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

// Fixed width and tabular figures so every row's date starts at the same x and
// the times line up under each other, leaving a blank where a time_log has no
// clock time to show.
function Stamp({ value }: { value: string }) {
  return (
    <span className="w-32 shrink-0 whitespace-nowrap text-xs text-muted-foreground tabular-nums">
      {formatStamp(value)}
    </span>
  );
}

// Every row reserves this slot whether or not it has buttons. Status changes
// are a record and cannot be edited, but if their rows skipped the slot the
// stamp column would sit further right on them than on the editable rows.
const ACTIONS_SLOT = "w-14 shrink-0";

function RowActions({ onEdit, onDelete }: { onEdit?: () => void; onDelete?: () => void }) {
  if (!onEdit || !onDelete) return <span className={ACTIONS_SLOT} aria-hidden />;
  return (
    <span className={`${ACTIONS_SLOT} flex justify-end opacity-0 transition-opacity group-hover:opacity-100`}>
      <Button size="sm" variant="ghost" className="h-6 px-1.5" onClick={onEdit}>
        <Pencil className="h-3.5 w-3.5" />
      </Button>
      <Button size="sm" variant="ghost" className="h-6 px-1.5" onClick={onDelete}>
        <Trash2 className="h-3.5 w-3.5" />
      </Button>
    </span>
  );
}

function LogRow({ log, who, onEdit, onDelete }: {
  log: TimeLog;
  who: string;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div>
      <div className="flex items-start gap-2">
        <Clock className="h-4 w-4 mt-0.5 shrink-0 text-muted-foreground" />
        <span className="text-sm flex-1">
          {t("timeline.loggedHours", { hours: log.hours })} · {who}
        </span>
        <Stamp value={log.work_date} />
        <RowActions onEdit={onEdit} onDelete={onDelete} />
      </div>
      {log.notes && <div className="ml-6 text-sm text-muted-foreground">{log.notes}</div>}
    </div>
  );
}

function EventRow({ event, onEdit, onDelete }: {
  event: TaskEvent;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const isNote = event.kind === "note";
  const headline = isNote
    ? null
    : event.from_status == null
      ? t("timeline.created")
      : t("timeline.changed", {
          from: t(`taskStatus.${event.from_status}`),
          to: t(`taskStatus.${event.to_status}`),
        });

  return (
    <div>
      <div className="flex items-start gap-2">
        {isNote
          ? <MessageSquare className="h-4 w-4 mt-0.5 shrink-0 text-muted-foreground" />
          : <GitCommitHorizontal className="h-4 w-4 mt-0.5 shrink-0 text-muted-foreground" />}
        <span className="text-sm flex-1">{headline ?? event.body}</span>
        <Stamp value={event.occurred_at} />
        <RowActions
          onEdit={isNote ? onEdit : undefined}
          onDelete={isNote ? onDelete : undefined}
        />
      </div>
      {!isNote && event.body && (
        <div className="ml-6 text-sm text-muted-foreground">{event.body}</div>
      )}
    </div>
  );
}
