# 全局搜索（Cmd+K）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 加一个 Cmd+K 命令面板，输入关键字即时列出匹配的项目与任务，回车直接跳过去（任务跳转要精确定位到那一行）。

**Architecture:** 后端新增一个 `search` Tauri 命令，对 `projects` 与 `tasks` 各跑一条 LIKE 查询，按公司隔离并排除软删除。前端新增一个自持状态的 `CommandPalette` 组件挂在 `AppLayout` 上，任务命中通过 `/projects/:id?task=N` 深链，由 `TasksPanel` 消费该参数完成筛选重置、翻页、滚动高亮。

**Tech Stack:** Rust + rusqlite（SQLCipher）、Tauri v2 command、React 19 + TypeScript、react-router-dom、zustand、shadcn Dialog、i18next。

**Spec:** `docs/superpowers/specs/2026-08-26-global-search-design.md`

## Global Constraints

- 代码注释一律 **English**；用户可见文案走 i18n，中文放 `src/i18n/zh-CN.json`。
- Commit 遵循 Conventional Commits，`type`/`scope` 小写英文，`subject` 中文，整行 ≤ 72 字符。
- 后端命令范式：`#[tauri::command]` 薄壳 + `with_conn`，真实逻辑放 `*_impl(conn, ...)`，测试只测 `*_impl`。
- `company_id` 由前端显式传入，不在后端读当前公司。
- 所有查询必须 `deleted_at IS NULL`；任务需同时排除自身与所属项目的软删除。
- LIKE 查询一律带 `ESCAPE '\'`，用户输入需转义 `\` `%` `_`。
- 每类结果上限默认 8（`limit` 参数指每类，非总数）。
- 不新增任何 npm / cargo 依赖。
- 前端无测试设施，前端任务以 `pnpm lint` + `pnpm build` 通过为准，行为验证靠手动。

---

### Task 1: 后端 `search` 命令与查询逻辑

**Files:**
- Create: `src-tauri/src/commands/search.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod search;`）
- Modify: `src-tauri/src/lib.rs`（`generate_handler!` 中注册 `commands::search::search`）
- Test: `src-tauri/src/commands/search.rs` 内 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::state::AppState`、`crate::error::{AppError, AppResult}`、`crate::commands::auth::setup_at`（仅测试用）。
- Produces: `SearchHit { kind: String, id: i64, title: String, subtitle: Option<String>, project_id: i64 }`；`search_impl(conn: &Connection, company_id: i64, query: &str, limit: u32) -> AppResult<Vec<SearchHit>>`；Tauri 命令名 `search`，参数 `companyId`、`query`、`limit`。

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/commands/search.rs` 末尾写入（文件此时还没有实现，先只放测试）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb {
        conn: Connection,
        _dir: TempDir,
    }

    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Other')", []).unwrap();
            conn.execute(
                "INSERT INTO clients(company_id, name) VALUES(1, '官网科技')",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }

        fn project(&self, company_id: i64, name: &str, client_id: Option<i64>) -> i64 {
            self.conn
                .execute(
                    "INSERT INTO projects(company_id, name, client_id) VALUES(?1, ?2, ?3)",
                    rusqlite::params![company_id, name, client_id],
                )
                .unwrap();
            self.conn.last_insert_rowid()
        }

        fn task(&self, project_id: i64, title: &str) -> i64 {
            self.conn
                .execute(
                    "INSERT INTO tasks(project_id, title) VALUES(?1, ?2)",
                    rusqlite::params![project_id, title],
                )
                .unwrap();
            self.conn.last_insert_rowid()
        }

        fn soft_delete(&self, table: &str, id: i64) {
            self.conn
                .execute(
                    &format!("UPDATE {table} SET deleted_at = datetime('now') WHERE id = ?1"),
                    rusqlite::params![id],
                )
                .unwrap();
        }
    }

    fn titles(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.title.as_str()).collect()
    }

    #[test]
    fn finds_project_by_name() {
        let db = TestDb::new();
        db.project(1, "官网改版", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(titles(&hits), vec!["官网改版"]);
        assert_eq!(hits[0].kind, "project");
        assert_eq!(hits[0].project_id, hits[0].id);
    }

    #[test]
    fn finds_project_by_client_name() {
        let db = TestDb::new();
        let p = db.project(1, "内部系统", Some(1));
        let hits = search_impl(&db.conn, 1, "官网科技", 8).unwrap();
        assert_eq!(titles(&hits), vec!["内部系统"]);
        assert_eq!(hits[0].id, p);
        assert_eq!(hits[0].subtitle.as_deref(), Some("官网科技"));
    }

    #[test]
    fn finds_project_without_client() {
        // LEFT JOIN, not JOIN: a project with no client must still be findable.
        let db = TestDb::new();
        db.project(1, "无客户项目", None);
        let hits = search_impl(&db.conn, 1, "无客户", 8).unwrap();
        assert_eq!(titles(&hits), vec!["无客户项目"]);
        assert_eq!(hits[0].subtitle, None);
    }

    #[test]
    fn finds_task_by_title_with_project_subtitle() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        let t = db.task(p, "首页切图");
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert_eq!(titles(&hits), vec!["首页切图"]);
        assert_eq!(hits[0].kind, "task");
        assert_eq!(hits[0].id, t);
        assert_eq!(hits[0].project_id, p);
        assert_eq!(hits[0].subtitle.as_deref(), Some("官网改版"));
    }

    #[test]
    fn projects_come_before_tasks() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        db.task(p, "官网切图");
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(hits[0].kind, "project");
        assert_eq!(hits[1].kind, "task");
    }

    #[test]
    fn scopes_to_company() {
        let db = TestDb::new();
        db.project(2, "别家的官网", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_soft_deleted_project() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        db.soft_delete("projects", p);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_soft_deleted_task() {
        let db = TestDb::new();
        let p = db.project(1, "某项目", None);
        let t = db.task(p, "首页切图");
        db.soft_delete("tasks", t);
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_task_whose_project_is_soft_deleted() {
        let db = TestDb::new();
        let p = db.project(1, "某项目", None);
        db.task(p, "首页切图");
        db.soft_delete("projects", p);
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn escapes_percent_wildcard() {
        // Without ESCAPE, a bare "%" would match every row.
        let db = TestDb::new();
        db.project(1, "普通项目", None);
        db.project(1, "折扣 100% 项目", None);
        let hits = search_impl(&db.conn, 1, "%", 8).unwrap();
        assert_eq!(titles(&hits), vec!["折扣 100% 项目"]);
    }

    #[test]
    fn escapes_underscore_wildcard() {
        let db = TestDb::new();
        db.project(1, "ab", None);
        db.project(1, "a_b", None);
        let hits = search_impl(&db.conn, 1, "a_b", 8).unwrap();
        assert_eq!(titles(&hits), vec!["a_b"]);
    }

    #[test]
    fn escapes_backslash() {
        let db = TestDb::new();
        db.project(1, r"路径\测试", None);
        let hits = search_impl(&db.conn, 1, r"\", 8).unwrap();
        assert_eq!(titles(&hits), vec![r"路径\测试"]);
    }

    #[test]
    fn prefix_matches_rank_first() {
        let db = TestDb::new();
        db.project(1, "改版官网", None);
        db.project(1, "官网改版", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(titles(&hits), vec!["官网改版", "改版官网"]);
    }

    #[test]
    fn blank_query_returns_empty() {
        let db = TestDb::new();
        db.project(1, "官网改版", None);
        assert!(search_impl(&db.conn, 1, "", 8).unwrap().is_empty());
        assert!(search_impl(&db.conn, 1, "   ", 8).unwrap().is_empty());
    }

    #[test]
    fn limit_applies_per_kind() {
        let db = TestDb::new();
        let p = db.project(1, "官网 A", None);
        db.project(1, "官网 B", None);
        db.project(1, "官网 C", None);
        db.task(p, "官网任务一");
        db.task(p, "官网任务二");
        db.task(p, "官网任务三");
        let hits = search_impl(&db.conn, 1, "官网", 2).unwrap();
        assert_eq!(hits.iter().filter(|h| h.kind == "project").count(), 2);
        assert_eq!(hits.iter().filter(|h| h.kind == "task").count(), 2);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib commands::search
```

预期：编译失败，报 `cannot find function search_impl` / `cannot find type SearchHit`。

- [ ] **Step 3: 写实现**

把以下内容放在 `src-tauri/src/commands/search.rs` 的**测试模块之前**：

```rust
use crate::error::AppResult;
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

const DEFAULT_LIMIT: u32 = 8;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// "project" or "task".
    pub kind: String,
    pub id: i64,
    pub title: String,
    /// Client name for a project hit, owning project name for a task hit.
    pub subtitle: Option<String>,
    /// Equals `id` for a project hit; the owning project for a task hit.
    pub project_id: i64,
}

/// Escape the LIKE wildcards so a user typing "%" does not match every row.
/// Pairs with `ESCAPE '\'` in every query below.
fn escape_like(raw: &str) -> String {
    raw.replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_")
}

pub(crate) fn search_impl(
    conn: &Connection,
    company_id: i64,
    query: &str,
    limit: u32,
) -> AppResult<Vec<SearchHit>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let escaped = escape_like(trimmed);
    let contains = format!("%{escaped}%");
    let prefix = format!("{escaped}%");

    let mut hits = Vec::new();

    let mut stmt = conn.prepare(
        r"SELECT p.id, p.name, c.name AS client_name
          FROM projects p
          LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
          WHERE p.company_id = ?1
            AND p.deleted_at IS NULL
            AND (p.name LIKE ?2 ESCAPE '\' OR c.name LIKE ?2 ESCAPE '\')
          ORDER BY CASE WHEN p.name LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
                   p.updated_at DESC
          LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![company_id, contains, prefix, limit],
        |row| {
            let id: i64 = row.get("id")?;
            Ok(SearchHit {
                kind: "project".into(),
                id,
                title: row.get("name")?,
                subtitle: row.get("client_name")?,
                project_id: id,
            })
        },
    )?;
    for r in rows {
        hits.push(r?);
    }

    let mut stmt = conn.prepare(
        r"SELECT t.id, t.title, t.project_id, p.name AS project_name
          FROM tasks t
          JOIN projects p ON p.id = t.project_id
          WHERE p.company_id = ?1
            AND t.deleted_at IS NULL
            AND p.deleted_at IS NULL
            AND t.title LIKE ?2 ESCAPE '\'
          ORDER BY CASE WHEN t.title LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
                   t.updated_at DESC
          LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![company_id, contains, prefix, limit],
        |row| {
            Ok(SearchHit {
                kind: "task".into(),
                id: row.get("id")?,
                title: row.get("title")?,
                subtitle: row.get("project_name")?,
                project_id: row.get("project_id")?,
            })
        },
    )?;
    for r in rows {
        hits.push(r?);
    }

    Ok(hits)
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(crate::error::AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn search(
    state: tauri::State<AppState>,
    company_id: i64,
    query: String,
    limit: Option<u32>,
) -> AppResult<Vec<SearchHit>> {
    with_conn(&state, |c| {
        search_impl(c, company_id, &query, limit.unwrap_or(DEFAULT_LIMIT))
    })
}
```

- [ ] **Step 4: 注册模块与命令**

在 `src-tauri/src/commands/mod.rs` 中按字母序插入（`projects` 之后、`tasks` 之前）：

```rust
pub mod search;
```

在 `src-tauri/src/lib.rs` 的 `tauri::generate_handler![...]` 列表里加入一行：

```rust
commands::search::search,
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-tauri && cargo test --lib commands::search
```

预期：15 个测试全部 PASS。

- [ ] **Step 6: 跑全量测试确认没打破别的**

```bash
cd src-tauri && cargo test
```

预期：原有 174 个 + 新增 15 个全部通过。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/search.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(search): 新增 search 命令搜索项目与任务"
```

---

### Task 2: 前端类型与 store

**Files:**
- Modify: `src/types/index.ts`
- Create: `src/stores/search.ts`

**Interfaces:**
- Consumes: Task 1 的 `search` 命令（参数 `companyId`、`query`、`limit`）；`call<T>` 来自 `src/lib/ipc.ts`。
- Produces: `SearchHit` TS 类型；`useSearchStore` 暴露 `{ hits, loading, run(companyId, query), clear() }`。

- [ ] **Step 1: 加类型**

在 `src/types/index.ts` 末尾追加：

```ts
export type SearchHit = {
  kind: "project" | "task";
  id: number;
  title: string;
  subtitle: string | null;
  project_id: number;
};
```

- [ ] **Step 2: 写 store**

创建 `src/stores/search.ts`：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { SearchHit } from "@/types";

type SearchState = {
  hits: SearchHit[];
  loading: boolean;
  run: (companyId: number, query: string) => Promise<void>;
  clear: () => void;
};

// Monotonic request id: a slow response must never overwrite a newer one.
let seq = 0;

export const useSearchStore = create<SearchState>((set) => ({
  hits: [],
  loading: false,

  async run(companyId, query) {
    const q = query.trim();
    if (!q) {
      seq += 1; // invalidate any in-flight request
      set({ hits: [], loading: false });
      return;
    }
    const mine = ++seq;
    set({ loading: true });
    try {
      const hits = await call<SearchHit[]>("search", { companyId, query: q });
      if (mine === seq) set({ hits, loading: false });
    } catch {
      // A failed search should quietly show nothing rather than toast at the
      // user on every keystroke.
      if (mine === seq) set({ hits: [], loading: false });
    }
  },

  clear() {
    seq += 1;
    set({ hits: [], loading: false });
  },
}));
```

- [ ] **Step 3: 类型检查**

```bash
npx tsc -b --pretty false
```

预期：exit 0，无输出。

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts src/stores/search.ts
git commit -m "feat(search): 新增前端搜索类型与 store"
```

---

### Task 3: i18n 文案

**Files:**
- Modify: `src/i18n/zh-CN.json`

**Interfaces:**
- Produces: `search.placeholder`、`search.groupProjects`、`search.groupTasks`、`search.empty`、`search.hint`。

- [ ] **Step 1: 加文案**

在 `src/i18n/zh-CN.json` 顶层加入一个 `search` 段（放在 `common` 之后，保持 2 空格缩进）：

```json
  "search": {
    "placeholder": "搜索项目或任务…",
    "groupProjects": "项目",
    "groupTasks": "任务",
    "empty": "没有匹配的项目或任务",
    "hint": "↑↓ 选择 · 回车跳转 · Esc 关闭"
  },
```

- [ ] **Step 2: 校验 JSON 合法**

```bash
node -e "JSON.parse(require('fs').readFileSync('src/i18n/zh-CN.json','utf8')); console.log('ok')"
```

预期：输出 `ok`。

- [ ] **Step 3: Commit**

```bash
git add src/i18n/zh-CN.json
git commit -m "feat(search): 补充搜索面板文案"
```

---

### Task 4: CommandPalette 组件与挂载

**Files:**
- Create: `src/components/search/CommandPalette.tsx`
- Modify: `src/components/layout/AppLayout.tsx`

**Interfaces:**
- Consumes: Task 2 的 `useSearchStore` 与 `SearchHit`；Task 3 的 `search.*` 文案；现有 `useCompanyStore`（当前公司 id）、`Dialog`/`DialogContent`。
- Produces: `<CommandPalette />`，无 props。

**注意：** 使用 `DialogContent` 而**非** `FormDialogContent`。后者是为防止误关丢失录入内容而设计的，搜索框里没有可丢的东西，Esc 与点遮罩就该直接关。

当前公司 id 取自 `useCompanyStore((s) => s.currentId)`，类型 `number | null`
（已核对 `src/stores/company.ts:7`）。为 `null` 时不发请求。

- [ ] **Step 1: 写组件**

创建 `src/components/search/CommandPalette.tsx`：

```tsx
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
```

- [ ] **Step 2: 挂到 AppLayout**

修改 `src/components/layout/AppLayout.tsx`，加 import 并在根 `div` 内渲染：

```tsx
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { CommandPalette } from "@/components/search/CommandPalette";

// A hair-warm workspace so the paper sidebar/header and the ledger dashboard
// panels sit on a tone that belongs to the same book; content cards stay light.
const WORKSPACE = "#FAF8F3";

export function AppLayout() {
  return (
    <div className="flex h-screen w-screen">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <Header />
        <main className="flex-1 overflow-auto p-6" style={{ background: WORKSPACE }}>
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
```

- [ ] **Step 3: 类型检查与 lint**

```bash
npx tsc -b --pretty false && pnpm lint
```

预期：tsc 无输出，lint exit 0（允许既有的 `only-export-components` warning）。

- [ ] **Step 4: Commit**

```bash
git add src/components/search/CommandPalette.tsx src/components/layout/AppLayout.tsx
git commit -m "feat(search): 新增 Cmd+K 命令面板"
```

---

### Task 5: 任务深链定位

**Files:**
- Modify: `src/routes/projects/detail.tsx`（`TasksPanel`，约 697–1071 行）

**Interfaces:**
- Consumes: Task 4 生成的 `/projects/:id?task=N` 路由。
- Produces: 无对外接口，纯页面行为。

**背景（实现者必读）：** `TasksPanel` 里有两个默认值会让"跳转"失效——
`statusFilter` 默认 `"__active"`，**会过滤掉 `closed` 任务**；任务列表是前端分页，
`PAGE_SIZE = 20`，命中项可能不在第一页。两者都必须处理。

`TasksPanel` 现有的分页与筛选，均已核对（`detail.tsx:713–738`）：

| 名字 | 是什么 | 注意 |
|------|--------|------|
| `statusFilter` / `setStatusFilter` | state，默认 `"__active"` | `"__active"` 会滤掉 `closed` |
| `moduleFilter` / `setModuleFilter` | state，默认 `"__all"` | — |
| `visibleTasks` | 筛选后的数组 | **分页作用于它**，算页码用它 |
| `PAGE_SIZE` | 常量 `20` | — |
| `page` / `setPage` | state，默认 `1` | 翻页用 `setPage`，**没有 `setCurrentPage`** |
| `currentPage` | 派生值 `Math.min(page, totalPages)` | 只读，不能赋值 |
| `pagedTasks` | `visibleTasks` 的当前页切片 | 表格渲染的就是它 |

- [ ] **Step 1: 加 import**

在 `src/routes/projects/detail.tsx` 顶部，把 react-router 的 import 补上 `useSearchParams`：

```tsx
import { useParams, useNavigate, useSearchParams } from "react-router-dom";
```

并确保 `useRef` 在 React import 里。

- [ ] **Step 2: 在 TasksPanel 内加定位逻辑**

在 `TasksPanel` 的 state 声明之后（`statusFilter` 那几行下面）插入：

```tsx
  const [searchParams, setSearchParams] = useSearchParams();
  const focusTaskId = Number(searchParams.get("task")) || null;
  const [highlightId, setHighlightId] = useState<number | null>(null);
  const rowRefs = useRef<Record<number, HTMLTableRowElement | null>>({});
  // Handed from phase 1 to phase 2 of the focus effect below. A ref, not state,
  // so setting it does not itself trigger a render.
  const pendingFocusRef = useRef<number | null>(null);
```

- [ ] **Step 3: 加定位 effect（两阶段）**

**为什么必须分两阶段：** `detail.tsx:738` 已有

```tsx
useEffect(() => { setPage(1); }, [statusFilter, moduleFilter]);
```

—— 改筛选会把页码强制拉回第 1 页。若在同一个 effect 里「先重置筛选、再设目标页」，
这个既有 effect 会在筛选变更提交后把页码盖回 1，翻页静默失效。所以要把
「重置筛选」与「翻页 + 高亮」拆到两次 effect：第二阶段在筛选已经稳定之后才执行。

另外注意分页作用于 `visibleTasks`（筛选后的列表），页码必须按 `visibleTasks`
的下标算，不能用 `tasks`。

放在 `pagedTasks` 计算之后：

```tsx
  // Phase 1 — arrived from the command palette. Clear the filters that could
  // hide the target (the default drops closed tasks) and park the id in a ref.
  // The URL param is dropped right away so a later re-render cannot re-trigger.
  useEffect(() => {
    if (focusTaskId == null) return;
    if (tasks.length === 0) return; // wait for the task list to load

    // Gone, or belongs to another project: drop the param and stop.
    if (!tasks.some((tk) => tk.id === focusTaskId)) {
      setSearchParams({}, { replace: true });
      return;
    }

    pendingFocusRef.current = focusTaskId;
    setStatusFilter("__all");
    setModuleFilter("__all");
    setSearchParams({}, { replace: true });
  }, [focusTaskId, tasks, setSearchParams]);

  // Phase 2 — filters have settled, so visibleTasks now contains the target.
  // Runs after the existing `setPage(1)` reset effect, which is exactly why
  // this cannot be folded into phase 1.
  useEffect(() => {
    const id = pendingFocusRef.current;
    if (id == null) return;
    if (statusFilter !== "__all" || moduleFilter !== "__all") return;

    const index = visibleTasks.findIndex((tk) => tk.id === id);
    if (index < 0) return;

    pendingFocusRef.current = null;
    setPage(Math.floor(index / PAGE_SIZE) + 1);
    setHighlightId(id);
  }, [visibleTasks, statusFilter, moduleFilter]);
```

- [ ] **Step 4: 滚动与高亮淡出**

```tsx
  useEffect(() => {
    if (highlightId == null) return;
    rowRefs.current[highlightId]?.scrollIntoView({ block: "center" });
    const id = setTimeout(() => setHighlightId(null), 2000);
    return () => clearTimeout(id);
  }, [highlightId]);
```

- [ ] **Step 5: 给任务行接上 ref 与高亮样式**

找到任务表格里渲染 `pagedTasks` 的 `<TableRow>`，加上 `ref` 与条件类名：

```tsx
<TableRow
  key={tk.id}
  ref={(el) => { rowRefs.current[tk.id] = el; }}
  className={highlightId === tk.id ? "bg-amber-100 transition-colors" : undefined}
>
```

若该 `<TableRow>` 已有 `className`，用模板字符串把两者拼起来，不要覆盖原有类名。

- [ ] **Step 6: 类型检查与 lint**

```bash
npx tsc -b --pretty false && pnpm lint
```

预期：tsc 无输出，lint exit 0。

- [ ] **Step 7: Commit**

```bash
git add src/routes/projects/detail.tsx
git commit -m "feat(search): 任务深链定位到具体行"
```

---

### Task 6: 手动验证与收尾

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: Task 1–5 的全部产出。

- [ ] **Step 1: 起 dev**

```bash
pnpm tauri dev
```

- [ ] **Step 2: 逐条走查**

前端没有自动化测试，以下每条都必须人工确认：

1. 任意页面按 `Cmd+K`，面板打开且输入框已聚焦。
2. 输入项目名的一部分，项目组出现候选；副标题显示客户名（未绑客户的显示为空）。
3. 输入客户名，能反查到该客户名下的项目。
4. 输入任务标题的一部分，任务组出现候选，副标题是所属项目名。
5. `↑` `↓` 能跨组连续移动，`Enter` 跳转，`Esc` 关闭。
6. 输入 `%`，**不应**列出全部记录（验 LIKE 转义）。
7. 搜一条**已关闭**的任务并回车 —— 能看到它（验筛选重置）。
8. 搜一条落在**第 2 页之后**的任务并回车 —— 能看到它（验翻页）。
9. 跳转后地址栏的 `?task=` 已消失；此时手动改筛选或翻页，不会被弹回原任务。
10. 切换到另一家公司，搜索结果只含该公司的数据。

- [ ] **Step 3: 全量校验**

```bash
cd src-tauri && cargo test && cd .. && pnpm lint && pnpm build
```

预期：cargo 全绿，lint exit 0，build exit 0。

- [ ] **Step 4: 写 CHANGELOG**

在 `CHANGELOG.md` 顶部已有的 `## Unreleased` 段中，`### Added` 分组下加入（若无该分组则新建，并置于 `### Changed` 之前）：

```markdown
### Added
- 新增全局搜索：按 Cmd+K（Windows / Linux 为 Ctrl+K）随时打开搜索框，输入关键字即时匹配项目（按项目名或客户名）与任务（按标题），回车直接跳转；跳到任务时会自动定位到那一行并高亮，不必再自己翻页查找
```

- [ ] **Step 5: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录全局搜索"
```

---

## 未决与风险

- **`Cmd+K` 是否会被 Tauri webview 或系统抢占，未验证。** 若 Task 6 第 1 条走查失败，退路是改绑 `Cmd+P`，或把快捷键做成设置项。
- **Windows 的 `Ctrl+K` 无法在本机验证**（开发机是 macOS）。代码里已同时监听 `metaKey` 与 `ctrlKey`，但真实行为需要在 Windows 上确认。
- **Task 5 的两阶段 effect 是为绕开一个既有副作用而设计的**，不是可以随手合并的：
  `detail.tsx:738` 的 `useEffect(() => { setPage(1); }, [statusFilter, moduleFilter])`
  会在筛选变更后把页码打回第 1 页。合成一个 effect 就会让翻页静默失效——
  症状是「搜到第 3 页的任务，跳过去却停在第 1 页」，而且没有任何报错。
  实现时若想简化这段，先确认这个副作用已被处理。
