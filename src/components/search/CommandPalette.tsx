import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { Dialog, DialogContent } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useCompanyStore } from "@/stores/company";
import { useSearchStore } from "@/stores/search";
import type { SearchHit } from "@/types";

const DEBOUNCE_MS = 200;

export function CommandPalette() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const companyId = useCompanyStore((s) => s.currentId);
  const { hits, run, clear } = useSearchStore();

  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const activeRef = useRef<HTMLButtonElement | null>(null);

  // Cmd+K / Ctrl+K opens from anywhere in the app shell.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
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
      <div key={label} className="py-1">
        <div className="px-3 py-1 text-xs text-muted-foreground">{label}</div>
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
              className={`flex w-full items-baseline gap-2 px-3 py-2 text-left text-sm ${
                active ? "bg-accent" : ""
              }`}
            >
              <span className="truncate">{hit.title}</span>
              {hit.subtitle && (
                <span className="truncate text-xs text-muted-foreground">
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
      <DialogContent className="max-w-xl p-0" onKeyDown={onKeyDown}>
        <div className="border-b p-3">
          <Input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("search.placeholder")}
          />
        </div>
        <div className="max-h-80 overflow-auto">
          {groups.flat.length === 0
            ? query.trim() && (
                <div className="px-3 py-6 text-center text-sm text-muted-foreground">
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
        <div className="border-t px-3 py-2 text-xs text-muted-foreground">
          {t("search.hint")}
        </div>
      </DialogContent>
    </Dialog>
  );
}
