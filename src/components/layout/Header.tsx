import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { CompanySwitcher } from "./CompanySwitcher";
import { useAuthStore } from "@/stores/auth";
import { useSearchStore } from "@/stores/search";
import { PAPER, INK_SOFT, RULE } from "@/lib/brand";
import { LogOut, Search } from "lucide-react";

// The keycap is a key name, not prose, so it stays out of i18n — same as the
// ↑↓ / Esc hints inside the palette.
const SHORTCUT = navigator.userAgent.includes("Mac") ? "⌘K" : "Ctrl K";

export function Header() {
  const { t } = useTranslation();
  const lock = useAuthStore((s) => s.lock);
  const setSearchOpen = useSearchStore((s) => s.setOpen);

  return (
    <header
      className="flex h-14 items-center gap-4 px-6"
      style={{ background: PAPER, borderBottom: `1px solid ${RULE}` }}
    >
      <CompanySwitcher />

      <div className="ml-auto flex items-center gap-2">
        <button
          type="button"
          onClick={() => setSearchOpen(true)}
          className="flex h-8 w-56 items-center gap-2 rounded-md px-3 text-sm transition-colors"
          style={{ background: "#FFFFFF", border: `1px solid ${RULE}`, color: INK_SOFT }}
        >
          <Search className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">{t("search.placeholder")}</span>
          {/* Lighter than the placeholder, but derived from it rather than a
              new hex, so it stays inside the brand palette. */}
          <kbd
            className="ml-auto shrink-0 font-sans text-[11px] tracking-wide"
            style={{ color: INK_SOFT, opacity: 0.55 }}
          >
            {SHORTCUT}
          </kbd>
        </button>

        <Button variant="ghost" size="sm" onClick={lock} className="gap-2">
          <LogOut className="h-4 w-4" />
          锁定
        </Button>
      </div>
    </header>
  );
}
