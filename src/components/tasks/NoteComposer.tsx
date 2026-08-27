import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

// Always-open composer: appending a note is the most frequent thing to do on
// this page, so it does not hide behind an "add" button. `initial` also lets
// the same component edit an existing note inside a dialog — remount it with a
// key when the target note changes.
export function NoteComposer({ onSubmit, initial = "", submitLabel }: {
  onSubmit: (body: string) => Promise<void>;
  initial?: string;
  submitLabel?: string;
}) {
  const { t } = useTranslation();
  const [body, setBody] = useState(initial);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const trimmed = body.trim();
    if (!trimmed) return;
    setBusy(true);
    try {
      await onSubmit(trimmed);
      setBody("");
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-2">
      <Textarea
        rows={3}
        value={body}
        placeholder={t("timeline.notePlaceholder")}
        onChange={(e) => setBody(e.target.value)}
      />
      <div className="flex justify-end">
        <Button size="sm" onClick={submit} disabled={busy || !body.trim()}>
          {submitLabel ?? t("timeline.addNote")}
        </Button>
      </div>
    </div>
  );
}
