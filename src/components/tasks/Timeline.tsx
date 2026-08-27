import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquare, GitCommitHorizontal, Clock, Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Member, TaskEvent, TimeLog } from "@/types";

type Item =
  | { at: string; sort: string; kind: "event"; event: TaskEvent }
  | { at: string; sort: string; kind: "log"; log: TimeLog };

// time_logs stays its own table — it carries a cost snapshot and has its own
// mutation paths — so the two sources merge here at render time rather than
// being double-written into task_events.
function merge(events: TaskEvent[], logs: TimeLog[]): Item[] {
  const items: Item[] = [
    ...events.map((event) => ({
      at: event.occurred_at,
      sort: `${event.occurred_at}#${event.id}`,
      kind: "event" as const,
      event,
    })),
    ...logs.map((log) => ({
      at: log.work_date,
      // A work_date has no time component; pin it to end of day so a log lands
      // after the same day's status changes rather than before them.
      sort: `${log.work_date} 23:59:59#${log.id}`,
      kind: "log" as const,
      log,
    })),
  ];
  return items.sort((a, b) => (a.sort < b.sort ? 1 : a.sort > b.sort ? -1 : 0));
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

function RowActions({ onEdit, onDelete }: { onEdit: () => void; onDelete: () => void }) {
  return (
    <span className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
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
        <span className="text-xs text-muted-foreground whitespace-nowrap">{log.work_date}</span>
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
        <span className="text-xs text-muted-foreground whitespace-nowrap">{event.occurred_at}</span>
        {isNote && <RowActions onEdit={onEdit} onDelete={onDelete} />}
      </div>
      {!isNote && event.body && (
        <div className="ml-6 text-sm text-muted-foreground">{event.body}</div>
      )}
    </div>
  );
}
