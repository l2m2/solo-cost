import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { listProjectStatusEvents } from "@/lib/ipc";
import { STATUS_OPTIONS, statusLabel } from "@/lib/status";
import { useProjectsStore } from "@/stores/projects";
import type { Project, ProjectFinancialSummary, ProjectStatusEvent } from "@/types";

type Action = [command: string, label: string];

function actionsFor(project: Project, financial: ProjectFinancialSummary | null): Action[] {
  const archive: Action = ["archive", "archive"];
  const cancel: Action = ["cancelled", "cancel"];
  if (project.is_archived) return [["unarchive", "unarchive"]];
  switch (project.status) {
    case "negotiating": return [["pending", "agree"], ["in_progress", "start"], cancel, archive];
    case "pending": return [["in_progress", "start"], ["negotiating", "negotiate"], cancel, archive];
    case "in_progress": {
      const paid = financial && financial.revenue_tax_inclusive_cents > 0
        && financial.actual_payment_cents >= financial.revenue_tax_inclusive_cents;
      return [paid ? ["settled", "deliverPaid"] : ["delivered", "deliver"], cancel, archive];
    }
    case "delivered": return [["settled", "settle"], ["in_progress", "rework"], archive];
    case "settled": return [archive, ["delivered", "undoSettlement"]];
    case "cancelled": return [["restore", "restore"], archive];
    default: return [];
  }
}

export function LifecycleActions({ project, financial, onChange }: {
  project: Project;
  financial: ProjectFinancialSummary | null;
  onChange: (project: Project) => void;
}) {
  const { t } = useTranslation();
  const setStatus = useProjectsStore((s) => s.setStatus);
  const [dialog, setDialog] = useState<Action | null>(null);
  const [reason, setReason] = useState("");
  const [target, setTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const [primary, ...more] = actionsFor(project, financial);
  const label = (action: Action) => t(`lifecycle.${action[1]}`);
  const execute = async (action: Action, next?: string, note?: string) => {
    setBusy(true);
    try {
      onChange(await setStatus(project.id, action[0], next, note));
      setDialog(null);
    } catch (e) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };
  const select = (action: Action) => {
    const [command] = action;
    if (command === "cancelled" || (command === "unarchive" && project.status === "unknown")
      || (command === "restore" && !project.cancelled_from)) {
      setReason("");
      setTarget("");
      setDialog(action);
    } else {
      void execute(action);
    }
  };
  if (!primary) return null;
  const cancelling = dialog?.[0] === "cancelled";
  const targets = dialog?.[0] === "restore"
    ? STATUS_OPTIONS.filter((s) => ["negotiating", "pending", "in_progress"].includes(s.value))
    : STATUS_OPTIONS;

  return (
    <div className="flex gap-2">
      <Button disabled={busy} onClick={() => select(primary)}>{label(primary)}</Button>
      {more.length > 0 && <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" disabled={busy}>{t("lifecycle.more")}</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {more.map((action) => <DropdownMenuItem key={action[0]} onSelect={() => select(action)}>
            {label(action)}
          </DropdownMenuItem>)}
        </DropdownMenuContent>
      </DropdownMenu>}
      <Dialog open={!!dialog} onOpenChange={(open) => { if (!open && !busy) setDialog(null); }}>
        <DialogContent className="max-w-sm">
          <DialogHeader><DialogTitle>{dialog && label(dialog)} · {project.name}</DialogTitle></DialogHeader>
          {cancelling
            ? <label className="space-y-2 text-sm">{t("lifecycle.reason")}
                <Textarea value={reason} onChange={(e) => setReason(e.target.value)} />
              </label>
            : <Select value={target} onValueChange={setTarget}>
                <SelectTrigger><SelectValue placeholder={t("lifecycle.choose")} /></SelectTrigger>
                <SelectContent>{targets.map((s) =>
                  <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>)}</SelectContent>
              </Select>}
          <DialogFooter>
            <Button variant="outline" disabled={busy} onClick={() => setDialog(null)}>{t("common.cancel")}</Button>
            <Button variant={cancelling ? "destructive" : "default"} disabled={busy || (!cancelling && !target)}
              onClick={() => dialog && execute(dialog, target || undefined, reason || undefined)}>
              {t("lifecycle.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export function StatusTimeline({ project }: { project: Project }) {
  const { t } = useTranslation();
  const [events, setEvents] = useState<ProjectStatusEvent[] | null>(null);
  const [error, setError] = useState("");
  const [expanded, setExpanded] = useState(false);
  useEffect(() => {
    let active = true;
    setError("");
    setEvents(null);
    listProjectStatusEvents(project.id)
      .then((rows) => { if (active) setEvents(rows); })
      .catch((e: unknown) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [project]);

  const describe = (event: ProjectStatusEvent) => {
    const next = statusLabel(event.to_status);
    if (event.action === "archive") return t("lifecycle.archive") + " · " + next;
    if (event.action === "unarchive") return t("lifecycle.unarchive") + " · " + next;
    if (!event.from_status) return t("lifecycle.created") + " · " + next;
    return statusLabel(event.from_status) + " → " + next;
  };

  if (events?.length === 0 && !error) {
    return <p className="border-t pt-3 text-sm text-muted-foreground">{t("lifecycle.empty")}</p>;
  }

  return <section className="space-y-2 border-t pt-4 text-sm">
      <h2 className="font-semibold">{t("lifecycle.history")}</h2>
      {error ? <p role="alert">{t("common.error", { msg: error })}</p>
        : !events && <p>{t("lifecycle.loading")}</p>}
      <ol className="space-y-2">
        {(expanded ? events : events?.slice(0, 3))?.map((event) => <li key={event.id} className="border-l-2 pl-3">
          <span>{describe(event)}</span>
          <time className="ml-3 text-xs text-muted-foreground">
            {new Date(event.created_at.replace(" ", "T") + "Z").toLocaleString("zh-CN", { hour12: false })}
          </time>
          {event.reason && <p className="whitespace-pre-wrap break-words text-muted-foreground">{event.reason}</p>}
        </li>)}
      </ol>
      {events && events.length > 3 && <Button variant="ghost" size="sm" onClick={() => setExpanded(!expanded)}>
        {expanded ? t("lifecycle.collapse") : t("lifecycle.expand", { count: events.length })}
      </Button>}
  </section>;
}
