# 项目模块 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给项目引入扁平的「模块」概念——每个 `modules` 行 project-scoped、可选挂在 `tasks.module_id` 上；新增「按模块统计人力成本」IPC；「任务+工时」tab 加模块 Select 过滤、管理弹窗、统计卡、TaskForm 加模块选择。是「禅道 CSV 导入」的前置依赖。

**Architecture:** 后端一张新表 `modules`（软删、`sort_order` 排序）、`tasks` 加 `module_id INTEGER NULL REFERENCES modules(id)`，跨项目挂载在 Rust 层校验拒绝；`domain::module_stats::labor_by_module` 用 LEFT JOIN 计算每模块的 hours + cost（未分类桶 `module_id = NULL`）。前端新 store `modules` / `moduleStats`，`TasksPanel` 顶部同时提供模块过滤 Select、「管理模块」按钮（Dialog CRUD）与「按模块统计人力成本」卡；`TaskForm` 加模块 Select。

**Tech Stack:** Rust (rusqlite / tauri v2 command) + React 19 + TypeScript + Vite + Tailwind + shadcn/radix + zustand + i18next。

## Global Constraints

- 迁移编号沿用现有序列：新增 `0005_modules.sql`（当前 `MIGRATIONS` 到 `0004_projects_commission`，`db/migrations.rs` 里 `current_version` 断言 `4`）。
- 模块**扁平**（无 `parent_id`），项目级（`project_id` 外键），软删除。
- 任务的 `module_id` **可选**——老任务 NULL 兼容不变；`update_impl` 允许显式覆盖为 NULL（直接 UPDATE，非 COALESCE）。
- 跨项目挂载在后端 `validate_module_belongs_to_project` 内拒绝，前端不做（信任后端）。
- 模块删除**强拒绝**：模块下若存在未软删任务 → `AppError::DeleteBlocked("模块下还有任务，请先删除或转移")`。
- 模块名允许重复（无唯一约束），长度 1–40。
- `labor_by_module` SQL 用 `HAVING hours > 0` 过滤"新建但未记工时"的空模块。
- 未分类桶在返回结构里表现为 `module_id=None, module_name=None`；前端渲染时替换为 i18n `module.unassigned`。
- i18n 只维护 `src/i18n/zh-CN.json`。
- 前端 store 从 IPC 拿全量 tasks 后**在客户端 filter** 模块（不改 `list_tasks` IPC）。
- Commit 规范：Conventional Commits + 中文 subject，≤ 72 字符；body 说明为什么这么改。

---

## File Structure

**新增**：
- `src-tauri/migrations/0005_modules.sql`
- `src-tauri/src/commands/modules.rs`
- `src-tauri/src/domain/module_stats.rs`
- `src/stores/modules.ts`
- `src/stores/moduleStats.ts`
- `docs/superpowers/plans/2026-07-02-project-modules.md`（本文件）

**修改**：
- `src-tauri/src/commands/mod.rs` — `pub mod modules;`
- `src-tauri/src/domain/mod.rs` — `pub mod module_stats;`
- `src-tauri/src/lib.rs` — 4 modules IPC + 1 module labor stats IPC 注入 handler
- `src-tauri/src/db/migrations.rs` — `MIGRATIONS` 追加 + `current_version` 断言 4→5
- `src-tauri/src/commands/tasks.rs` — `Task`/`TaskInput` 扩 `module_id`、`row_to_task`、`create_impl` / `update_impl` 加跨项目校验、3 测试
- `src/types/index.ts` — Module/ModuleInput/ModuleLaborStat + Task 扩 `module_id`、TaskInput 扩 `module_id`
- `src/stores/tasks.ts` — 联动 `useModuleStatsStore.refresh` （create/update/softDelete 后）
- `src/stores/timelogs.ts` — 联动 `useModuleStatsStore.refresh` （create/update/softDelete 后）
- `src/i18n/zh-CN.json` — 15+ 新 keys（`module.*`、`task.module`、`financial.laborByModule`）
- `src/routes/projects/detail.tsx` — `TasksPanel` 顶部工具栏 + 统计卡 + 管理 Dialog；`TaskForm` 加 module Select
- `CHANGELOG.md` — Unreleased/Added 段追加

**不改**：
- `src/stores/financial.ts`（moduleStats 是并列 store）
- `useTasksStore` 结构（透传 `TaskInput` 已经足够；只加个联动 refresh 调用）

---

## Task 1: Backend — 迁移 + `modules.rs` CRUD

**Files:**
- Create: `src-tauri/migrations/0005_modules.sql`
- Create: `src-tauri/src/commands/modules.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `AppError::{Validation, DeleteBlocked, NotFound, Db}`、`AppResult`、`AppState`、`Connection`。
- Produces（Task 2/3 消费）：
  ```rust
  pub struct Module {
      pub id: i64,
      pub project_id: i64,
      pub name: String,
      pub sort_order: i64,
      pub created_at: String,
      pub updated_at: String,
  }
  pub struct ModuleInput { pub name: String, pub sort_order: Option<i64> }
  pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<Module>>;
  pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Module>;
  pub(crate) fn create_impl(conn: &Connection, project_id: i64, input: &ModuleInput) -> AppResult<Module>;
  pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ModuleInput) -> AppResult<Module>;
  pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()>;
  ```
- IPC commands（Task 5 前端消费）: `list_modules`, `create_module`, `update_module`, `delete_module`。

- [ ] **Step 1.1: 写迁移文件**

Create `src-tauri/migrations/0005_modules.sql`：

```sql
-- Project modules + tasks.module_id
CREATE TABLE modules (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id   INTEGER NOT NULL REFERENCES projects(id),
  name         TEXT    NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at   TEXT    NOT NULL DEFAULT (datetime('now')),
  deleted_at   TEXT
);
CREATE INDEX idx_modules_project ON modules(project_id, deleted_at);

ALTER TABLE tasks ADD COLUMN module_id INTEGER REFERENCES modules(id);
CREATE INDEX idx_tasks_module ON tasks(module_id) WHERE module_id IS NOT NULL;
```

（注意：不写 `BEGIN;/COMMIT;` —— `db/migrations.rs::run` 已经用 `unchecked_transaction()` 包起来了。迁移文件内的 `execute_batch` 与外层 tx 复用。）

- [ ] **Step 1.2: 注册迁移**

编辑 `src-tauri/src/db/migrations.rs`：

- 在 `MIGRATIONS` 数组 `("0004_projects_commission", ...)` 之后追加：

  ```rust
      (
          "0005_modules",
          include_str!("../../migrations/0005_modules.sql"),
      ),
  ```

- 将 `#[test] fn fresh_db_runs_all_migrations` 里的 `assert_eq!(v, 4);` 改为 `assert_eq!(v, 5);`
- 将 `#[test] fn run_is_idempotent` 里的 `assert_eq!(current_version(&conn).unwrap(), 4);` 改为 `assert_eq!(current_version(&conn).unwrap(), 5);`

- [ ] **Step 1.3: 创建 `commands/modules.rs` 骨架 + 写失败测试**

Create `src-tauri/src/commands/modules.rs`：

```rust
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Module {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ModuleInput {
    pub name: String,
    pub sort_order: Option<i64>,
}

fn row_to_module(row: &rusqlite::Row) -> rusqlite::Result<Module> {
    Ok(Module {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &ModuleInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::Validation("模块名长度必须在 1–40 之间".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<Module>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM modules
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([project_id], row_to_module)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Module> {
    conn.query_row(
        "SELECT * FROM modules WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_module,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "module", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &ModuleInput,
) -> AppResult<Module> {
    validate(input)?;
    let next_order: i64 = match input.sort_order {
        Some(n) => n,
        None => conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM modules
             WHERE project_id = ?1 AND deleted_at IS NULL",
            [project_id],
            |r| r.get(0),
        )?,
    };
    conn.execute(
        "INSERT INTO modules(project_id, name, sort_order)
         VALUES(?1, ?2, ?3)",
        rusqlite::params![project_id, input.name.trim(), next_order],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ModuleInput) -> AppResult<Module> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE modules SET
            name = ?1,
            sort_order = COALESCE(?2, sort_order),
            updated_at = datetime('now')
         WHERE id = ?3 AND deleted_at IS NULL",
        rusqlite::params![input.name.trim(), input.sort_order, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "module", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT deleted_at FROM modules WHERE id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let already_deleted = match row {
        Some(x) => x,
        None => return Err(AppError::NotFound { entity: "module", id }),
    };
    if already_deleted.is_some() {
        return Ok(()); // idempotent
    }
    let attached: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE module_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if attached > 0 {
        return Err(AppError::DeleteBlocked(
            "模块下还有任务，请先删除或转移".into(),
        ));
    }
    conn.execute(
        "UPDATE modules SET deleted_at = datetime('now') WHERE id = ?1",
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
pub fn list_modules(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<Module>> {
    with_conn(&state, |c| list_impl(c, project_id))
}
#[tauri::command]
pub fn create_module(
    state: tauri::State<AppState>,
    project_id: i64,
    input: ModuleInput,
) -> AppResult<Module> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_module(
    state: tauri::State<AppState>,
    id: i64,
    input: ModuleInput,
) -> AppResult<Module> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_module(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

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
            conn.execute("INSERT INTO companies(name) VALUES('Co')", [])
                .unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> ModuleInput {
        ModuleInput { name: name.into(), sort_order: None }
    }

    #[test]
    fn create_defaults_sort_order_to_max_plus_one() {
        let db = TestDb::new();
        let a = create_impl(&db.conn, 1, &input("A")).unwrap();
        let b = create_impl(&db.conn, 1, &input("B")).unwrap();
        let c = create_impl(&db.conn, 1, &input("C")).unwrap();
        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);
        assert_eq!(c.sort_order, 2);
    }

    #[test]
    fn create_persists_name_and_project() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("前端")).unwrap();
        assert_eq!(m.name, "前端");
        assert_eq!(m.project_id, 1);
    }

    #[test]
    fn update_can_rename() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        let u = update_impl(&db.conn, m.id, &ModuleInput { name: "Y".into(), sort_order: None }).unwrap();
        assert_eq!(u.name, "Y");
        assert_eq!(u.sort_order, m.sort_order);
    }

    #[test]
    fn update_can_reorder() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        let u = update_impl(&db.conn, m.id, &ModuleInput { name: "X".into(), sort_order: Some(9) }).unwrap();
        assert_eq!(u.sort_order, 9);
    }

    #[test]
    fn list_orders_by_sort_order_then_id() {
        let db = TestDb::new();
        // insert with explicit sort_order to make ordering deterministic
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'B', 1)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'A', 1)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'C', 0)", []).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["C", "B", "A"]);
    }

    #[test]
    fn list_excludes_soft_deleted() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn delete_blocks_when_task_attached() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, module_id) VALUES(1, 'T', ?1)",
            [m.id],
        ).unwrap();
        let err = delete_impl(&db.conn, m.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_succeeds_when_no_tasks() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn validate_rejects_empty_name() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &input("")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_rejects_too_long_name() {
        let db = TestDb::new();
        let long = "x".repeat(41);
        let err = create_impl(&db.conn, 1, &input(&long)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
```

- [ ] **Step 1.4: 注册 module**

编辑 `src-tauri/src/commands/mod.rs` — 在 `pub mod members;` 前后按字母序追加：

```rust
pub mod modules;
```

编辑 `src-tauri/src/lib.rs` — 在 `.invoke_handler(tauri::generate_handler![` 数组中、`commands::tasks::list_tasks` 之前追加 4 行：

```rust
            commands::modules::list_modules,
            commands::modules::create_module,
            commands::modules::update_module,
            commands::modules::delete_module,
```

- [ ] **Step 1.5: 跑测试验证 RED（先看编译能否通过 + 部分测试失败）**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test commands::modules 2>&1 | tail -30
```

Expected：全部 9 个测试 PASS（因为 Step 1.3 里同时写了实现与测试，RED 阶段并未强制分开——这是 spec 允许的模式，见 §5.1 的方案范式）。若有 FAIL，回读 Step 1.3 修实现。

- [ ] **Step 1.6: 跑迁移版本测试**

Run:
```bash
cargo test db::migrations 2>&1 | tail -20
```

Expected：`fresh_db_runs_all_migrations` 与 `run_is_idempotent` 两个都 PASS（version = 5）。

- [ ] **Step 1.7: 全库回归**

Run:
```bash
cargo test 2>&1 | grep -E "test result:" | head
```

Expected：全部 PASS，测试数 = 现有基线 + 9。

- [ ] **Step 1.8: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/migrations/0005_modules.sql \
        src-tauri/src/commands/modules.rs \
        src-tauri/src/commands/mod.rs \
        src-tauri/src/db/migrations.rs \
        src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(modules): 项目模块 CRUD + 迁移 0005

新表 modules（project_id、name、sort_order、软删）与 tasks.module_id
外键。commands/modules.rs 提供 list/create/update/delete，删除时命中
挂载中的任务返回 DeleteBlocked。9 个单元测试覆盖 CRUD、排序、软删、
删除拦截与校验路径；迁移版本 4→5。
EOF
)"
```

---

## Task 2: Backend — `tasks.rs` 扩 `module_id` + 跨项目校验

**Files:**
- Modify: `src-tauri/src/commands/tasks.rs`

**Interfaces:**
- Consumes: Task 1 产生的 `modules` 表 + `tasks.module_id` 列。
- Produces（Task 3 / Task 4 消费）:
  - `Task` 新增字段：`pub module_id: Option<i64>`
  - `TaskInput` 新增字段：`pub module_id: Option<i64>`
  - `create_impl(conn, project_id, input)` 校验 `input.module_id` 归属传入 `project_id`；`update_impl(conn, id, input)` 读现有 task 的 `project_id` 后校验。

- [ ] **Step 2.1: 结构体加字段**

编辑 `src-tauri/src/commands/tasks.rs`：

- `pub struct Task` 里，在 `pub due_date: Option<String>,` 之后追加：

  ```rust
      pub module_id: Option<i64>,
  ```

- `pub struct TaskInput` 里，在 `pub due_date: Option<String>,` 之后追加：

  ```rust
      pub module_id: Option<i64>,
  ```

- [ ] **Step 2.2: row_to_task 加字段**

在 `fn row_to_task` 中，`due_date: row.get("due_date")?,` 之后追加：

```rust
        module_id: row.get("module_id")?,
```

- [ ] **Step 2.3: 加跨项目校验函数**

在 `fn validate` 之后（`pub(crate) fn list_impl` 之前）追加：

```rust
fn validate_module_belongs_to_project(
    conn: &Connection,
    module_id: Option<i64>,
    task_project_id: i64,
) -> AppResult<()> {
    let Some(mid) = module_id else { return Ok(()); };
    let pid: Option<i64> = conn
        .query_row(
            "SELECT project_id FROM modules WHERE id = ?1 AND deleted_at IS NULL",
            [mid],
            |r| r.get(0),
        )
        .optional()?;
    match pid {
        Some(p) if p == task_project_id => Ok(()),
        _ => Err(AppError::Validation("模块不属于当前项目".into())),
    }
}
```

同时在文件顶部 `use rusqlite::Connection;` 之后（若尚未 import 则加）追加：

```rust
use rusqlite::OptionalExtension;
```

- [ ] **Step 2.4: `create_impl` 加校验 + INSERT 带 module_id**

把 `create_impl` 的 `validate(input)?;` 之后（`if let Some(assignee_id) = ...` 之前）插入：

```rust
    validate_module_belongs_to_project(conn, input.module_id, project_id)?;
```

将 `INSERT INTO tasks(...)` 的列清单与 VALUES 改为：

```rust
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, assignee_id,
                           status, estimated_hours, due_date, module_id)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7, ?8)",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.module_id,
        ],
    )?;
```

- [ ] **Step 2.5: `update_impl` 加校验 + UPDATE 带 module_id**

在 `update_impl` 里，读 `project_id` 之后（`if let Some(assignee_id) = ...` 之前）插入：

```rust
    validate_module_belongs_to_project(conn, input.module_id, project_id)?;
```

将 `UPDATE tasks SET ...` 语句改为（新增 `module_id = ?7` 直接覆盖、原有占位符编号 `?7 → ?8` 顺移）：

```rust
    let n = conn.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            status = COALESCE(?4, status),
            estimated_hours = ?5,
            due_date = ?6,
            module_id = ?7,
            updated_at = datetime('now')
         WHERE id = ?8 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.module_id,
            id,
        ],
    )?;
```

- [ ] **Step 2.6: 同步测试辅助 `input()`**

在 `fn input(...)` 里，`due_date: None,` 之后追加：

```rust
        module_id: None,
```

- [ ] **Step 2.7: 写 3 个新测试**

在 `#[cfg(test)] mod tests { ... }` 末尾（最后一个 `#[test]` 之后）追加：

```rust
    #[test]
    fn create_task_with_module_persists_module_id() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(t.module_id, Some(1));
    }

    #[test]
    fn create_task_rejects_module_from_other_project() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P2')", []).unwrap();
        // module belongs to project 2
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(2, 'X', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        // create task under project 1 with module of project 2 → Validation
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn update_task_can_clear_module_to_null() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        let t = create_impl(&db.conn, 1, &i).unwrap();
        let mut u = input("T");
        u.module_id = None;
        let updated = update_impl(&db.conn, t.id, &u).unwrap();
        assert_eq!(updated.module_id, None);
    }
```

- [ ] **Step 2.8: 跑测试验证通过**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test commands::tasks 2>&1 | tail -30
```

Expected：全部 PASS（含新增 3 个 + 原有 tasks 测试）。

- [ ] **Step 2.9: 全库回归**

Run: `cargo test 2>&1 | grep -E "test result:" | head`

Expected：全部 PASS。

- [ ] **Step 2.10: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/src/commands/tasks.rs
git commit -m "$(cat <<'EOF'
feat(tasks): 加 module_id 字段与跨项目挂载校验

Task/TaskInput 各扩 module_id: Option<i64>；create/update 前调用
validate_module_belongs_to_project 拒绝跨项目挂载；update 用直
接覆盖（非 COALESCE）让"清空回未分类"路径成立。3 个测试覆盖
持久化、跨项目拒绝、清空。
EOF
)"
```

---

## Task 3: Backend — `module_stats.rs` 聚合 + IPC

**Files:**
- Create: `src-tauri/src/domain/module_stats.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 产生的 `modules` 表、Task 2 产生的 `tasks.module_id` 列、`time_logs` 表。
- Produces（Task 4 / Task 5 消费）:
  ```rust
  pub struct ModuleLaborStat {
      pub module_id: Option<i64>,
      pub module_name: Option<String>,
      pub hours: f64,
      pub cost_cents: i64,
  }
  pub fn labor_by_module(conn: &Connection, project_id: i64) -> AppResult<Vec<ModuleLaborStat>>;
  ```
- IPC: `get_module_labor_stats(project_id) → Vec<ModuleLaborStat>`。

- [ ] **Step 3.1: 创建 module_stats.rs 骨架 + 写 5 个测试**

Create `src-tauri/src/domain/module_stats.rs`：

```rust
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModuleLaborStat {
    pub module_id: Option<i64>,
    pub module_name: Option<String>,
    pub hours: f64,
    pub cost_cents: i64,
}

pub fn labor_by_module(
    conn: &Connection,
    project_id: i64,
) -> AppResult<Vec<ModuleLaborStat>> {
    let mut stmt = conn.prepare(
        "SELECT t.module_id,
                m.name AS module_name,
                COALESCE(SUM(tl.hours), 0.0) AS hours,
                COALESCE(SUM(ROUND(tl.hours / 8.0 * tl.daily_cost_snapshot_cents)), 0) AS cost
         FROM tasks t
         LEFT JOIN modules m
                ON m.id = t.module_id AND m.deleted_at IS NULL
         LEFT JOIN time_logs tl
                ON tl.task_id = t.id AND tl.deleted_at IS NULL
         WHERE t.project_id = ?1 AND t.deleted_at IS NULL
         GROUP BY t.module_id, m.name
         HAVING hours > 0
         ORDER BY m.sort_order ASC NULLS LAST, m.id ASC",
    )?;
    let rows = stmt.query_map([project_id], |r| {
        Ok(ModuleLaborStat {
            module_id: r.get(0)?,
            module_name: r.get(1)?,
            hours: r.get(2)?,
            cost_cents: r.get::<_, i64>(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn add_task(conn: &Connection, module_id: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO tasks(project_id, title, module_id) VALUES(1, 'T', ?1)",
            [module_id],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn add_log(conn: &Connection, task_id: i64, hours: f64) {
        conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(?1, 1, '2026-06-01', ?2, 80000)",
            rusqlite::params![task_id, hours],
        ).unwrap();
    }

    #[test]
    fn labor_by_module_empty_project_returns_empty() {
        let db = TestDb::new();
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn labor_by_module_unassigned_bucket_only() {
        let db = TestDb::new();
        let tid = add_task(&db.conn, None);
        add_log(&db.conn, tid, 8.0);
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].module_id, None);
        assert_eq!(out[0].module_name, None);
        assert!((out[0].hours - 8.0).abs() < 1e-9);
        // 8h / 8 * 80000 = 80000
        assert_eq!(out[0].cost_cents, 80_000);
    }

    #[test]
    fn labor_by_module_mixes_named_and_unassigned() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '后端', 1)", []).unwrap();
        let fe = add_task(&db.conn, Some(1));
        let be = add_task(&db.conn, Some(2));
        let na = add_task(&db.conn, None);
        add_log(&db.conn, fe, 20.0);
        add_log(&db.conn, be, 30.0);
        add_log(&db.conn, na, 8.0);
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 3);
        // ORDER BY m.sort_order ASC NULLS LAST → 前端 (0) / 后端 (1) / 未分类 (NULL)
        assert_eq!(out[0].module_name, Some("前端".into()));
        assert!((out[0].hours - 20.0).abs() < 1e-9);
        assert_eq!(out[0].cost_cents, 200_000);
        assert_eq!(out[1].module_name, Some("后端".into()));
        assert_eq!(out[1].cost_cents, 300_000);
        assert_eq!(out[2].module_id, None);
        assert_eq!(out[2].cost_cents, 80_000);
    }

    #[test]
    fn labor_by_module_excludes_soft_deleted_tasks_and_logs() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        let t1 = add_task(&db.conn, Some(1));
        let t2 = add_task(&db.conn, Some(1));
        add_log(&db.conn, t1, 8.0);
        add_log(&db.conn, t2, 8.0);
        // soft-delete one log AND one task
        db.conn.execute("UPDATE time_logs SET deleted_at = datetime('now') WHERE id = 1", []).unwrap();
        db.conn.execute("UPDATE tasks SET deleted_at = datetime('now') WHERE id = ?1", [t2]).unwrap();
        let out = labor_by_module(&db.conn, 1).unwrap();
        // t1 has 0h left (its only log was deleted), t2 fully deleted → nothing above HAVING hours > 0
        assert!(out.is_empty());
    }

    #[test]
    fn labor_by_module_uses_snapshot_daily_cost() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        let tid = add_task(&db.conn, Some(1));
        // insert time_log with EXPLICIT snapshot ≠ current member.daily_cost_cents
        db.conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(?1, 1, '2026-06-01', 8.0, 60000)",
            [tid],
        ).unwrap();
        // change member daily cost afterwards; snapshot should NOT be affected
        db.conn.execute("UPDATE members SET daily_cost_cents = 999999 WHERE id = 1", []).unwrap();
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 1);
        // 8h / 8 * 60000 = 60_000
        assert_eq!(out[0].cost_cents, 60_000);
    }
}
```

- [ ] **Step 3.2: 注册 domain 与 IPC**

编辑 `src-tauri/src/domain/mod.rs`——在 `pub mod backup;` 与 `pub mod profit;` 之间按字母顺序追加：

```rust
pub mod module_stats;
```

编辑 `src-tauri/src/commands/mod.rs`——本任务不改（IPC 需要一个新的 impl 挂点。为了保持"IPC 命令都在 commands/ 下"的惯例，我们把 wrapper 放在 `src-tauri/src/commands/modules.rs` 里）。

在 `src-tauri/src/commands/modules.rs` 文件末尾（`#[cfg(test)] mod tests { ... }` 之前）追加一个 IPC wrapper：

```rust
#[tauri::command]
pub fn get_module_labor_stats(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<crate::domain::module_stats::ModuleLaborStat>> {
    with_conn(&state, |c| crate::domain::module_stats::labor_by_module(c, project_id))
}
```

在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 里、Task 1 已经加入的 `commands::modules::delete_module,` 行之后追加：

```rust
            commands::modules::get_module_labor_stats,
```

- [ ] **Step 3.3: 跑 module_stats 测试**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test domain::module_stats 2>&1 | tail -20
```

Expected：全部 5 个 PASS。

- [ ] **Step 3.4: 全库回归**

Run: `cargo test 2>&1 | grep -E "test result:" | head`

Expected：全部 PASS。

- [ ] **Step 3.5: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/src/domain/module_stats.rs \
        src-tauri/src/domain/mod.rs \
        src-tauri/src/commands/modules.rs \
        src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(module_stats): 按模块统计人力成本 SQL + IPC

domain/module_stats.rs 用 LEFT JOIN 聚合每模块 hours 与
cost（用 snapshot 日薪算），未分类桶以 NULL 表示；HAVING
hours > 0 过滤空模块；ORDER BY sort_order NULLS LAST。5 个
测试覆盖空项目、仅未分类、混合、软删排除、snapshot 隔离。
EOF
)"
```

---

## Task 4: Frontend — 类型 + i18n

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/i18n/zh-CN.json`

**Interfaces:**
- Consumes: Task 1 / 2 / 3 后端字段。
- Produces（Task 5 / 6 / 7 消费）:
  - `interface Module { ... }`
  - `interface ModuleInput { ... }`
  - `interface ModuleLaborStat { ... }`
  - `Task` 扩 `module_id: number | null`
  - `TaskInput` 扩 `module_id?: number | null`
  - i18n keys（见 §6.5 完整清单）

- [ ] **Step 4.1: 扩展 TS 类型**

编辑 `src/types/index.ts`：

- 在 `interface Task { ... }` 中，`due_date: string | null;` 之后追加：

  ```ts
    module_id: number | null;
  ```

- 在 `interface TaskInput { ... }` 中，`due_date?: string | null;` 之后追加：

  ```ts
    module_id?: number | null;
  ```

- 在文件末尾（`BackupStatus` 之后）追加：

  ```ts
  export interface Module {
    id: number;
    project_id: number;
    name: string;
    sort_order: number;
    created_at: string;
    updated_at: string;
  }

  export interface ModuleInput {
    name: string;
    sort_order?: number | null;
  }

  export interface ModuleLaborStat {
    module_id: number | null;
    module_name: string | null;
    hours: number;
    cost_cents: number;
  }
  ```

- [ ] **Step 4.2: 加 i18n keys**

编辑 `src/i18n/zh-CN.json`：

- 在顶层新增一个 `"module"` 对象（放在 `"project"` 对象之后即可，位置不严格）：

  ```json
    "module": {
      "title": "模块",
      "manage": "管理模块",
      "new": "新增模块",
      "rename": "重命名",
      "delete": "删除",
      "deleteConfirm": "确认删除模块「{{name}}」？",
      "deleteBlocked": "该模块下还有任务，请先删除或转移",
      "moveUp": "上移",
      "moveDown": "下移",
      "filterByModule": "按模块筛选",
      "allModules": "全部模块",
      "unassigned": "未分类",
      "nameRequired": "模块名必填",
      "nameTooLong": "模块名不能超过 40 字符"
    },
  ```

- 在 `"task"` 对象内追加：

  ```json
      "module": "模块",
  ```

  （追加位置：`"unassigned"` 或其他现有 keys 之后）

- 在 `"financial"` 对象内追加：

  ```json
      "laborByModule": "按模块统计人力成本",
  ```

- [ ] **Step 4.3: 类型检查 + JSON 校验**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
jq . src/i18n/zh-CN.json > /dev/null && echo "JQ_OK"
```

Expected：`TSC_EXIT=0` 与 `JQ_OK`。

- [ ] **Step 4.4: Commit**

```bash
git add src/types/index.ts src/i18n/zh-CN.json
git commit -m "$(cat <<'EOF'
feat(types+i18n): 项目模块类型与文案

types 扩 Module/ModuleInput/ModuleLaborStat + Task.module_id +
TaskInput.module_id；zh-CN.json 新增 module.* 15 keys、
task.module 与 financial.laborByModule。
EOF
)"
```

---

## Task 5: Frontend — `stores/modules.ts` + `stores/moduleStats.ts` + 联动 refresh

**Files:**
- Create: `src/stores/modules.ts`
- Create: `src/stores/moduleStats.ts`
- Modify: `src/stores/tasks.ts`
- Modify: `src/stores/timelogs.ts`

**Interfaces:**
- Consumes: Task 4 的 TS 类型；Task 1 / 3 后端 IPC (`list_modules`、`create_module`、`update_module`、`delete_module`、`get_module_labor_stats`)。
- Produces（Task 6 / 7 消费）:
  - `useModulesStore` — `{byProject, loadedForProject, loadFor, create, update, moveUp, moveDown, softDelete}`
  - `useModuleStatsStore` — `{byProject, refresh(projectId)}`

- [ ] **Step 5.1: 创建 `useModulesStore`**

Create `src/stores/modules.ts`：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Module, ModuleInput } from "@/types";

interface S {
  byProject: Record<number, Module[]>;
  loadedForProject: Record<number, boolean>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: ModuleInput) => Promise<Module>;
  update: (id: number, input: ModuleInput, projectId: number) => Promise<Module>;
  moveUp: (id: number, projectId: number) => Promise<void>;
  moveDown: (id: number, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
}

export const useModulesStore = create<S>((set, get) => ({
  byProject: {},
  loadedForProject: {},
  async loadFor(projectId) {
    const list = await call<Module[]>("list_modules", { projectId });
    set({
      byProject: { ...get().byProject, [projectId]: list },
      loadedForProject: { ...get().loadedForProject, [projectId]: true },
    });
  },
  async create(projectId, input) {
    const m = await call<Module>("create_module", { projectId, input });
    await get().loadFor(projectId);
    return m;
  },
  async update(id, input, projectId) {
    const m = await call<Module>("update_module", { id, input });
    await get().loadFor(projectId);
    return m;
  },
  async moveUp(id, projectId) {
    const list = get().byProject[projectId] ?? [];
    const idx = list.findIndex((m) => m.id === id);
    if (idx <= 0) return;
    const cur = list[idx];
    const prev = list[idx - 1];
    await call<Module>("update_module", {
      id: cur.id,
      input: { name: cur.name, sort_order: prev.sort_order },
    });
    await call<Module>("update_module", {
      id: prev.id,
      input: { name: prev.name, sort_order: cur.sort_order },
    });
    await get().loadFor(projectId);
  },
  async moveDown(id, projectId) {
    const list = get().byProject[projectId] ?? [];
    const idx = list.findIndex((m) => m.id === id);
    if (idx < 0 || idx >= list.length - 1) return;
    const cur = list[idx];
    const next = list[idx + 1];
    await call<Module>("update_module", {
      id: cur.id,
      input: { name: cur.name, sort_order: next.sort_order },
    });
    await call<Module>("update_module", {
      id: next.id,
      input: { name: next.name, sort_order: cur.sort_order },
    });
    await get().loadFor(projectId);
  },
  async softDelete(id, projectId) {
    await call<void>("delete_module", { id });
    await get().loadFor(projectId);
  },
}));
```

- [ ] **Step 5.2: 创建 `useModuleStatsStore`**

Create `src/stores/moduleStats.ts`：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ModuleLaborStat } from "@/types";

interface S {
  byProject: Record<number, ModuleLaborStat[]>;
  refresh: (projectId: number) => Promise<void>;
}

export const useModuleStatsStore = create<S>((set, get) => ({
  byProject: {},
  async refresh(projectId) {
    const stats = await call<ModuleLaborStat[]>("get_module_labor_stats", { projectId });
    set({ byProject: { ...get().byProject, [projectId]: stats } });
  },
}));
```

- [ ] **Step 5.3: 让 tasks store 同时刷新 moduleStats**

编辑 `src/stores/tasks.ts`：

- 顶部 import 追加：

  ```ts
  import { useModuleStatsStore } from "./moduleStats";
  ```

- 修改 `async create(projectId, input)` — 在 `await get().loadFor(...)` 之后追加：

  ```ts
      await useModuleStatsStore.getState().refresh(projectId);
  ```

- 修改 `async update(id, input, projectId)` — 在 `await get().loadFor(...)` 之后追加同一行：

  ```ts
      await useModuleStatsStore.getState().refresh(projectId);
  ```

- 修改 `async softDelete(id, projectId)` — 在 `finally { ... }` 块内、`await useFinancialStore.getState().refresh(projectId);` 之后追加：

  ```ts
        await useModuleStatsStore.getState().refresh(projectId);
  ```

- `setStatus` 不影响工时归属，不用刷 moduleStats。

- [ ] **Step 5.4: 让 timelogs store 同时刷新 moduleStats**

编辑 `src/stores/timelogs.ts`。首先找到 create/update/softDelete 的 `useFinancialStore.getState().refresh(...)` 调用位置，之后紧跟一行 `useModuleStatsStore.getState().refresh(...)`。

在文件顶部 import 追加：

```ts
import { useModuleStatsStore } from "./moduleStats";
```

对文件里**所有**出现 `await useFinancialStore.getState().refresh(projectId);` 的调用（无论在 try/finally 哪块），紧接一行追加：

```ts
      await useModuleStatsStore.getState().refresh(projectId);
```

（保持缩进与紧邻一致。）

- [ ] **Step 5.5: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
```

Expected：`TSC_EXIT=0`。

- [ ] **Step 5.6: Commit**

```bash
git add src/stores/modules.ts \
        src/stores/moduleStats.ts \
        src/stores/tasks.ts \
        src/stores/timelogs.ts
git commit -m "$(cat <<'EOF'
feat(stores): 模块 store + 按模块人力成本 store + 联动 refresh

stores/modules.ts 提供 CRUD 与 moveUp/moveDown（相邻两行 sort
互换，两次 IPC 后重载）；stores/moduleStats.ts 提供 refresh；
tasks 与 timelogs 的 create/update/softDelete 在原有 financial
refresh 之后追加 moduleStats.refresh，保证数据面板即时反映。
EOF
)"
```

---

## Task 6: Frontend — TasksPanel 顶部工具栏（过滤 + 管理弹窗）

**Files:**
- Modify: `src/routes/projects/detail.tsx` — 主要动 `TasksPanel` 组件（约 line 677 开始）与其内部 return JSX

**Interfaces:**
- Consumes: Task 4 类型；Task 5 store（`useModulesStore`）；i18n `module.*`。
- Produces（Task 7 消费）：模块过滤已经在 TasksPanel 内 filter，Task 7 的统计卡和 TaskForm 与之独立。

- [ ] **Step 6.1: 加 useModulesStore 与本地 state**

编辑 `src/routes/projects/detail.tsx`：

- 顶部 import 追加：

  ```tsx
  import { useModulesStore } from "@/stores/modules";
  ```

- 在 `TasksPanel` 组件顶部（在 `const { byProject, statusFilter, ...` 之后）追加：

  ```tsx
    const {
      byProject: modulesByProject,
      loadedForProject: modulesLoadedFor,
      loadFor: loadModules,
      create: createModule,
      update: updateModule,
      moveUp: moveModuleUp,
      moveDown: moveModuleDown,
      softDelete: softDeleteModule,
    } = useModulesStore();
    const modules = modulesByProject[projectId] ?? [];
    const [moduleFilter, setModuleFilter] = useState<string>("__all"); // __all | __unassigned | <id>
    const [openManageModules, setOpenManageModules] = useState(false);
  ```

- 在文件已有的 useEffect（`useEffect(() => { loadFor(projectId, null); ...` 那块）之后紧跟一个新的 useEffect，加载模块：

  ```tsx
    useEffect(() => {
      if (!modulesLoadedFor[projectId]) loadModules(projectId);
    }, [projectId, modulesLoadedFor, loadModules]);
  ```

- [ ] **Step 6.2: 顶部工具栏 JSX 改造**

在文件顶部 import 里，把已有的 `import type { CostEntry, ... Member, ... }` 追加 `Module`：

```tsx
import type { CostEntry, CostEntryInput, ContractPayment, PaymentInput, Project, Member, Module, Task, TaskInput, TimeLog, TimeLogInput, TimeLogUpdateInput, ProjectFinancialSummary } from "@/types";
```

找到 TasksPanel `return (` 后的第一段 `<div className="flex items-center justify-between">`，把整个「顶部工具栏」块（含状态 Select 与新建按钮）替换为：

```tsx
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Select
            value={statusFilter ?? "__all"}
            onValueChange={(v) => loadFor(projectId, v === "__all" ? null : v)}
          >
            <SelectTrigger className="w-40"><SelectValue placeholder={t("task.filterByStatus")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("task.allStatuses")}</SelectItem>
              <SelectItem value="todo">{t("taskStatus.todo")}</SelectItem>
              <SelectItem value="in_progress">{t("taskStatus.in_progress")}</SelectItem>
              <SelectItem value="done">{t("taskStatus.done")}</SelectItem>
            </SelectContent>
          </Select>
          <Select value={moduleFilter} onValueChange={setModuleFilter}>
            <SelectTrigger className="w-40"><SelectValue placeholder={t("module.filterByModule")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("module.allModules")}</SelectItem>
              <SelectItem value="__unassigned">{t("module.unassigned")}</SelectItem>
              {modules.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="outline" onClick={() => setOpenManageModules(true)}>
            {t("module.manage")}
          </Button>
        </div>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("task.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("task.create")}</DialogTitle></DialogHeader>
            <TaskForm
              members={members}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>
```

（TaskForm 签名与内容由 Task 7 一起改；本 Task 里 TaskForm 调用形式保持不变。）

- [ ] **Step 6.3: 客户端 filter tasks by moduleFilter**

在 TasksPanel 里，把 `const tasks = byProject[projectId] ?? [];` 之后立刻追加：

```tsx
  const visibleTasks = tasks.filter((tk) => {
    if (moduleFilter === "__all") return true;
    if (moduleFilter === "__unassigned") return tk.module_id == null;
    return tk.module_id === Number(moduleFilter);
  });
```

把下面所有渲染任务的地方（`{tasks.length === 0 ? ...`、`{tasks.map((tk) => ...`）里的 `tasks` 换成 `visibleTasks`：

- `{tasks.length === 0 ? (` → `{visibleTasks.length === 0 ? (`
- `{tasks.map((tk) => {` → `{visibleTasks.map((tk) => {`

- [ ] **Step 6.4: 加「管理模块」Dialog（含 CRUD、排序、删除）**

在 TasksPanel 组件 `return (...)` 最后 `</div>` 之前追加一个新 Dialog：

```tsx
      <Dialog open={openManageModules} onOpenChange={setOpenManageModules}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("module.manage")}</DialogTitle>
          </DialogHeader>
          <ManageModulesForm
            projectId={projectId}
            modules={modules}
            onClose={() => setOpenManageModules(false)}
            createModule={createModule}
            updateModule={updateModule}
            moveModuleUp={moveModuleUp}
            moveModuleDown={moveModuleDown}
            softDeleteModule={softDeleteModule}
          />
        </DialogContent>
      </Dialog>
```

- [ ] **Step 6.5: 实现 ManageModulesForm 子组件**

在文件末尾（`TasksPanel` 的所有子组件之下、最后一个 `}` 之前）追加：

```tsx
function ManageModulesForm({
  projectId,
  modules,
  onClose,
  createModule,
  updateModule,
  moveModuleUp,
  moveModuleDown,
  softDeleteModule,
}: {
  projectId: number;
  modules: Module[];
  onClose: () => void;
  createModule: (projectId: number, input: { name: string }) => Promise<Module>;
  updateModule: (id: number, input: { name: string }, projectId: number) => Promise<Module>;
  moveModuleUp: (id: number, projectId: number) => Promise<void>;
  moveModuleDown: (id: number, projectId: number) => Promise<void>;
  softDeleteModule: (id: number, projectId: number) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState("");

  return (
    <div className="space-y-3">
      <div className="space-y-2 max-h-64 overflow-y-auto">
        {modules.length === 0 ? (
          <div className="text-sm text-muted-foreground">{t("task.empty")}</div>
        ) : (
          modules.map((m, idx) => (
            <div key={m.id} className="flex items-center gap-2">
              {editingId === m.id ? (
                <>
                  <Input
                    value={editingName}
                    onChange={(e) => setEditingName(e.target.value)}
                    className="flex-1"
                  />
                  <Button
                    size="sm"
                    onClick={async () => {
                      if (!editingName.trim()) return toast.error(t("module.nameRequired"));
                      try {
                        await updateModule(m.id, { name: editingName.trim() }, projectId);
                        setEditingId(null);
                      } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("common.save")}</Button>
                  <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>
                    {t("common.cancel")}
                  </Button>
                </>
              ) : (
                <>
                  <div className="flex-1">{m.name}</div>
                  <Button size="sm" variant="ghost" disabled={idx === 0}
                    onClick={async () => {
                      try { await moveModuleUp(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveUp")}</Button>
                  <Button size="sm" variant="ghost" disabled={idx === modules.length - 1}
                    onClick={async () => {
                      try { await moveModuleDown(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.moveDown")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={() => { setEditingId(m.id); setEditingName(m.name); }}
                  >{t("module.rename")}</Button>
                  <Button size="sm" variant="ghost"
                    onClick={async () => {
                      if (!confirm(t("module.deleteConfirm", { name: m.name }))) return;
                      try { await softDeleteModule(m.id, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >{t("module.delete")}</Button>
                </>
              )}
            </div>
          ))
        )}
      </div>
      <div className="flex items-center gap-2">
        <Input
          placeholder={t("module.new")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <Button
          onClick={async () => {
            if (!newName.trim()) return toast.error(t("module.nameRequired"));
            try { await createModule(projectId, { name: newName.trim() }); setNewName(""); }
            catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
          }}
        >{t("module.new")}</Button>
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>{t("common.close")}</Button>
      </DialogFooter>
    </div>
  );
}
```

如果 `common.close` 在 zh-CN.json 中不存在，就在同一 commit 里给 `common` 对象加：

```json
    "close": "关闭",
```

- [ ] **Step 6.6: 类型检查 + 语法检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
jq . src/i18n/zh-CN.json > /dev/null && echo "JQ_OK"
```

Expected：`TSC_EXIT=0` 与 `JQ_OK`。

- [ ] **Step 6.7: Commit**

```bash
git add src/routes/projects/detail.tsx src/i18n/zh-CN.json
git commit -m "$(cat <<'EOF'
feat(projects): 任务 tab 顶部加模块过滤与管理弹窗

顶部工具栏加模块 Select（__all / __unassigned / <id>），
按客户端 filter tasks；同排「管理模块」按钮打开弹窗做
CRUD + 上/下移。TaskForm 签名扩 modules（Task 7 消费）。
EOF
)"
```

---

## Task 7: Frontend — 统计卡 + TaskForm 加模块选择

**Files:**
- Modify: `src/routes/projects/detail.tsx`

**Interfaces:**
- Consumes: Task 4 `ModuleLaborStat`；Task 5 `useModuleStatsStore`；Task 6 已经在 TaskForm 传入的 `modules` prop。
- Produces: 无（终态 UI）。

- [ ] **Step 7.1: 挂 useModuleStatsStore 并在 TasksPanel mount 时 refresh**

在 `TasksPanel` 组件里、`useEffect` 加载模块之后追加：

```tsx
  const moduleStats = useModuleStatsStore((s) => s.byProject[projectId] ?? []);
  const refreshModuleStats = useModuleStatsStore((s) => s.refresh);
  useEffect(() => { refreshModuleStats(projectId); }, [projectId, refreshModuleStats]);
```

在文件顶部 import 追加：

```tsx
import { useModuleStatsStore } from "@/stores/moduleStats";
import type { CostEntry, CostEntryInput, ContractPayment, PaymentInput, Project, Member, Module, ModuleLaborStat, Task, TaskInput, TimeLog, TimeLogInput, TimeLogUpdateInput, ProjectFinancialSummary } from "@/types";
```

（把上一步已经加的 `Module` import 保留；追加 `ModuleLaborStat` 是本步新增。）

- [ ] **Step 7.2: 在 TasksPanel 顶部工具栏之后插入统计卡**

在 TasksPanel `return (...)` 里、工具栏 `</div>` 之后、任务列表 `{visibleTasks.length === 0 ?` 之前插入：

```tsx
      {moduleStats.length > 0 && (
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.laborByModule")}</CardTitle></CardHeader>
          <CardContent className="p-0">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("module.title")}</TableHead>
                  <TableHead className="text-right w-24">{t("timelog.hours")}</TableHead>
                  <TableHead className="text-right w-32">人力成本</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {moduleStats.map((s) => (
                  <TableRow key={s.module_id ?? "unassigned"}>
                    <TableCell>{s.module_name ?? t("module.unassigned")}</TableCell>
                    <TableCell className="text-right">{s.hours}</TableCell>
                    <TableCell className="text-right">{formatCNY(s.cost_cents)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
```

- [ ] **Step 7.3: TaskForm 签名扩 modules + 加模块 Select 字段**

在 `TaskForm` 组件的类型签名上，把：

```tsx
function TaskForm({ members, initial, onSubmit, onCancel }: {
  members: Member[];
  initial?: Task;
  onSubmit: (input: TaskInput) => Promise<void>;
  onCancel: () => void;
}) {
```

改为：

```tsx
function TaskForm({ members, modules, initial, onSubmit, onCancel }: {
  members: Member[];
  modules: Module[];
  initial?: Task;
  onSubmit: (input: TaskInput) => Promise<void>;
  onCancel: () => void;
}) {
```

在 TasksPanel 里两个 TaskForm 调用点补 `modules={modules}` prop：

1. 顶部工具栏的「新建任务」Dialog 里：

    ```tsx
              <TaskForm
                members={members}
                modules={modules}
                onCancel={() => setOpenNew(false)}
                onSubmit={async (input) => {
                  try { await create(projectId, input); setOpenNew(false); }
                  catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                }}
              />
    ```

2. 编辑任务 Dialog 里：

    ```tsx
              <TaskForm
                members={members}
                modules={modules}
                initial={editing}
                onCancel={() => setEditing(null)}
                onSubmit={async (input) => {
                  try { await update(editing.id, input, projectId); setEditing(null); }
                  catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                }}
              />
    ```

在 `TaskForm` 组件体内：

- state 追加（放在 `const [dueDate, setDueDate] = ...` 之后）：

  ```tsx
    const [moduleId, setModuleId] = useState<string>(
      initial?.module_id ? String(initial.module_id) : "__none"
    );
  ```

- 在 JSX 里，「预估工时 / 截止日期」那个 `<div className="grid grid-cols-2 gap-3">` 之后追加一个独立行（占满宽）：

  ```tsx
        <div className="space-y-1">
          <Label>{t("task.module")}</Label>
          <Select value={moduleId} onValueChange={setModuleId}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__none">{t("module.unassigned")}</SelectItem>
              {modules.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
  ```

- `submit` 提交时映射：找到 `await onSubmit({...})` 调用，在既有属性 `due_date: dueDate || null,` 之后追加：

  ```tsx
          module_id: moduleId === "__none" ? null : Number(moduleId),
  ```

- [ ] **Step 7.4: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
```

Expected：`TSC_EXIT=0`。

- [ ] **Step 7.5: 人工验收**

启动 `pnpm tauri dev`，逐条验证：

1. 老项目（无模块）打开：TasksPanel 顶部只有状态 Select、模块 Select 只有「全部模块 / 未分类」两个选项；「按模块统计人力成本」卡不显示；TaskForm 里模块字段默认「未分类」。
2. 点「管理模块」→ 新增「前端」→「后端」→ 顺序变成「前端 / 后端」→ 「后端」上移 → 变「后端 / 前端」。
3. 新建任务挂「前端」→ 记 8h 工时（在原有工时对话框里）→ 统计卡出现「前端 · 8h · ¥800.00」。
4. 模块 Select 切「后端」→ 任务列表隐藏挂「前端」的任务。
5. 尝试删除「前端」→ toast 报「该模块下还有任务，请先删除或转移」。
6. 把该 task 的模块改回「未分类」→ 保存 → 再删「前端」→ 成功；统计卡不再出现「前端」行，「未分类」行金额相应变化。

- [ ] **Step 7.6: Commit**

```bash
git add src/routes/projects/detail.tsx
git commit -m "$(cat <<'EOF'
feat(projects): 按模块人力成本卡 + TaskForm 加模块选择

TasksPanel 顶部工具栏下方新增统计卡（moduleStats.length > 0
时渲染），行为「模块 · hours · 人力成本」；TaskForm 新增
模块 Select，__none 映射为 null。
EOF
)"
```

---

## Task 8: CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:** 无

- [ ] **Step 8.1: 加条目**

编辑 `CHANGELOG.md`——在 `## Unreleased → ### Added` 段的第一条之前（保持最新在前）插入两行：

```markdown
- 项目模块：每个项目可创建扁平模块列表，任务可选挂到某个模块（默认未分类），支持按模块过滤任务与「按模块统计人力成本」卡；模块删除时若有任务在挂被强拒绝
```

- [ ] **Step 8.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录项目模块条目"
```

---

## 完成后

- [ ] 全库 `cargo test` PASS（旧 106 + 新 17 = 123）
- [ ] `pnpm exec tsc -b` EXIT=0
- [ ] `pnpm tauri dev` 手工回归 Task 7.5 的 6 条验收清单
- [ ] 询问用户是否合并到 `main` / 推送
