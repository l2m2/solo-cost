import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";

/**
 * The task's description — what this task is — above the activity that
 * explains how it went. It gets the wide column because it is prose; a
 * paragraph in the narrow info sidebar wraps every few words.
 *
 * Editing lives here rather than in TaskForm so the field has exactly one
 * entry point on this page. TaskForm keeps it for the create dialog, where
 * there is no card to edit yet.
 */
export function TaskDescriptionCard({ description, onSave }: {
  description: string | null;
  // Resolves false when the save failed, so the draft survives the error.
  onSave: (description: string | null) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(description ?? "");
  const [busy, setBusy] = useState(false);

  const startEditing = () => {
    setDraft(description ?? "");
    setEditing(true);
  };

  const save = async () => {
    setBusy(true);
    try {
      if (await onSave(draft.trim() || null)) setEditing(false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
        <CardTitle className="text-sm">{t("task.description")}</CardTitle>
        {!editing && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7 px-2 -mr-2 text-muted-foreground hover:text-foreground"
            onClick={startEditing}
          >
            {t("task.edit")}
          </Button>
        )}
      </CardHeader>
      <CardContent>
        {editing ? (
          <div className="space-y-2">
            <Textarea
              autoFocus
              rows={5}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
            />
            <div className="flex justify-end gap-2">
              <Button size="sm" variant="outline" onClick={() => setEditing(false)} disabled={busy}>
                {t("common.cancel")}
              </Button>
              <Button size="sm" onClick={save} disabled={busy}>{t("task.save")}</Button>
            </div>
          </div>
        ) : description ? (
          <p className="text-sm whitespace-pre-wrap">{description}</p>
        ) : (
          <button
            className="text-sm text-muted-foreground hover:text-foreground"
            onClick={startEditing}
          >
            {t("task.noDescription")}
          </button>
        )}
      </CardContent>
    </Card>
  );
}
