import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { INK, INK_SOFT, PAPER, RULE, SERIF, VERMILION } from "@/lib/brand";
import { useCompanyStore } from "@/stores/company";
import { useSearchStore } from "@/stores/search";
import type { SearchHit } from "@/types";

const DEBOUNCE_MS = 200;

// Derived from VERMILION rather than a new hex, so the selected row reads as
// the same accent the sidebar uses for its active item.
const SELECTED_WASH = "rgba(178, 58, 46, 0.07)";

export function CommandPalette() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const companyId = useCompanyStore((s) => s.currentId);
  const { open, setOpen, hits, run, clear } = useSearchStore();

  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const activeRef = useRef<HTMLButtonElement | null>(null);

  // Cmd+K / Ctrl+K toggles from anywhere in the app shell. Read and write the
  // store through getState() so the listener is bound once and never restaged.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        const s = useSearchStore.getState();
        s.setOpen(!s.open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Debounce so a fast typist does not fire one query per keystroke.
  useEffect(() => {
    if (!open || companyId == null) return;
    const id = setTimeout(() => { void run(companyId, query); }, DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [open, query, companyId, run]);

  useEffect(() => { setActiveIndex(0); }, [hits]);

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const groups = useMemo(() => {
    const projects = hits.filter((h) => h.kind === "project");
    const tasks = hits.filter((h) => h.kind === "task");
    return { projects, tasks, flat: [...projects, ...tasks] };
  }, [hits]);

  const close = () => {
    setOpen(false);
    setQuery("");
    clear();
  };

  const go = (hit: SearchHit) => {
    navigate(
      hit.kind === "project"
        ? `/projects/${hit.project_id}`
        : `/projects/${hit.project_id}?task=${hit.id}`
    );
    close();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (groups.flat.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => (i + 1) % groups.flat.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => (i - 1 + groups.flat.length) % groups.flat.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      go(groups.flat[activeIndex]);
    }
  };

  const renderGroup = (label: string, items: SearchHit[], offset: number) =>
    items.length === 0 ? null : (
      <div key={label}>
        <div
          className="px-4 pb-1 pt-3 text-[11px] tracking-widest"
          style={{ ...SERIF, color: INK_SOFT }}
        >
          {label}
        </div>
        {items.map((hit, i) => {
          const idx = offset + i;
          const active = idx === activeIndex;
          return (
            <button
              key={`${hit.kind}-${hit.id}`}
              ref={active ? activeRef : null}
              type="button"
              onClick={() => go(hit)}
              onMouseEnter={() => setActiveIndex(idx)}
              className="flex w-full items-baseline gap-2.5 py-2 pl-4 pr-4 text-left text-sm"
              style={{
                // Transparent when idle so the text never shifts on selection.
                borderLeft: `3px solid ${active ? VERMILION : "transparent"}`,
                background: active ? SELECTED_WASH : "transparent",
                paddingLeft: "calc(1rem - 3px)",
              }}
            >
              <span className="truncate" style={{ color: INK }}>{hit.title}</span>
              {hit.subtitle && (
                <span className="truncate text-xs" style={{ color: INK_SOFT }}>
                  {hit.subtitle}
                </span>
              )}
            </button>
          );
        })}
      </div>
    );

  return (
    <Dialog open={open} onOpenChange={(o) => (o ? setOpen(true) : close())}>
      <DialogContent
        hideClose
        className="max-w-2xl gap-0 overflow-hidden p-0"
        onKeyDown={onKeyDown}
      >
        {/* Radix needs a title for screen readers; the field itself is the
            visible label, so this one stays off-screen. */}
        <DialogTitle className="sr-only">{t("search.placeholder")}</DialogTitle>

        <div
          className="flex items-center gap-2.5 px-4"
          style={{ borderBottom: `1px solid ${RULE}` }}
        >
          <Search className="h-4 w-4 shrink-0" style={{ color: INK_SOFT }} />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("search.placeholder")}
            className="h-12 w-full bg-transparent text-sm outline-none placeholder:opacity-60"
            style={{ color: INK }}
          />
        </div>

        {/* min and max are close together on purpose: the panel opens at a
            usable height and barely resizes as results come and go. */}
        <div className="max-h-96 min-h-[20rem] overflow-auto pb-2">
          {groups.flat.length === 0
            ? query.trim() && (
                <div className="px-4 py-10 text-center text-sm" style={{ color: INK_SOFT }}>
                  {t("search.empty")}
                </div>
              )
            : (
              <>
                {renderGroup(t("search.groupProjects"), groups.projects, 0)}
                {renderGroup(t("search.groupTasks"), groups.tasks, groups.projects.length)}
              </>
            )}
        </div>

        <div
          className="px-4 py-2 text-[11px]"
          style={{ background: PAPER, color: INK_SOFT }}
        >
          {t("search.hint")}
        </div>
      </DialogContent>
    </Dialog>
  );
}
