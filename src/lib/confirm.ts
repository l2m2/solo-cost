import { confirm as tauriConfirm } from "@tauri-apps/plugin-dialog";
import i18n from "@/i18n";

type ConfirmOptions = {
  title?: string;
  okLabel?: string;
  cancelLabel?: string;
  kind?: "info" | "warning" | "error";
};

// Native window.confirm() does not reliably block in this Tauri webview — it
// returns without waiting for the user, so a guarded `if (!confirm()) return`
// never actually stops the action. Route every confirmation through the plugin
// dialog, which is async and truly blocks. Resolves true when the user confirms.
export function confirmDialog(message: string, options: ConfirmOptions = {}): Promise<boolean> {
  return tauriConfirm(message, {
    kind: options.kind ?? "warning",
    title: options.title,
    okLabel: options.okLabel ?? i18n.t("common.confirm"),
    cancelLabel: options.cancelLabel ?? i18n.t("common.cancel"),
  });
}
