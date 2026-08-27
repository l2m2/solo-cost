# 任务暂停与任务详情页 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为任务引入 `paused` 状态与追加式时间线（备注 + 状态流转 + 工时合并展示），并把任务详情从对话框改为独立路由页面。

**Architecture:** SQLite 迁移 `0009` 重建 `tasks` 表加入 `paused` 状态，并新建单张 `task_events` 表同时承载手写备注与状态流转事件；所有状态变更收敛到 Rust 侧的 `apply_transition`，与字段更新在同一事务内写事件。前端新增路由 `/projects/:projectId/tasks/:taskId`，把 `detail.tsx` 中的 `TasksPanel` 拆出，编辑与工时两个对话框下线。

**Tech Stack:** Tauri 2 + Rust + rusqlite（bundled-sqlcipher）；React 19 + TypeScript + Vite + zustand + react-router-dom 6 + shadcn/ui + Tailwind + i18next。

**Spec:** `docs/superpowers/specs/2026-08-27-task-pause-and-detail-page-design.md`

## Global Constraints

- 注释一律用英文；提交信息 subject 用中文，整行 ≤ 72 字符，遵循 Conventional Commits。
- 用户可见文案一律走 i18n（`src/i18n/zh-CN.json`），不硬编码中文到组件里。**例外**：`detail.tsx` 与 `dashboard.tsx` 现有代码中已有硬编码的按钮 `title`（如 `title="开始"`），新增按钮沿用同样风格，不趁机整改。
- Rust 侧错误信息用中文，通过 `AppError::Validation(String)` 返回。
- 状态机：`todo → in_progress|closed`、`in_progress → paused|done|closed`、`paused → in_progress|done|closed`、`done → closed`、`closed → ✗`。同状态到同状态为 no-op。
- `body` 长度上限 2000 字符。`to_status='paused'` 时 `body` 必填。
- 不改动 `tasks.description` 的语义；不做字段级 diff 日志；不把备注加入搜索索引；不做 tab 路由化。
- 后端测试命令：`cd src-tauri && cargo test`。前端无测试框架，验证靠 `pnpm build` 与 `pnpm lint`。
- 每个 Task 结束时提交一次。

## 文件结构

**新建**

| 文件 | 职责 |
|---|---|
| `src-tauri/migrations/0009_task_events.sql` | 重建 `tasks` 加 `paused`；建 `task_events`；回填历史事件 |
| `src-tauri/src/commands/task_events.rs` | 事件查询与备注的增删改命令 |
| `src/stores/taskEvents.ts` | 事件 store |
| `src/routes/projects/tasks/panel.tsx` | `TasksPanel` + `TaskForm`（从 `detail.tsx` 迁出） |
| `src/routes/projects/tasks/detail.tsx` | 任务详情页 |
| `src/components/tasks/Timeline.tsx` | 时间线：事件 + 工时合并渲染 |
| `src/components/tasks/NoteComposer.tsx` | 备注输入框 |
| `src/components/tasks/TimeLogForm.tsx` | 工时表单（从 `detail.tsx` 迁出） |

**修改**

| 文件 | 改动 |
|---|---|
| `src-tauri/src/db/migrations.rs` | 注册 `0009`；测试断言版本 8 → 9 |
| `src-tauri/src/commands/tasks.rs` | `ALLOWED_STATUSES` 加 `paused`；状态机；`apply_transition`；命令签名 |
| `src-tauri/src/commands/mod.rs` | `pub mod task_events;` |
| `src-tauri/src/lib.rs` | 注册 4 个新命令 |
| `src-tauri/src/domain/soft_delete.rs` | 4 个函数级联 `task_events` |
| `src-tauri/src/commands/trash.rs` | purge 时硬删 `task_events` |
| `src-tauri/src/domain/dashboard.rs` | 逾期判定排除 `paused`；排序插入 `paused` 键 |
| `src/types/index.ts` | `TaskEvent`、`StatusChange` |
| `src/stores/tasks.ts` | `update` / `setStatus` 签名 |
| `src/stores/auth.ts` | 登出时 reset 新 store |
| `src/components/tasks/StatusTransitionDialog.tsx` | `paused` 分支；badge 配色 |
| `src/routes/projects/detail.tsx` | 迁出任务面板；下线两个对话框 |
| `src/routes/dashboard.tsx` | 暂停/恢复按钮；筛选项；状态点配色 |
| `src/components/search/CommandPalette.tsx` | 任务跳转指向详情页 |
| `src/App.tsx` | 新路由 |
| `src/i18n/zh-CN.json` | 新文案 |

---

### Task 1: 迁移 0009 — paused 状态、task_events 表、历史回填

**Files:**
- Create: `src-tauri/migrations/0009_task_events.sql`
- Modify: `src-tauri/src/db/migrations.rs:4-34`（`MIGRATIONS` 数组）、`src-tauri/src/db/migrations.rs:126`（版本断言）
- Test: `src-tauri/src/db/migrations.rs`（文件内 `mod tests`）

**Interfaces:**
- Consumes: 无
- Produces: 表 `task_events(id, task_id, kind, from_status, to_status, body, occurred_at, created_at, deleted_at)`；`tasks.status` 允许 `'paused'`。

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/db/migrations.rs` 的 `mod tests` 中追加。先阅读文件末尾现有测试，沿用它建连接的方式（现有测试用 `Connection::open_in_memory()` 后调 `run`）。

```rust
    #[test]
    fn migration_0009_allows_paused_status() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title, status) VALUES(1, 'T', 'paused')",
            [],
        )
        .unwrap();
        let s: String = conn
            .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(s, "paused");
    }

    #[test]
    fn migration_0009_backfills_history_for_existing_tasks() {
        // Simulate a pre-0009 database: apply every migration except the last,
        // insert a finished task, then let 0009 run and check the timeline.
        let conn = Connection::open_in_memory().unwrap();
        ensure_meta_table(&conn).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        let upto = (MIGRATIONS.len() - 1) as i64;
        for (idx, (_, sql)) in MIGRATIONS.iter().enumerate() {
            if (idx as i64) >= upto {
                break;
            }
            conn.execute_batch(sql).unwrap();
        }
        conn.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [upto.to_string()],
        )
        .unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title, status, started_at, completed_at, created_at)
             VALUES(1, 'T', 'done', '2026-01-02 09:00:00', '2026-01-05 18:00:00',
                    '2026-01-01 08:00:00')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT to_status, occurred_at FROM task_events
                 WHERE task_id = 1 ORDER BY occurred_at ASC",
            )
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            rows,
            vec![
                ("todo".to_string(), "2026-01-01 08:00:00".to_string()),
                ("in_progress".to_string(), "2026-01-02 09:00:00".to_string()),
                ("done".to_string(), "2026-01-05 18:00:00".to_string()),
            ]
        );
    }
```

同时把现有的版本断言从 8 改成 9：

```rust
        assert_eq!(current_version(&conn).unwrap(), 9);
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib db::migrations
```

预期：`migration_0009_*` 两个测试失败（`no such table: task_events` / CHECK 约束拒绝 `paused`），版本断言测试失败（`8 != 9`）。

- [ ] **Step 3: 写迁移 SQL**

创建 `src-tauri/migrations/0009_task_events.sql`：

```sql
-- Adds the `paused` task status and the task_events timeline table.
-- SQLite cannot alter a CHECK constraint in place, so tasks is rebuilt again
-- (see 0008). The migration runner disables foreign_keys for the whole pass,
-- so dropping tasks while time_logs references it is safe.

CREATE TABLE tasks_new (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       INTEGER NOT NULL REFERENCES projects(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    assignee_id      INTEGER REFERENCES members(id),
    status           TEXT    NOT NULL DEFAULT 'todo'
                              CHECK (status IN ('todo','in_progress','paused','done','closed')),
    estimated_hours  REAL    CHECK (estimated_hours IS NULL OR (estimated_hours >= 0 AND estimated_hours <= 9999)),
    due_date         TEXT,
    started_at       TEXT,
    completed_at     TEXT,
    module_id        INTEGER REFERENCES modules(id),
    external_ref     TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at       TEXT
);

INSERT INTO tasks_new (id, project_id, title, description, assignee_id, status,
                       estimated_hours, due_date, started_at, completed_at,
                       module_id, external_ref, created_at, updated_at, deleted_at)
SELECT id, project_id, title, description, assignee_id, status,
       estimated_hours, due_date, started_at, completed_at,
       module_id, external_ref, created_at, updated_at, deleted_at
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status, deleted_at);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id, deleted_at);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;

-- One table carries both hand-written notes and status transitions so the
-- detail page can render them as a single narrative.
-- occurred_at is when the fact happened (users may backdate a start time);
-- created_at is when the row was written. The timeline sorts by the former.
CREATE TABLE task_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id      INTEGER NOT NULL REFERENCES tasks(id),
    kind         TEXT    NOT NULL CHECK (kind IN ('note','status_change')),
    from_status  TEXT,
    to_status    TEXT,
    body         TEXT,
    occurred_at  TEXT    NOT NULL,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at   TEXT
);

CREATE INDEX idx_task_events_task ON task_events(task_id, deleted_at, occurred_at);

-- Backfill: synthesise history from the date columns tasks already carry, so
-- pre-existing and zentao-imported tasks do not open onto an empty timeline.
INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change', NULL, 'todo', created_at, created_at
FROM tasks WHERE deleted_at IS NULL;

INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change', 'todo', 'in_progress', started_at, started_at
FROM tasks WHERE deleted_at IS NULL AND started_at IS NOT NULL;

INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT id, 'status_change',
       CASE WHEN started_at IS NOT NULL THEN 'in_progress' ELSE 'todo' END,
       CASE WHEN status = 'closed' THEN 'closed' ELSE 'done' END,
       completed_at, completed_at
FROM tasks WHERE deleted_at IS NULL AND completed_at IS NOT NULL;

-- Catch-all: a task whose current status produced no event above (e.g. imported
-- as 'done' with no completed_at) would show a timeline contradicting its badge.
INSERT INTO task_events (task_id, kind, from_status, to_status, occurred_at, created_at)
SELECT t.id, 'status_change', NULL, t.status, t.created_at, t.created_at
FROM tasks t
WHERE t.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM task_events e WHERE e.task_id = t.id AND e.to_status = t.status
  );
```

- [ ] **Step 4: 注册迁移**

在 `src-tauri/src/db/migrations.rs` 的 `MIGRATIONS` 数组末尾追加：

```rust
    (
        "0009_task_events",
        include_str!("../../migrations/0009_task_events.sql"),
    ),
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-tauri && cargo test --lib db::migrations
```

预期：全部 PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/migrations/0009_task_events.sql src-tauri/src/db/migrations.rs
git commit -m "feat(db): 新增 paused 状态与 task_events 时间线表

tasks 表第三次重建以放开 CHECK 约束。task_events 单表同时承载
手写备注与状态流转，occurred_at 与 created_at 分离以支持补填。
迁移内按 created_at/started_at/completed_at 回填历史事件，避免
老任务和禅道导入任务打开后时间线为空。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: 状态机与 apply_transition

**Files:**
- Modify: `src-tauri/src/commands/tasks.rs`
- Test: `src-tauri/src/commands/tasks.rs`（文件内 `mod tests`）

**Interfaces:**
- Consumes: Task 1 的 `task_events` 表。
- Produces:
  - `pub struct StatusChange { pub to: String, pub occurred_at: Option<String>, pub body: Option<String> }`
  - `pub(crate) fn update_impl(conn: &Connection, id: i64, input: &TaskInput, change: Option<&StatusChange>) -> AppResult<Task>`
  - `pub(crate) fn set_status_impl(conn: &Connection, id: i64, change: &StatusChange) -> AppResult<Task>`
  - 命令 `update_task(id: i64, input: TaskInput, change: Option<StatusChange>)`、`set_task_status(id: i64, change: StatusChange)`

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/commands/tasks.rs` 的 `mod tests` 中追加。现有的 `set_status_changes_state` 测试要一并改签名。

```rust
    fn change(to: &str) -> StatusChange {
        StatusChange { to: to.into(), occurred_at: None, body: None }
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let u = set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn illegal_transition_rejected() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        // todo -> done is not a legal edge; a task must be started first.
        let err = set_status_impl(&db.conn, t.id, &change("done")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn same_status_is_a_noop_without_event() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let before = event_count(&db.conn, t.id);
        set_status_impl(&db.conn, t.id, &change("todo")).unwrap();
        assert_eq!(event_count(&db.conn, t.id), before);
    }

    #[test]
    fn pause_without_body_rejected() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let err = set_status_impl(&db.conn, t.id, &change("paused")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn pause_with_body_writes_event() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let c = StatusChange {
            to: "paused".into(),
            occurred_at: Some("2026-03-01 10:00:00".into()),
            body: Some("等客户素材".into()),
        };
        let u = set_status_impl(&db.conn, t.id, &c).unwrap();
        assert_eq!(u.status, "paused");
        let (from, to, body, at): (String, String, String, String) = db
            .conn
            .query_row(
                "SELECT from_status, to_status, body, occurred_at FROM task_events
                 WHERE task_id = ?1 AND to_status = 'paused'",
                [t.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((from.as_str(), to.as_str(), body.as_str(), at.as_str()),
                   ("in_progress", "paused", "等客户素材", "2026-03-01 10:00:00"));
    }

    #[test]
    fn resume_from_paused_allowed() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let c = StatusChange {
            to: "paused".into(),
            occurred_at: None,
            body: Some("等客户".into()),
        };
        set_status_impl(&db.conn, t.id, &c).unwrap();
        let u = set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn update_ignores_status_in_input() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let mut i = input("T");
        i.status = Some("done".into());
        let u = update_impl(&db.conn, t.id, &i, None).unwrap();
        assert_eq!(u.status, "todo");
    }

    #[test]
    fn update_with_change_applies_fields_and_status_together() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let mut i = input("T");
        i.description = Some("改过的描述".into());
        i.started_at = Some("2026-03-01 09:00:00".into());
        let c = StatusChange {
            to: "in_progress".into(),
            occurred_at: Some("2026-03-01 09:00:00".into()),
            body: None,
        };
        let u = update_impl(&db.conn, t.id, &i, Some(&c)).unwrap();
        assert_eq!(u.status, "in_progress");
        assert_eq!(u.description.as_deref(), Some("改过的描述"));
        assert_eq!(u.started_at.as_deref(), Some("2026-03-01 09:00:00"));
    }

    #[test]
    fn create_seeds_timeline() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let to: String = db
            .conn
            .query_row(
                "SELECT to_status FROM task_events WHERE task_id = ?1",
                [t.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(to, "todo");
    }

    #[test]
    fn create_with_source_dates_seeds_full_timeline() {
        // Mirrors a zentao import: the task arrives already finished.
        let db = TestDb::new();
        let mut i = input("T");
        i.status = Some("done".into());
        i.started_at = Some("2026-01-02 09:00:00".into());
        i.completed_at = Some("2026-01-05 18:00:00".into());
        i.created_at = Some("2026-01-01 08:00:00".into());
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(event_count(&db.conn, t.id), 3);
    }

    fn event_count(conn: &Connection, task_id: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND deleted_at IS NULL",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
    }
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib commands::tasks
```

预期：编译失败（`StatusChange` 未定义、`update_impl` 参数个数不符）。

- [ ] **Step 3: 实现状态机与事务化的流转**

在 `src-tauri/src/commands/tasks.rs` 顶部把常量改成 5 个：

```rust
const ALLOWED_STATUSES: [&str; 5] = ["todo", "in_progress", "paused", "done", "closed"];
```

在 `validate` 函数之后加入：

```rust
#[derive(Debug, Deserialize)]
pub struct StatusChange {
    pub to: String,
    #[serde(default)]
    pub occurred_at: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

fn status_label(s: &str) -> &str {
    match s {
        "todo" => "待办",
        "in_progress" => "进行中",
        "paused" => "已暂停",
        "done" => "已完成",
        "closed" => "已关闭",
        other => other,
    }
}

fn transition_allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("todo", "in_progress")
            | ("todo", "closed")
            | ("in_progress", "paused")
            | ("in_progress", "done")
            | ("in_progress", "closed")
            | ("paused", "in_progress")
            | ("paused", "done")
            | ("paused", "closed")
            | ("done", "closed")
    )
}

/// Trims the body and rejects blanks / oversized text. Shared by transitions
/// and by the note commands so both enforce the same limit.
pub(crate) fn normalized_body(body: Option<&str>) -> AppResult<Option<String>> {
    let Some(b) = body.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if b.chars().count() > 2000 {
        return Err(AppError::Validation("内容长度不能超过 2000 字".into()));
    }
    Ok(Some(b.to_string()))
}

/// Moves a task to a new status and records it on the timeline. Returns false
/// when the task already sits in the target status (no-op, no event written).
/// Every status change goes through here — a second write path that skipped the
/// event is exactly what makes a timeline untrustworthy.
fn apply_transition(conn: &Connection, id: i64, change: &StatusChange) -> AppResult<bool> {
    if !ALLOWED_STATUSES.contains(&change.to.as_str()) {
        return Err(AppError::Validation(format!("非法状态：{}", change.to)));
    }
    let from: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
            other => AppError::Db(other),
        })?;
    if from == change.to {
        return Ok(false);
    }
    if !transition_allowed(&from, &change.to) {
        return Err(AppError::Validation(format!(
            "任务不能从「{}」变为「{}」",
            status_label(&from),
            status_label(&change.to)
        )));
    }
    let body = normalized_body(change.body.as_deref())?;
    if change.to == "paused" && body.is_none() {
        return Err(AppError::Validation("暂停任务必须填写原因".into()));
    }
    conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![change.to, id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, body, occurred_at)
         VALUES(?1, 'status_change', ?2, ?3, ?4, COALESCE(?5, datetime('now')))",
        rusqlite::params![id, from, change.to, body, change.occurred_at.as_deref()],
    )?;
    Ok(true)
}

/// Seeds the timeline for a freshly created task. Mirrors the 0009 backfill so
/// imported tasks (which arrive with source dates already set) get the same
/// history as tasks that predate the events table.
fn seed_creation_events(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change', NULL, 'todo', created_at FROM tasks WHERE id = ?1",
        [id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change', 'todo', 'in_progress', started_at
         FROM tasks WHERE id = ?1 AND started_at IS NOT NULL",
        [id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change',
                CASE WHEN started_at IS NOT NULL THEN 'in_progress' ELSE 'todo' END,
                CASE WHEN status = 'closed' THEN 'closed' ELSE 'done' END,
                completed_at
         FROM tasks WHERE id = ?1 AND completed_at IS NOT NULL",
        [id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT t.id, 'status_change', NULL, t.status, t.created_at
         FROM tasks t
         WHERE t.id = ?1
           AND NOT EXISTS (
               SELECT 1 FROM task_events e WHERE e.task_id = t.id AND e.to_status = t.status
           )",
        [id],
    )?;
    Ok(())
}
```

- [ ] **Step 4: 改造 create_impl / update_impl / set_status_impl**

`create_impl` 末尾，把

```rust
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
```

改为

```rust
    let id = conn.last_insert_rowid();
    seed_creation_events(conn, id)?;
    get_impl(conn, id)
```

`update_impl` 换签名并包进事务。**把 `status = COALESCE(?4, status),` 这一行从 UPDATE 语句里删掉**，同时删掉参数列表里对应的 `input.status.as_deref(),`，其余参数序号依次前移：

```rust
pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TaskInput,
    change: Option<&StatusChange>,
) -> AppResult<Task> {
    validate(input)?;
    let project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
            other => AppError::Db(other),
        })?;
    validate_module_belongs_to_project(conn, input.module_id, project_id)?;
    if let Some(assignee_id) = input.assignee_id {
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM members m
             JOIN projects p ON p.company_id = m.company_id
             WHERE p.id = ?1 AND m.id = ?2 AND m.deleted_at IS NULL",
            [project_id, assignee_id],
            |r| r.get(0),
        )?;
        if ok == 0 {
            return Err(AppError::Validation(
                "负责人不属该项目所在公司或已归档/删除".into(),
            ));
        }
    }
    // Fields and the status change land in one transaction: the completion
    // dialog submits a timestamp, a description and the new status together.
    let tx = conn.unchecked_transaction()?;
    // input.status is deliberately not applied here. It survives only for
    // create_task / zentao import; routing every status change through
    // apply_transition is what keeps the timeline complete.
    let n = tx.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            estimated_hours = ?4,
            due_date = ?5,
            started_at = ?6,
            completed_at = ?7,
            module_id = ?8,
            updated_at = datetime('now')
         WHERE id = ?9 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.estimated_hours,
            input.due_date.as_deref(),
            input.started_at.as_deref(),
            input.completed_at.as_deref(),
            input.module_id,
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    if let Some(c) = change {
        apply_transition(&tx, id, c)?;
    }
    tx.commit()?;
    get_impl(conn, id)
}
```

`set_status_impl` 整体替换：

```rust
pub(crate) fn set_status_impl(
    conn: &Connection,
    id: i64,
    change: &StatusChange,
) -> AppResult<Task> {
    let tx = conn.unchecked_transaction()?;
    apply_transition(&tx, id, change)?;
    tx.commit()?;
    get_impl(conn, id)
}
```

两个 `#[tauri::command]` 包装函数同步改签名：

```rust
#[tauri::command]
pub fn update_task(
    state: tauri::State<AppState>,
    id: i64,
    input: TaskInput,
    change: Option<StatusChange>,
) -> AppResult<Task> {
    with_conn(&state, |c| update_impl(c, id, &input, change.as_ref()))
}
#[tauri::command]
pub fn set_task_status(
    state: tauri::State<AppState>,
    id: i64,
    change: StatusChange,
) -> AppResult<Task> {
    with_conn(&state, |c| set_status_impl(c, id, &change))
}
```

- [ ] **Step 5: 修复其他调用点**

`update_impl` 在 `commands/zentao_import.rs` 或别处可能被调用。执行：

```bash
cd src-tauri && grep -rn "update_impl\|set_status_impl" src/
```

对每个调用点补 `None` 第四参数或改用 `StatusChange`。文件内已有的测试 `update_task_can_clear_module_to_null` 也要补 `None`。

- [ ] **Step 6: 运行测试确认通过**

```bash
cd src-tauri && cargo test
```

预期：全部 PASS。若 `zentao_import` 的测试因缺少事件而失败，检查它是否走 `create_impl`——走了就应自动获得 seeded 事件。

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/commands/tasks.rs
git commit -m "feat(tasks): 状态机校验与流转事件落库

新增 paused 及合法转换表，所有状态变更收敛到 apply_transition，
与字段更新同事务写入 task_events。update 路径不再应用
TaskInput.status——保留两条改状态的路径而其中一条不写事件，
正是时间线失真的成因。create 时按源日期补齐初始事件，使禅道
导入的任务与迁移回填的老任务时间线一致。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: task_events 命令模块

**Files:**
- Create: `src-tauri/src/commands/task_events.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs:99-104` 附近
- Test: `src-tauri/src/commands/task_events.rs`（文件内 `mod tests`）

**Interfaces:**
- Consumes: Task 2 的 `pub(crate) fn normalized_body(Option<&str>) -> AppResult<Option<String>>`。
- Produces: 命令 `list_task_events(taskId)` → `Vec<TaskEvent>`、`create_task_note(taskId, body, occurredAt?)` → `TaskEvent`、`update_task_note(id, body)` → `TaskEvent`、`delete_task_note(id)` → `()`。
  `TaskEvent` 序列化字段：`id, task_id, kind, from_status, to_status, body, occurred_at, created_at`。

- [ ] **Step 1: 写失败的测试**

创建 `src-tauri/src/commands/task_events.rs`，先只写测试模块与用到的签名（下一步补实现）。完整文件在 Step 3 给出，这里先写测试内容以便对照：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use crate::commands::tasks::{create_impl as create_task_impl, TaskInput};
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            create_task_impl(&conn, 1, &task_input("T")).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn task_input(title: &str) -> TaskInput {
        TaskInput {
            title: title.into(),
            description: None,
            assignee_id: None,
            status: None,
            estimated_hours: None,
            due_date: None,
            started_at: None,
            completed_at: None,
            module_id: None,
            external_ref: None,
            created_at: None,
        }
    }

    #[test]
    fn create_note_persists_and_lists() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "客户说下周再定", None).unwrap();
        assert_eq!(e.kind, "note");
        assert_eq!(e.body.as_deref(), Some("客户说下周再定"));
        let all = list_impl(&db.conn, 1).unwrap();
        assert!(all.iter().any(|x| x.id == e.id));
    }

    #[test]
    fn create_note_rejects_blank_body() {
        let db = TestDb::new();
        let err = create_note_impl(&db.conn, 1, "   ", None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn create_note_rejects_oversized_body() {
        let db = TestDb::new();
        let long = "字".repeat(2001);
        let err = create_note_impl(&db.conn, 1, &long, None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn create_note_on_missing_task_rejected() {
        let db = TestDb::new();
        let err = create_note_impl(&db.conn, 999, "x", None).unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[test]
    fn list_returns_events_in_occurred_order() {
        let db = TestDb::new();
        create_note_impl(&db.conn, 1, "第二条", Some("2026-03-02 10:00:00")).unwrap();
        create_note_impl(&db.conn, 1, "第一条", Some("2026-03-01 10:00:00")).unwrap();
        let all = list_impl(&db.conn, 1).unwrap();
        let notes: Vec<&str> = all
            .iter()
            .filter(|e| e.kind == "note")
            .map(|e| e.body.as_deref().unwrap())
            .collect();
        assert_eq!(notes, vec!["第一条", "第二条"]);
    }

    #[test]
    fn update_note_changes_body() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "旧的", None).unwrap();
        let u = update_note_impl(&db.conn, e.id, "新的").unwrap();
        assert_eq!(u.body.as_deref(), Some("新的"));
    }

    #[test]
    fn update_status_event_rejected() {
        // The seeded creation event is a status_change; it is a record, not content.
        let db = TestDb::new();
        let all = list_impl(&db.conn, 1).unwrap();
        let seeded = all.iter().find(|e| e.kind == "status_change").unwrap();
        let err = update_note_impl(&db.conn, seeded.id, "改不了").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_status_event_rejected() {
        let db = TestDb::new();
        let all = list_impl(&db.conn, 1).unwrap();
        let seeded = all.iter().find(|e| e.kind == "status_change").unwrap();
        let err = delete_note_impl(&db.conn, seeded.id).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_note_hides_it_from_list() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "删掉我", None).unwrap();
        delete_note_impl(&db.conn, e.id).unwrap();
        let all = list_impl(&db.conn, 1).unwrap();
        assert!(!all.iter().any(|x| x.id == e.id));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib commands::task_events
```

预期：编译失败（模块未注册、`list_impl` 等未定义）。

- [ ] **Step 3: 写实现**

把下面的内容放在 `src-tauri/src/commands/task_events.rs` 的测试模块**之前**：

```rust
use crate::commands::tasks::normalized_body;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    pub id: i64,
    pub task_id: i64,
    pub kind: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub body: Option<String>,
    pub occurred_at: String,
    pub created_at: String,
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<TaskEvent> {
    Ok(TaskEvent {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        kind: row.get("kind")?,
        from_status: row.get("from_status")?,
        to_status: row.get("to_status")?,
        body: row.get("body")?,
        occurred_at: row.get("occurred_at")?,
        created_at: row.get("created_at")?,
    })
}

fn get_impl(conn: &Connection, id: i64) -> AppResult<TaskEvent> {
    conn.query_row(
        "SELECT * FROM task_events WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_event,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "task_event",
            id,
        },
        other => AppError::Db(other),
    })
}

/// Notes are editable content; status_change rows are a record of what happened
/// and must stay immutable, so every mutation checks the kind first.
fn ensure_note(conn: &Connection, id: i64) -> AppResult<()> {
    let kind = get_impl(conn, id)?.kind;
    if kind != "note" {
        return Err(AppError::Validation("状态变更记录不可修改或删除".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, task_id: i64) -> AppResult<Vec<TaskEvent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM task_events
         WHERE task_id = ?1 AND deleted_at IS NULL
         ORDER BY occurred_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([task_id], row_to_event)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn create_note_impl(
    conn: &Connection,
    task_id: i64,
    body: &str,
    occurred_at: Option<&str>,
) -> AppResult<TaskEvent> {
    let body = normalized_body(Some(body))?
        .ok_or_else(|| AppError::Validation("备注内容不能为空".into()))?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        [task_id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound {
            entity: "task",
            id: task_id,
        });
    }
    conn.execute(
        "INSERT INTO task_events(task_id, kind, body, occurred_at)
         VALUES(?1, 'note', ?2, COALESCE(?3, datetime('now')))",
        rusqlite::params![task_id, body, occurred_at],
    )?;
    get_impl(conn, conn.last_insert_rowid())
}

pub(crate) fn update_note_impl(conn: &Connection, id: i64, body: &str) -> AppResult<TaskEvent> {
    ensure_note(conn, id)?;
    let body = normalized_body(Some(body))?
        .ok_or_else(|| AppError::Validation("备注内容不能为空".into()))?;
    conn.execute(
        "UPDATE task_events SET body = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![body, id],
    )?;
    get_impl(conn, id)
}

pub(crate) fn delete_note_impl(conn: &Connection, id: i64) -> AppResult<()> {
    ensure_note(conn, id)?;
    conn.execute(
        "UPDATE task_events SET deleted_at = datetime('now')
         WHERE id = ?1 AND deleted_at IS NULL",
        [id],
    )?;
    Ok(())
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn list_task_events(state: tauri::State<AppState>, task_id: i64) -> AppResult<Vec<TaskEvent>> {
    with_conn(&state, |c| list_impl(c, task_id))
}
#[tauri::command]
pub fn create_task_note(
    state: tauri::State<AppState>,
    task_id: i64,
    body: String,
    occurred_at: Option<String>,
) -> AppResult<TaskEvent> {
    with_conn(&state, |c| {
        create_note_impl(c, task_id, &body, occurred_at.as_deref())
    })
}
#[tauri::command]
pub fn update_task_note(
    state: tauri::State<AppState>,
    id: i64,
    body: String,
) -> AppResult<TaskEvent> {
    with_conn(&state, |c| update_note_impl(c, id, &body))
}
#[tauri::command]
pub fn delete_task_note(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_note_impl(c, id))
}
```

- [ ] **Step 4: 注册模块与命令**

`src-tauri/src/commands/mod.rs` 在 `pub mod tasks;` 之前按字母序插入：

```rust
pub mod task_events;
```

`src-tauri/src/lib.rs` 在 `commands::tasks::delete_task,` 之后插入：

```rust
            commands::task_events::list_task_events,
            commands::task_events::create_task_note,
            commands::task_events::update_task_note,
            commands::task_events::delete_task_note,
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-tauri && cargo test
```

预期：全部 PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/commands/task_events.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(tasks): 任务事件查询与备注增删改命令

备注可改可删，status_change 记录不可变——它是发生过什么的记录，
不是内容。两者共用 normalized_body 保证长度口径一致。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: 软删级联与回收站硬删

**Files:**
- Modify: `src-tauri/src/domain/soft_delete.rs`（`soft_delete_task`、`restore_task`、`soft_delete_project`、`restore_project`）、`src-tauri/src/commands/trash.rs:145-180`
- Test: `src-tauri/src/commands/trash.rs`（文件内 `mod tests`）

**Interfaces:**
- Consumes: Task 1 的 `task_events` 表。
- Produces: 无新签名，仅行为变更。

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/commands/trash.rs` 的 `mod tests` 中追加。现有测试已有插入 task 与 time_log 的模式，沿用它。

```rust
    #[test]
    fn soft_delete_task_cascades_to_events() {
        let db = TestDb::new();
        db.conn
            .execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", [])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO task_events(task_id, kind, body, occurred_at)
                 VALUES(1, 'note', '备注', datetime('now'))",
                [],
            )
            .unwrap();
        soft_delete::soft_delete_task(&db.conn, 1).unwrap();
        let alive: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = 1 AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(alive, 0);
        soft_delete::restore_task(&db.conn, 1).unwrap();
        let back: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = 1 AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(back, 1);
    }

    #[test]
    fn purge_task_hard_deletes_events() {
        let db = TestDb::new();
        db.conn
            .execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", [])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO task_events(task_id, kind, body, occurred_at)
                 VALUES(1, 'note', '备注', datetime('now'))",
                [],
            )
            .unwrap();
        soft_delete::soft_delete_task(&db.conn, 1).unwrap();
        purge_impl(&db.conn, "task", 1).unwrap();
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM task_events WHERE task_id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn purge_project_hard_deletes_task_events() {
        let db = TestDb::new();
        db.conn
            .execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", [])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO task_events(task_id, kind, body, occurred_at)
                 VALUES(1, 'note', '备注', datetime('now'))",
                [],
            )
            .unwrap();
        soft_delete::soft_delete_project(&db.conn, 1).unwrap();
        purge_impl(&db.conn, "project", 1).unwrap();
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM task_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
```

若 `mod tests` 顶部未 `use` 到 `soft_delete`，补 `use crate::domain::soft_delete;`。

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib commands::trash
```

预期：三个新测试失败（事件仍然存活 / 未被硬删）。

- [ ] **Step 3: 实现级联**

`src-tauri/src/domain/soft_delete.rs`：

`soft_delete_task` 中，在 `time_logs` 那条 execute 之后插入：

```rust
    tx.execute(
        "UPDATE task_events SET deleted_at = ?1
         WHERE task_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
```

`restore_task` 中，在 `time_logs` 还原那条之后插入：

```rust
    tx.execute(
        "UPDATE task_events SET deleted_at = NULL
         WHERE task_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
```

`soft_delete_project` 中，在 `time_logs` 那条之后、`tasks` 那条**之前**插入（`tasks` 打标后子查询就选不到了）：

```rust
    tx.execute(
        "UPDATE task_events SET deleted_at = ?1
         WHERE deleted_at IS NULL
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?2 AND deleted_at IS NULL)",
        rusqlite::params![ts, id],
    )?;
```

`restore_project` 中，在 `time_logs` 还原那条之后插入：

```rust
    tx.execute(
        "UPDATE task_events SET deleted_at = NULL
         WHERE deleted_at = ?2
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
        rusqlite::params![id, ts],
    )?;
```

- [ ] **Step 4: 实现硬删**

`src-tauri/src/commands/trash.rs` 的 `purge_impl`：

`entity_type == "project"` 分支里，在 `DELETE FROM time_logs` **之前**插入：

```rust
        tx.execute(
            "DELETE FROM task_events
             WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            [id],
        )?;
```

`entity_type == "task"` 分支里，在 `DELETE FROM time_logs WHERE task_id = ?1` 之前插入：

```rust
        tx.execute("DELETE FROM task_events WHERE task_id = ?1", [id])?;
```

- [ ] **Step 5: 运行测试确认通过**

```bash
cd src-tauri && cargo test
```

预期：全部 PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/domain/soft_delete.rs src-tauri/src/commands/trash.rs
git commit -m "feat(trash): task_events 跟随任务与项目软删及硬删

备注有 deleted_at 的唯一理由是删除任务后还原时它要跟着回来，
因此不进回收站列表，只做级联。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Dashboard 后端 — 暂停不计逾期

**Files:**
- Modify: `src-tauri/src/domain/dashboard.rs:417-446`
- Test: `src-tauri/src/domain/dashboard.rs`（文件内 `mod tests`）

**Interfaces:**
- Consumes: Task 1 的 `paused` 状态。
- Produces: `DashTaskRow.overdue` 对 `paused` 恒为 `false`；`todo_tasks` 排序为 活跃 → 暂停 → 已完成。

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/domain/dashboard.rs` 的 `mod tests`（`:467`）中追加。入口函数是 `company_dashboard(conn, company_id, today)`，测试沿用 `empty_company_all_zero`（`:610`）的建库方式。

```rust
    #[test]
    fn paused_task_past_due_is_not_overdue() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title, status, due_date)
             VALUES(1, '被卡住的任务', 'paused', '2020-01-01')",
            [],
        )
        .unwrap();
        let d = company_dashboard(&conn, 1, "2026-07-04").unwrap();
        let row = d.todo_tasks.iter().find(|r| r.title == "被卡住的任务").unwrap();
        assert!(!row.overdue, "暂停的任务在等外部依赖，不该算逾期");
    }

    #[test]
    fn paused_tasks_sort_after_active_and_before_done() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        for (title, status) in [("A活跃", "in_progress"), ("B暂停", "paused"), ("C完成", "done")] {
            conn.execute(
                "INSERT INTO tasks(project_id, title, status) VALUES(1, ?1, ?2)",
                rusqlite::params![title, status],
            )
            .unwrap();
        }
        let d = company_dashboard(&conn, 1, "2026-07-04").unwrap();
        let titles: Vec<&str> = d.todo_tasks.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["A活跃", "B暂停", "C完成"]);
    }
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd src-tauri && cargo test --lib domain::dashboard
```

预期：两个新测试失败。

- [ ] **Step 3: 改查询**

`src-tauri/src/domain/dashboard.rs`：

排序子句（约 `:440`）

```sql
         ORDER BY (t.status = 'done'), (t.due_date IS NULL), t.due_date ASC, t.id ASC",
```

改为

```sql
         ORDER BY (t.status = 'done'), (t.status = 'paused'),
                  (t.due_date IS NULL), t.due_date ASC, t.id ASC",
```

逾期判定（约 `:445`）

```rust
        let overdue = status != "done" && due_date.as_deref().is_some_and(|d| d < today);
```

改为

```rust
        // A paused task is blocked on something external; flagging it overdue
        // blames the wrong party.
        let overdue = status != "done"
            && status != "paused"
            && due_date.as_deref().is_some_and(|d| d < today);
```

同时更新 `:417-421` 那段注释，说明暂停任务仍进待办列表但排在活跃任务之后、不计逾期。

- [ ] **Step 4: 运行测试确认通过**

```bash
cd src-tauri && cargo test
```

预期：全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/dashboard.rs
git commit -m "fix(dashboard): 暂停任务不计逾期并排在活跃任务之后

任务停着是在等外部依赖，标成逾期是归因错误。暂停仍属待办，
只是优先级低于活跃任务。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: 前端类型与 store

**Files:**
- Create: `src/stores/taskEvents.ts`
- Modify: `src/types/index.ts:177-206`、`src/stores/tasks.ts`、`src/stores/auth.ts:56-68`、`src/routes/projects/detail.tsx`（`setStatus` / `update` 调用点）、`src/routes/dashboard.tsx`（同上）
- Test: 无（前端无测试框架），门槛为 `pnpm build`

**Interfaces:**
- Consumes: Task 2、Task 3 的命令。
- Produces:
  - `TaskEvent`、`StatusChange`（`src/types/index.ts`）
  - `useTaskEventsStore`：`byTask`、`loadFor(taskId)`、`createNote(taskId, body, occurredAt?)`、`updateNote(id, body, taskId)`、`deleteNote(id, taskId)`、`reset()`
  - `useTasksStore.update(id, input, projectId, change?)`、`useTasksStore.setStatus(id, change, projectId)`

- [ ] **Step 1: 加类型**

`src/types/index.ts`，在 `TaskInput` 之后插入：

```ts
export type TaskEventKind = "note" | "status_change";

export interface TaskEvent {
  id: number;
  task_id: number;
  kind: TaskEventKind;
  from_status: string | null;
  to_status: string | null;
  body: string | null;
  occurred_at: string;
  created_at: string;
}

// Carries a status transition to the backend. `occurred_at` lets the user
// backdate when the change actually happened; `body` is the note attached to
// it (mandatory when `to` is "paused").
export interface StatusChange {
  to: string;
  occurred_at?: string | null;
  body?: string | null;
}
```

- [ ] **Step 2: 写事件 store**

创建 `src/stores/taskEvents.ts`：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TaskEvent } from "@/types";

interface S {
  byTask: Record<number, TaskEvent[]>;
  loadFor: (taskId: number) => Promise<void>;
  createNote: (taskId: number, body: string, occurredAt?: string | null) => Promise<void>;
  updateNote: (id: number, body: string, taskId: number) => Promise<void>;
  deleteNote: (id: number, taskId: number) => Promise<void>;
  reset: () => void;
}

export const useTaskEventsStore = create<S>((set, get) => ({
  byTask: {},
  async loadFor(taskId) {
    const list = await call<TaskEvent[]>("list_task_events", { taskId });
    set({ byTask: { ...get().byTask, [taskId]: list } });
  },
  async createNote(taskId, body, occurredAt = null) {
    await call<TaskEvent>("create_task_note", { taskId, body, occurredAt });
    await get().loadFor(taskId);
  },
  async updateNote(id, body, taskId) {
    await call<TaskEvent>("update_task_note", { id, body });
    await get().loadFor(taskId);
  },
  async deleteNote(id, taskId) {
    await call<void>("delete_task_note", { id });
    await get().loadFor(taskId);
  },
  reset() {
    set({ byTask: {} });
  },
}));
```

- [ ] **Step 3: 改任务 store**

`src/stores/tasks.ts` 整体替换 interface 与两个方法：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { StatusChange, Task, TaskInput } from "@/types";
import { useFinancialStore } from "./financial";
import { useModuleStatsStore } from "./moduleStats";
import { useTaskEventsStore } from "./taskEvents";

interface S {
  byProject: Record<number, Task[]>;
  statusFilter: string | null;
  loadFor: (projectId: number, statusFilter?: string | null) => Promise<void>;
  create: (projectId: number, input: TaskInput) => Promise<Task>;
  update: (id: number, input: TaskInput, projectId: number, change?: StatusChange) => Promise<Task>;
  setStatus: (id: number, change: StatusChange, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

// The timeline is only worth refetching for a task whose detail page is open;
// elsewhere the events have never been loaded and nobody is looking at them.
async function refreshEventsIfLoaded(taskId: number) {
  if (useTaskEventsStore.getState().byTask[taskId]) {
    await useTaskEventsStore.getState().loadFor(taskId);
  }
}
```

`update` 与 `setStatus` 的实现：

```ts
  async update(id, input, projectId, change) {
    const t = await call<Task>("update_task", { id, input, change: change ?? null });
    await get().loadFor(projectId, get().statusFilter);
    await useModuleStatsStore.getState().refresh(projectId);
    await refreshEventsIfLoaded(id);
    return t;
  },
  async setStatus(id, change, projectId) {
    await call<Task>("set_task_status", { id, change });
    await get().loadFor(projectId, get().statusFilter);
    await refreshEventsIfLoaded(id);
  },
```

其余方法保持原样，`reset` 不变。

- [ ] **Step 4: 登出时清空新 store**

`src/stores/auth.ts` 顶部补 `import { useTaskEventsStore } from "./taskEvents";`，在 `useTimelogsStore.getState().reset();` 之后插入：

```ts
    useTaskEventsStore.getState().reset();
```

- [ ] **Step 5: 修调用点**

```bash
grep -rn "setStatus(" src/routes src/components
```

把每处 `setStatus(<id>, "closed", <projectId>)` 改为 `setStatus(<id>, { to: "closed" }, <projectId>)`。已知两处：
- `src/routes/projects/detail.tsx` 约 `:985`
- `src/routes/dashboard.tsx` 约 `:311`

`update(...)` 的调用点暂不传 `change`（第四参可选），Task 8 再补。

- [ ] **Step 6: 构建确认通过**

```bash
pnpm build && pnpm lint
```

预期：无 TypeScript 错误，无 lint 错误。

- [ ] **Step 7: 提交**

```bash
git add src/types/index.ts src/stores/taskEvents.ts src/stores/tasks.ts src/stores/auth.ts src/routes/projects/detail.tsx src/routes/dashboard.tsx
git commit -m "feat(stores): 任务事件 store 与状态流转签名调整

setStatus 与 update 改为携带 StatusChange，使前端调用与后端的
单一流转入口对齐。事件只在详情页已加载该任务时刷新，避免列表
页产生无人查看的查询。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: 拆分 TasksPanel（纯搬运，零行为变化）

**Files:**
- Create: `src/routes/projects/tasks/panel.tsx`、`src/components/tasks/TimeLogForm.tsx`
- Modify: `src/routes/projects/detail.tsx`
- Test: 无，门槛为 `pnpm build` + 手工点检

**Interfaces:**
- Consumes: 无
- Produces:
  - `src/routes/projects/tasks/panel.tsx` 默认导出 `TasksPanel`，props 为 `{ projectId: number; companyId: number }`
  - 同文件具名导出 `TaskForm`、`TimeLogsSection`、`toDatetimeLocal`、`fromDatetimeLocal`
  - `src/components/tasks/TimeLogForm.tsx` 默认导出 `TimeLogForm`，props 沿用现有定义（`{ taskId, members, initial?, onSubmit, onCancel }`，以现有代码为准）

- [ ] **Step 1: 搬运**

从 `src/routes/projects/detail.tsx` 剪切以下内容到 `src/routes/projects/tasks/panel.tsx`：

- `// ─── Tasks + TimeLogs ───` 注释之后的全部内容，直到 `TimeLogForm` 之前
- `toDatetimeLocal` / `fromDatetimeLocal` 两个工具函数
- `TaskForm`
- `TimeLogsSection`
- `ManageModulesForm`（`detail.tsx:1531` 定义，唯一调用点在 `:1077` 的任务面板内，一并搬走）

`TimeLogForm` 单独剪到 `src/components/tasks/TimeLogForm.tsx`，因为 Task 10 的详情页要单独用它。

补齐两个新文件的 import（从 `detail.tsx` 的 import 块里挑，不要整块复制）。`TasksPanel` 改为默认导出。

**这一步不改任何行为**——不动逻辑、不动 JSX、不重命名。唯一允许的改动是 import 路径和导出方式。

- [ ] **Step 2: 在 detail.tsx 中引用**

`src/routes/projects/detail.tsx` 顶部加：

```tsx
import TasksPanel from "@/routes/projects/tasks/panel";
```

删掉随搬运一起失效的 import（`Play`、`CheckCircle`、`Archive`、`Clock`、`useTasksStore`、`useTimelogsStore`、`useModulesStore`、`StatusTransitionDialog`、`ZentaoImportDialog` 等）。让 `pnpm lint` 告诉你哪些没用了。

- [ ] **Step 3: 构建确认通过**

```bash
pnpm build && pnpm lint
```

预期：无错误。`detail.tsx` 行数应从 1634 降到 700 上下。

- [ ] **Step 4: 手工点检**

```bash
pnpm tauri dev
```

打开一个项目 → 任务+工时 tab，确认：任务列表渲染正常、新建任务、编辑任务、录入工时、开始/完成、状态筛选、分页、禅道导入入口全部与改动前一致。

- [ ] **Step 5: 提交**

```bash
git add src/routes/projects/tasks/panel.tsx src/components/tasks/TimeLogForm.tsx src/routes/projects/detail.tsx
git commit -m "refactor(projects): 任务面板从项目详情页拆出

detail.tsx 已达 1634 行且任务部分占近半。纯搬运，无行为变化，
为后续接入任务详情页腾出边界。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: 暂停与恢复动作

**Files:**
- Modify: `src/components/tasks/StatusTransitionDialog.tsx`、`src/routes/projects/tasks/panel.tsx`、`src/routes/dashboard.tsx`、`src/i18n/zh-CN.json`
- Test: 无，门槛为 `pnpm build` + 手工点检

**Interfaces:**
- Consumes: Task 6 的 `useTasksStore.update(id, input, projectId, change)` 与 `StatusChange`。
- Produces:
  - `StatusTransitionDialog` 的 props 变为 `{ task, mode, existingHours?, onSubmit, onCancel }`，其中 `mode: "start" | "complete" | "pause" | "resume"`。
  - `onSubmit(input: TaskInput, change: StatusChange)` — 两个参数，调用方直接转发给 store。
  - 具名导出 `TASK_STATUS_BADGE_CLASS` 保持不变，新增 `paused` 键。

- [ ] **Step 1: 加 i18n 文案**

`src/i18n/zh-CN.json`：

`taskStatus` 对象加一个键（放在 `in_progress` 之后）：

```json
    "paused": "已暂停",
```

`task` 对象追加：

```json
    "pause": "暂停",
    "resume": "恢复",
    "pauseTitle": "暂停任务",
    "resumeTitle": "恢复任务",
    "pausedAt": "暂停时间",
    "resumedAt": "恢复时间",
    "pauseReason": "暂停原因",
    "pauseReasonRequired": "暂停任务必须填写原因",
    "pauseReasonPlaceholder": "在等什么？例如：等客户确认设计稿",
    "resumeNote": "备注（选填）",
    "pausedFor": "已暂停 {{days}} 天",
```

- [ ] **Step 2: 改造 StatusTransitionDialog**

`src/components/tasks/StatusTransitionDialog.tsx` 整体替换为：

```tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { DialogFooter } from "@/components/ui/dialog";
import { nowDatetimeLocal } from "@/lib/time";
import type { StatusChange, Task, TaskInput } from "@/types";

export const TASK_STATUS_BADGE_CLASS: Record<string, string> = {
  todo: "bg-slate-100 text-slate-700",
  in_progress: "bg-amber-100 text-amber-700",
  paused: "bg-rose-100 text-rose-700",
  done: "bg-emerald-100 text-emerald-700",
  closed: "bg-zinc-200 text-zinc-500",
};

export type TransitionMode = "start" | "complete" | "pause" | "resume";

const TARGET_STATUS: Record<TransitionMode, string> = {
  start: "in_progress",
  complete: "done",
  pause: "paused",
  resume: "in_progress",
};

// Shared transition dialog for a task, used by the project task panel, the task
// detail page and the dashboard todo card so all three behave identically:
// pick a timestamp, optionally log hours (on complete), attach a note.
export function StatusTransitionDialog({
  task, mode, existingHours, onSubmit, onCancel,
}: {
  task: Task;
  mode: TransitionMode;
  existingHours?: number;
  onSubmit: (input: TaskInput & { hours?: number }, change: StatusChange) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const showHours = mode === "complete";
  // Pause and resume do not own a column on tasks; only start/complete write
  // back to started_at / completed_at.
  const dateField = mode === "start" ? "started_at" : mode === "complete" ? "completed_at" : null;

  const [datetime, setDatetime] = useState(
    dateField && task[dateField]
      ? task[dateField]!.replace(" ", "T").slice(0, 16)
      : nowDatetimeLocal()
  );
  const [startedAt, setStartedAt] = useState(
    task.started_at ? task.started_at.replace(" ", "T").slice(0, 16) : ""
  );
  const [description, setDescription] = useState(task.description ?? "");
  const [note, setNote] = useState("");
  const [hours, setHours] = useState(0);
  const [busy, setBusy] = useState(false);

  const dateLabel =
    mode === "start" ? t("task.startedAt")
    : mode === "complete" ? t("task.completedAt")
    : mode === "pause" ? t("task.pausedAt")
    : t("task.resumedAt");

  const handleSubmit = async () => {
    if (mode === "pause" && !note.trim()) {
      toast.error(t("task.pauseReasonRequired"));
      return;
    }
    if (showHours) {
      // Completing a task must attribute its hours to someone, and the task's
      // total actual hours (already logged + this session) must end up > 0.
      if (task.assignee_id == null) {
        toast.error("请先为任务指定负责人，再完成任务");
        return;
      }
      if ((existingHours ?? 0) + hours <= 0) {
        toast.error("完成任务时总工时须大于 0（已有工时 + 本次）");
        return;
      }
    }
    setBusy(true);
    try {
      const stored = datetime ? datetime.replace("T", " ") : null;
      const storedStartedAt = showHours && startedAt ? startedAt.replace("T", " ") : null;
      const input: TaskInput & { hours?: number } = {
        title: task.title,
        description: description.trim() || null,
        assignee_id: task.assignee_id,
        estimated_hours: task.estimated_hours,
        due_date: task.due_date,
        started_at: storedStartedAt ?? task.started_at,
        completed_at: task.completed_at,
        module_id: task.module_id,
        external_ref: task.external_ref,
      };
      if (dateField) input[dateField] = stored;
      if (showHours && hours > 0) input.hours = hours;
      const change: StatusChange = {
        to: TARGET_STATUS[mode],
        occurred_at: stored,
        body: note.trim() || null,
      };
      await onSubmit(input, change);
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="text-sm text-muted-foreground">{task.title}</div>
      {showHours && (
        <div className="space-y-1">
          <Label>{t("task.startedAt")}</Label>
          <Input
            type="datetime-local"
            value={startedAt}
            onChange={(e) => setStartedAt(e.target.value)}
          />
        </div>
      )}
      <div className="space-y-1">
        <Label>{dateLabel}</Label>
        <Input
          type="datetime-local"
          value={datetime}
          onChange={(e) => setDatetime(e.target.value)}
        />
      </div>
      {showHours && (
        <div className="space-y-1">
          <Label>本次工时 (h)<span className="text-muted-foreground font-normal"> — 已有工时 {(existingHours ?? 0)}h</span></Label>
          <Input
            autoFocus
            type="number"
            inputMode="decimal"
            min="0"
            max="24"
            step="0.25"
            value={hours === 0 ? "" : String(hours)}
            placeholder="0"
            onChange={(e) => {
              const n = Number(e.target.value);
              if (Number.isFinite(n) && n >= 0 && n <= 24) setHours(n);
            }}
          />
        </div>
      )}
      {mode === "pause" && (
        <div className="space-y-1">
          <Label>{t("task.pauseReason")}</Label>
          <Textarea
            autoFocus
            rows={3}
            value={note}
            placeholder={t("task.pauseReasonPlaceholder")}
            onChange={(e) => setNote(e.target.value)}
          />
        </div>
      )}
      {mode === "resume" && (
        <div className="space-y-1">
          <Label>{t("task.resumeNote")}</Label>
          <Textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
        </div>
      )}
      {mode !== "pause" && mode !== "resume" && (
        <div className="space-y-1">
          <Label>{t("task.description")}</Label>
          <Textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
        </div>
      )}
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={handleSubmit} disabled={busy}>确定</Button>
      </DialogFooter>
    </div>
  );
}
```

- [ ] **Step 3: 接入任务面板**

`src/routes/projects/tasks/panel.tsx`：

顶部 import 加 `Pause`、`PlayCircle`：

```tsx
import { Play, Pause, PlayCircle, CheckCircle, Archive, Clock } from "lucide-react";
```

（保留文件里原有的其它 lucide 图标名。）

state 加两个：

```tsx
  const [pausingTask, setPausingTask] = useState<Task | null>(null);
  const [resumingTask, setResumingTask] = useState<Task | null>(null);
```

操作列在「开始」按钮之后、「完成」按钮之前插入：

```tsx
                        {tk.status === "in_progress" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title={t("task.pause")}
                            onClick={() => setPausingTask(tk)}
                          ><Pause className="h-4 w-4" /></Button>
                        )}
                        {tk.status === "paused" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 px-2"
                            title={t("task.resume")}
                            onClick={() => setResumingTask(tk)}
                          ><PlayCircle className="h-4 w-4" /></Button>
                        )}
```

状态筛选下拉在 `in_progress` 之后插入：

```tsx
              <SelectItem value="paused">{t("taskStatus.paused")}</SelectItem>
```

现有的开始/完成两个 Dialog 改成新的两参回调，并新增暂停/恢复两个 Dialog。四个都放在组件末尾：

```tsx
      <Dialog open={!!startingTask} onOpenChange={(o) => !o && setStartingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>开始任务</DialogTitle></DialogHeader>
          {startingTask && (
            <StatusTransitionDialog
              task={startingTask}
              mode="start"
              onSubmit={async (input, change) => {
                try { await update(startingTask.id, input, projectId, change); setStartingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setStartingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!pausingTask} onOpenChange={(o) => !o && setPausingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.pauseTitle")}</DialogTitle></DialogHeader>
          {pausingTask && (
            <StatusTransitionDialog
              task={pausingTask}
              mode="pause"
              onSubmit={async (_input, change) => {
                try { await setStatus(pausingTask.id, change, projectId); setPausingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setPausingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!resumingTask} onOpenChange={(o) => !o && setResumingTask(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.resumeTitle")}</DialogTitle></DialogHeader>
          {resumingTask && (
            <StatusTransitionDialog
              task={resumingTask}
              mode="resume"
              onSubmit={async (_input, change) => {
                try { await setStatus(resumingTask.id, change, projectId); setResumingTask(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
              onCancel={() => setResumingTask(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>
```

完成任务的 Dialog 同样改成两参回调，保留原有的建工时逻辑：

```tsx
              onSubmit={async (input, change) => {
                try {
                  await update(completingTask.id, input, projectId, change);
                  const h = input.hours;
                  if (typeof h === "number" && h > 0 && completingTask.assignee_id != null) {
                    await createTimelog({
                      task_id: completingTask.id,
                      member_id: completingTask.assignee_id,
                      work_date: new Date().toISOString().slice(0, 10),
                      hours: h,
                    }, projectId);
                  }
                  setCompletingTask(null);
                }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
```

「暂停」的动作走 `setStatus` 而非 `update`——它不改任何字段，只是状态加原因。

- [ ] **Step 4: 接入 Dashboard**

`src/routes/dashboard.tsx`：

`LEDGER_STATUS_DOT`（`:121`）加一项，用 `VERMILION` 让暂停在账本配色里可见：

```tsx
  paused: VERMILION,
```

`TodoTasksCard` 的 props 加 `onPause` 与 `onResume`，签名同 `onStart`。状态筛选下拉在 `in_progress` 之后加：

```tsx
              <SelectItem value="paused">{t("taskStatus.paused")}</SelectItem>
```

操作列在「开始」之后插入：

```tsx
                {r.status === "in_progress" && (
                  <Button size="sm" variant="ghost" className="h-7 px-2" title={t("task.pause")} style={{ color: INK_SOFT }} onClick={() => onPause(r)}>
                    <Pause className="h-4 w-4" />
                  </Button>
                )}
                {r.status === "paused" && (
                  <Button size="sm" variant="ghost" className="h-7 px-2" title={t("task.resume")} style={{ color: INK_SOFT }} onClick={() => onResume(r)}>
                    <PlayCircle className="h-4 w-4" />
                  </Button>
                )}
```

`import { RefreshCw, Play, Pause, PlayCircle, CheckCircle, Archive } from "lucide-react";`

页面组件里把 `openTaskAction` 扩成四种：

```tsx
  const [pausingTask, setPausingTask] = useState<Task | null>(null);
  const [resumingTask, setResumingTask] = useState<Task | null>(null);

  const openTaskAction = async (
    row: DashTaskRow,
    kind: "start" | "complete" | "pause" | "resume",
  ) => {
    try {
      const task = await call<Task>("get_task", { id: row.task_id });
      if (kind === "start") setStartingTask(task);
      else if (kind === "complete") setCompletingTask(task);
      else if (kind === "pause") setPausingTask(task);
      else setResumingTask(task);
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    }
  };
```

`<TodoTasksCard ... onPause={(r) => openTaskAction(r, "pause")} onResume={(r) => openTaskAction(r, "resume")} />`

`closeTask` 改为 `await setTaskStatus(row.task_id, { to: "closed" }, row.project_id);`（Task 6 已改，此处确认即可）。

现有的两个 `StatusTransitionDialog`（`:507`、`:528`）改成 `mode` prop 与两参回调，并新增暂停/恢复两个，成功后 `if (currentId != null) await loadFor(currentId);` 刷新看板。

- [ ] **Step 5: 构建确认通过**

```bash
pnpm build && pnpm lint
```

- [ ] **Step 6: 手工点检**

```bash
pnpm tauri dev
```

- 项目任务列表：进行中的任务出现暂停按钮；点暂停不填原因 → 提示「暂停任务必须填写原因」；填写后状态变「已暂停」，徽章为玫红。
- 暂停的任务出现恢复按钮，点击后回到「进行中」。
- 状态筛选选「已暂停」能筛出来。
- Dashboard 待办卡片同样能暂停/恢复；暂停且过期的任务日期不再是朱红色。

- [ ] **Step 7: 提交**

```bash
git add src/components/tasks/StatusTransitionDialog.tsx src/routes/projects/tasks/panel.tsx src/routes/dashboard.tsx src/i18n/zh-CN.json
git commit -m "feat(tasks): 列表与看板支持暂停和恢复

StatusTransitionDialog 从 fieldKey 改为 mode 驱动，四种流转共用
一个组件，避免列表页与看板出现行为分叉。暂停原因必填。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: 时间线与备注输入组件

**Files:**
- Create: `src/components/tasks/Timeline.tsx`、`src/components/tasks/NoteComposer.tsx`
- Modify: `src/i18n/zh-CN.json`
- Test: 无，门槛为 `pnpm build`

**Interfaces:**
- Consumes: `TaskEvent`（Task 6）、`TimeLog`、`Member`。
- Produces:
  - `Timeline`，props `{ events: TaskEvent[]; logs: TimeLog[]; members: Member[]; onEditNote: (e: TaskEvent) => void; onDeleteNote: (e: TaskEvent) => void; onEditLog: (l: TimeLog) => void; onDeleteLog: (l: TimeLog) => void }`
  - `NoteComposer`，props `{ onSubmit: (body: string) => Promise<void>; initial?: string; submitLabel?: string }`

- [ ] **Step 1: 加 i18n 文案**

`src/i18n/zh-CN.json` 顶层加一个 `timeline` 对象：

```json
  "timeline": {
    "title": "动态",
    "empty": "暂无动态",
    "notePlaceholder": "写点什么…… 例如今天卡在哪儿了",
    "addNote": "添加备注",
    "loggedHours": "记录工时 {{hours}}h",
    "created": "创建任务",
    "changed": "{{from}} → {{to}}",
    "noteDeleteConfirm": "确认删除这条备注？删除后无法从回收站恢复。"
  },
```

- [ ] **Step 2: 写 NoteComposer**

创建 `src/components/tasks/NoteComposer.tsx`：

```tsx
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
```

- [ ] **Step 3: 写 Timeline**

创建 `src/components/tasks/Timeline.tsx`：

```tsx
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
```

- [ ] **Step 4: 构建确认通过**

```bash
pnpm build && pnpm lint
```

预期：无错误。此时两个组件还没有调用方，`pnpm lint` 可能提示未使用的导出——`oxlint` 默认不报导出未使用，若报了则先忽略，Task 10 会接上。

- [ ] **Step 5: 提交**

```bash
git add src/components/tasks/Timeline.tsx src/components/tasks/NoteComposer.tsx src/i18n/zh-CN.json
git commit -m "feat(tasks): 时间线与备注输入组件

事件与工时在渲染层按时间合并成一条叙事。工时不写进 task_events：
它带成本快照且有独立的增删改路径，双写必然不同步。工时按工作日
末尾排序，使其落在同日状态变更之后。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: 任务详情页与路由

**Files:**
- Create: `src/routes/projects/tasks/detail.tsx`
- Modify: `src/App.tsx:44-52`、`src/i18n/zh-CN.json`
- Test: 无，门槛为 `pnpm build` + 手工点检

**Interfaces:**
- Consumes: Task 8 的 `StatusTransitionDialog`（`mode` prop）、Task 9 的 `Timeline` / `NoteComposer`、Task 7 的 `TaskForm` 与 `TimeLogForm`、Task 6 的两个 store。
- Produces: 默认导出 `TaskDetailPage`，无 props，从 `useParams` 取 `projectId` / `taskId`。

- [ ] **Step 1: 让 TaskForm 能脱离对话框**

`src/routes/projects/tasks/panel.tsx` 与 `src/components/ui/form-dialog.tsx`：

`useFormDialog()` 目前在 context 缺失时抛错，详情页上没有 `FormDialogContent` 包裹，会直接崩。改 `src/components/ui/form-dialog.tsx`：

```ts
/** Returns a no-op outside a FormDialogContent, so the same form component can
 *  also render inline on a page. */
export function useFormDialog(): FormDialogContextValue {
  const ctx = React.useContext(FormDialogContext);
  return ctx ?? NO_DIALOG;
}

const NO_DIALOG: FormDialogContextValue = { markDirty: () => {} };
```

把 `NO_DIALOG` 常量定义放在 `useFormDialog` 之前。

`TaskForm` 的 footer 目前用 `DialogFooter`，在页面上也能渲染（它只是个带 flex 的 div），不必改。

- [ ] **Step 2: 加 i18n 文案**

`src/i18n/zh-CN.json` 的 `task` 对象追加：

```json
    "backToList": "返回任务列表",
    "detailTitle": "任务详情",
    "basicInfo": "基本信息",
```

- [ ] **Step 3: 写详情页**

创建 `src/routes/projects/tasks/detail.tsx`：

```tsx
import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { ChevronLeft, Play, Pause, PlayCircle, CheckCircle, Archive, Trash2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { FormDialogContent } from "@/components/ui/form-dialog";
import { call } from "@/lib/ipc";
import { confirmDialog } from "@/lib/confirm";
import { useMembersStore } from "@/stores/members";
import { useModulesStore } from "@/stores/modules";
import { useTasksStore } from "@/stores/tasks";
import { useTaskEventsStore } from "@/stores/taskEvents";
import { useTimelogsStore } from "@/stores/timelogs";
import { StatusTransitionDialog, TASK_STATUS_BADGE_CLASS, type TransitionMode }
  from "@/components/tasks/StatusTransitionDialog";
import { Timeline } from "@/components/tasks/Timeline";
import { NoteComposer } from "@/components/tasks/NoteComposer";
import TimeLogForm from "@/components/tasks/TimeLogForm";
import { TaskForm } from "@/routes/projects/tasks/panel";
import type { Project, StatusChange, Task, TaskEvent, TaskInput, TimeLog } from "@/types";

export default function TaskDetailPage() {
  const { projectId, taskId } = useParams<{ projectId: string; taskId: string }>();
  const pid = Number(projectId);
  const tid = Number(taskId);
  const navigate = useNavigate();
  const { t } = useTranslation();

  const [project, setProject] = useState<Project | null>(null);
  const [task, setTask] = useState<Task | null>(null);
  const [mode, setMode] = useState<TransitionMode | null>(null);
  const [openNewLog, setOpenNewLog] = useState(false);
  const [editingLog, setEditingLog] = useState<TimeLog | null>(null);
  const [editingNote, setEditingNote] = useState<TaskEvent | null>(null);

  const update = useTasksStore((s) => s.update);
  const setStatus = useTasksStore((s) => s.setStatus);
  const softDeleteTask = useTasksStore((s) => s.softDelete);
  const events = useTaskEventsStore((s) => s.byTask[tid] ?? []);
  const loadEvents = useTaskEventsStore((s) => s.loadFor);
  const createNote = useTaskEventsStore((s) => s.createNote);
  const updateNote = useTaskEventsStore((s) => s.updateNote);
  const deleteNote = useTaskEventsStore((s) => s.deleteNote);
  const logs = useTimelogsStore((s) => s.byTask[tid] ?? []);
  const loadLogs = useTimelogsStore((s) => s.loadFor);
  const createLog = useTimelogsStore((s) => s.create);
  const updateLog = useTimelogsStore((s) => s.update);
  const deleteLog = useTimelogsStore((s) => s.softDelete);
  const members = useMembersStore((s) => s.list);
  const loadMembers = useMembersStore((s) => s.loadFor);
  const modules = useModulesStore((s) => s.byProject[pid] ?? []);
  const loadModules = useModulesStore((s) => s.loadFor);

  // Reload the task itself after any mutation: actual_hours and status live on
  // it and the list store does not necessarily hold this project.
  const reloadTask = useMemo(
    () => async () => {
      try {
        setTask(await call<Task>("get_task", { id: tid }));
      } catch (e: unknown) {
        toast.error(t("common.error", { msg: String(e) }));
        navigate(`/projects/${pid}`);
      }
    },
    [tid, pid, navigate, t],
  );

  useEffect(() => {
    if (Number.isNaN(tid) || Number.isNaN(pid)) return;
    call<Project>("get_project", { id: pid }).then(setProject).catch(() => navigate("/projects"));
    reloadTask();
    loadEvents(tid);
    loadLogs(tid);
    loadModules(pid);
  }, [tid, pid, navigate, reloadTask, loadEvents, loadLogs, loadModules]);

  useEffect(() => {
    if (project) loadMembers(project.company_id);
  }, [project, loadMembers]);

  const pausedDays = useMemo(() => {
    if (task?.status !== "paused") return null;
    const last = [...events].reverse().find((e) => e.to_status === "paused");
    if (!last) return null;
    const since = new Date(last.occurred_at.replace(" ", "T")).getTime();
    return { days: Math.floor((Date.now() - since) / 86_400_000), reason: last.body };
  }, [task, events]);

  if (!task || !project) return null;

  const backToList = () => navigate(`/projects/${pid}?task=${tid}`);

  const runTransition = async (input: TaskInput & { hours?: number }, change: StatusChange) => {
    try {
      if (mode === "pause" || mode === "resume") {
        await setStatus(tid, change, pid);
      } else {
        await update(tid, input, pid, change);
        if (mode === "complete" && typeof input.hours === "number" && input.hours > 0
            && task.assignee_id != null) {
          await createLog({
            task_id: tid,
            member_id: task.assignee_id,
            work_date: new Date().toISOString().slice(0, 10),
            hours: input.hours,
          }, pid);
        }
      }
      setMode(null);
      await reloadTask();
      await loadEvents(tid);
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    }
  };

  return (
    <div className="space-y-4">
      <button
        className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        onClick={backToList}
      >
        <ChevronLeft className="h-4 w-4" />
        {project.name} / {t("task.title")}
      </button>

      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-2 min-w-0">
          <h1 className="text-xl font-semibold truncate">{task.title}</h1>
          <Badge variant="secondary" className={`whitespace-nowrap ${TASK_STATUS_BADGE_CLASS[task.status] ?? ""}`}>
            {t(`taskStatus.${task.status}`)}
          </Badge>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {task.status === "todo" && (
            <Button size="sm" variant="outline" onClick={() => setMode("start")}>
              <Play className="h-4 w-4 mr-1" />{t("taskStatus.in_progress")}
            </Button>
          )}
          {task.status === "in_progress" && (
            <Button size="sm" variant="outline" onClick={() => setMode("pause")}>
              <Pause className="h-4 w-4 mr-1" />{t("task.pause")}
            </Button>
          )}
          {task.status === "paused" && (
            <Button size="sm" variant="outline" onClick={() => setMode("resume")}>
              <PlayCircle className="h-4 w-4 mr-1" />{t("task.resume")}
            </Button>
          )}
          {task.status !== "done" && task.status !== "closed" && (
            <Button size="sm" variant="outline" onClick={() => setMode("complete")}>
              <CheckCircle className="h-4 w-4 mr-1" />{t("taskStatus.done")}
            </Button>
          )}
          {task.status === "done" && (
            <Button
              size="sm"
              variant="outline"
              onClick={async () => {
                try { await setStatus(tid, { to: "closed" }, pid); await reloadTask(); await loadEvents(tid); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            ><Archive className="h-4 w-4 mr-1" />{t("taskStatus.closed")}</Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            onClick={async () => {
              const ok = await confirmDialog(t("task.deleteConfirm", { title: task.title }), {
                title: t("task.delete"),
                kind: "warning",
                okLabel: t("task.delete"),
                cancelLabel: t("common.cancel"),
              });
              if (!ok) return;
              try { await softDeleteTask(tid, pid); navigate(`/projects/${pid}`); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          ><Trash2 className="h-4 w-4" /></Button>
        </div>
      </div>

      {pausedDays && (
        <div className="rounded-md border border-rose-200 bg-rose-50 px-4 py-2 text-sm text-rose-800">
          {t("task.pausedFor", { days: pausedDays.days })}
          {pausedDays.reason ? ` · ${pausedDays.reason}` : ""}
        </div>
      )}

      <Card>
        <CardContent className="p-4">
          <TaskForm
            members={members}
            modules={modules}
            initial={task}
            onCancel={backToList}
            onSubmit={async (input) => {
              try { await update(tid, input, pid); await reloadTask(); toast.success(t("task.save")); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          />
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-4 space-y-4">
          <div className="flex items-start gap-3">
            <div className="flex-1"><NoteComposer onSubmit={(body) => createNote(tid, body)} /></div>
            <Button size="sm" variant="outline" onClick={() => setOpenNewLog(true)}>
              <Plus className="h-4 w-4 mr-1" />{t("timelog.add")}
            </Button>
          </div>
          <Timeline
            events={events}
            logs={logs}
            members={members}
            onEditNote={setEditingNote}
            onDeleteNote={async (e) => {
              if (!(await confirmDialog(t("timeline.noteDeleteConfirm"), {
                title: t("common.delete"), kind: "warning",
                okLabel: t("common.delete"), cancelLabel: t("common.cancel"),
              }))) return;
              try { await deleteNote(e.id, tid); }
              catch (err: unknown) { toast.error(t("common.error", { msg: String(err) })); }
            }}
            onEditLog={setEditingLog}
            onDeleteLog={async (l) => {
              if (!(await confirmDialog(t("timelog.deleteConfirm"), {
                title: t("common.delete"), kind: "warning",
                okLabel: t("common.delete"), cancelLabel: t("common.cancel"),
              }))) return;
              try { await deleteLog(l.id, tid, pid); await reloadTask(); }
              catch (err: unknown) { toast.error(t("common.error", { msg: String(err) })); }
            }}
          />
        </CardContent>
      </Card>

      <Dialog open={mode !== null} onOpenChange={(o) => !o && setMode(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.title")}</DialogTitle></DialogHeader>
          {mode && (
            <StatusTransitionDialog
              task={task}
              mode={mode}
              existingHours={task.actual_hours}
              onSubmit={runTransition}
              onCancel={() => setMode(null)}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={openNewLog} onOpenChange={setOpenNewLog}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("timelog.add")}</DialogTitle></DialogHeader>
          <TimeLogForm
            taskId={tid}
            members={members.filter((m) => m.is_active)}
            onCancel={() => setOpenNewLog(false)}
            onSubmit={async (input) => {
              try { await createLog(input, pid); setOpenNewLog(false); await reloadTask(); }
              catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
            }}
          />
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!editingLog} onOpenChange={(o) => !o && setEditingLog(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("timelog.edit")}</DialogTitle></DialogHeader>
          {editingLog && (
            <TimeLogForm
              taskId={tid}
              members={members.filter((m) => m.is_active)}
              initial={editingLog}
              onCancel={() => setEditingLog(null)}
              onSubmit={async (input) => {
                try {
                  await updateLog(editingLog.id, {
                    work_date: input.work_date, hours: input.hours, notes: input.notes ?? null,
                  }, tid, pid);
                  setEditingLog(null);
                  await reloadTask();
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </FormDialogContent>
      </Dialog>

      <Dialog open={!!editingNote} onOpenChange={(o) => !o && setEditingNote(null)}>
        <FormDialogContent>
          <DialogHeader><DialogTitle>{t("task.notes")}</DialogTitle></DialogHeader>
          {editingNote && (
            <NoteComposer
              key={editingNote.id}
              initial={editingNote.body ?? ""}
              submitLabel={t("task.save")}
              onSubmit={async (body) => { await updateNote(editingNote.id, body, tid); setEditingNote(null); }}
            />
          )}
        </FormDialogContent>
      </Dialog>
    </div>
  );
}
```

已核对的 store 契约（照此写，不要改名）：

- `useMembersStore`：成员数组字段名是 **`list`**（不是 `items`），`loadFor(companyId)`，已按「在职优先 + 姓名」排好序。
- `useModulesStore`：`byProject[projectId]`，`loadFor(projectId)`。该 store **没有** `reset()`，不要在 auth 里加。
- `useTimelogsStore.update(id, input, taskId, projectId)` 的 `input` 类型是 `TimeLogUpdateInput = { work_date: string; hours: number; notes?: string | null }`——只有这三个字段。

`TimeLogForm` 的 props 以 Task 7 搬出的原始定义为准，不要臆造。

编辑备注复用 `NoteComposer` 时要带初值与重挂载，见上面 `editingNote` 那个 Dialog 的写法：

```tsx
            <NoteComposer
              key={editingNote.id}
              initial={editingNote.body ?? ""}
              submitLabel={t("task.save")}
              onSubmit={async (body) => { await updateNote(editingNote.id, body, tid); setEditingNote(null); }}
            />
```

- [ ] **Step 4: 挂路由**

`src/App.tsx` 在 `<Route path="projects/:id" ... />` 之后插入：

```tsx
            <Route path="projects/:projectId/tasks/:taskId" element={<TaskDetailPage />} />
```

顶部加 `import TaskDetailPage from "@/routes/projects/tasks/detail";`

- [ ] **Step 5: 构建确认通过**

```bash
pnpm build && pnpm lint
```

- [ ] **Step 6: 手工点检**

```bash
pnpm tauri dev
```

浏览器地址栏直接访问 `/projects/1/tasks/1`（或从列表点进去，Task 11 才接线）。确认：

- 面包屑显示项目名，点击返回项目详情页并停在任务 tab。
- 基本信息表单能保存。
- 备注能添加、编辑、删除；删除有二次确认。
- 记录工时后时间线出现工时条目，实际工时数字更新。
- 开始 → 暂停 → 恢复 → 完成 全链路，每步在时间线新增一条。
- 暂停后横幅显示「已暂停 0 天 · 原因」。

- [ ] **Step 7: 提交**

```bash
git add src/routes/projects/tasks/detail.tsx src/App.tsx src/components/ui/form-dialog.tsx src/i18n/zh-CN.json
git commit -m "feat(tasks): 任务详情页

持续追加备注与翻历史是停留式浏览，与对话框「做完就关」的心智
模型冲突。详情页把编辑、工时、备注、时间线收在一处。
useFormDialog 在缺少 context 时降级为 no-op，使同一份 TaskForm
既能在对话框里也能在页面上渲染。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 11: 接线与旧对话框下线

**Files:**
- Modify: `src/routes/projects/tasks/panel.tsx`、`src/components/search/CommandPalette.tsx:66-72`、`src/routes/dashboard.tsx`
- Test: 无，门槛为 `pnpm build` + 手工点检

**Interfaces:**
- Consumes: Task 10 的路由 `/projects/:projectId/tasks/:taskId`。
- Produces: 无新签名。

- [ ] **Step 1: 列表页改为跳转**

`src/routes/projects/tasks/panel.tsx`：

顶部补 `import { useNavigate } from "react-router-dom";`，组件内 `const navigate = useNavigate();`

任务标题按钮：

```tsx
                        <button
                          className="text-left hover:underline cursor-pointer"
                          onClick={() => navigate(`/projects/${projectId}/tasks/${tk.id}`)}
                        >{tk.title}</button>
```

工时图标按钮的 `onClick` 同样改为该 `navigate`。

删除以下内容：
- `editing` 与 `openLogs` 两个 state
- 「编辑任务」`<Dialog>` 整块
- 「工时」`<Dialog>` 整块
- `TimeLogsSection` 函数定义（它的表格职责已由时间线接管）
- 因此失效的 import（`Clock` 图标若仍用于跳转按钮则保留）

`TaskForm` 的 `onClose` / `onDelete` 两个 props 现在只剩详情页一个调用方，且详情页不传它们——把这两个可选 prop 及其对应的 footer 按钮从 `TaskForm` 中删掉（关闭与删除已迁到详情页标题行）。

- [ ] **Step 2: 全局搜索直达详情页**

`src/components/search/CommandPalette.tsx` 约 `:71`：

```tsx
        : `/projects/${hit.project_id}/tasks/${hit.id}`
```

- [ ] **Step 3: Dashboard 任务标题跳详情页**

`src/routes/dashboard.tsx` 的 `TodoTasksCard`：`onOpen` 目前只收 `projectId`。把任务标题那一格改为跳详情页，项目名那一格保持跳项目。

props 增加：

```tsx
  onOpenTask: (projectId: number, taskId: number) => void;
```

任务标题按钮：

```tsx
                <button className="text-left hover:underline cursor-pointer" onClick={() => onOpenTask(r.project_id, r.task_id)}>
                  {r.title}
                </button>
```

页面组件传入 `onOpenTask={(p, id) => navigate(`/projects/${p}/tasks/${id}`)}`（沿用该文件已有的 `navigate`）。

- [ ] **Step 4: 构建确认通过**

```bash
pnpm build && pnpm lint
```

预期：无未使用变量告警。若 `oxlint` 报 `TimeLogsSection` 或 `TASK_STATUS_BADGE_CLASS` 未使用，检查是否漏删或漏引。

- [ ] **Step 5: 全量手工验证**

```bash
pnpm tauri dev
```

对着这张表逐条走：

| 场景 | 预期 |
|---|---|
| 打开一个迁移前就存在的老任务 | 时间线非空，创建/开始/完成时间与列表列一致 |
| 项目列表点任务标题 | 进入详情页 |
| 项目列表点工时图标 | 进入详情页 |
| 详情页面包屑返回 | 回到项目详情页任务 tab，目标行高亮，筛选与分页未丢 |
| 全局搜索命中任务 | 直达详情页 |
| Dashboard 待办点任务标题 | 直达详情页 |
| Dashboard 待办点项目名 | 进入项目详情页 |
| 列表页暂停不填原因 | 报错，状态不变 |
| 暂停后列表筛选「已暂停」 | 能筛出 |
| 暂停且已过截止日期的任务在 Dashboard | 日期不是朱红色，排在活跃任务之后 |
| 详情页删除任务 | 二次确认后返回项目页，任务进回收站 |
| 回收站还原该任务 | 再次打开详情页，备注和时间线都在 |

- [ ] **Step 6: 提交**

```bash
git add src/routes/projects/tasks/panel.tsx src/components/search/CommandPalette.tsx src/routes/dashboard.tsx
git commit -m "feat(tasks): 列表与搜索接入任务详情页，旧对话框下线

编辑与工时两个对话框由详情页接管；开始/完成/暂停等快捷打点保留
在列表上，扫一眼就地处理是本工具的高频动作。搜索命中任务直达
详情页，不再只是列表里高亮一行。

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 12: CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 写条目**

调用 `/changelog` skill 生成条目。若不可用，在 `CHANGELOG.md` 的 Unreleased 段（无则新建）写入：

```markdown
## [Unreleased]

### Added
- 任务新增「已暂停」状态，暂停时必须填写原因
- 任务详情页 `/projects/:projectId/tasks/:taskId`：基本信息、备注、时间线收在一处
- 任务时间线：手写备注、状态流转与工时记录按时间合并展示
- 迁移时按现有日期回填历史事件，老任务与禅道导入任务的时间线不为空

### Changed
- 任务详情由对话框改为独立页面；编辑任务与工时清单两个对话框下线
- 全局搜索命中任务直接跳转任务详情页
- Dashboard 待办：暂停任务不再判定逾期，排序位于活跃任务之后
```

- [ ] **Step 2: 提交**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录任务暂停与任务详情页

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
