import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { DialogFooter } from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { useFormDialog } from "@/components/ui/form-dialog";
import { todayIso } from "@/lib/time";
import type { Member, TimeLogInput } from "@/types";

export default function TimeLogForm({ taskId, members, onSubmit, onCancel }: {
  taskId: number;
  members: Member[];
  onSubmit: (input: TimeLogInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const { markDirty } = useFormDialog();
  const [memberId, setMemberId] = useState(members[0]?.id ?? 0);
  const [date, setDate] = useState(todayIso());
  const [hours, setHours] = useState(8);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!memberId) return toast.error(t("timelog.memberRequired"));
    if (!date) return toast.error(t("timelog.dateRequired"));
    if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
    setBusy(true);
    try {
      await onSubmit({
        task_id: taskId,
        member_id: memberId,
        work_date: date,
        hours,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("timelog.member")}</Label>
          <Select value={String(memberId)} onValueChange={(v) => { markDirty(); setMemberId(Number(v)); }}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {members.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("timelog.workDate")}</Label>
          <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
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
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}
