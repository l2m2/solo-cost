# solo-cost M2 (Projects + Costs + Trash) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M1 基础上补齐「记账闭环」：项目 CRUD（含 6 状态生命周期）+ 成本科目（9 个 preset + 自定义）+ 成本录入（按项目+科目记账）+ 软删除 UI（项目/成本可软删 + `/trash` 回收站可恢复）。M2 完工后用户可以「建公司 → 在公司下建项目 → 在项目里录成本 → 看到当前公司维度的项目列表 + 成本汇总 → 误删可在回收站恢复」。

**Architecture:** 后端延续 `commands → service-thin → db pool` 三层。新增 `domain/` 目录承载脱离 IPC 的纯计算与软删级联（域逻辑可单测）。前端延续 `store → ipc → routes/components`，新增 `lib/money.ts`（分↔元转换）与 `components/forms/MoneyInput`（金额统一控件）。所有金额仍走 INTEGER cents，禁止浮点中转。

**Tech Stack:** 继承 M1 全部技术栈。本里程碑不新增依赖（不引 Vitest、不引图表库、不引 ORM）。

## Global Constraints

适用所有任务（每个任务的要求隐式包含本节）：

- **包管理**：pnpm（统一锁文件，禁用 npm/yarn 混用）
- **金额单位**：所有金额字段一律 `INTEGER`（分）；前端展示通过 `lib/money.ts` 转元
- **软删除字段**：所有业务表必须有 `deleted_at TIMESTAMP NULL DEFAULT NULL`；业务查询默认 `WHERE deleted_at IS NULL`；级联软删使用**同一时间戳**保证「整组恢复」
- **错误处理**：Rust 端所有 `#[tauri::command]` 返回 `Result<T, AppError>`；禁用 `unwrap()` / `expect()` 在非测试代码（`Mutex::lock().unwrap()` 是公认惯例例外）
- **SQL 安全**：rusqlite 一律绑定参数，禁止字符串拼接（PRAGMA 例外仍用 `escape_sqlite_string`）
- **跨表写入用事务**：项目删除 → cost_entries 级联软删 必须在一个事务里
- **代码注释**：英文；公开 API 写简明 doc comment；不写 WHAT-only 注释
- **提交规约**：Conventional Commits；`type`/`scope` 小写英文；`subject` 中文 ≤ 72 字符，整行不超 72 字符，结尾不加句号；body 写"为什么"
- **CHANGELOG**：每个 task commit 之后，由 implementer 单独跑 `/changelog` skill 追加条目（保留 M1 的两-commit 模式）
- **测试纪律**：domain 层用 TDD（先写测试再实现）；commands 层先写实现后补测试
- **金额计算**：在 Rust 后端进行；前端只负责展示与录入（金额 INTEGER cents 跨 IPC）
- **状态校验**：项目 6 状态字段在 DB CHECK + Rust validate 双层把关；状态迁移合法性 M2 暂不强制（M4 收口）
- **代码注释语言**：英文；UI 文案中文（i18n 或内联，按 M1 习惯）
- **目标平台**：macOS 主开发；不引入 OS 特定逻辑
- **不引入新前端依赖**：M2 全部用 M1 已装的 shadcn 组件（Card / Dialog / Input / Label / Button / DropdownMenu / Sonner）+ Tailwind utilities
- **shadcn 补装**：仅当 `pnpm dlx shadcn@latest add <name>` 添加现成原子组件（如 Tabs / Select / Table / Badge / RadioGroup）时允许，且必须在 task report 中明示

---

## File Structure (M2 完成后的产物增量)

```
solo-cost/
├── src-tauri/
│   ├── migrations/
│   │   └── 0002_projects_costs.sql       新增：cost_categories / projects / cost_entries
│   └── src/
│       ├── error.rs                       MODIFY：加 DeleteBlocked 变体
│       ├── lib.rs                         MODIFY：注册 14 条新命令
│       ├── domain/                        NEW
│       │   ├── mod.rs
│       │   └── soft_delete.rs             级联软删/恢复（含 cost_entries），单测
│       └── commands/
│           ├── mod.rs                     MODIFY：导出新模块
│           ├── categories.rs              NEW：成本科目 CRUD + preset 自动 seed
│           ├── projects.rs                NEW：项目 CRUD + 状态字段
│           ├── costs.rs                   NEW：成本录入 CRUD + 项目级聚合
│           └── trash.rs                   NEW：列出/恢复/清空 软删项
└── src/
    ├── lib/
    │   ├── money.ts                       NEW：cents↔yuan + formatCNY
    │   └── status.ts                      NEW：项目状态 label/color 映射
    ├── components/
    │   └── forms/
    │       └── MoneyInput.tsx             NEW：金额录入（输入元、对外分）
    ├── components/ui/
    │   ├── tabs.tsx                       NEW（shadcn add）
    │   ├── badge.tsx                      NEW（shadcn add）
    │   ├── select.tsx                     NEW（shadcn add）
    │   ├── table.tsx                      NEW（shadcn add）
    │   └── textarea.tsx                   NEW（shadcn add）
    ├── types/index.ts                     MODIFY：加 Project/CostCategory/CostEntry 等
    ├── stores/
    │   ├── categories.ts                  NEW
    │   ├── projects.ts                    NEW
    │   ├── costs.ts                       NEW（项目维度，按 projectId 缓存）
    │   └── trash.ts                       NEW
    ├── i18n/zh-CN.json                    MODIFY：加项目/成本/科目/状态/回收站文案
    ├── components/layout/Sidebar.tsx      MODIFY：加 项目/成本科目/回收站 三项
    ├── App.tsx                            MODIFY：注册 4 条新路由
    └── routes/
        ├── categories.tsx                 NEW：成本科目管理
        ├── projects/
        │   ├── list.tsx                   NEW：项目列表（状态分组 + 筛选）
        │   └── detail.tsx                 NEW：详情 Tabs（概览 + 成本，其余占位）
        └── trash.tsx                      NEW：回收站
```

---

## Task 1: 0002 迁移 + 错误模型扩展

**Files:**
- Create: `src-tauri/migrations/0002_projects_costs.sql`
- Modify: `src-tauri/src/error.rs`

**Interfaces:**
- Produces:
  - DB tables `cost_categories` / `projects` / `cost_entries` with indices + CHECK constraints
  - `AppError::DeleteBlocked(String)` variant（域为后续 Task 提供"拒绝删除"语义；M2 范围内尚不会主动抛出，但 M3 成员删除将使用；先建好声明避免变体频繁变化）
- Consumes: 无（独立基础设施）

- [ ] **Step 1：写 0002 迁移 SQL**

文件 `src-tauri/migrations/0002_projects_costs.sql` 全文：

```sql
-- M2: cost_categories / projects / cost_entries
-- All money values stored as INTEGER cents.
-- All business tables include deleted_at for soft delete with cascade-by-timestamp.

CREATE TABLE cost_categories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id   INTEGER NOT NULL REFERENCES companies(id),
    name         TEXT    NOT NULL,
    is_system    INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    sort_order   INTEGER NOT NULL DEFAULT 0,
    deleted_at   TEXT
);

CREATE INDEX idx_cost_categories_company ON cost_categories(company_id, deleted_at);

CREATE TABLE projects (
    id                                INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id                        INTEGER NOT NULL REFERENCES companies(id),
    name                              TEXT    NOT NULL,
    client_name                       TEXT,
    status                            TEXT    NOT NULL DEFAULT 'pending'
                                              CHECK (status IN ('negotiating','pending','in_progress',
                                                                'delivered','settled','archived')),
    contract_amount_cents             INTEGER NOT NULL DEFAULT 0 CHECK (contract_amount_cents >= 0),
    contract_amount_is_tax_inclusive  INTEGER NOT NULL DEFAULT 1 CHECK (contract_amount_is_tax_inclusive IN (0,1)),
    tax_rate                          REAL    NOT NULL DEFAULT 0.06 CHECK (tax_rate >= 0 AND tax_rate < 1),
    start_date                        TEXT,
    end_date                          TEXT,
    actual_delivered_at               TEXT,
    notes                             TEXT,
    created_at                        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at                        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at                        TEXT
);

CREATE INDEX idx_projects_company_status ON projects(company_id, status, deleted_at);
CREATE INDEX idx_projects_deleted_at ON projects(deleted_at);

CREATE TABLE cost_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id    INTEGER NOT NULL REFERENCES projects(id),
    category_id   INTEGER NOT NULL REFERENCES cost_categories(id),
    incurred_at   TEXT    NOT NULL,
    amount_cents  INTEGER NOT NULL CHECK (amount_cents >= 0),
    description   TEXT,
    notes         TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at    TEXT
);

CREATE INDEX idx_cost_entries_project ON cost_entries(project_id, deleted_at);
CREATE INDEX idx_cost_entries_category ON cost_entries(category_id, deleted_at);
CREATE INDEX idx_cost_entries_incurred ON cost_entries(incurred_at);

-- schema_version is rewritten by the migration runner's INSERT ... ON CONFLICT DO UPDATE,
-- but stating the intent here documents the migration boundary.
INSERT INTO app_meta(key, value) VALUES ('schema_version', '2')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
```

- [ ] **Step 2：扩展 `AppError`**

修改 `src-tauri/src/error.rs`，在 `Internal(String)` 之前插入：

```rust
    #[error("cannot delete: {0}")]
    DeleteBlocked(String),
```

完整文件变更后应为（仅展示需要新增的位置上下文）：

```rust
    #[error("not found: {entity} #{id}")]
    NotFound { entity: &'static str, id: i64 },

    #[error("cannot delete: {0}")]
    DeleteBlocked(String),

    #[error("internal: {0}")]
    Internal(String),
```

- [ ] **Step 3：跑现有测试 + 一次干净启动确认迁移自动应用**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
cd src-tauri && cargo test 2>&1 | tail -10
```
预期：14/14 仍通过（M2 暂未增减 cargo 测试）。

实机验证（不强求 subagent 完成；report 中标注由用户在最终验收时做）：删除 `~/Library/Application Support/solo-cost/data.db`，重启 `pnpm tauri dev`，应该看到日志 `applied migration 0001_init` 然后 `applied migration 0002_projects_costs`，且不报错。

- [ ] **Step 4：Commit**

```bash
git add src-tauri/migrations/0002_projects_costs.sql src-tauri/src/error.rs
git commit -m "feat(db): 0002 迁移建项目/成本/科目表 + DeleteBlocked"
```

- [ ] **Step 5：CHANGELOG**

运行 `/changelog` skill 追加：
- `Added` — 项目、成本科目、成本录入三张表（schema_version 2）
- `Added` — `AppError::DeleteBlocked` 变体

---

## Task 2: domain/soft_delete.rs — 级联软删/恢复（TDD）

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Create: `src-tauri/src/domain/soft_delete.rs`
- Modify: `src-tauri/src/lib.rs`（加 `mod domain;`）

**Interfaces:**
- Produces:
  - `pub fn soft_delete_project(conn: &Connection, id: i64) -> AppResult<()>`
    - 在事务内：把 `projects.deleted_at = now` 与所有 `cost_entries WHERE project_id = id AND deleted_at IS NULL` 的 `deleted_at = now`（同一字符串）
  - `pub fn restore_project(conn: &Connection, id: i64) -> AppResult<()>`
    - 在事务内：读出该 project 的 `deleted_at` 时间戳 `t`，将自身 `deleted_at = NULL`，并把所有 `cost_entries WHERE project_id = id AND deleted_at = t` 一并 `deleted_at = NULL`
  - `pub fn soft_delete_cost_entry(conn: &Connection, id: i64) -> AppResult<()>`
    - 简单：把单条 `cost_entries.deleted_at = now`
  - `pub fn restore_cost_entry(conn: &Connection, id: i64) -> AppResult<()>`
    - 简单：把单条 `cost_entries.deleted_at = NULL`，但若其 `project_id` 对应 project 已删，返回 `DeleteBlocked("项目已删除，请先恢复项目")`
  - `pub fn now_iso() -> String`：`datetime('now')` 在 Rust 端取等价值（用于 commands 也能调用）
- Consumes:
  - 已存在的 `AppError`、`AppResult`
  - `rusqlite::Connection`

- [ ] **Step 1：创建 `src-tauri/src/domain/mod.rs`**

```rust
pub mod soft_delete;
```

- [ ] **Step 2：在 `src-tauri/src/lib.rs` 加入 `mod domain;`**

具体位置：与 `mod commands;` 同级，放在 `mod commands;` 上方一行。

- [ ] **Step 3：先写 RED 测试 — 创建 `soft_delete.rs` 框架并写测试**

文件 `src-tauri/src/domain/soft_delete.rs`：

```rust
use crate::error::{AppError, AppResult};
use rusqlite::Connection;

pub fn soft_delete_project(_conn: &Connection, _id: i64) -> AppResult<()> {
    unimplemented!()
}

pub fn restore_project(_conn: &Connection, _id: i64) -> AppResult<()> {
    unimplemented!()
}

pub fn soft_delete_cost_entry(_conn: &Connection, _id: i64) -> AppResult<()> {
    unimplemented!()
}

pub fn restore_cost_entry(_conn: &Connection, _id: i64) -> AppResult<()> {
    unimplemented!()
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
            // create one company and one project + two cost entries for fixtures
            conn.execute("INSERT INTO companies(name) VALUES('C')", []).unwrap();
            conn.execute(
                "INSERT INTO projects(company_id, name) VALUES(1, 'P')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system) VALUES(1, '差旅', 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-01', 12345)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-02', 6789)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn deleted_at(conn: &Connection, table: &str, id: i64) -> Option<String> {
        conn.query_row(
            &format!("SELECT deleted_at FROM {table} WHERE id = ?1"),
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
    }

    #[test]
    fn project_delete_cascades_to_cost_entries_with_same_timestamp() {
        let db = TestDb::new();
        soft_delete_project(&db.conn, 1).unwrap();
        let pt = deleted_at(&db.conn, "projects", 1).unwrap();
        let c1 = deleted_at(&db.conn, "cost_entries", 1).unwrap();
        let c2 = deleted_at(&db.conn, "cost_entries", 2).unwrap();
        assert_eq!(pt, c1);
        assert_eq!(pt, c2);
    }

    #[test]
    fn restore_project_only_restores_entries_with_matching_timestamp() {
        let db = TestDb::new();
        // independently delete entry 2 first (different timestamp)
        soft_delete_cost_entry(&db.conn, 2).unwrap();
        let entry2_deleted_at = deleted_at(&db.conn, "cost_entries", 2).unwrap();

        // ensure project delete uses a distinct timestamp
        std::thread::sleep(std::time::Duration::from_millis(1100));
        soft_delete_project(&db.conn, 1).unwrap();
        let project_ts = deleted_at(&db.conn, "projects", 1).unwrap();
        assert_ne!(project_ts, entry2_deleted_at);

        // restore project: entry 1 (matched the cascade) is restored, entry 2 (pre-deleted) stays deleted
        restore_project(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "projects", 1).is_none());
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_none());
        assert_eq!(deleted_at(&db.conn, "cost_entries", 2).unwrap(), entry2_deleted_at);
    }

    #[test]
    fn restore_cost_entry_under_deleted_project_blocked() {
        let db = TestDb::new();
        soft_delete_project(&db.conn, 1).unwrap();
        let err = restore_cost_entry(&db.conn, 1).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn soft_delete_then_restore_single_cost_entry_when_project_alive() {
        let db = TestDb::new();
        soft_delete_cost_entry(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_some());
        restore_cost_entry(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_none());
    }
}
```

- [ ] **Step 4：跑 RED**

```bash
cd src-tauri
cargo test --lib domain::soft_delete::tests 2>&1 | tail -25
```
预期：4 个测试全 fail（panic: not yet implemented）。

- [ ] **Step 5：写 GREEN 实现 — 替换 4 个 `unimplemented!()`**

把 `soft_delete.rs` 顶部 4 个函数替换为：

```rust
fn now_iso(conn: &Connection) -> AppResult<String> {
    let s: String = conn.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
    Ok(s)
}

pub fn soft_delete_project(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE projects SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "project", id });
    }
    tx.execute(
        "UPDATE cost_entries SET deleted_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore_project(conn: &Connection, id: i64) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let ts: Option<String> = tx.query_row(
        "SELECT deleted_at FROM projects WHERE id = ?1",
        [id],
        |r| r.get(0),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "project", id },
        other => AppError::Db(other),
    })?;
    let ts = match ts {
        Some(t) => t,
        None => return Ok(()), // already active, no-op
    };
    tx.execute(
        "UPDATE projects SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    tx.execute(
        "UPDATE cost_entries SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn soft_delete_cost_entry(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE cost_entries SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "cost_entry", id });
    }
    Ok(())
}

pub fn restore_cost_entry(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn.query_row(
        "SELECT ce.project_id, p.deleted_at
         FROM cost_entries ce JOIN projects p ON p.id = ce.project_id
         WHERE ce.id = ?1",
        [id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
    ).optional()?;
    let (_project_id, project_deleted_at) = match row {
        Some(t) => t,
        None => return Err(AppError::NotFound { entity: "cost_entry", id }),
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked(
            "项目已删除，请先恢复项目".into(),
        ));
    }
    conn.execute(
        "UPDATE cost_entries SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}
```

注意 `restore_cost_entry` 需要 `use rusqlite::OptionalExtension;` 才能用 `.optional()`。在文件顶部 `use` 块加：

```rust
use rusqlite::OptionalExtension;
```

- [ ] **Step 6：跑 GREEN**

```bash
cargo test --lib domain::soft_delete::tests 2>&1 | tail -10
```
预期：4 passed; 0 failed.

- [ ] **Step 7：跑全量测试确保无回归**

```bash
cargo test 2>&1 | tail -10
```
预期：14 + 4 = 18 passed.

- [ ] **Step 8：Commit**

```bash
git add src-tauri/src/domain src-tauri/src/lib.rs
git commit -m "feat(domain): 项目/成本级联软删恢复 + 时间戳分组"
```

- [ ] **Step 9：CHANGELOG**

`/changelog` 追加 `Added` 项：`domain::soft_delete` 模块（同时间戳级联软删/按时间戳整组恢复）。

---

## Task 3: 成本科目后端 CRUD + preset 自动 seed

**Files:**
- Create: `src-tauri/src/commands/categories.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod categories;`）
- Modify: `src-tauri/src/lib.rs`（注册 5 条命令）

**Interfaces:**
- Produces:
  - `pub struct CostCategory { id, company_id, name, is_system, sort_order }`
  - `pub struct CostCategoryInput { name: String }` （新建/更新只允许改 name，is_system 由后端决定）
  - 5 个 Tauri commands:
    - `list_categories(company_id: i64) -> Vec<CostCategory>`
    - `create_category(company_id: i64, input: CostCategoryInput) -> CostCategory`
    - `update_category(id: i64, input: CostCategoryInput) -> CostCategory`
    - `delete_category(id: i64) -> ()`（拒绝删 `is_system = 1` 的；拒绝删被 cost_entries 引用的 → DeleteBlocked）
    - `seed_preset_categories_if_empty(company_id: i64) -> Vec<CostCategory>`（首次调用 list 时由前端先 call 一次；幂等：若该公司已有任何 category 则跳过）
  - `pub(crate) fn ensure_presets(conn: &Connection, company_id: i64) -> AppResult<()>`（内部辅助，给 Task 4 项目创建后调用以避免新公司空科目场景）
- Consumes:
  - `AppState`, `with_conn` 模式（参考 `companies.rs`）

- [ ] **Step 1：创建 `commands/categories.rs` 骨架**

```rust
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const PRESET_NAMES: [&str; 9] = [
    "外包成本",
    "硬件采购",
    "服务器与SaaS",
    "差旅",
    "办公耗材",
    "市场推广",
    "税费与手续费",
    "培训与资料",
    "其它",
];

#[derive(Debug, Clone, Serialize)]
pub struct CostCategory {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub is_system: bool,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
pub struct CostCategoryInput {
    pub name: String,
}

fn row_to_category(row: &rusqlite::Row) -> rusqlite::Result<CostCategory> {
    Ok(CostCategory {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        is_system: row.get::<_, i64>("is_system")? != 0,
        sort_order: row.get("sort_order")?,
    })
}

fn validate(input: &CostCategoryInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::Validation("科目名长度必须在 1–40 之间".into()));
    }
    Ok(())
}

pub(crate) fn ensure_presets(conn: &Connection, company_id: i64) -> AppResult<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories WHERE company_id = ?1 AND deleted_at IS NULL",
        [company_id],
        |r| r.get(0),
    )?;
    if count > 0 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    for (i, name) in PRESET_NAMES.iter().enumerate() {
        tx.execute(
            "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
             VALUES(?1, ?2, 1, ?3)",
            rusqlite::params![company_id, name, i as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<CostCategory>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cost_categories
         WHERE company_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([company_id], row_to_category)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &CostCategoryInput,
) -> AppResult<CostCategory> {
    validate(input)?;
    // pick sort_order = max + 1
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM cost_categories WHERE company_id = ?1",
        [company_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
         VALUES(?1, ?2, 0, ?3)",
        rusqlite::params![company_id, input.name.trim(), next_order],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &CostCategoryInput,
) -> AppResult<CostCategory> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE cost_categories SET name = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![input.name.trim(), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "cost_category", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn.query_row(
        "SELECT is_system, deleted_at FROM cost_categories WHERE id = ?1",
        [id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
    ).optional()?;
    let (is_system, already_deleted) = match row {
        Some(x) => x,
        None => return Err(AppError::NotFound { entity: "cost_category", id }),
    };
    if is_system == 1 {
        return Err(AppError::DeleteBlocked("预设科目不可删除".into()));
    }
    if already_deleted.is_some() {
        return Ok(()); // idempotent
    }
    let used: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_entries WHERE category_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if used > 0 {
        return Err(AppError::DeleteBlocked(format!(
            "该科目下还有 {used} 条成本记录，请先迁移或删除"
        )));
    }
    let ts: String = conn.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
    conn.execute(
        "UPDATE cost_categories SET deleted_at = ?1 WHERE id = ?2",
        rusqlite::params![ts, id],
    )?;
    Ok(())
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<CostCategory> {
    conn.query_row(
        "SELECT * FROM cost_categories WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_category,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "cost_category",
            id,
        },
        other => AppError::Db(other),
    })
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
pub fn list_categories(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<Vec<CostCategory>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn create_category(
    state: tauri::State<AppState>,
    company_id: i64,
    input: CostCategoryInput,
) -> AppResult<CostCategory> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_category(
    state: tauri::State<AppState>,
    id: i64,
    input: CostCategoryInput,
) -> AppResult<CostCategory> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_category(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}
#[tauri::command]
pub fn seed_preset_categories_if_empty(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<Vec<CostCategory>> {
    with_conn(&state, |c| {
        ensure_presets(c, company_id)?;
        list_impl(c, company_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use rusqlite::OptionalExtension;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            Self { conn, _dir: dir }
        }
    }

    #[test]
    fn seed_creates_nine_preset_categories() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.len(), 9);
        assert!(list.iter().all(|c| c.is_system));
        assert_eq!(list[0].name, "外包成本");
    }

    #[test]
    fn seed_is_idempotent() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        ensure_presets(&db.conn, 1).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 9);
    }

    #[test]
    fn create_custom_category() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &CostCategoryInput { name: "广告投放".into() }).unwrap();
        assert!(!c.is_system);
        assert_eq!(c.name, "广告投放");
    }

    #[test]
    fn delete_system_blocked() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        let err = delete_impl(&db.conn, list[0].id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_in_use_blocked() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        db.conn.execute(
            "INSERT INTO projects(company_id, name) VALUES(1, 'P')",
            [],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(1, ?1, '2026-06-01', 100)",
            [cat.id],
        ).unwrap();
        let err = delete_impl(&db.conn, cat.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_unused_custom_succeeds() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        delete_impl(&db.conn, cat.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn update_renames() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        let new = update_impl(&db.conn, cat.id, &CostCategoryInput { name: "Y".into() }).unwrap();
        assert_eq!(new.name, "Y");
    }
}
```

- [ ] **Step 2：在 `src-tauri/src/commands/mod.rs` 加 `pub mod categories;`**

放在已有 `pub mod companies;` 下一行。

- [ ] **Step 3：注册命令到 `src-tauri/src/lib.rs` invoke_handler**

找到 `tauri::generate_handler![...]` 块，把现有命令保留，在末尾追加：

```rust
            commands::categories::list_categories,
            commands::categories::create_category,
            commands::categories::update_category,
            commands::categories::delete_category,
            commands::categories::seed_preset_categories_if_empty,
```

- [ ] **Step 4：跑测试**

```bash
cd src-tauri
cargo test 2>&1 | tail -15
```
预期：18 + 7 = 25 passed.

- [ ] **Step 5：Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(categories): 成本科目 crud + 9 个预设种子幂等"
```

- [ ] **Step 6：CHANGELOG**

`/changelog` 追加：成本科目 CRUD、预设种子（首次访问公司自动建 9 个不可删科目）、删预设/被引用科目返回 DeleteBlocked。

---

## Task 4: 项目后端 CRUD

**Files:**
- Create: `src-tauri/src/commands/projects.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod projects;`）
- Modify: `src-tauri/src/lib.rs`（注册 6 条命令）

**Interfaces:**
- Produces:
  - `pub struct Project { id, company_id, name, client_name, status, contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate, start_date, end_date, actual_delivered_at, notes, created_at, updated_at }`
  - `pub struct ProjectInput { name, client_name?, status?, contract_amount_cents?, contract_amount_is_tax_inclusive?, tax_rate?, start_date?, end_date?, actual_delivered_at?, notes? }`
  - 6 个 commands:
    - `list_projects(company_id: i64, status: Option<String>) -> Vec<Project>`
    - `get_project(id: i64) -> Project`
    - `create_project(company_id: i64, input: ProjectInput) -> Project`
    - `update_project(id: i64, input: ProjectInput) -> Project`
    - `delete_project(id: i64) -> ()` （走 `domain::soft_delete::soft_delete_project`）
    - `set_project_status(id: i64, status: String) -> Project`
- Consumes:
  - `domain::soft_delete::soft_delete_project`
  - `categories::ensure_presets`（在 `create_project` 内部首次为公司种好科目，避免空科目场景）

- [ ] **Step 1：创建 `commands/projects.rs`**

```rust
use crate::commands::categories;
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const ALLOWED_STATUSES: [&str; 6] = [
    "negotiating", "pending", "in_progress", "delivered", "settled", "archived",
];

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub client_name: Option<String>,
    pub status: String,
    pub contract_amount_cents: i64,
    pub contract_amount_is_tax_inclusive: bool,
    pub tax_rate: f64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub client_name: Option<String>,
    pub status: Option<String>,
    pub contract_amount_cents: Option<i64>,
    pub contract_amount_is_tax_inclusive: Option<bool>,
    pub tax_rate: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        client_name: row.get("client_name")?,
        status: row.get("status")?,
        contract_amount_cents: row.get("contract_amount_cents")?,
        contract_amount_is_tax_inclusive: row.get::<_, i64>("contract_amount_is_tax_inclusive")? != 0,
        tax_rate: row.get("tax_rate")?,
        start_date: row.get("start_date")?,
        end_date: row.get("end_date")?,
        actual_delivered_at: row.get("actual_delivered_at")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &ProjectInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::Validation("项目名长度必须在 1–120 之间".into()));
    }
    if let Some(ref s) = input.status {
        if !ALLOWED_STATUSES.contains(&s.as_str()) {
            return Err(AppError::Validation(format!("非法状态：{s}")));
        }
    }
    if let Some(rate) = input.tax_rate {
        if !(0.0..1.0).contains(&rate) {
            return Err(AppError::Validation("税率必须在 [0, 1) 之间".into()));
        }
    }
    if let Some(amt) = input.contract_amount_cents {
        if amt < 0 {
            return Err(AppError::Validation("合同金额不能为负".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(
    conn: &Connection,
    company_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Project>> {
    let (sql, params): (&str, Vec<rusqlite::types::Value>) = match status {
        Some(s) => (
            "SELECT * FROM projects
             WHERE company_id = ?1 AND status = ?2 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![company_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT * FROM projects
             WHERE company_id = ?1 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![company_id.into()],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_project)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Project> {
    conn.query_row(
        "SELECT * FROM projects WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_project,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "project", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &ProjectInput,
) -> AppResult<Project> {
    validate(input)?;
    // ensure presets exist for this company so the first cost entry has at least one category
    categories::ensure_presets(conn, company_id)?;
    conn.execute(
        "INSERT INTO projects(
            company_id, name, client_name, status,
            contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
            start_date, end_date, actual_delivered_at, notes
         ) VALUES(
            ?1, ?2, ?3, COALESCE(?4, 'pending'),
            COALESCE(?5, 0), COALESCE(?6, 1), COALESCE(?7, 0.06),
            ?8, ?9, ?10, ?11
         )",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.client_name.as_deref(),
            input.status.as_deref(),
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &ProjectInput,
) -> AppResult<Project> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE projects SET
            name = ?1,
            client_name = ?2,
            status = COALESCE(?3, status),
            contract_amount_cents = COALESCE(?4, contract_amount_cents),
            contract_amount_is_tax_inclusive = COALESCE(?5, contract_amount_is_tax_inclusive),
            tax_rate = COALESCE(?6, tax_rate),
            start_date = ?7,
            end_date = ?8,
            actual_delivered_at = ?9,
            notes = ?10,
            updated_at = datetime('now')
         WHERE id = ?11 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.client_name.as_deref(),
            input.status.as_deref(),
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "project", id });
    }
    get_impl(conn, id)
}

pub(crate) fn set_status_impl(conn: &Connection, id: i64, status: &str) -> AppResult<Project> {
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(AppError::Validation(format!("非法状态：{status}")));
    }
    let n = conn.execute(
        "UPDATE projects SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![status, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "project", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_project(conn, id)
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
pub fn list_projects(
    state: tauri::State<AppState>,
    company_id: i64,
    status: Option<String>,
) -> AppResult<Vec<Project>> {
    with_conn(&state, |c| list_impl(c, company_id, status.as_deref()))
}
#[tauri::command]
pub fn get_project(state: tauri::State<AppState>, id: i64) -> AppResult<Project> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_project(
    state: tauri::State<AppState>,
    company_id: i64,
    input: ProjectInput,
) -> AppResult<Project> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_project(
    state: tauri::State<AppState>,
    id: i64,
    input: ProjectInput,
) -> AppResult<Project> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_project_status(
    state: tauri::State<AppState>,
    id: i64,
    status: String,
) -> AppResult<Project> {
    with_conn(&state, |c| set_status_impl(c, id, &status))
}
#[tauri::command]
pub fn delete_project(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            name: name.into(),
            client_name: None,
            status: None,
            contract_amount_cents: None,
            contract_amount_is_tax_inclusive: None,
            tax_rate: None,
            start_date: None,
            end_date: None,
            actual_delivered_at: None,
            notes: None,
        }
    }

    #[test]
    fn create_with_defaults_status_pending() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        assert_eq!(p.status, "pending");
        assert_eq!(p.contract_amount_cents, 0);
        assert!(p.contract_amount_is_tax_inclusive);
        assert!((p.tax_rate - 0.06).abs() < 1e-9);
    }

    #[test]
    fn create_seeds_categories_for_company() {
        let db = TestDb::new();
        create_impl(&db.conn, 1, &input("P")).unwrap();
        let n: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM cost_categories WHERE company_id = 1",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(n, 9);
    }

    #[test]
    fn validate_empty_name() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &input("")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_bad_status() {
        let db = TestDb::new();
        let mut i = input("P");
        i.status = Some("foo".into());
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let db = TestDb::new();
        let mut a = input("A"); a.status = Some("in_progress".into());
        let mut b = input("B"); b.status = Some("delivered".into());
        create_impl(&db.conn, 1, &a).unwrap();
        create_impl(&db.conn, 1, &b).unwrap();
        assert_eq!(list_impl(&db.conn, 1, None).unwrap().len(), 2);
        assert_eq!(list_impl(&db.conn, 1, Some("delivered")).unwrap().len(), 1);
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        let u = set_status_impl(&db.conn, p.id, "in_progress").unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn delete_cascades_to_cost_entries() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        let cat_id: i64 = db.conn.query_row(
            "SELECT id FROM cost_categories WHERE company_id = 1 LIMIT 1",
            [], |r| r.get(0),
        ).unwrap();
        db.conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(?1, ?2, '2026-06-01', 100)",
            [p.id, cat_id],
        ).unwrap();
        delete_impl(&db.conn, p.id).unwrap();
        // project gone from active list
        assert_eq!(list_impl(&db.conn, 1, None).unwrap().len(), 0);
        // cost entry soft deleted too
        let entry_del: Option<String> = db.conn.query_row(
            "SELECT deleted_at FROM cost_entries WHERE project_id = ?1",
            [p.id], |r| r.get(0),
        ).unwrap();
        assert!(entry_del.is_some());
    }
}
```

- [ ] **Step 2：在 `src-tauri/src/commands/mod.rs` 加 `pub mod projects;`**

- [ ] **Step 3：注册命令到 `src-tauri/src/lib.rs`**

```rust
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::set_project_status,
            commands::projects::delete_project,
```

- [ ] **Step 4：跑测试**

```bash
cargo test 2>&1 | tail -15
```
预期：25 + 7 = 32 passed.

- [ ] **Step 5：Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(projects): 项目 crud + 6 状态 + 删除级联成本"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：项目 CRUD（含按状态筛选、状态切换、级联软删成本）；新建项目自动为公司种 9 个预设科目。

---

## Task 5: 成本录入后端 CRUD + 项目汇总

**Files:**
- Create: `src-tauri/src/commands/costs.rs`
- Create: `src-tauri/src/domain/profit.rs`
- Modify: `src-tauri/src/domain/mod.rs`（加 `pub mod profit;`）
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod costs;`）
- Modify: `src-tauri/src/lib.rs`（注册 5 条命令）

**Interfaces:**
- Produces:
  - `pub struct CostEntry { id, project_id, category_id, incurred_at, amount_cents, description, notes, created_at }`
  - `pub struct CostEntryInput { category_id, incurred_at, amount_cents, description?, notes? }`
  - `pub struct ProjectCostSummary { total_cents: i64, by_category: Vec<CategoryBreakdown> }`
  - `pub struct CategoryBreakdown { category_id: i64, category_name: String, total_cents: i64 }`
  - 5 个 commands:
    - `list_cost_entries(project_id: i64) -> Vec<CostEntry>`
    - `create_cost_entry(project_id: i64, input: CostEntryInput) -> CostEntry`
    - `update_cost_entry(id: i64, input: CostEntryInput) -> CostEntry`
    - `delete_cost_entry(id: i64) -> ()`（走 `domain::soft_delete::soft_delete_cost_entry`）
    - `project_cost_summary(project_id: i64) -> ProjectCostSummary`（域函数封装）
- Consumes:
  - `domain::soft_delete::soft_delete_cost_entry`
  - 已存在的 categories / projects 表

- [ ] **Step 1：写 `domain/profit.rs` (TDD)**

```rust
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CategoryBreakdown {
    pub category_id: i64,
    pub category_name: String,
    pub total_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectCostSummary {
    pub total_cents: i64,
    pub by_category: Vec<CategoryBreakdown>,
}

pub fn project_cost_summary(conn: &Connection, project_id: i64) -> AppResult<ProjectCostSummary> {
    let mut stmt = conn.prepare(
        "SELECT cc.id, cc.name, COALESCE(SUM(ce.amount_cents), 0) AS total
         FROM cost_categories cc
         LEFT JOIN cost_entries ce
           ON ce.category_id = cc.id AND ce.project_id = ?1 AND ce.deleted_at IS NULL
         WHERE cc.company_id = (
             SELECT company_id FROM projects WHERE id = ?1
         ) AND cc.deleted_at IS NULL
         GROUP BY cc.id, cc.name
         HAVING total > 0
         ORDER BY total DESC",
    )?;
    let rows = stmt.query_map([project_id], |r| {
        Ok(CategoryBreakdown {
            category_id: r.get(0)?,
            category_name: r.get(1)?,
            total_cents: r.get(2)?,
        })
    })?;
    let mut by_category = Vec::new();
    let mut total: i64 = 0;
    for r in rows {
        let b = r?;
        total += b.total_cents;
        by_category.push(b);
    }
    Ok(ProjectCostSummary { total_cents: total, by_category })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
                 VALUES(1, '差旅', 1, 0), (1, '硬件', 1, 1)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    #[test]
    fn empty_project_returns_zero() {
        let db = TestDb::new();
        let s = project_cost_summary(&db.conn, 1).unwrap();
        assert_eq!(s.total_cents, 0);
        assert!(s.by_category.is_empty());
    }

    #[test]
    fn aggregates_per_category_descending() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES (1, 1, '2026-06-01', 100), (1, 1, '2026-06-02', 200), (1, 2, '2026-06-03', 500)",
            [],
        ).unwrap();
        let s = project_cost_summary(&db.conn, 1).unwrap();
        assert_eq!(s.total_cents, 800);
        assert_eq!(s.by_category.len(), 2);
        assert_eq!(s.by_category[0].category_id, 2);
        assert_eq!(s.by_category[0].total_cents, 500);
        assert_eq!(s.by_category[1].total_cents, 300);
    }

    #[test]
    fn excludes_soft_deleted_entries() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents, deleted_at)
             VALUES (1, 1, '2026-06-01', 999, '2026-06-04 10:00:00')",
            [],
        ).unwrap();
        let s = project_cost_summary(&db.conn, 1).unwrap();
        assert_eq!(s.total_cents, 0);
    }
}
```

- [ ] **Step 2：`domain/mod.rs` 加 `pub mod profit;`**

- [ ] **Step 3：跑 profit 测试**

```bash
cargo test --lib domain::profit::tests 2>&1 | tail -10
```
预期：3 passed.

- [ ] **Step 4：写 `commands/costs.rs`**

```rust
use crate::domain::profit::{project_cost_summary, ProjectCostSummary};
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CostEntry {
    pub id: i64,
    pub project_id: i64,
    pub category_id: i64,
    pub incurred_at: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CostEntryInput {
    pub category_id: i64,
    pub incurred_at: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub notes: Option<String>,
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<CostEntry> {
    Ok(CostEntry {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        category_id: row.get("category_id")?,
        incurred_at: row.get("incurred_at")?,
        amount_cents: row.get("amount_cents")?,
        description: row.get("description")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
    })
}

fn validate(input: &CostEntryInput) -> AppResult<()> {
    if input.amount_cents < 0 {
        return Err(AppError::Validation("金额不能为负".into()));
    }
    if input.incurred_at.trim().is_empty() {
        return Err(AppError::Validation("发生日期必填".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<CostEntry>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cost_entries
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY incurred_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([project_id], row_to_entry)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &CostEntryInput,
) -> AppResult<CostEntry> {
    validate(input)?;
    // verify category belongs to project's company (defense in depth)
    let ok: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories cc
         JOIN projects p ON p.company_id = cc.company_id
         WHERE p.id = ?1 AND cc.id = ?2 AND cc.deleted_at IS NULL",
        [project_id, input.category_id],
        |r| r.get(0),
    )?;
    if ok == 0 {
        return Err(AppError::Validation("科目与项目公司不匹配或科目不存在".into()));
    }
    conn.execute(
        "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents, description, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            project_id,
            input.category_id,
            input.incurred_at.trim(),
            input.amount_cents,
            input.description.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &CostEntryInput,
) -> AppResult<CostEntry> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE cost_entries SET
            category_id = ?1,
            incurred_at = ?2,
            amount_cents = ?3,
            description = ?4,
            notes = ?5
         WHERE id = ?6 AND deleted_at IS NULL",
        rusqlite::params![
            input.category_id,
            input.incurred_at.trim(),
            input.amount_cents,
            input.description.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "cost_entry", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_cost_entry(conn, id)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<CostEntry> {
    conn.query_row(
        "SELECT * FROM cost_entries WHERE id = ?1 AND deleted_at IS NULL",
        [id], row_to_entry,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "cost_entry", id },
        other => AppError::Db(other),
    })
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
pub fn list_cost_entries(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<CostEntry>> {
    with_conn(&state, |c| list_impl(c, project_id))
}
#[tauri::command]
pub fn create_cost_entry(
    state: tauri::State<AppState>,
    project_id: i64,
    input: CostEntryInput,
) -> AppResult<CostEntry> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_cost_entry(
    state: tauri::State<AppState>,
    id: i64,
    input: CostEntryInput,
) -> AppResult<CostEntry> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_cost_entry(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}
#[tauri::command]
pub fn get_project_cost_summary(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<ProjectCostSummary> {
    with_conn(&state, |c| project_cost_summary(c, project_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
                 VALUES(1, '差旅', 1, 0)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn ce(amount: i64) -> CostEntryInput {
        CostEntryInput {
            category_id: 1,
            incurred_at: "2026-06-15".into(),
            amount_cents: amount,
            description: Some("交通".into()),
            notes: None,
        }
    }

    #[test]
    fn create_and_list() {
        let db = TestDb::new();
        let e = create_impl(&db.conn, 1, &ce(12345)).unwrap();
        assert_eq!(e.amount_cents, 12345);
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn negative_amount_rejected() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &ce(-1)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn category_company_mismatch_rejected() {
        let db = TestDb::new();
        // create a second company and a category under it
        db.conn.execute("INSERT INTO companies(name) VALUES('Other')", []).unwrap();
        db.conn.execute(
            "INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(2, 'X', 0, 0)",
            [],
        ).unwrap();
        let mut bad = ce(100);
        bad.category_id = 2;
        let err = create_impl(&db.conn, 1, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_soft_only() {
        let db = TestDb::new();
        let e = create_impl(&db.conn, 1, &ce(50)).unwrap();
        delete_impl(&db.conn, e.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
        // row still exists with deleted_at
        let dt: Option<String> = db.conn.query_row(
            "SELECT deleted_at FROM cost_entries WHERE id = ?1",
            [e.id], |r| r.get(0),
        ).unwrap();
        assert!(dt.is_some());
    }
}
```

- [ ] **Step 5：`commands/mod.rs` 加 `pub mod costs;`**

- [ ] **Step 6：注册命令到 `lib.rs`**

```rust
            commands::costs::list_cost_entries,
            commands::costs::create_cost_entry,
            commands::costs::update_cost_entry,
            commands::costs::delete_cost_entry,
            commands::costs::get_project_cost_summary,
```

- [ ] **Step 7：跑测试**

```bash
cargo test 2>&1 | tail -15
```
预期：32 + 3 (profit) + 4 (costs) = 39 passed.

- [ ] **Step 8：Commit**

```bash
git add src-tauri/src
git commit -m "feat(costs): 成本录入 crud + 项目维度科目聚合"
```

- [ ] **Step 9：CHANGELOG**

`/changelog`：成本录入 CRUD（按项目+科目），跨公司科目防错；`get_project_cost_summary` 按科目汇总。

---

## Task 6: 回收站后端命令

**Files:**
- Create: `src-tauri/src/commands/trash.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod trash;`）
- Modify: `src-tauri/src/lib.rs`（注册 3 条命令）

**Interfaces:**
- Produces:
  - `pub struct TrashItem { id, entity_type, name, deleted_at, project_id (Option) }`
    - `entity_type` 枚举：`"project"` / `"cost_entry"`
    - `name`：项目名 或 "成本 ¥xx.xx (描述)"
    - `project_id`：仅 cost_entry 时有值，便于显示「所属项目（已删则灰色）」
  - 3 个 commands:
    - `list_trash(company_id: i64) -> Vec<TrashItem>`
    - `restore_trash_item(entity_type: String, id: i64) -> ()`
    - `purge_trash_item(entity_type: String, id: i64) -> ()` （物理删；M2 暂用 DELETE，附件清理 M4 再说）
- Consumes:
  - `domain::soft_delete::{restore_project, restore_cost_entry}`

- [ ] **Step 1：创建 `commands/trash.rs`**

```rust
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TrashItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub deleted_at: String,
    pub project_id: Option<i64>,
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<TrashItem>> {
    let mut out = Vec::new();

    // soft-deleted projects in this company
    let mut sp = conn.prepare(
        "SELECT id, name, deleted_at FROM projects
         WHERE company_id = ?1 AND deleted_at IS NOT NULL
         ORDER BY deleted_at DESC",
    )?;
    let rows = sp.query_map([company_id], |r| {
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "project".into(),
            name: r.get::<_, String>(1)?,
            deleted_at: r.get::<_, String>(2)?,
            project_id: None,
        })
    })?;
    for r in rows { out.push(r?); }

    // soft-deleted cost entries whose project belongs to this company
    let mut sc = conn.prepare(
        "SELECT ce.id, ce.project_id, ce.amount_cents, ce.description, ce.deleted_at
         FROM cost_entries ce
         JOIN projects p ON p.id = ce.project_id
         WHERE p.company_id = ?1 AND ce.deleted_at IS NOT NULL
         ORDER BY ce.deleted_at DESC",
    )?;
    let rows = sc.query_map([company_id], |r| {
        let pid: i64 = r.get(1)?;
        let amt: i64 = r.get(2)?;
        let desc: Option<String> = r.get(3)?;
        let yuan = amt as f64 / 100.0;
        let name = match desc {
            Some(d) if !d.is_empty() => format!("成本 ¥{:.2} ({d})", yuan),
            _ => format!("成本 ¥{:.2}", yuan),
        };
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "cost_entry".into(),
            name,
            deleted_at: r.get::<_, String>(4)?,
            project_id: Some(pid),
        })
    })?;
    for r in rows { out.push(r?); }

    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}

pub(crate) fn restore_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    match entity_type {
        "project" => soft_delete::restore_project(conn, id),
        "cost_entry" => soft_delete::restore_cost_entry(conn, id),
        other => Err(AppError::Validation(format!("未知实体类型：{other}"))),
    }
}

pub(crate) fn purge_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    let table = match entity_type {
        "project" => "projects",
        "cost_entry" => "cost_entries",
        other => return Err(AppError::Validation(format!("未知实体类型：{other}"))),
    };
    let tx = conn.unchecked_transaction()?;
    if entity_type == "project" {
        // physically delete cost_entries first to respect FK
        tx.execute("DELETE FROM cost_entries WHERE project_id = ?1", [id])?;
    }
    let n = tx.execute(
        &format!("DELETE FROM {table} WHERE id = ?1 AND deleted_at IS NOT NULL"),
        [id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "trash_item", id });
    }
    tx.commit()?;
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
pub fn list_trash(state: tauri::State<AppState>, company_id: i64) -> AppResult<Vec<TrashItem>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn restore_trash_item(
    state: tauri::State<AppState>,
    entity_type: String,
    id: i64,
) -> AppResult<()> {
    with_conn(&state, |c| restore_impl(c, &entity_type, id))
}
#[tauri::command]
pub fn purge_trash_item(
    state: tauri::State<AppState>,
    entity_type: String,
    id: i64,
) -> AppResult<()> {
    with_conn(&state, |c| purge_impl(c, &entity_type, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(1, 'X', 1, 0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-01', 200)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    #[test]
    fn list_returns_soft_deleted() {
        let db = TestDb::new();
        soft_delete::soft_delete_cost_entry(&db.conn, 1).unwrap();
        let items = list_impl(&db.conn, 1).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].entity_type, "cost_entry");
        assert_eq!(items[0].project_id, Some(1));
    }

    #[test]
    fn restore_brings_back() {
        let db = TestDb::new();
        soft_delete::soft_delete_cost_entry(&db.conn, 1).unwrap();
        restore_impl(&db.conn, "cost_entry", 1).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }

    #[test]
    fn purge_project_cascades_physical_delete() {
        let db = TestDb::new();
        soft_delete::soft_delete_project(&db.conn, 1).unwrap();
        purge_impl(&db.conn, "project", 1).unwrap();
        let n: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE id = 1",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(n, 0);
        let n2: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM cost_entries WHERE project_id = 1",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn unknown_entity_type_validation_error() {
        let db = TestDb::new();
        let err = restore_impl(&db.conn, "task", 1).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
```

- [ ] **Step 2：`commands/mod.rs` 加 `pub mod trash;`**

- [ ] **Step 3：注册命令到 `lib.rs`**

```rust
            commands::trash::list_trash,
            commands::trash::restore_trash_item,
            commands::trash::purge_trash_item,
```

- [ ] **Step 4：跑测试**

```bash
cargo test 2>&1 | tail -15
```
预期：39 + 4 = 43 passed.

- [ ] **Step 5：Commit**

```bash
git add src-tauri/src
git commit -m "feat(trash): 回收站列表/恢复/物理清理命令"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：回收站后端（按当前公司列出已软删项目/成本，整组恢复/物理清理）。

---

## Task 7: 前端基础库 + 类型 + ipc 扩展

**Files:**
- Create: `src/lib/money.ts`
- Create: `src/lib/status.ts`
- Create: `src/components/forms/MoneyInput.tsx`
- Modify: `src/types/index.ts`
- Modify: `src/i18n/zh-CN.json`（加 项目/科目/状态/成本/回收站文案）

**Interfaces:**
- Produces:
  - `toCents(yuan: string | number): number`（"123.45" → 12345，空串 → 0，NaN/负 → 抛 RangeError）
  - `fromCents(cents: number): string`（12345 → "123.45"）
  - `formatCNY(cents: number): string`（12345 → "¥123.45"，带千位分隔）
  - `<MoneyInput value={cents} onChange={(cents) => ...} />` 组件
  - `STATUS_OPTIONS` 数组：6 个 `{value, label, color}`
  - `statusLabel(s: string): string` 与 `statusBadgeClass(s: string): string`
  - TS 类型：`Project`, `ProjectInput`, `CostCategory`, `CostCategoryInput`, `CostEntry`, `CostEntryInput`, `ProjectCostSummary`, `CategoryBreakdown`, `TrashItem`
- Consumes: 无新增

- [ ] **Step 1：写 `src/lib/money.ts`**

```typescript
// All money values cross the IPC boundary as INTEGER cents.
// These helpers convert at the UI edge only.

export function toCents(yuan: string | number): number {
  if (yuan === "" || yuan === null || yuan === undefined) return 0;
  const n = typeof yuan === "number" ? yuan : Number(yuan);
  if (!Number.isFinite(n)) throw new RangeError(`invalid amount: ${yuan}`);
  if (n < 0) throw new RangeError("amount cannot be negative");
  return Math.round(n * 100);
}

export function fromCents(cents: number): string {
  return (cents / 100).toFixed(2);
}

const CNY = new Intl.NumberFormat("zh-CN", {
  style: "currency",
  currency: "CNY",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function formatCNY(cents: number): string {
  return CNY.format(cents / 100);
}
```

- [ ] **Step 2：写 `src/lib/status.ts`**

```typescript
export const STATUS_VALUES = [
  "negotiating",
  "pending",
  "in_progress",
  "delivered",
  "settled",
  "archived",
] as const;

export type ProjectStatus = typeof STATUS_VALUES[number];

const LABELS: Record<ProjectStatus, string> = {
  negotiating: "商务洽谈",
  pending: "待启动",
  in_progress: "进行中",
  delivered: "已交付待结款",
  settled: "已结款",
  archived: "已归档",
};

const BADGE_CLASSES: Record<ProjectStatus, string> = {
  negotiating: "bg-slate-100 text-slate-700",
  pending: "bg-blue-100 text-blue-700",
  in_progress: "bg-amber-100 text-amber-700",
  delivered: "bg-purple-100 text-purple-700",
  settled: "bg-emerald-100 text-emerald-700",
  archived: "bg-zinc-100 text-zinc-500",
};

export function statusLabel(s: string): string {
  return LABELS[s as ProjectStatus] ?? s;
}

export function statusBadgeClass(s: string): string {
  return BADGE_CLASSES[s as ProjectStatus] ?? "bg-zinc-100 text-zinc-700";
}

export const STATUS_OPTIONS = STATUS_VALUES.map((v) => ({
  value: v,
  label: LABELS[v],
}));
```

- [ ] **Step 3：写 `src/components/forms/MoneyInput.tsx`**

```typescript
import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { fromCents, toCents } from "@/lib/money";

interface Props {
  value: number; // cents
  onChange: (cents: number) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function MoneyInput({ value, onChange, disabled, placeholder }: Props) {
  const [text, setText] = useState(value > 0 ? fromCents(value) : "");

  useEffect(() => {
    // sync when parent resets value (e.g., dialog close)
    setText(value > 0 ? fromCents(value) : "");
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    try {
      onChange(toCents(raw));
    } catch {
      // keep text; do not propagate invalid value
    }
  };

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">¥</span>
      <Input
        inputMode="decimal"
        value={text}
        disabled={disabled}
        placeholder={placeholder ?? "0.00"}
        onChange={(e) => commit(e.target.value)}
      />
    </div>
  );
}
```

- [ ] **Step 4：扩展 `src/types/index.ts`**

在文件末尾追加（保留已有 Company / CompanyInput）：

```typescript
export interface CostCategory {
  id: number;
  company_id: number;
  name: string;
  is_system: boolean;
  sort_order: number;
}

export interface CostCategoryInput {
  name: string;
}

export interface Project {
  id: number;
  company_id: number;
  name: string;
  client_name: string | null;
  status: string;
  contract_amount_cents: number;
  contract_amount_is_tax_inclusive: boolean;
  tax_rate: number;
  start_date: string | null;
  end_date: string | null;
  actual_delivered_at: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProjectInput {
  name: string;
  client_name?: string | null;
  status?: string | null;
  contract_amount_cents?: number | null;
  contract_amount_is_tax_inclusive?: boolean | null;
  tax_rate?: number | null;
  start_date?: string | null;
  end_date?: string | null;
  actual_delivered_at?: string | null;
  notes?: string | null;
}

export interface CostEntry {
  id: number;
  project_id: number;
  category_id: number;
  incurred_at: string;
  amount_cents: number;
  description: string | null;
  notes: string | null;
  created_at: string;
}

export interface CostEntryInput {
  category_id: number;
  incurred_at: string;
  amount_cents: number;
  description?: string | null;
  notes?: string | null;
}

export interface CategoryBreakdown {
  category_id: number;
  category_name: string;
  total_cents: number;
}

export interface ProjectCostSummary {
  total_cents: number;
  by_category: CategoryBreakdown[];
}

export interface TrashItem {
  id: number;
  entity_type: "project" | "cost_entry";
  name: string;
  deleted_at: string;
  project_id: number | null;
}
```

- [ ] **Step 5：扩展 `src/i18n/zh-CN.json`**

在已有 `common` 对象之前/之后加（与现有保持平级结构）：

```json
  "category": {
    "title": "成本科目",
    "create": "新建科目",
    "edit": "编辑科目",
    "delete": "删除",
    "name": "名称",
    "nameRequired": "名称必填",
    "preset": "预设",
    "custom": "自定义",
    "empty": "暂无科目，新建一个或重新进入页面自动初始化"
  },
  "project": {
    "title": "项目",
    "create": "新建项目",
    "edit": "编辑项目",
    "name": "项目名",
    "client": "客户",
    "contractAmount": "合同总价",
    "taxInclusive": "含税",
    "taxRate": "税率",
    "startDate": "开始日期",
    "endDate": "结束日期",
    "deliveredAt": "实际交付日",
    "notes": "备注",
    "empty": "当前公司还没有项目",
    "filterByStatus": "按状态筛选",
    "allStatuses": "全部状态",
    "save": "保存",
    "nameRequired": "项目名必填",
    "delete": "删除项目",
    "deleteConfirm": "确认删除项目「{{name}}」？该项目下的成本将一并被软删除，可在回收站恢复。",
    "openDetail": "查看详情"
  },
  "cost": {
    "title": "成本",
    "add": "录入一笔",
    "edit": "编辑",
    "delete": "删除",
    "deleteConfirm": "确认删除该成本记录？",
    "category": "科目",
    "incurredAt": "发生日期",
    "amount": "金额",
    "description": "摘要",
    "notes": "备注",
    "empty": "该项目还没有成本记录",
    "totalLabel": "成本合计",
    "save": "保存",
    "categoryRequired": "请选择科目",
    "incurredAtRequired": "发生日期必填",
    "amountInvalid": "金额需 ≥ 0"
  },
  "trash": {
    "title": "回收站",
    "empty": "回收站是空的",
    "restore": "恢复",
    "purge": "彻底删除",
    "purgeConfirm": "彻底删除将无法恢复，确定？",
    "type": "类型",
    "name": "名称",
    "deletedAt": "删除时间",
    "entityProject": "项目",
    "entityCostEntry": "成本",
    "projectGone": "（所属项目已删除）"
  },
  "status": {
    "negotiating": "商务洽谈",
    "pending": "待启动",
    "in_progress": "进行中",
    "delivered": "已交付待结款",
    "settled": "已结款",
    "archived": "已归档"
  }
```

并在 `nav` 对象里把现有键保留，确保 `categories`、`projects`、`trash` 都存在（M1 已有，校对一遍即可）。

- [ ] **Step 6：补 shadcn 组件**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm dlx shadcn@latest add tabs select badge table textarea
```
预期：分别在 `src/components/ui/` 加 5 个文件。

- [ ] **Step 7：TS 编译验证**

```bash
pnpm tsc --noEmit
```
预期：0 errors（MoneyInput / lib / types 都能解析）。

- [ ] **Step 8：Commit**

```bash
git add src/lib src/components/forms src/components/ui src/types src/i18n components.json
git commit -m "feat(ui): money 工具 + 项目状态映射 + MoneyInput + 类型补全"
```

- [ ] **Step 9：CHANGELOG**

`/changelog`：money 工具（cents↔元 + formatCNY）；状态映射工具；MoneyInput 组件；ProjectStatus 等 TS 类型；shadcn `tabs/select/badge/table/textarea` 组件。

---

## Task 8: 成本科目 UI

**Files:**
- Create: `src/stores/categories.ts`
- Create: `src/routes/categories.tsx`
- Modify: `src/App.tsx`（替换占位 `/settings` 路由旁的占位 `categories`；M1 里没有 `categories` 路由项，本 task 新增）
- Modify: `src/components/layout/Sidebar.tsx`（在公司管理下方插入"成本科目"项）

**Interfaces:**
- Produces:
  - `useCategoriesStore`：`{ list, loaded, loadFor(companyId), create(companyId, name), update(id, name), remove(id) }`
- Consumes:
  - `useCompanyStore.currentId`
  - Task 3 命令：`list_categories`, `create_category`, `update_category`, `delete_category`, `seed_preset_categories_if_empty`

- [ ] **Step 1：写 `src/stores/categories.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { CostCategory } from "@/types";

interface S {
  list: CostCategory[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  create: (companyId: number, name: string) => Promise<void>;
  update: (id: number, name: string) => Promise<void>;
  remove: (id: number) => Promise<void>;
}

export const useCategoriesStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    // seed presets first (idempotent), then refresh
    const list = await call<CostCategory[]>("seed_preset_categories_if_empty", {
      companyId,
    });
    set({ list, loadedForCompany: companyId });
  },
  async create(companyId, name) {
    const c = await call<CostCategory>("create_category", {
      companyId,
      input: { name },
    });
    set({ list: [...get().list, c] });
  },
  async update(id, name) {
    const c = await call<CostCategory>("update_category", { id, input: { name } });
    set({ list: get().list.map((x) => (x.id === id ? c : x)) });
  },
  async remove(id) {
    await call<void>("delete_category", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
}));
```

- [ ] **Step 2：写 `src/routes/categories.tsx`**

```typescript
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { useCompanyStore } from "@/stores/company";
import { useCategoriesStore } from "@/stores/categories";
import type { CostCategory } from "@/types";

export default function CategoriesPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, loadFor, create, update, remove } =
    useCategoriesStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<CostCategory | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("category.title")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("category.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("category.create")}</DialogTitle></DialogHeader>
            <NameForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (name) => {
                try {
                  await create(currentId, name);
                  setOpenNew(false);
                } catch (e: any) {
                  toast.error(t("common.error", { msg: String(e) }));
                }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("category.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-2">
          {list.map((c) => (
            <Card key={c.id}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 py-3">
                <CardTitle className="text-sm font-medium flex items-center gap-2">
                  <span>{c.name}</span>
                  <Badge variant={c.is_system ? "secondary" : "outline"}>
                    {c.is_system ? t("category.preset") : t("category.custom")}
                  </Badge>
                </CardTitle>
                <div className="flex gap-2">
                  {!c.is_system && (
                    <>
                      <Button size="sm" variant="ghost" onClick={() => setEditing(c)}>
                        {t("category.edit")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={async () => {
                          try { await remove(c.id); }
                          catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("category.delete")}
                      </Button>
                    </>
                  )}
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("category.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <NameForm
              initial={editing.name}
              onCancel={() => setEditing(null)}
              onSubmit={async (name) => {
                try {
                  await update(editing.id, name);
                  setEditing(null);
                } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function NameForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  onSubmit: (name: string) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial ?? "");
  const [busy, setBusy] = useState(false);
  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("category.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!name.trim()) return;
            setBusy(true);
            try { await onSubmit(name.trim()); } finally { setBusy(false); }
          }}
        >
          {t("common.confirm")}
        </Button>
      </DialogFooter>
    </div>
  );
}
```

- [ ] **Step 3：`src/components/layout/Sidebar.tsx` 加菜单项**

把 ITEMS 数组改为（保留 dashboard/companies/settings，新增 projects/categories/trash）：

```typescript
import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { LayoutDashboard, Building2, FolderKanban, Tag, Trash2, Settings } from "lucide-react";

const ITEMS = [
  { to: "/dashboard", icon: LayoutDashboard, key: "nav.dashboard" as const },
  { to: "/projects", icon: FolderKanban, key: "nav.projects" as const },
  { to: "/categories", icon: Tag, key: "nav.categories" as const },
  { to: "/companies", icon: Building2, key: "nav.companies" as const },
  { to: "/trash", icon: Trash2, key: "nav.trash" as const },
  { to: "/settings", icon: Settings, key: "nav.settings" as const },
];

export function Sidebar() {
  const { t } = useTranslation();
  return (
    <aside className="w-56 border-r bg-background flex flex-col">
      <div className="px-4 h-14 flex items-center font-semibold">{t("app.name")}</div>
      <nav className="flex-1 px-2 space-y-1">
        {ITEMS.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2 px-3 py-2 rounded-md text-sm hover:bg-accent",
                isActive && "bg-accent",
              )
            }
          >
            <it.icon className="h-4 w-4" />
            <span>{t(it.key)}</span>
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
```

- [ ] **Step 4：在 `src/App.tsx` 注册 `/categories` 路由**

找到 `<Route path="/" element={<AppLayout />}>` 内的 Routes 块，把原 `<Route path="companies" .../>` 与 `<Route path="settings" .../>` 保留，在之间加：

```typescript
            <Route path="projects" element={<div>项目（Task 9 实现）</div>} />
            <Route path="categories" element={<CategoriesPage />} />
            <Route path="trash" element={<div>回收站（Task 11 实现）</div>} />
```

并在文件顶部加 import：`import CategoriesPage from "@/routes/categories";`

- [ ] **Step 5：TS 编译 + dev 启动**

```bash
pnpm tsc --noEmit
```
预期：0 errors。

- [ ] **Step 6：Commit**

```bash
git add src/stores/categories.ts src/routes/categories.tsx src/components/layout/Sidebar.tsx src/App.tsx
git commit -m "feat(ui): 成本科目管理 + sidebar 加项目/科目/回收站"
```

- [ ] **Step 7：CHANGELOG**

`/changelog`：成本科目管理页（新建/编辑/删除自定义，预设科目只读 + Badge 标记）；sidebar 加 项目/科目/回收站 三项。

---

## Task 9: 项目列表 UI

**Files:**
- Create: `src/stores/projects.ts`
- Create: `src/routes/projects/list.tsx`
- Modify: `src/App.tsx`（把 Task 8 留的 `/projects` 占位换成真路由 + `/projects/:id` 占位）

**Interfaces:**
- Produces:
  - `useProjectsStore`：`{ list, loadedForCompany, loadFor(companyId, statusFilter?), create(companyId, input), update(id, input), setStatus(id, status), softDelete(id), refresh() }`
- Consumes:
  - Task 4 命令；Task 7 状态映射 + MoneyInput

- [ ] **Step 1：写 `src/stores/projects.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Project, ProjectInput } from "@/types";

interface S {
  list: Project[];
  loadedForCompany: number | null;
  statusFilter: string | null;
  loadFor: (companyId: number, statusFilter?: string | null) => Promise<void>;
  create: (companyId: number, input: ProjectInput) => Promise<Project>;
  update: (id: number, input: ProjectInput) => Promise<Project>;
  setStatus: (id: number, status: string) => Promise<void>;
  softDelete: (id: number) => Promise<void>;
}

export const useProjectsStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  statusFilter: null,
  async loadFor(companyId, statusFilter = null) {
    const list = await call<Project[]>("list_projects", {
      companyId,
      status: statusFilter,
    });
    set({ list, loadedForCompany: companyId, statusFilter });
  },
  async create(companyId, input) {
    const p = await call<Project>("create_project", { companyId, input });
    set({ list: [p, ...get().list] });
    return p;
  },
  async update(id, input) {
    const p = await call<Project>("update_project", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? p : x)) });
    return p;
  },
  async setStatus(id, status) {
    const p = await call<Project>("set_project_status", { id, status });
    set({ list: get().list.map((x) => (x.id === id ? p : x)) });
  },
  async softDelete(id) {
    await call<void>("delete_project", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
}));
```

- [ ] **Step 2：写 `src/routes/projects/list.tsx`**

```typescript
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { STATUS_OPTIONS, statusBadgeClass, statusLabel } from "@/lib/status";
import { useCompanyStore } from "@/stores/company";
import { useProjectsStore } from "@/stores/projects";
import type { Project, ProjectInput } from "@/types";

export default function ProjectsListPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, statusFilter, loadFor, create, update, softDelete } =
    useProjectsStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Project | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) {
      loadFor(currentId, null);
    }
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("project.title")}</h1>
        <div className="flex items-center gap-2">
          <Select
            value={statusFilter ?? "__all"}
            onValueChange={(v) => loadFor(currentId, v === "__all" ? null : v)}
          >
            <SelectTrigger className="w-40"><SelectValue placeholder={t("project.filterByStatus")} /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__all">{t("project.allStatuses")}</SelectItem>
              {STATUS_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Dialog open={openNew} onOpenChange={setOpenNew}>
            <DialogTrigger asChild><Button>{t("project.create")}</Button></DialogTrigger>
            <DialogContent className="max-w-lg">
              <DialogHeader><DialogTitle>{t("project.create")}</DialogTitle></DialogHeader>
              <ProjectForm
                onCancel={() => setOpenNew(false)}
                onSubmit={async (input) => {
                  try {
                    await create(currentId, input);
                    setOpenNew(false);
                  } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                }}
              />
            </DialogContent>
          </Dialog>
        </div>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("project.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {list.map((p) => (
            <Card key={p.id}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0">
                <div className="space-y-1">
                  <CardTitle className="text-base flex items-center gap-2">
                    <Link to={`/projects/${p.id}`} className="hover:underline">{p.name}</Link>
                    <span className={`text-xs px-2 py-0.5 rounded ${statusBadgeClass(p.status)}`}>
                      {statusLabel(p.status)}
                    </span>
                  </CardTitle>
                  {p.client_name && (
                    <div className="text-xs text-muted-foreground">{t("project.client")}：{p.client_name}</div>
                  )}
                </div>
                <div className="flex items-center gap-3">
                  <div className="text-right">
                    <div className="text-sm font-medium">{formatCNY(p.contract_amount_cents)}</div>
                    <div className="text-xs text-muted-foreground">
                      {p.contract_amount_is_tax_inclusive ? "含税" : "不含税"} · 税率 {(p.tax_rate * 100).toFixed(2)}%
                    </div>
                  </div>
                  <div className="flex gap-1">
                    <Button asChild size="sm" variant="ghost"><Link to={`/projects/${p.id}`}>{t("project.openDetail")}</Link></Button>
                    <Button size="sm" variant="ghost" onClick={() => setEditing(p)}>编辑</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={async () => {
                        if (!confirm(t("project.deleteConfirm", { name: p.name }))) return;
                        try { await softDelete(p.id); }
                        catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >
                      {t("project.delete")}
                    </Button>
                  </div>
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader><DialogTitle>{t("project.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <ProjectForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try {
                  await update(editing.id, input);
                  setEditing(null);
                } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ProjectForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial?: Project;
  onSubmit: (input: ProjectInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [client, setClient] = useState(initial?.client_name ?? "");
  const [status, setStatus] = useState(initial?.status ?? "pending");
  const [amount, setAmount] = useState(initial?.contract_amount_cents ?? 0);
  const [inclusive, setInclusive] = useState(initial?.contract_amount_is_tax_inclusive ?? true);
  const [taxRate, setTaxRate] = useState(String(initial?.tax_rate ?? 0.06));
  const [startDate, setStartDate] = useState(initial?.start_date ?? "");
  const [endDate, setEndDate] = useState(initial?.end_date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("project.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        client_name: client.trim() || null,
        status,
        contract_amount_cents: amount,
        contract_amount_is_tax_inclusive: inclusive,
        tax_rate: Number(taxRate),
        start_date: startDate || null,
        end_date: endDate || null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("project.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("project.client")}</Label>
          <Input value={client} onChange={(e) => setClient(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>状态</Label>
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {STATUS_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("project.contractAmount")}</Label>
          <MoneyInput value={amount} onChange={setAmount} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.taxInclusive")}</Label>
          <Select value={inclusive ? "1" : "0"} onValueChange={(v) => setInclusive(v === "1")}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="1">含税</SelectItem>
              <SelectItem value="0">不含税</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1">
          <Label>{t("project.taxRate")}</Label>
          <Input type="number" step="0.01" min="0" max="0.99" value={taxRate} onChange={(e) => setTaxRate(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.startDate")}</Label>
          <Input type="date" value={startDate ?? ""} onChange={(e) => setStartDate(e.target.value)} />
        </div>
        <div className="space-y-1">
          <Label>{t("project.endDate")}</Label>
          <Input type="date" value={endDate ?? ""} onChange={(e) => setEndDate(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("project.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("project.save")}</Button>
      </DialogFooter>
    </div>
  );
}
```

- [ ] **Step 3：替换 `src/App.tsx` 中 `/projects` 路由**

把 Task 8 留的 `<Route path="projects" element={<div>项目（Task 9 实现）</div>} />` 改成：

```typescript
            <Route path="projects" element={<ProjectsListPage />} />
            <Route path="projects/:id" element={<div>项目详情（Task 10 实现）</div>} />
```

并加 import：`import ProjectsListPage from "@/routes/projects/list";`

- [ ] **Step 4：TS 编译**

```bash
pnpm tsc --noEmit
```
预期：0 errors。

- [ ] **Step 5：Commit**

```bash
git add src/stores/projects.ts src/routes/projects/list.tsx src/App.tsx
git commit -m "feat(projects): 项目列表 + 新建/编辑/删除 + 状态筛选"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：项目列表页（新建/编辑/软删/按状态筛选，含金额展示、合同含税切换、6 状态徽章）。

---

## Task 10: 项目详情（Tabs 框架 + 概览 Tab + 成本 Tab）

**Files:**
- Create: `src/stores/costs.ts`
- Create: `src/routes/projects/detail.tsx`
- Modify: `src/App.tsx`（把 Task 9 留的 `/projects/:id` 占位换成真路由）

**Interfaces:**
- Produces:
  - `useCostsStore`：`{ entriesByProject, summaryByProject, loadFor(projectId), create(projectId, input), update(id, input), remove(id), refreshSummary(projectId) }`
  - 路由 `/projects/:id` 渲染 Tabs：概览 / 成本 / 收款（占位） / 任务+工时（占位） / 附件（占位）
- Consumes:
  - Task 5 命令：`list_cost_entries`, `create_cost_entry`, `update_cost_entry`, `delete_cost_entry`, `get_project_cost_summary`
  - Task 4 命令：`get_project`, `set_project_status`
  - Task 8 store：`useCategoriesStore`

- [ ] **Step 1：写 `src/stores/costs.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { CostEntry, CostEntryInput, ProjectCostSummary } from "@/types";

interface S {
  entriesByProject: Record<number, CostEntry[]>;
  summaryByProject: Record<number, ProjectCostSummary>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: CostEntryInput) => Promise<void>;
  update: (id: number, input: CostEntryInput, projectId: number) => Promise<void>;
  remove: (id: number, projectId: number) => Promise<void>;
}

async function refresh(projectId: number) {
  const [entries, summary] = await Promise.all([
    call<CostEntry[]>("list_cost_entries", { projectId }),
    call<ProjectCostSummary>("get_project_cost_summary", { projectId }),
  ]);
  return { entries, summary };
}

export const useCostsStore = create<S>((set, get) => ({
  entriesByProject: {},
  summaryByProject: {},
  async loadFor(projectId) {
    const { entries, summary } = await refresh(projectId);
    set({
      entriesByProject: { ...get().entriesByProject, [projectId]: entries },
      summaryByProject: { ...get().summaryByProject, [projectId]: summary },
    });
  },
  async create(projectId, input) {
    await call<CostEntry>("create_cost_entry", { projectId, input });
    await get().loadFor(projectId);
  },
  async update(id, input, projectId) {
    await call<CostEntry>("update_cost_entry", { id, input });
    await get().loadFor(projectId);
  },
  async remove(id, projectId) {
    await call<void>("delete_cost_entry", { id });
    await get().loadFor(projectId);
  },
}));
```

- [ ] **Step 2：写 `src/routes/projects/detail.tsx`**

```typescript
import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { STATUS_OPTIONS, statusBadgeClass, statusLabel } from "@/lib/status";
import { call } from "@/lib/ipc";
import { useCompanyStore } from "@/stores/company";
import { useCategoriesStore } from "@/stores/categories";
import { useCostsStore } from "@/stores/costs";
import type { CostEntry, CostEntryInput, Project } from "@/types";

export default function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const pid = id ? Number(id) : NaN;
  const currentCompanyId = useCompanyStore((s) => s.currentId);
  const { loadedForCompany, loadFor: loadCats } = useCategoriesStore();
  const { loadFor: loadCosts } = useCostsStore();
  const [project, setProject] = useState<Project | null>(null);

  useEffect(() => {
    if (Number.isNaN(pid)) return;
    call<Project>("get_project", { id: pid })
      .then(setProject)
      .catch((e) => {
        toast.error(t("common.error", { msg: String(e) }));
        navigate("/projects");
      });
  }, [pid, navigate, t]);

  useEffect(() => {
    if (project && currentCompanyId === project.company_id && loadedForCompany !== currentCompanyId) {
      loadCats(currentCompanyId);
    }
  }, [project, currentCompanyId, loadedForCompany, loadCats]);

  useEffect(() => {
    if (!Number.isNaN(pid)) loadCosts(pid);
  }, [pid, loadCosts]);

  if (!project) return null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h1 className="text-xl font-semibold">{project.name}</h1>
          <span className={`text-xs px-2 py-0.5 rounded ${statusBadgeClass(project.status)}`}>
            {statusLabel(project.status)}
          </span>
        </div>
        <Select
          value={project.status}
          onValueChange={async (v) => {
            try {
              const p = await call<Project>("set_project_status", { id: project.id, status: v });
              setProject(p);
            } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
          }}
        >
          <SelectTrigger className="w-40"><SelectValue /></SelectTrigger>
          <SelectContent>
            {STATUS_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">概览</TabsTrigger>
          <TabsTrigger value="costs">成本</TabsTrigger>
          <TabsTrigger value="payments" disabled>收款（M3）</TabsTrigger>
          <TabsTrigger value="tasks" disabled>任务+工时（M3）</TabsTrigger>
          <TabsTrigger value="attachments" disabled>附件（M4）</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="mt-4">
          <OverviewPanel project={project} />
        </TabsContent>

        <TabsContent value="costs" className="mt-4">
          <CostsPanel projectId={project.id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function OverviewPanel({ project }: { project: Project }) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <Card>
        <CardHeader><CardTitle className="text-sm">合同总价</CardTitle></CardHeader>
        <CardContent className="text-2xl font-semibold">
          {formatCNY(project.contract_amount_cents)}
          <div className="text-xs text-muted-foreground mt-1">
            {project.contract_amount_is_tax_inclusive ? "含税" : "不含税"} · 税率 {(project.tax_rate * 100).toFixed(2)}%
          </div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle className="text-sm">客户</CardTitle></CardHeader>
        <CardContent>{project.client_name ?? "—"}</CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle className="text-sm">开始日期</CardTitle></CardHeader>
        <CardContent>{project.start_date ?? "—"}</CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle className="text-sm">结束日期</CardTitle></CardHeader>
        <CardContent>{project.end_date ?? "—"}</CardContent>
      </Card>
      {project.notes && (
        <Card className="col-span-2">
          <CardHeader><CardTitle className="text-sm">备注</CardTitle></CardHeader>
          <CardContent className="whitespace-pre-wrap text-sm">{project.notes}</CardContent>
        </Card>
      )}
    </div>
  );
}

function CostsPanel({ projectId }: { projectId: number }) {
  const { t } = useTranslation();
  const { list: cats } = useCategoriesStore();
  const { entriesByProject, summaryByProject, create, update, remove } = useCostsStore();
  const entries = entriesByProject[projectId] ?? [];
  const summary = summaryByProject[projectId];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<CostEntry | null>(null);

  const findCatName = (cid: number) => cats.find((c) => c.id === cid)?.name ?? `#${cid}`;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="text-sm">
          {t("cost.totalLabel")}：<span className="font-semibold">{formatCNY(summary?.total_cents ?? 0)}</span>
        </div>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("cost.add")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("cost.add")}</DialogTitle></DialogHeader>
            <CostForm
              cats={cats}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {summary && summary.by_category.length > 0 && (
        <Card>
          <CardHeader><CardTitle className="text-sm">按科目汇总</CardTitle></CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-2">
              {summary.by_category.map((b) => (
                <div key={b.category_id} className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{b.category_name}</span>
                  <span className="font-medium">{formatCNY(b.total_cents)}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {entries.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("cost.empty")}</CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-28">{t("cost.incurredAt")}</TableHead>
                  <TableHead className="w-32">{t("cost.category")}</TableHead>
                  <TableHead className="text-right w-32">{t("cost.amount")}</TableHead>
                  <TableHead>{t("cost.description")}</TableHead>
                  <TableHead className="w-32 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((e) => (
                  <TableRow key={e.id}>
                    <TableCell>{e.incurred_at}</TableCell>
                    <TableCell>{findCatName(e.category_id)}</TableCell>
                    <TableCell className="text-right">{formatCNY(e.amount_cents)}</TableCell>
                    <TableCell className="text-sm text-muted-foreground">{e.description ?? ""}</TableCell>
                    <TableCell className="text-right">
                      <Button size="sm" variant="ghost" onClick={() => setEditing(e)}>{t("cost.edit")}</Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={async () => {
                          if (!confirm(t("cost.deleteConfirm"))) return;
                          try { await remove(e.id, projectId); }
                          catch (err: any) { toast.error(t("common.error", { msg: String(err) })); }
                        }}
                      >
                        {t("cost.delete")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("cost.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <CostForm
              cats={cats}
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try {
                  await update(editing.id, input, projectId);
                  setEditing(null);
                } catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function CostForm({
  cats,
  initial,
  onSubmit,
  onCancel,
}: {
  cats: { id: number; name: string }[];
  initial?: CostEntry;
  onSubmit: (input: CostEntryInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [categoryId, setCategoryId] = useState(initial?.category_id ?? cats[0]?.id ?? 0);
  const [date, setDate] = useState(initial?.incurred_at ?? new Date().toISOString().slice(0, 10));
  const [amount, setAmount] = useState(initial?.amount_cents ?? 0);
  const [desc, setDesc] = useState(initial?.description ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!categoryId) return toast.error(t("cost.categoryRequired"));
    if (!date) return toast.error(t("cost.incurredAtRequired"));
    if (amount < 0) return toast.error(t("cost.amountInvalid"));
    setBusy(true);
    try {
      await onSubmit({
        category_id: categoryId,
        incurred_at: date,
        amount_cents: amount,
        description: desc.trim() || null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("cost.category")}</Label>
          <Select value={String(categoryId)} onValueChange={(v) => setCategoryId(Number(v))}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {cats.map((c) => (
                <SelectItem key={c.id} value={String(c.id)}>{c.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label>{t("cost.incurredAt")}</Label>
          <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("cost.amount")}</Label>
        <MoneyInput value={amount} onChange={setAmount} />
      </div>
      <div className="space-y-1">
        <Label>{t("cost.description")}</Label>
        <Input value={desc} onChange={(e) => setDesc(e.target.value)} />
      </div>
      <div className="space-y-1">
        <Label>{t("cost.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("cost.save")}</Button>
      </DialogFooter>
    </div>
  );
}
```

- [ ] **Step 3：注册 `/projects/:id` 真路由**

把 Task 9 留的 `<Route path="projects/:id" element={<div>项目详情（Task 10 实现）</div>} />` 改成：

```typescript
            <Route path="projects/:id" element={<ProjectDetailPage />} />
```

并加 import：`import ProjectDetailPage from "@/routes/projects/detail";`

- [ ] **Step 4：TS 编译**

```bash
pnpm tsc --noEmit
```
预期：0 errors。

- [ ] **Step 5：Commit**

```bash
git add src/stores/costs.ts src/routes/projects/detail.tsx src/App.tsx
git commit -m "feat(projects): 项目详情 tabs + 概览 + 成本录入"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：项目详情 Tabs 框架（含 5 Tab，其中收款/任务+工时/附件占位禁用）；概览 Tab；成本 Tab（按科目汇总 + 成本明细表格 + 增改删）；详情页可切换项目状态。

---

## Task 11: 回收站 UI + 验收 + CHANGELOG 汇总

**Files:**
- Create: `src/stores/trash.ts`
- Create: `src/routes/trash.tsx`
- Modify: `src/App.tsx`（把 `/trash` 占位换成真路由）

**Interfaces:**
- Produces:
  - `useTrashStore`：`{ items, loadedForCompany, loadFor(companyId), restore(entityType, id), purge(entityType, id) }`
- Consumes:
  - Task 6 命令：`list_trash`, `restore_trash_item`, `purge_trash_item`

- [ ] **Step 1：写 `src/stores/trash.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TrashItem } from "@/types";

interface S {
  items: TrashItem[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  restore: (entityType: string, id: number, companyId: number) => Promise<void>;
  purge: (entityType: string, id: number, companyId: number) => Promise<void>;
}

export const useTrashStore = create<S>((set, get) => ({
  items: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    const items = await call<TrashItem[]>("list_trash", { companyId });
    set({ items, loadedForCompany: companyId });
  },
  async restore(entityType, id, companyId) {
    await call<void>("restore_trash_item", { entityType, id });
    await get().loadFor(companyId);
  },
  async purge(entityType, id, companyId) {
    await call<void>("purge_trash_item", { entityType, id });
    await get().loadFor(companyId);
  },
}));
```

- [ ] **Step 2：写 `src/routes/trash.tsx`**

```typescript
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { useCompanyStore } from "@/stores/company";
import { useTrashStore } from "@/stores/trash";

const TYPE_LABEL: Record<string, string> = {
  project: "项目",
  cost_entry: "成本",
};

export default function TrashPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { items, loadedForCompany, loadFor, restore, purge } = useTrashStore();

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("trash.title")}</h1>
      {items.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("trash.empty")}</CardContent></Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-20">{t("trash.type")}</TableHead>
                  <TableHead>{t("trash.name")}</TableHead>
                  <TableHead className="w-44">{t("trash.deletedAt")}</TableHead>
                  <TableHead className="w-44 text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.map((it) => (
                  <TableRow key={`${it.entity_type}-${it.id}`}>
                    <TableCell>
                      <Badge variant="outline">{TYPE_LABEL[it.entity_type] ?? it.entity_type}</Badge>
                    </TableCell>
                    <TableCell>{it.name}</TableCell>
                    <TableCell className="text-sm text-muted-foreground">{it.deleted_at}</TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={async () => {
                          try { await restore(it.entity_type, it.id, currentId); }
                          catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("trash.restore")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={async () => {
                          if (!confirm(t("trash.purgeConfirm"))) return;
                          try { await purge(it.entity_type, it.id, currentId); }
                          catch (e: any) { toast.error(t("common.error", { msg: String(e) })); }
                        }}
                      >
                        {t("trash.purge")}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
```

- [ ] **Step 3：注册 `/trash` 真路由**

把 Task 8 留的 `<Route path="trash" element={<div>回收站（Task 11 实现）</div>} />` 改成：

```typescript
            <Route path="trash" element={<TrashPage />} />
```

并加 import：`import TrashPage from "@/routes/trash";`

- [ ] **Step 4：全量构建 + 后端测试 + clippy + fmt**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm tsc --noEmit && pnpm build && cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && cd ..
```
预期：tsc 0 errors / build 成功 / 43 tests passing / clippy 0 warnings / fmt OK。如有 clippy/fmt 问题立即修。

- [ ] **Step 5：Commit**

```bash
git add src/stores/trash.ts src/routes/trash.tsx src/App.tsx
git commit -m "feat(trash): 回收站页 + 恢复/彻底删除"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：回收站页面（按当前公司列已软删项目/成本，可恢复或彻底删除）。

- [ ] **Step 7：写 M2 验收清单到 `.superpowers/sdd/m2-acceptance.md`**

文件全文：

```markdown
# M2 手动验收清单

前置：M1 已通过验收。M2 完成后跑 `pnpm tauri dev`，按以下流程逐项核对。

## 1. 数据迁移
- [ ] 首次启动应用：日志看到 `applied migration 0001_init`、`applied migration 0002_projects_costs`
- [ ] 没有 schema 错误，DB 文件存在于 `~/Library/Application Support/solo-cost/data.db`

## 2. 成本科目
- [ ] 进入「成本科目」首次自动出现 9 个预设（外包成本…其它），每条带「预设」徽章
- [ ] 预设科目无「编辑」「删除」按钮（应该被隐藏）
- [ ] 新建自定义科目「广告投放」，出现在列表底部，带「自定义」徽章
- [ ] 编辑「广告投放」改名为「广告」，列表更新
- [ ] 删除「广告」，列表少一项
- [ ] 切换到另一家公司，自动种自己的 9 个预设科目（与第一家互不干扰）

## 3. 项目 CRUD
- [ ] 项目列表初次为空
- [ ] 新建项目「项目甲」：合同 100000.00 元、含税、税率 6%、状态默认 `pending`，保存成功
- [ ] 列表显示 ¥100,000.00 + 含税 + 6% + 「待启动」徽章
- [ ] 编辑改成「不含税」、税率改 13%、状态改 `in_progress`，保存后列表更新
- [ ] 按状态筛选 `in_progress`，只显示「项目甲」
- [ ] 按状态筛选 `delivered`，列表空
- [ ] 状态筛选切回「全部状态」恢复
- [ ] 删除「项目甲」弹确认框，确认后从列表消失

## 4. 项目详情 + 成本录入
- [ ] 点列表里项目名进入详情，URL 为 `/projects/<id>`
- [ ] 概览 Tab 显示合同总价、含税状态、税率、客户、起止日期、备注
- [ ] 详情页右上角下拉切状态，徽章颜色改变
- [ ] 切「成本」Tab，初始为空，「成本合计 ¥0.00」
- [ ] 录一笔：科目「差旅」，日期今天，金额 199.99，描述「打车」，保存
- [ ] 表格出现一行，按科目汇总 Card 显示「差旅 ¥199.99」，合计 ¥199.99
- [ ] 再录一笔差旅 50 元，汇总变 ¥249.99
- [ ] 再录一笔「硬件采购」1500，汇总按金额降序：硬件采购 ¥1,500.00 / 差旅 ¥249.99
- [ ] 编辑差旅 50 → 80，列表合计随之改变
- [ ] 删除差旅 80，弹确认，确认后表格行消失，合计扣减
- [ ] 收款/任务+工时/附件三个 Tab 显示禁用，不能点击

## 5. 回收站
- [ ] 在 `/projects` 软删一个项目，成本明细同步消失（验证级联）
- [ ] 进入「回收站」，看到该项目（类型「项目」）
- [ ] 还能看到该项目下软删的每条成本（类型「成本」），按删除时间倒序
- [ ] 「恢复」该项目：回收站项目和它的成本一同消失；回到 `/projects` 项目重新出现，详情成本明细恢复
- [ ] 再次软删该项目；在回收站点「彻底删除」，弹确认；项目和成本物理消失，回收站列表空
- [ ] 单独软删一条成本（不删项目）→ 回收站只显示该成本 → 点「恢复」→ 该成本回到项目详情成本明细

## 6. 多公司隔离
- [ ] 在公司 A 建项目「A 的项目」，回到公司 B 切换 → 项目列表为空（按 company_id 隔离）
- [ ] 公司 B 建项目「B 的项目」 → 切回 A 看不到 B 的，反之亦然
- [ ] 在 A 的项目里录成本时科目下拉只显示 A 的 9 个 + 自定义；不会出现 B 的科目

## 7. 锁定/解锁后状态保留
- [ ] 在公司 A 选中、并在「项目甲」详情；点 Header「锁定」回 `/login`
- [ ] 解锁后回到主框架（路由可能跳到 `/dashboard` 或保持上一页，取决于 AuthGate 当前行为；至少不应数据错乱）
- [ ] 数据应当与锁前完全一致

## 8. 回归（M1 仍可用）
- [ ] 公司管理仍可新建/编辑/切换
- [ ] sidebar 各项点击不报错
- [ ] 关闭应用 + 重启 → 走 `/login` 而非 `/setup`
```

- [ ] **Step 8：Commit acceptance + final summary**

```bash
git add .superpowers/sdd/m2-acceptance.md
git commit -m "docs(m2): 验收清单 + 标记里程碑完工"
```

- [ ] **Step 9：CHANGELOG 总收尾**

`/changelog`：M2 里程碑完工总条目（一句话总结：项目 + 成本科目 + 成本录入 + 回收站，含按科目汇总、级联软删、整组恢复）。

---

## Self-Review 结论（plan 提交前自检）

按 writing-plans skill 要求，对照 M2 范围与 spec 自检：

### 1. 覆盖范围

- 项目 CRUD + 6 状态生命周期 → Task 1（schema）+ Task 4（后端 CRUD）+ Task 9（列表 UI）+ Task 10（详情 + 状态切换）✓
- 成本科目（9 preset + 自定义）→ Task 1（schema）+ Task 3（后端 + 种子）+ Task 8（UI）✓
- 成本录入 → Task 1（schema）+ Task 5（后端 + 汇总）+ Task 10（成本 Tab UI）✓
- 软删除 UI + 回收站 → Task 2（domain 级联）+ Task 6（后端）+ Task 11（UI）✓
- 项目维度成本汇总 → Task 5（profit::project_cost_summary）+ Task 10（按科目汇总 Card）✓

### 2. 占位符扫描

- 无 "TBD/TODO/implement later" 字符串
- 每一步涉及代码都给了完整代码
- 收款/任务+工时/附件 Tab **明确占位禁用并标注里程碑**（不算占位符违规——是范围决策的可见性）
- `/projects` 在 Task 8 临时显示 "Task 9 实现" 文案，Task 9 立即替换为真路由；这是 task 之间的合法接力，不留长期占位

### 3. 类型一致性

- Rust `Project` ↔ TS `Project`：字段一一对齐（注意 `contract_amount_is_tax_inclusive` 在 Rust 是 `bool`，DB 存 INTEGER `0/1`，Rust 读时 `!=0` 转换；TS 直接 `boolean`）
- Rust `CostCategory.is_system: bool` ↔ TS `boolean`：DB 存 INTEGER，Rust 读时 `!=0`，TS 直接 boolean ✓
- 命令名前后端一致：list_projects / get_project / create_project / update_project / set_project_status / delete_project / list_categories / create_category / update_category / delete_category / seed_preset_categories_if_empty / list_cost_entries / create_cost_entry / update_cost_entry / delete_cost_entry / get_project_cost_summary / list_trash / restore_trash_item / purge_trash_item ✓
- Tauri 2 invoke 自动 camelCase：所有命令多词参数（`company_id`, `project_id`, `category_id`, `entity_type`, `status_filter`）前端调用必须用 `companyId` / `projectId` / `categoryId` / `entityType` 形式 — 已在每个 store 中按此约定使用 ✓

### 4. 范围控制

- 未引入 M3+ 的成员/任务/工时/合同/收款节点表
- 未引入 M4 的附件/备份/导出
- 未引入 Vitest 或 Playwright（前端测试推迟到 spec §7 规定的 M4）
- 未做项目状态迁移合法性校验（spec 允许 M4 收口）
- 未做"30 天物理清理"定时（spec 5.3，M4 做）

### 5. 风险点

- **Task 1 迁移**：现有用户的 db 在主密码 unlock 时自动跑 0002，已设计；但需要在 Step 3 的实机验证里覆盖「旧 db + 新代码」场景。任务 brief 必须提示 implementer 不要 `rm data.db`，而是用现有 db 跑迁移
- **Task 2 时间戳冲突**：`datetime('now')` 精度只到秒，连续两次 soft delete 可能拿到相同时间戳。测试 `restore_project_only_restores_entries_with_matching_timestamp` 通过 `sleep(1100ms)` 规避；生产环境此精度足够，但需在 implementer brief 中提示这是有意为之
- **Task 7 i18n JSON 合并**：增量补 keys 时容易破坏现有 JSON 结构。implementer 应该读取现有 `zh-CN.json`，在已有顶层对象之间插入新键，**不要整体重写**

---

## Demoable End-State

完成 M2 全部 11 个 task 后，应能：

- 在公司维度新建项目「项目甲」，录入合同 ¥100,000、税率 13%、状态切换商务洽谈 → 进行中 → 已交付
- 在项目详情录入若干成本（差旅、硬件、推广…），按科目汇总
- 误删项目 → 回收站可恢复（成本一同回来）；想彻底清理也可以
- 删预设科目被拒（DeleteBlocked toast）；删被引用的自定义科目被拒；新建/重命名科目正常
- 多公司之间项目和科目互不干扰

---
