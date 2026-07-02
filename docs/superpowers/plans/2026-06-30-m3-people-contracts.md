# solo-cost M3 (People + Contracts + Tasks + Timelogs) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M2 基础上补齐 MVP 的「人 + 合同」闭环：成员（含日成本与归档）+ 合同收款节点 + 任务 + 工时录入（写时快照日成本）。完工后项目详情可看到完整利润链：不含税收入 - 一般成本 - 人力成本 = 毛利润 + 回款率。

**Architecture:** 后端延续 `commands → domain → db pool` 三层。`domain/soft_delete` 扩展级联到 `contract_payments / tasks / time_logs`，并加成员删除阻断。`domain/profit` 扩展 `ProjectFinancialSummary`（收入/税额/一般成本/人力成本/总成本/毛利润/利润率/回款率）。前端激活项目详情 Tabs 中的「收款」「任务+工时」两个 M2 占位 Tab，新增 `/members` 路由。所有金额仍 INTEGER cents；工时为 REAL（CHECK 0-24）；人力成本计算在 Rust 后端：每条 time_log 折算成 cents 后再求和（i64 累加避免浮点漂移）。

**Tech Stack:** 继承 M1/M2 全部技术栈。本里程碑不新增前端依赖（不引图表库、不引 Vitest，全部在 M4 收口）。

## Global Constraints

适用所有任务（每个任务的要求隐式包含本节）：

- **包管理**：pnpm（统一锁文件，禁用 npm/yarn 混用）
- **金额单位**：所有金额字段一律 `INTEGER`（分）；前端展示通过 `lib/money.ts` 转元
- **工时单位**：`time_logs.hours REAL CHECK (hours >= 0 AND hours <= 24)`；人天 = 8 小时
- **快照不可变**：`time_logs.daily_cost_snapshot_cents` 在 CREATE 时从 member 当下值拷贝；update 工时只允许改 `hours / work_date / notes`，禁止改 `daily_cost_snapshot_cents` 与 `member_id`
- **软删除字段**：所有业务表 `deleted_at TEXT NULL`；级联软删使用**同一时间戳**保证整组恢复
- **删除阻断**：删成员若有任何 `time_logs WHERE deleted_at IS NULL AND member_id = ?` → `AppError::DeleteBlocked(...)`
- **错误处理**：Rust `Result<T, AppError>`；非测试禁用 `unwrap()` / `expect()`（`Mutex::lock().unwrap()` 是惯例例外）
- **SQL 安全**：rusqlite 一律绑定参数（PRAGMA 例外仍用 `escape_sqlite_string`）
- **跨表写入用事务**：删项目级联收款/任务/工时；删任务级联工时；恢复同理
- **代码注释语言**：英文；UI 文案中文（i18n 或内联，按 M1/M2 习惯）
- **TS catch 模式**：M2 起统一 `catch (e: unknown)`，新增/修改代码遵循
- **Tauri 2 IPC arg case**：Rust `snake_case` 参数 → 前端 `call(..., { camelCase })`
- **提交规约**：Conventional Commits；`type`/`scope` 小写英文；`subject` 中文 ≤ 72 字符整行；结尾不加句号；body 写"为什么"
- **CHANGELOG**：每个 task feat commit 之后单独跑 `/changelog` skill 写一条 docs(changelog) commit（M1/M2 已确立的双 commit 模式）
- **测试纪律**：domain 层 TDD（先 RED 再 GREEN）；commands 层先写实现后补测试
- **不引入新前端依赖**：M3 全部用 M1/M2 已装的 shadcn 组件（Card / Dialog / Input / Label / Button / Select / Table / Tabs / Badge / Textarea / DropdownMenu / Sonner）+ Tailwind。如需 Checkbox（用于 `is_active` 切换），可通过 `pnpm dlx shadcn@latest add checkbox` 补装一个；除此之外不增加任何前端 npm 包
- **目标平台**：macOS 主开发；不引入 OS 特定逻辑

---

## File Structure (M3 完成后的产物增量)

```
solo-cost/
├── src-tauri/
│   ├── migrations/
│   │   └── 0003_people_contracts.sql      新增：members / contract_payments / tasks / time_logs
│   └── src/
│       ├── db/migrations.rs                MODIFY：注册 0003 到 MIGRATIONS 数组、测试断言 v == 3
│       ├── domain/
│       │   ├── soft_delete.rs              MODIFY：项目级联扩展（contract_payments + tasks + time_logs）；任务级联工时；成员删除阻断
│       │   └── profit.rs                   MODIFY：新增 ProjectFinancialSummary (revenue/tax/general/labor/total/profit/profit_rate/collection_rate)
│       └── commands/
│           ├── mod.rs                      MODIFY：导出新模块
│           ├── members.rs                  NEW：成员 CRUD + is_active 归档
│           ├── payments.rs                 NEW：收款节点 CRUD
│           ├── tasks.rs                    NEW：任务 CRUD（todo/in_progress/done）
│           ├── timelogs.rs                 NEW：工时 CRUD + 写时快照
│           ├── trash.rs                    MODIFY：list 增加 task / contract_payment / time_log 类型；restore/purge 对应扩展
│           ├── projects.rs                 MODIFY：commit 时调用 financial_summary 命令
│           └── costs.rs                    MODIFY：update_cost_entry 加 category↔company 防御（M2 final review M-1）
└── src/
    ├── stores/
    │   ├── auth.ts                          MODIFY：lock() 调用所有 store reset（M2 final review I-1）
    │   ├── company.ts                       MODIFY：加 reset()
    │   ├── categories.ts                    MODIFY：加 reset()
    │   ├── projects.ts                      MODIFY：加 reset()
    │   ├── costs.ts                         MODIFY：加 reset()
    │   ├── trash.ts                         MODIFY：加 reset()
    │   ├── members.ts                       NEW
    │   ├── payments.ts                      NEW
    │   ├── tasks.ts                         NEW
    │   └── timelogs.ts                      NEW
    ├── components/forms/
    │   └── HoursInput.tsx                   NEW：工时受控输入（0-24, 小数允许 0.25 精度）
    ├── components/ui/
    │   └── checkbox.tsx                     NEW（shadcn add）
    ├── types/index.ts                       MODIFY：加 Member / Task / TimeLog / ContractPayment / ProjectFinancialSummary 等
    ├── lib/
    │   └── time.ts                          NEW：日期/小时格式化（前端轻工具）
    ├── i18n/zh-CN.json                      MODIFY：加 member / payment / task / timelog / taskStatus 命名空间
    ├── components/layout/Sidebar.tsx        MODIFY：加 「成员」一项
    ├── App.tsx                              MODIFY：注册 /members 路由
    └── routes/
        ├── members.tsx                      NEW：成员管理
        └── projects/detail.tsx              MODIFY：激活 PaymentsPanel + TasksPanel；OverviewPanel 改为 FinancialPanel
```

---

## Task 1: 0003 迁移 + 注册

**Files:**
- Create: `src-tauri/migrations/0003_people_contracts.sql`
- Modify: `src-tauri/src/db/migrations.rs`

**Interfaces:**
- Produces: 4 张表 (`members`, `contract_payments`, `tasks`, `time_logs`)，FK 引用 companies / projects / members / tasks；CHECK 约束 + 索引
- Consumes: 无

- [ ] **Step 1：写 0003 SQL**

`src-tauri/migrations/0003_people_contracts.sql`:

```sql
-- M3: members / contract_payments / tasks / time_logs
-- All money values stored as INTEGER cents.
-- All business tables include deleted_at for soft delete with cascade-by-timestamp.
-- FK omits ON DELETE CASCADE intentionally — we soft-delete via domain layer.

CREATE TABLE members (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id        INTEGER NOT NULL REFERENCES companies(id),
    name              TEXT    NOT NULL,
    role              TEXT,
    daily_cost_cents  INTEGER NOT NULL DEFAULT 0 CHECK (daily_cost_cents >= 0),
    effective_from    TEXT,
    is_active         INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
    notes             TEXT,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at        TEXT
);

CREATE INDEX idx_members_company_active ON members(company_id, is_active, deleted_at);
CREATE INDEX idx_members_deleted_at ON members(deleted_at);

CREATE TABLE contract_payments (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id            INTEGER NOT NULL REFERENCES projects(id),
    name                  TEXT    NOT NULL,
    expected_amount_cents INTEGER NOT NULL DEFAULT 0 CHECK (expected_amount_cents >= 0),
    expected_date         TEXT,
    actual_amount_cents   INTEGER CHECK (actual_amount_cents IS NULL OR actual_amount_cents >= 0),
    actual_received_at    TEXT,
    sort_order            INTEGER NOT NULL DEFAULT 0,
    notes                 TEXT,
    deleted_at            TEXT
);

CREATE INDEX idx_contract_payments_project ON contract_payments(project_id, deleted_at);

CREATE TABLE tasks (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       INTEGER NOT NULL REFERENCES projects(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    assignee_id      INTEGER REFERENCES members(id),
    status           TEXT    NOT NULL DEFAULT 'todo'
                              CHECK (status IN ('todo','in_progress','done')),
    estimated_hours  REAL    CHECK (estimated_hours IS NULL OR (estimated_hours >= 0 AND estimated_hours <= 9999)),
    due_date         TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at       TEXT
);

CREATE INDEX idx_tasks_project_status ON tasks(project_id, status, deleted_at);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id, deleted_at);

CREATE TABLE time_logs (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id                     INTEGER NOT NULL REFERENCES tasks(id),
    member_id                   INTEGER NOT NULL REFERENCES members(id),
    work_date                   TEXT    NOT NULL,
    hours                       REAL    NOT NULL CHECK (hours >= 0 AND hours <= 24),
    daily_cost_snapshot_cents   INTEGER NOT NULL CHECK (daily_cost_snapshot_cents >= 0),
    notes                       TEXT,
    created_at                  TEXT    NOT NULL DEFAULT (datetime('now')),
    deleted_at                  TEXT
);

CREATE INDEX idx_time_logs_task ON time_logs(task_id, deleted_at);
CREATE INDEX idx_time_logs_member ON time_logs(member_id, deleted_at);
CREATE INDEX idx_time_logs_work_date ON time_logs(work_date);

-- schema_version bumped to 3 (runner also updates via ON CONFLICT — see 0002 note).
INSERT INTO app_meta(key, value) VALUES ('schema_version', '3')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
```

- [ ] **Step 2：注册 0003 到 `db/migrations.rs`**

把 `MIGRATIONS` 数组改为：

```rust
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../../migrations/0001_init.sql")),
    (
        "0002_projects_costs",
        include_str!("../../migrations/0002_projects_costs.sql"),
    ),
    (
        "0003_people_contracts",
        include_str!("../../migrations/0003_people_contracts.sql"),
    ),
];
```

并把 `migrations.rs` 末尾测试模块里两个 `assert_eq!(v, 2)` 改为 `assert_eq!(v, 3)`（`fresh_db_runs_all_migrations` 和 `run_is_idempotent`）。

- [ ] **Step 3：跑测试 + clippy + fmt**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
cd src-tauri && cargo test 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
cargo fmt -- --check
```
预期：43/43 通过；clippy/fmt 全绿。

- [ ] **Step 4：Commit**

```bash
git add src-tauri/migrations/0003_people_contracts.sql src-tauri/src/db/migrations.rs
git commit -m "feat(db): 0003 迁移建成员/合同收款/任务/工时表"
```

- [ ] **Step 5：CHANGELOG**

`/changelog`：四张新表（schema_version 3），含 hours 0-24 与 daily_cost_snapshot_cents 不可负的 CHECK 约束。

---

## Task 2: domain/soft_delete 扩展级联

**Files:**
- Modify: `src-tauri/src/domain/soft_delete.rs`

**Interfaces:**
- Produces (new):
  - `pub fn soft_delete_task(conn, id) -> AppResult<()>` — 同时间戳级联 time_logs
  - `pub fn restore_task(conn, id) -> AppResult<()>` — 时间戳整组恢复 time_logs；若所属 project 已删则 `DeleteBlocked`
  - `pub fn soft_delete_payment(conn, id) -> AppResult<()>` — 单条
  - `pub fn restore_payment(conn, id) -> AppResult<()>` — 若所属 project 已删则 `DeleteBlocked`
  - `pub fn soft_delete_time_log(conn, id) -> AppResult<()>` — 单条
  - `pub fn restore_time_log(conn, id) -> AppResult<()>` — 若所属 task 已删则 `DeleteBlocked`（链式 project 已删的情况会先在 task 恢复时阻断）
  - `pub fn soft_delete_member(conn, id) -> AppResult<()>` — 检查 time_logs 引用；存在则 `DeleteBlocked`
- Produces (modified):
  - `soft_delete_project` 现在级联到 `cost_entries`（已存）+ `contract_payments`（新）+ `tasks`（新）+ `time_logs`（通过 task_id IN (...) 形式，新）
  - `restore_project` 现在按时间戳恢复同样 4 类子表

- [ ] **Step 1：先写所有新测试（RED）**

在 `domain/soft_delete.rs` 测试模块底部追加。先让 `TestDb` 多铺一些 fixture（task + time_log + payment）。修改 `TestDb::new()`：

```rust
impl TestDb {
    fn new() -> Self {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('C')", [])
            .unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO cost_categories(company_id, name, is_system) VALUES(1, '差旅', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(1, 1, '2026-06-01', 12345),
                    (1, 1, '2026-06-02', 6789)",
            [],
        )
        .unwrap();
        // M3 fixtures
        conn.execute(
            "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title) VALUES(1, 'T1'), (1, 'T2')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO contract_payments(project_id, name, expected_amount_cents)
             VALUES(1, '预付', 50000),
                    (1, '尾款', 50000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(1, 1, '2026-06-01', 8.0, 80000),
                    (1, 1, '2026-06-02', 4.0, 80000),
                    (2, 1, '2026-06-03', 8.0, 80000)",
            [],
        )
        .unwrap();
        Self { conn, _dir: dir }
    }
}
```

追加测试（每个先 panic stub 不写实现）：

```rust
#[test]
fn project_delete_now_cascades_to_payments_tasks_timelogs() {
    let db = TestDb::new();
    soft_delete_project(&db.conn, 1).unwrap();
    let pt = deleted_at(&db.conn, "projects", 1).unwrap();
    // existing cost_entries cascade (regression check)
    assert_eq!(pt, deleted_at(&db.conn, "cost_entries", 1).unwrap());
    // payments
    assert_eq!(pt, deleted_at(&db.conn, "contract_payments", 1).unwrap());
    assert_eq!(pt, deleted_at(&db.conn, "contract_payments", 2).unwrap());
    // tasks
    assert_eq!(pt, deleted_at(&db.conn, "tasks", 1).unwrap());
    assert_eq!(pt, deleted_at(&db.conn, "tasks", 2).unwrap());
    // time_logs (through tasks)
    assert_eq!(pt, deleted_at(&db.conn, "time_logs", 1).unwrap());
    assert_eq!(pt, deleted_at(&db.conn, "time_logs", 2).unwrap());
    assert_eq!(pt, deleted_at(&db.conn, "time_logs", 3).unwrap());
}

#[test]
fn restore_project_brings_back_full_subtree() {
    let db = TestDb::new();
    soft_delete_project(&db.conn, 1).unwrap();
    restore_project(&db.conn, 1).unwrap();
    assert!(deleted_at(&db.conn, "projects", 1).is_none());
    assert!(deleted_at(&db.conn, "cost_entries", 1).is_none());
    assert!(deleted_at(&db.conn, "contract_payments", 1).is_none());
    assert!(deleted_at(&db.conn, "tasks", 1).is_none());
    assert!(deleted_at(&db.conn, "time_logs", 1).is_none());
}

#[test]
fn task_delete_cascades_timelogs_same_timestamp() {
    let db = TestDb::new();
    soft_delete_task(&db.conn, 1).unwrap();
    let tt = deleted_at(&db.conn, "tasks", 1).unwrap();
    assert_eq!(tt, deleted_at(&db.conn, "time_logs", 1).unwrap());
    assert_eq!(tt, deleted_at(&db.conn, "time_logs", 2).unwrap());
    // task 2 untouched
    assert!(deleted_at(&db.conn, "tasks", 2).is_none());
    assert!(deleted_at(&db.conn, "time_logs", 3).is_none());
}

#[test]
fn restore_task_under_deleted_project_blocked() {
    let db = TestDb::new();
    soft_delete_project(&db.conn, 1).unwrap();
    let err = restore_task(&db.conn, 1).unwrap_err();
    assert!(matches!(err, AppError::DeleteBlocked(_)));
}

#[test]
fn restore_payment_under_deleted_project_blocked() {
    let db = TestDb::new();
    soft_delete_project(&db.conn, 1).unwrap();
    let err = restore_payment(&db.conn, 1).unwrap_err();
    assert!(matches!(err, AppError::DeleteBlocked(_)));
}

#[test]
fn restore_time_log_under_deleted_task_blocked() {
    let db = TestDb::new();
    soft_delete_task(&db.conn, 1).unwrap();
    let err = restore_time_log(&db.conn, 1).unwrap_err();
    assert!(matches!(err, AppError::DeleteBlocked(_)));
}

#[test]
fn delete_member_with_active_time_logs_blocked() {
    let db = TestDb::new();
    let err = soft_delete_member(&db.conn, 1).unwrap_err();
    assert!(matches!(err, AppError::DeleteBlocked(_)));
}

#[test]
fn delete_member_with_only_soft_deleted_time_logs_succeeds() {
    let db = TestDb::new();
    // soft-delete all member's time_logs first
    db.conn
        .execute(
            "UPDATE time_logs SET deleted_at = datetime('now') WHERE member_id = 1",
            [],
        )
        .unwrap();
    soft_delete_member(&db.conn, 1).unwrap();
    assert!(deleted_at(&db.conn, "members", 1).is_some());
}
```

加 8 个新函数的 stub（暂时 panic）：

```rust
pub fn soft_delete_task(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn restore_task(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn soft_delete_payment(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn restore_payment(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn soft_delete_time_log(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn restore_time_log(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
pub fn soft_delete_member(_conn: &Connection, _id: i64) -> AppResult<()> { unimplemented!() }
```

- [ ] **Step 2：跑 RED**

```bash
cd src-tauri
cargo test --lib domain::soft_delete::tests 2>&1 | tail -25
```
预期：原 4 个 + 新 8 个 = 12 个失败/panic（旧 4 个会跟新行为冲突，因为旧 `soft_delete_project` 还没扩展级联）。

- [ ] **Step 3：扩展 `soft_delete_project` + `restore_project`**

替换两者完整实现：

```rust
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
    tx.execute(
        "UPDATE contract_payments SET deleted_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    // time_logs cascade through tasks: capture which tasks were active before tagging tasks
    tx.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE deleted_at IS NULL
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?2 AND deleted_at IS NULL)",
        rusqlite::params![ts, id],
    )?;
    tx.execute(
        "UPDATE tasks SET deleted_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore_project(conn: &Connection, id: i64) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let ts: Option<String> = tx
        .query_row("SELECT deleted_at FROM projects WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "project", id },
            other => AppError::Db(other),
        })?;
    let ts = match ts {
        Some(t) => t,
        None => return Ok(()),
    };
    tx.execute("UPDATE projects SET deleted_at = NULL WHERE id = ?1", [id])?;
    tx.execute(
        "UPDATE cost_entries SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE contract_payments SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE time_logs SET deleted_at = NULL
         WHERE deleted_at = ?2
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE tasks SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4：实现新 7 个函数**

```rust
pub fn soft_delete_task(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    tx.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE task_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore_task(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT t.deleted_at IS NOT NULL, p.deleted_at
             FROM tasks t JOIN projects p ON p.id = t.project_id
             WHERE t.id = ?1",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let (_task_is_deleted, project_deleted_at) = match row {
        Some(t) => t,
        None => return Err(AppError::NotFound { entity: "task", id }),
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("项目已删除，请先恢复项目".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let ts: Option<String> = tx
        .query_row("SELECT deleted_at FROM tasks WHERE id = ?1", [id], |r| {
            r.get(0)
        })?;
    let ts = match ts {
        Some(t) => t,
        None => return Ok(()),
    };
    tx.execute("UPDATE tasks SET deleted_at = NULL WHERE id = ?1", [id])?;
    tx.execute(
        "UPDATE time_logs SET deleted_at = NULL
         WHERE task_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn soft_delete_payment(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE contract_payments SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "contract_payment", id });
    }
    Ok(())
}

pub fn restore_payment(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT p.deleted_at
             FROM contract_payments cp JOIN projects p ON p.id = cp.project_id
             WHERE cp.id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let project_deleted_at = match row {
        Some(t) => t,
        None => return Err(AppError::NotFound { entity: "contract_payment", id }),
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("项目已删除，请先恢复项目".into()));
    }
    conn.execute(
        "UPDATE contract_payments SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn soft_delete_time_log(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "time_log", id });
    }
    Ok(())
}

pub fn restore_time_log(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT t.deleted_at
             FROM time_logs tl JOIN tasks t ON t.id = tl.task_id
             WHERE tl.id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let task_deleted_at = match row {
        Some(t) => t,
        None => return Err(AppError::NotFound { entity: "time_log", id }),
    };
    if task_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("任务已删除，请先恢复任务".into()));
    }
    conn.execute(
        "UPDATE time_logs SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn soft_delete_member(conn: &Connection, id: i64) -> AppResult<()> {
    let active_logs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM time_logs WHERE member_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if active_logs > 0 {
        return Err(AppError::DeleteBlocked(format!(
            "该成员有 {active_logs} 条工时记录，请先归档（设 is_active=0）"
        )));
    }
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE members SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "member", id });
    }
    Ok(())
}
```

- [ ] **Step 5：跑 GREEN**

```bash
cargo test --lib domain::soft_delete::tests 2>&1 | tail -20
```
预期：12 个全部通过（旧 4 + 新 8）。

- [ ] **Step 6：跑全量**

```bash
cargo test 2>&1 | tail -5
```
预期：43 + 8 = 51 passing。

- [ ] **Step 7：Commit**

```bash
git add src-tauri/src/domain/soft_delete.rs
git commit -m "feat(domain): 软删级联扩展到合同/任务/工时 + 成员删除阻断"
```

- [ ] **Step 8：CHANGELOG**

`/changelog`：项目级联软删现覆盖收款节点/任务/工时；任务级联软删工时；成员若有有效工时则拒绝软删（DeleteBlocked）。

---

## Task 3: domain/profit 扩展 ProjectFinancialSummary

**Files:**
- Modify: `src-tauri/src/domain/profit.rs`

**Interfaces:**
- Produces (new):
  - `pub struct ProjectFinancialSummary { revenue_tax_inclusive_cents, revenue_tax_exclusive_cents, tax_amount_cents, general_cost_cents, labor_cost_cents, total_cost_cents, gross_profit_cents, profit_rate, expected_payment_cents, actual_payment_cents, collection_rate }`
  - `pub fn project_financial_summary(conn, project_id) -> AppResult<ProjectFinancialSummary>`
- Produces (preserved): `project_cost_summary` 仍存在（前端 cost Tab 用），不修改
- Calc rules（与 spec §3.3 对齐）：
  - `revenue_tax_inclusive_cents` = project.contract_amount_cents（is_tax_inclusive=1 时直接；=0 时 = base × (1+rate)）
  - `revenue_tax_exclusive_cents` = is_tax_inclusive=1 时 = base / (1+rate)；=0 时 = base
  - `tax_amount_cents` = revenue_tax_inclusive_cents - revenue_tax_exclusive_cents
  - `general_cost_cents` = Σ cost_entries.amount_cents
  - `labor_cost_cents` = Σ round(time_log.hours / 8.0 × snapshot_cents) — 在 Rust 端逐条 round 后 i64 求和
  - `total_cost_cents` = general + labor
  - `gross_profit_cents` = revenue_tax_exclusive_cents - total_cost_cents
  - `profit_rate` = gross_profit / revenue_tax_exclusive，分母为 0 时 0.0
  - `expected_payment_cents` = Σ contract_payments.expected_amount_cents
  - `actual_payment_cents` = Σ COALESCE(contract_payments.actual_amount_cents, 0) where actual_received_at IS NOT NULL
  - `collection_rate` = actual / expected，分母为 0 时 0.0

- [ ] **Step 1：先写测试（RED）**

追加到 `profit.rs` 测试模块。`TestDb` 增加 fixtures（含 contract_amount/tax + 成员/任务/工时/收款）：

```rust
fn make_full_fixture(conn: &Connection) {
    // already has company 1, project 1, two categories, no entries
    // give project a contract: 含税 ¥10,000.00 @ 6%
    conn.execute(
        "UPDATE projects SET
            contract_amount_cents = 1000000,
            contract_amount_is_tax_inclusive = 1,
            tax_rate = 0.06
         WHERE id = 1",
        [],
    )
    .unwrap();
    // member ¥800/day, task, 4 time logs totaling 16 hours = 2 person-days = ¥1600
    conn.execute(
        "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks(project_id, title) VALUES(1, 'T')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
         VALUES(1, 1, '2026-06-01', 8.0, 80000),
                (1, 1, '2026-06-02', 8.0, 80000)",
        [],
    )
    .unwrap();
    // 2 cost entries totaling ¥500
    conn.execute(
        "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
         VALUES(1, 1, '2026-06-03', 30000),
                (1, 1, '2026-06-04', 20000)",
        [],
    )
    .unwrap();
    // payments: expected ¥10,000 (50% + 50%), actual ¥5,000 received
    conn.execute(
        "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                                       actual_amount_cents, actual_received_at)
         VALUES(1, '预付', 500000, 500000, '2026-06-05'),
                (1, '尾款', 500000, NULL, NULL)",
        [],
    )
    .unwrap();
}

#[test]
fn financial_summary_full_calculation() {
    let db = TestDb::new();
    make_full_fixture(&db.conn);
    let s = project_financial_summary(&db.conn, 1).unwrap();
    // revenue tax inclusive = 1,000,000 cents
    assert_eq!(s.revenue_tax_inclusive_cents, 1_000_000);
    // revenue tax exclusive = 1,000,000 / 1.06 = 943,396 (rounded)
    assert_eq!(s.revenue_tax_exclusive_cents, 943_396);
    // tax = 56,604
    assert_eq!(s.tax_amount_cents, 56_604);
    // general cost = 50,000
    assert_eq!(s.general_cost_cents, 50_000);
    // labor cost = 16 hours / 8 * 80,000 = 160,000
    assert_eq!(s.labor_cost_cents, 160_000);
    // total cost = 210,000
    assert_eq!(s.total_cost_cents, 210_000);
    // gross profit = 943,396 - 210,000 = 733,396
    assert_eq!(s.gross_profit_cents, 733_396);
    // profit rate ≈ 0.7774
    assert!((s.profit_rate - 0.7774).abs() < 0.001);
    // expected payment = 1,000,000
    assert_eq!(s.expected_payment_cents, 1_000_000);
    // actual = 500,000
    assert_eq!(s.actual_payment_cents, 500_000);
    // collection = 0.5
    assert!((s.collection_rate - 0.5).abs() < 1e-9);
}

#[test]
fn financial_summary_tax_exclusive_contract() {
    let db = TestDb::new();
    // project with contract = ¥1,000 不含税
    db.conn.execute(
        "UPDATE projects SET
            contract_amount_cents = 100000,
            contract_amount_is_tax_inclusive = 0,
            tax_rate = 0.13
         WHERE id = 1",
        [],
    ).unwrap();
    let s = project_financial_summary(&db.conn, 1).unwrap();
    assert_eq!(s.revenue_tax_exclusive_cents, 100_000);
    // revenue inclusive = 100,000 * 1.13 = 113,000
    assert_eq!(s.revenue_tax_inclusive_cents, 113_000);
    assert_eq!(s.tax_amount_cents, 13_000);
}

#[test]
fn financial_summary_empty_project_zero_rates() {
    let db = TestDb::new();
    // project at default state from TestDb (contract=0, no logs, no costs, no payments)
    let s = project_financial_summary(&db.conn, 1).unwrap();
    assert_eq!(s.revenue_tax_inclusive_cents, 0);
    assert_eq!(s.revenue_tax_exclusive_cents, 0);
    assert_eq!(s.gross_profit_cents, 0);
    assert_eq!(s.profit_rate, 0.0);
    assert_eq!(s.collection_rate, 0.0);
}

#[test]
fn financial_summary_excludes_soft_deleted() {
    let db = TestDb::new();
    make_full_fixture(&db.conn);
    db.conn
        .execute(
            "UPDATE time_logs SET deleted_at = datetime('now') WHERE id = 1",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "UPDATE cost_entries SET deleted_at = datetime('now') WHERE id = 1",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "UPDATE contract_payments SET deleted_at = datetime('now') WHERE id = 1",
            [],
        )
        .unwrap();
    let s = project_financial_summary(&db.conn, 1).unwrap();
    // labor = 8 hours / 8 * 80,000 = 80,000
    assert_eq!(s.labor_cost_cents, 80_000);
    // general = 20,000
    assert_eq!(s.general_cost_cents, 20_000);
    // expected drops to 500,000 (only payment 2)
    assert_eq!(s.expected_payment_cents, 500_000);
    // actual drops to 0
    assert_eq!(s.actual_payment_cents, 0);
}
```

加 stub：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ProjectFinancialSummary {
    pub revenue_tax_inclusive_cents: i64,
    pub revenue_tax_exclusive_cents: i64,
    pub tax_amount_cents: i64,
    pub general_cost_cents: i64,
    pub labor_cost_cents: i64,
    pub total_cost_cents: i64,
    pub gross_profit_cents: i64,
    pub profit_rate: f64,
    pub expected_payment_cents: i64,
    pub actual_payment_cents: i64,
    pub collection_rate: f64,
}

pub fn project_financial_summary(
    _conn: &Connection,
    _project_id: i64,
) -> AppResult<ProjectFinancialSummary> {
    unimplemented!()
}
```

- [ ] **Step 2：跑 RED**

```bash
cargo test --lib domain::profit::tests 2>&1 | tail -20
```
预期：现有 3 个仍通过 + 新 4 个 panic。

- [ ] **Step 3：实现 `project_financial_summary`**

```rust
pub fn project_financial_summary(
    conn: &Connection,
    project_id: i64,
) -> AppResult<ProjectFinancialSummary> {
    // load project core
    let (contract, inclusive, rate): (i64, i64, f64) = conn.query_row(
        "SELECT contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate
         FROM projects WHERE id = ?1 AND deleted_at IS NULL",
        [project_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "project", id: project_id },
        other => AppError::Db(other),
    })?;
    let is_inclusive = inclusive != 0;
    let one_plus = 1.0 + rate;
    let (revenue_inc, revenue_exc) = if is_inclusive {
        let exc = (contract as f64 / one_plus).round() as i64;
        (contract, exc)
    } else {
        let inc = (contract as f64 * one_plus).round() as i64;
        (inc, contract)
    };
    let tax = revenue_inc - revenue_exc;

    // general cost
    let general: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM cost_entries
         WHERE project_id = ?1 AND deleted_at IS NULL",
        [project_id],
        |r| r.get(0),
    )?;

    // labor cost — per-log compute then sum (precision-safe)
    let mut stmt = conn.prepare(
        "SELECT tl.hours, tl.daily_cost_snapshot_cents
         FROM time_logs tl JOIN tasks t ON t.id = tl.task_id
         WHERE t.project_id = ?1 AND tl.deleted_at IS NULL AND t.deleted_at IS NULL",
    )?;
    let mut labor: i64 = 0;
    let rows = stmt.query_map([project_id], |r| {
        Ok((r.get::<_, f64>(0)?, r.get::<_, i64>(1)?))
    })?;
    for r in rows {
        let (hours, snap) = r?;
        let cost = (hours / 8.0 * snap as f64).round() as i64;
        labor += cost;
    }

    let total_cost = general + labor;
    let gross = revenue_exc - total_cost;
    let profit_rate = if revenue_exc == 0 {
        0.0
    } else {
        gross as f64 / revenue_exc as f64
    };

    // payments
    let expected: i64 = conn.query_row(
        "SELECT COALESCE(SUM(expected_amount_cents), 0) FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL",
        [project_id],
        |r| r.get(0),
    )?;
    let actual: i64 = conn.query_row(
        "SELECT COALESCE(SUM(actual_amount_cents), 0) FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND actual_received_at IS NOT NULL",
        [project_id],
        |r| r.get(0),
    )?;
    let collection_rate = if expected == 0 {
        0.0
    } else {
        actual as f64 / expected as f64
    };

    Ok(ProjectFinancialSummary {
        revenue_tax_inclusive_cents: revenue_inc,
        revenue_tax_exclusive_cents: revenue_exc,
        tax_amount_cents: tax,
        general_cost_cents: general,
        labor_cost_cents: labor,
        total_cost_cents: total_cost,
        gross_profit_cents: gross,
        profit_rate,
        expected_payment_cents: expected,
        actual_payment_cents: actual,
        collection_rate,
    })
}
```

- [ ] **Step 4：跑 GREEN + 全量**

```bash
cargo test 2>&1 | tail -5
```
预期：51 + 4 = 55 passing。

- [ ] **Step 5：Commit**

```bash
git add src-tauri/src/domain/profit.rs
git commit -m "feat(profit): 项目财务汇总（收入/税额/人力成本/利润/回款率）"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：domain::profit::project_financial_summary 计算完整利润链；不含税收入按含税合同除以 1+rate，人力成本逐条 round 后累加避免浮点误差。

---

## Task 4: 成员后端 CRUD + 归档

**Files:**
- Create: `src-tauri/src/commands/members.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod members;`）
- Modify: `src-tauri/src/lib.rs`（注册 6 条命令）

**Interfaces:**
- Produces:
  - `pub struct Member { id, company_id, name, role, daily_cost_cents, effective_from, is_active, notes, created_at, updated_at }`
  - `pub struct MemberInput { name, role?, daily_cost_cents?, effective_from?, is_active?, notes? }`
  - 6 commands：`list_members(company_id) -> Vec<Member>`、`get_member(id)`、`create_member(company_id, input)`、`update_member(id, input)`、`set_member_active(id, is_active: bool)`、`delete_member(id)`（走 domain::soft_delete::soft_delete_member）
- Consumes: `domain::soft_delete::soft_delete_member`

- [ ] **Step 1：写 `commands/members.rs`**

```rust
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub role: Option<String>,
    pub daily_cost_cents: i64,
    pub effective_from: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct MemberInput {
    pub name: String,
    pub role: Option<String>,
    pub daily_cost_cents: Option<i64>,
    pub effective_from: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

fn row_to_member(row: &rusqlite::Row) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        role: row.get("role")?,
        daily_cost_cents: row.get("daily_cost_cents")?,
        effective_from: row.get("effective_from")?,
        is_active: row.get::<_, i64>("is_active")? != 0,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &MemberInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation("成员名长度必须在 1–80 之间".into()));
    }
    if let Some(d) = input.daily_cost_cents {
        if d < 0 {
            return Err(AppError::Validation("日成本不能为负".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<Member>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM members
         WHERE company_id = ?1 AND deleted_at IS NULL
         ORDER BY is_active DESC, id DESC",
    )?;
    let rows = stmt.query_map([company_id], row_to_member)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Member> {
    conn.query_row(
        "SELECT * FROM members WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_member,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "member", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &MemberInput,
) -> AppResult<Member> {
    validate(input)?;
    conn.execute(
        "INSERT INTO members(company_id, name, role, daily_cost_cents,
                             effective_from, is_active, notes)
         VALUES(?1, ?2, ?3, COALESCE(?4, 0), ?5, COALESCE(?6, 1), ?7)",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.role.as_deref(),
            input.daily_cost_cents,
            input.effective_from.as_deref(),
            input.is_active.map(|b| b as i64),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &MemberInput,
) -> AppResult<Member> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE members SET
            name = ?1,
            role = ?2,
            daily_cost_cents = COALESCE(?3, daily_cost_cents),
            effective_from = ?4,
            is_active = COALESCE(?5, is_active),
            notes = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.role.as_deref(),
            input.daily_cost_cents,
            input.effective_from.as_deref(),
            input.is_active.map(|b| b as i64),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "member", id });
    }
    get_impl(conn, id)
}

pub(crate) fn set_active_impl(conn: &Connection, id: i64, is_active: bool) -> AppResult<Member> {
    let n = conn.execute(
        "UPDATE members SET is_active = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![is_active as i64, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "member", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_member(conn, id)
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
pub fn list_members(state: tauri::State<AppState>, company_id: i64) -> AppResult<Vec<Member>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn get_member(state: tauri::State<AppState>, id: i64) -> AppResult<Member> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_member(
    state: tauri::State<AppState>,
    company_id: i64,
    input: MemberInput,
) -> AppResult<Member> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_member(
    state: tauri::State<AppState>,
    id: i64,
    input: MemberInput,
) -> AppResult<Member> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_member_active(
    state: tauri::State<AppState>,
    id: i64,
    is_active: bool,
) -> AppResult<Member> {
    with_conn(&state, |c| set_active_impl(c, id, is_active))
}
#[tauri::command]
pub fn delete_member(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> MemberInput {
        MemberInput {
            name: name.into(),
            role: None,
            daily_cost_cents: None,
            effective_from: None,
            is_active: None,
            notes: None,
        }
    }

    #[test]
    fn create_with_defaults_is_active() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("张三")).unwrap();
        assert!(m.is_active);
        assert_eq!(m.daily_cost_cents, 0);
    }

    #[test]
    fn validate_negative_daily_cost() {
        let db = TestDb::new();
        let mut i = input("X");
        i.daily_cost_cents = Some(-1);
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn set_active_toggles() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("A")).unwrap();
        let inactive = set_active_impl(&db.conn, m.id, false).unwrap();
        assert!(!inactive.is_active);
        let active = set_active_impl(&db.conn, m.id, true).unwrap();
        assert!(active.is_active);
    }

    #[test]
    fn delete_member_with_active_logs_blocked() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("M")).unwrap();
        // create project + task + time_log referencing this member
        db.conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
        db.conn.execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", []).unwrap();
        db.conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(1, ?1, '2026-06-01', 8.0, 80000)",
            [m.id],
        ).unwrap();
        let err = delete_impl(&db.conn, m.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_member_without_logs_succeeds() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("M")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }
}
```

- [ ] **Step 2：`commands/mod.rs` 加 `pub mod members;`**

按字母顺序插入：

```rust
pub mod auth;
pub mod categories;
pub mod companies;
pub mod costs;
pub mod members;
pub mod projects;
pub mod trash;
```

- [ ] **Step 3：注册 6 个命令到 `lib.rs` invoke_handler**

在 `commands::trash::*` 之后追加：

```rust
            commands::members::list_members,
            commands::members::get_member,
            commands::members::create_member,
            commands::members::update_member,
            commands::members::set_member_active,
            commands::members::delete_member,
```

- [ ] **Step 4：跑测试**

```bash
cargo test 2>&1 | tail -5
```
预期：55 + 5 = 60 passing.

- [ ] **Step 5：Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(members): 成员 crud + 归档切换 + 有工时则拒删"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：成员 CRUD + `set_member_active` 归档切换；`delete_member` 当成员有有效工时时返回 DeleteBlocked。

---

## Task 5: 合同收款节点后端 CRUD

**Files:**
- Create: `src-tauri/src/commands/payments.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod payments;`）
- Modify: `src-tauri/src/lib.rs`（注册 6 条命令）

**Interfaces:**
- Produces:
  - `pub struct ContractPayment { id, project_id, name, expected_amount_cents, expected_date, actual_amount_cents, actual_received_at, sort_order, notes }`
  - `pub struct PaymentInput { name, expected_amount_cents, expected_date?, actual_amount_cents?, actual_received_at?, notes? }`
  - 6 commands: `list_payments(project_id)`, `get_payment(id)`, `create_payment(project_id, input)`, `update_payment(id, input)`, `mark_payment_received(id, actual_amount_cents, actual_received_at)`, `delete_payment(id)`（走 domain::soft_delete）
- Consumes: `domain::soft_delete::soft_delete_payment`

- [ ] **Step 1：写 `commands/payments.rs`**

```rust
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ContractPayment {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: Option<String>,
    pub actual_amount_cents: Option<i64>,
    pub actual_received_at: Option<String>,
    pub sort_order: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentInput {
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: Option<String>,
    pub actual_amount_cents: Option<i64>,
    pub actual_received_at: Option<String>,
    pub notes: Option<String>,
}

fn row_to_payment(row: &rusqlite::Row) -> rusqlite::Result<ContractPayment> {
    Ok(ContractPayment {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        expected_amount_cents: row.get("expected_amount_cents")?,
        expected_date: row.get("expected_date")?,
        actual_amount_cents: row.get("actual_amount_cents")?,
        actual_received_at: row.get("actual_received_at")?,
        sort_order: row.get("sort_order")?,
        notes: row.get("notes")?,
    })
}

fn validate(input: &PaymentInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::Validation("收款节点名长度必须在 1–60 之间".into()));
    }
    if input.expected_amount_cents < 0 {
        return Err(AppError::Validation("预期金额不能为负".into()));
    }
    if let Some(a) = input.actual_amount_cents {
        if a < 0 {
            return Err(AppError::Validation("实收金额不能为负".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<ContractPayment>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([project_id], row_to_payment)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<ContractPayment> {
    conn.query_row(
        "SELECT * FROM contract_payments WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_payment,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "contract_payment", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &PaymentInput,
) -> AppResult<ContractPayment> {
    validate(input)?;
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM contract_payments WHERE project_id = ?1",
        [project_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                                       expected_date, actual_amount_cents,
                                       actual_received_at, sort_order, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            project_id,
            input.name.trim(),
            input.expected_amount_cents,
            input.expected_date.as_deref(),
            input.actual_amount_cents,
            input.actual_received_at.as_deref(),
            next_order,
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &PaymentInput,
) -> AppResult<ContractPayment> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE contract_payments SET
            name = ?1,
            expected_amount_cents = ?2,
            expected_date = ?3,
            actual_amount_cents = ?4,
            actual_received_at = ?5,
            notes = ?6
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.expected_amount_cents,
            input.expected_date.as_deref(),
            input.actual_amount_cents,
            input.actual_received_at.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "contract_payment", id });
    }
    get_impl(conn, id)
}

pub(crate) fn mark_received_impl(
    conn: &Connection,
    id: i64,
    actual_amount_cents: i64,
    actual_received_at: &str,
) -> AppResult<ContractPayment> {
    if actual_amount_cents < 0 {
        return Err(AppError::Validation("实收金额不能为负".into()));
    }
    if actual_received_at.trim().is_empty() {
        return Err(AppError::Validation("实收日期必填".into()));
    }
    let n = conn.execute(
        "UPDATE contract_payments SET
            actual_amount_cents = ?1,
            actual_received_at = ?2
         WHERE id = ?3 AND deleted_at IS NULL",
        rusqlite::params![actual_amount_cents, actual_received_at.trim(), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "contract_payment", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_payment(conn, id)
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
pub fn list_payments(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<ContractPayment>> {
    with_conn(&state, |c| list_impl(c, project_id))
}
#[tauri::command]
pub fn get_payment(state: tauri::State<AppState>, id: i64) -> AppResult<ContractPayment> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_payment(
    state: tauri::State<AppState>,
    project_id: i64,
    input: PaymentInput,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_payment(
    state: tauri::State<AppState>,
    id: i64,
    input: PaymentInput,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn mark_payment_received(
    state: tauri::State<AppState>,
    id: i64,
    actual_amount_cents: i64,
    actual_received_at: String,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| mark_received_impl(c, id, actual_amount_cents, &actual_received_at))
}
#[tauri::command]
pub fn delete_payment(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn make(name: &str, expected: i64) -> PaymentInput {
        PaymentInput {
            name: name.into(),
            expected_amount_cents: expected,
            expected_date: None,
            actual_amount_cents: None,
            actual_received_at: None,
            notes: None,
        }
    }

    #[test]
    fn create_and_list_with_sort_order() {
        let db = TestDb::new();
        let p1 = create_impl(&db.conn, 1, &make("预付", 500000)).unwrap();
        let p2 = create_impl(&db.conn, 1, &make("尾款", 500000)).unwrap();
        assert_eq!(p1.sort_order, 0);
        assert_eq!(p2.sort_order, 1);
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, p1.id);
    }

    #[test]
    fn validate_negative_expected_rejected() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &make("X", -1)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn mark_received_sets_both_fields() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &make("预付", 500000)).unwrap();
        let m = mark_received_impl(&db.conn, p.id, 480000, "2026-06-05").unwrap();
        assert_eq!(m.actual_amount_cents, Some(480000));
        assert_eq!(m.actual_received_at.as_deref(), Some("2026-06-05"));
    }

    #[test]
    fn delete_payment_soft_only() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &make("X", 100)).unwrap();
        delete_impl(&db.conn, p.id).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }
}
```

- [ ] **Step 2：`commands/mod.rs` 加 `pub mod payments;`**

- [ ] **Step 3：注册 6 个命令到 `lib.rs`**

```rust
            commands::payments::list_payments,
            commands::payments::get_payment,
            commands::payments::create_payment,
            commands::payments::update_payment,
            commands::payments::mark_payment_received,
            commands::payments::delete_payment,
```

- [ ] **Step 4：跑测试**

预期：60 + 4 = 64 passing.

- [ ] **Step 5：Commit**

```bash
git commit -m "feat(payments): 合同收款节点 crud + 标记实收"
```

- [ ] **Step 6：CHANGELOG**

`/changelog`：合同收款节点 CRUD；`mark_payment_received` 一键标实收（金额 + 日期）；按 sort_order 顺序列出。

---

## Task 6: 任务后端 CRUD

**Files:**
- Create: `src-tauri/src/commands/tasks.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod tasks;`）
- Modify: `src-tauri/src/lib.rs`（注册 6 条命令）

**Interfaces:**
- Produces:
  - `pub struct Task { id, project_id, title, description, assignee_id, status, estimated_hours, due_date, created_at, updated_at }`
  - `pub struct TaskInput { title, description?, assignee_id?, status?, estimated_hours?, due_date? }`
  - 6 commands: `list_tasks(project_id, status?)`, `get_task(id)`, `create_task(project_id, input)`, `update_task(id, input)`, `set_task_status(id, status)`, `delete_task(id)`（走 domain::soft_delete）
- Consumes: `domain::soft_delete::soft_delete_task`

- [ ] **Step 1：写 `commands/tasks.rs`**

```rust
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const ALLOWED_STATUSES: [&str; 3] = ["todo", "in_progress", "done"];

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub status: String,
    pub estimated_hours: Option<f64>,
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskInput {
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub status: Option<String>,
    pub estimated_hours: Option<f64>,
    pub due_date: Option<String>,
}

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        assignee_id: row.get("assignee_id")?,
        status: row.get("status")?,
        estimated_hours: row.get("estimated_hours")?,
        due_date: row.get("due_date")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &TaskInput) -> AppResult<()> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(AppError::Validation("任务标题长度必须在 1–120 之间".into()));
    }
    if let Some(s) = &input.status {
        if !ALLOWED_STATUSES.contains(&s.as_str()) {
            return Err(AppError::Validation(format!("非法状态：{s}")));
        }
    }
    if let Some(h) = input.estimated_hours {
        if !(0.0..=9999.0).contains(&h) {
            return Err(AppError::Validation("预估工时需在 [0, 9999] 之间".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(
    conn: &Connection,
    project_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Task>> {
    let (sql, params): (&str, Vec<rusqlite::types::Value>) = match status {
        Some(s) => (
            "SELECT * FROM tasks
             WHERE project_id = ?1 AND status = ?2 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![project_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT * FROM tasks
             WHERE project_id = ?1 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![project_id.into()],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Task> {
    conn.query_row(
        "SELECT * FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_task,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &TaskInput,
) -> AppResult<Task> {
    validate(input)?;
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, assignee_id,
                           status, estimated_hours, due_date)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7)",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TaskInput,
) -> AppResult<Task> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            status = COALESCE(?4, status),
            estimated_hours = ?5,
            due_date = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    get_impl(conn, id)
}

pub(crate) fn set_status_impl(conn: &Connection, id: i64, status: &str) -> AppResult<Task> {
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(AppError::Validation(format!("非法状态：{status}")));
    }
    let n = conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![status, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_task(conn, id)
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
pub fn list_tasks(
    state: tauri::State<AppState>,
    project_id: i64,
    status: Option<String>,
) -> AppResult<Vec<Task>> {
    with_conn(&state, |c| list_impl(c, project_id, status.as_deref()))
}
#[tauri::command]
pub fn get_task(state: tauri::State<AppState>, id: i64) -> AppResult<Task> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_task(
    state: tauri::State<AppState>,
    project_id: i64,
    input: TaskInput,
) -> AppResult<Task> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_task(
    state: tauri::State<AppState>,
    id: i64,
    input: TaskInput,
) -> AppResult<Task> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_task_status(
    state: tauri::State<AppState>,
    id: i64,
    status: String,
) -> AppResult<Task> {
    with_conn(&state, |c| set_status_impl(c, id, &status))
}
#[tauri::command]
pub fn delete_task(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input(title: &str) -> TaskInput {
        TaskInput {
            title: title.into(),
            description: None,
            assignee_id: None,
            status: None,
            estimated_hours: None,
            due_date: None,
        }
    }

    #[test]
    fn create_defaults_status_todo() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        assert_eq!(t.status, "todo");
    }

    #[test]
    fn validate_bad_status() {
        let db = TestDb::new();
        let mut i = input("T");
        i.status = Some("foo".into());
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let db = TestDb::new();
        let mut a = input("A"); a.status = Some("todo".into());
        let mut b = input("B"); b.status = Some("done".into());
        create_impl(&db.conn, 1, &a).unwrap();
        create_impl(&db.conn, 1, &b).unwrap();
        assert_eq!(list_impl(&db.conn, 1, Some("done")).unwrap().len(), 1);
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let u = set_status_impl(&db.conn, t.id, "in_progress").unwrap();
        assert_eq!(u.status, "in_progress");
    }
}
```

- [ ] **Step 2-3：`commands/mod.rs` 加 `pub mod tasks;` + lib.rs 注册 6 个命令**

```rust
            commands::tasks::list_tasks,
            commands::tasks::get_task,
            commands::tasks::create_task,
            commands::tasks::update_task,
            commands::tasks::set_task_status,
            commands::tasks::delete_task,
```

- [ ] **Step 4：跑测试 → 64 + 4 = 68 passing**

- [ ] **Step 5-6：Commit + CHANGELOG**

```bash
git commit -m "feat(tasks): 任务 crud + 3 状态 + 按状态筛选 + 删级联工时"
```

`/changelog`：任务 CRUD（todo / in_progress / done），可按状态筛；删除任务级联软删该任务的所有工时。

---

## Task 7: 工时后端 CRUD（写时快照）

**Files:**
- Create: `src-tauri/src/commands/timelogs.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod timelogs;`）
- Modify: `src-tauri/src/lib.rs`（注册 5 条命令）

**Interfaces:**
- Produces:
  - `pub struct TimeLog { id, task_id, member_id, work_date, hours, daily_cost_snapshot_cents, notes, created_at }`
  - `pub struct TimeLogInput { task_id, member_id, work_date, hours, notes? }` — 不接受 `daily_cost_snapshot_cents`（写时从 member 拷贝）
  - `pub struct TimeLogUpdateInput { work_date, hours, notes? }` — 只允许改这三项；禁止改 member_id、task_id、daily_cost_snapshot_cents
  - 5 commands: `list_time_logs_by_task(task_id)`, `list_time_logs_by_project(project_id)`, `create_time_log(input)`, `update_time_log(id, input)`, `delete_time_log(id)`
- Consumes: `domain::soft_delete::soft_delete_time_log`

- [ ] **Step 1：写 `commands/timelogs.rs`**

```rust
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct TimeLog {
    pub id: i64,
    pub task_id: i64,
    pub member_id: i64,
    pub work_date: String,
    pub hours: f64,
    pub daily_cost_snapshot_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TimeLogInput {
    pub task_id: i64,
    pub member_id: i64,
    pub work_date: String,
    pub hours: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimeLogUpdateInput {
    pub work_date: String,
    pub hours: f64,
    pub notes: Option<String>,
}

fn row_to_log(row: &rusqlite::Row) -> rusqlite::Result<TimeLog> {
    Ok(TimeLog {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        member_id: row.get("member_id")?,
        work_date: row.get("work_date")?,
        hours: row.get("hours")?,
        daily_cost_snapshot_cents: row.get("daily_cost_snapshot_cents")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
    })
}

fn validate_hours(hours: f64) -> AppResult<()> {
    if !(0.0..=24.0).contains(&hours) {
        return Err(AppError::Validation("工时需在 [0, 24] 之间".into()));
    }
    Ok(())
}

fn validate_date(date: &str) -> AppResult<()> {
    if date.trim().is_empty() {
        return Err(AppError::Validation("工作日期必填".into()));
    }
    Ok(())
}

pub(crate) fn list_by_task_impl(conn: &Connection, task_id: i64) -> AppResult<Vec<TimeLog>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM time_logs
         WHERE task_id = ?1 AND deleted_at IS NULL
         ORDER BY work_date DESC, id DESC",
    )?;
    let rows = stmt.query_map([task_id], row_to_log)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn list_by_project_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<TimeLog>> {
    let mut stmt = conn.prepare(
        "SELECT tl.* FROM time_logs tl
         JOIN tasks t ON t.id = tl.task_id
         WHERE t.project_id = ?1 AND tl.deleted_at IS NULL AND t.deleted_at IS NULL
         ORDER BY tl.work_date DESC, tl.id DESC",
    )?;
    let rows = stmt.query_map([project_id], row_to_log)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub(crate) fn create_impl(conn: &Connection, input: &TimeLogInput) -> AppResult<TimeLog> {
    validate_hours(input.hours)?;
    validate_date(&input.work_date)?;
    // verify task is active and load member's current daily_cost_cents + assert task↔member share company
    let row: Option<(i64, i64)> = conn.query_row(
        "SELECT m.daily_cost_cents, p.company_id
         FROM members m JOIN projects p ON p.company_id = m.company_id
         JOIN tasks t ON t.project_id = p.id
         WHERE t.id = ?1 AND m.id = ?2 AND t.deleted_at IS NULL AND m.deleted_at IS NULL",
        [input.task_id, input.member_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    let (snapshot, _company_id) = match row {
        Some(t) => t,
        None => {
            return Err(AppError::Validation(
                "任务与成员公司不一致或资源不存在".into(),
            ));
        }
    };
    conn.execute(
        "INSERT INTO time_logs(task_id, member_id, work_date, hours,
                               daily_cost_snapshot_cents, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            input.task_id,
            input.member_id,
            input.work_date.trim(),
            input.hours,
            snapshot,
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TimeLogUpdateInput,
) -> AppResult<TimeLog> {
    validate_hours(input.hours)?;
    validate_date(&input.work_date)?;
    let n = conn.execute(
        "UPDATE time_logs SET
            work_date = ?1,
            hours = ?2,
            notes = ?3
         WHERE id = ?4 AND deleted_at IS NULL",
        rusqlite::params![
            input.work_date.trim(),
            input.hours,
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "time_log", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_time_log(conn, id)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<TimeLog> {
    conn.query_row(
        "SELECT * FROM time_logs WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_log,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "time_log", id },
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
pub fn list_time_logs_by_task(
    state: tauri::State<AppState>,
    task_id: i64,
) -> AppResult<Vec<TimeLog>> {
    with_conn(&state, |c| list_by_task_impl(c, task_id))
}
#[tauri::command]
pub fn list_time_logs_by_project(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<TimeLog>> {
    with_conn(&state, |c| list_by_project_impl(c, project_id))
}
#[tauri::command]
pub fn create_time_log(
    state: tauri::State<AppState>,
    input: TimeLogInput,
) -> AppResult<TimeLog> {
    with_conn(&state, |c| create_impl(c, &input))
}
#[tauri::command]
pub fn update_time_log(
    state: tauri::State<AppState>,
    id: i64,
    input: TimeLogUpdateInput,
) -> AppResult<TimeLog> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_time_log(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", []).unwrap();
            conn.execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input() -> TimeLogInput {
        TimeLogInput {
            task_id: 1,
            member_id: 1,
            work_date: "2026-06-01".into(),
            hours: 8.0,
            notes: None,
        }
    }

    #[test]
    fn create_snapshots_member_daily_cost() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        assert_eq!(log.daily_cost_snapshot_cents, 80000);
    }

    #[test]
    fn snapshot_does_not_change_when_member_repriced() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        db.conn.execute(
            "UPDATE members SET daily_cost_cents = 999999 WHERE id = 1",
            [],
        ).unwrap();
        let refetched = get_impl(&db.conn, log.id).unwrap();
        assert_eq!(refetched.daily_cost_snapshot_cents, 80000);
    }

    #[test]
    fn update_only_hours_date_notes() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        let updated = update_impl(
            &db.conn,
            log.id,
            &TimeLogUpdateInput {
                work_date: "2026-06-02".into(),
                hours: 4.0,
                notes: Some("延期".into()),
            },
        ).unwrap();
        assert_eq!(updated.work_date, "2026-06-02");
        assert_eq!(updated.hours, 4.0);
        assert_eq!(updated.member_id, 1); // unchanged
        assert_eq!(updated.task_id, 1); // unchanged
        assert_eq!(updated.daily_cost_snapshot_cents, 80000); // unchanged
    }

    #[test]
    fn hours_out_of_range_rejected() {
        let db = TestDb::new();
        let mut bad = input();
        bad.hours = 25.0;
        let err = create_impl(&db.conn, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn cross_company_task_member_rejected() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO companies(name) VALUES('Other')", []).unwrap();
        db.conn.execute(
            "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(2, 'Foreign', 60000)",
            [],
        ).unwrap();
        let mut bad = input();
        bad.member_id = 2;
        let err = create_impl(&db.conn, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
```

- [ ] **Step 2-3：`commands/mod.rs` 加 `pub mod timelogs;` + lib.rs 注册 5 个命令**

```rust
            commands::timelogs::list_time_logs_by_task,
            commands::timelogs::list_time_logs_by_project,
            commands::timelogs::create_time_log,
            commands::timelogs::update_time_log,
            commands::timelogs::delete_time_log,
```

- [ ] **Step 4：跑测试 → 68 + 5 = 73 passing**

- [ ] **Step 5-6：Commit + CHANGELOG**

```bash
git commit -m "feat(timelogs): 工时 crud + 写时快照日成本 + 跨公司防御"
```

`/changelog`：工时 CRUD；创建时从 member 当前 daily_cost_cents 拷贝到 `daily_cost_snapshot_cents`（调薪不影响历史）；update 只允许改 hours/work_date/notes；写入时校验 task↔member 同公司。

---

## Task 8: 回收站后端扩展 + 财务命令 + costs update 防御

**Files:**
- Modify: `src-tauri/src/commands/trash.rs`（list_impl 加 task / contract_payment / time_log；purge_impl 加这三类）
- Modify: `src-tauri/src/commands/costs.rs`（update_impl 加 category↔company 防御 — M2 final review M-1）
- Create: `src-tauri/src/commands/projects.rs` 增 `get_project_financial_summary` 命令（M2 final review §3 闭环所需）
- Modify: `src-tauri/src/lib.rs`（注册 1 个新命令）

**Interfaces:**
- Produces:
  - 1 new command: `get_project_financial_summary(project_id) -> ProjectFinancialSummary`
  - 修改 `list_trash` 返回值结构不变（仍是 `Vec<TrashItem>`），但 `entity_type` 可取值现在多了 `"task"` / `"contract_payment"` / `"time_log"`
- Consumes: `domain::profit::project_financial_summary`、`domain::soft_delete::{restore_task, restore_payment, restore_time_log, soft_delete_*}`

- [ ] **Step 1：修改 `commands/trash.rs::list_impl`**

替换整个 `list_impl`：

```rust
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

    // soft-deleted cost entries
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

    // soft-deleted tasks
    let mut st = conn.prepare(
        "SELECT t.id, t.project_id, t.title, t.deleted_at
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE p.company_id = ?1 AND t.deleted_at IS NOT NULL
         ORDER BY t.deleted_at DESC",
    )?;
    let rows = st.query_map([company_id], |r| {
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "task".into(),
            name: format!("任务: {}", r.get::<_, String>(2)?),
            deleted_at: r.get::<_, String>(3)?,
            project_id: Some(r.get::<_, i64>(1)?),
        })
    })?;
    for r in rows { out.push(r?); }

    // soft-deleted contract payments
    let mut spay = conn.prepare(
        "SELECT cp.id, cp.project_id, cp.name, cp.expected_amount_cents, cp.deleted_at
         FROM contract_payments cp JOIN projects p ON p.id = cp.project_id
         WHERE p.company_id = ?1 AND cp.deleted_at IS NOT NULL
         ORDER BY cp.deleted_at DESC",
    )?;
    let rows = spay.query_map([company_id], |r| {
        let amt: i64 = r.get(3)?;
        let yuan = amt as f64 / 100.0;
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "contract_payment".into(),
            name: format!("收款 ¥{:.2} ({})", yuan, r.get::<_, String>(2)?),
            deleted_at: r.get::<_, String>(4)?,
            project_id: Some(r.get::<_, i64>(1)?),
        })
    })?;
    for r in rows { out.push(r?); }

    // soft-deleted time logs
    let mut sl = conn.prepare(
        "SELECT tl.id, t.project_id, tl.work_date, tl.hours, tl.deleted_at
         FROM time_logs tl
         JOIN tasks t ON t.id = tl.task_id
         JOIN projects p ON p.id = t.project_id
         WHERE p.company_id = ?1 AND tl.deleted_at IS NOT NULL
         ORDER BY tl.deleted_at DESC",
    )?;
    let rows = sl.query_map([company_id], |r| {
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "time_log".into(),
            name: format!("工时 {} {}h", r.get::<_, String>(2)?, r.get::<_, f64>(3)?),
            deleted_at: r.get::<_, String>(4)?,
            project_id: Some(r.get::<_, i64>(1)?),
        })
    })?;
    for r in rows { out.push(r?); }

    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}
```

- [ ] **Step 2：修改 `commands/trash.rs::restore_impl`**

```rust
pub(crate) fn restore_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    match entity_type {
        "project" => soft_delete::restore_project(conn, id),
        "cost_entry" => soft_delete::restore_cost_entry(conn, id),
        "task" => soft_delete::restore_task(conn, id),
        "contract_payment" => soft_delete::restore_payment(conn, id),
        "time_log" => soft_delete::restore_time_log(conn, id),
        other => Err(AppError::Validation(format!("未知实体类型：{other}"))),
    }
}
```

- [ ] **Step 3：修改 `commands/trash.rs::purge_impl`**

替换整个 `purge_impl`（注意 project 的物理级联现在要清更多表）：

```rust
pub(crate) fn purge_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    let table = match entity_type {
        "project" => "projects",
        "cost_entry" => "cost_entries",
        "task" => "tasks",
        "contract_payment" => "contract_payments",
        "time_log" => "time_logs",
        other => return Err(AppError::Validation(format!("未知实体类型：{other}"))),
    };
    let tx = conn.unchecked_transaction()?;
    if entity_type == "project" {
        // physically delete children first to respect FK
        tx.execute(
            "DELETE FROM time_logs
             WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            [id],
        )?;
        tx.execute("DELETE FROM tasks WHERE project_id = ?1", [id])?;
        tx.execute("DELETE FROM cost_entries WHERE project_id = ?1", [id])?;
        tx.execute("DELETE FROM contract_payments WHERE project_id = ?1", [id])?;
    } else if entity_type == "task" {
        tx.execute("DELETE FROM time_logs WHERE task_id = ?1", [id])?;
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
```

- [ ] **Step 4：在 `commands/trash.rs::tests` 追加新测试**

```rust
#[test]
fn list_includes_task_payment_timelog() {
    let db = TestDb::new();
    // add fixtures for the 3 new entity types
    db.conn.execute(
        "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO tasks(project_id, title) VALUES(1, 'T')",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO contract_payments(project_id, name, expected_amount_cents)
         VALUES(1, '预付', 50000)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
         VALUES(1, 1, '2026-06-01', 8.0, 80000)",
        [],
    ).unwrap();
    soft_delete::soft_delete_task(&db.conn, 1).unwrap(); // also cascades time_log 1
    soft_delete::soft_delete_payment(&db.conn, 1).unwrap();
    let items = list_impl(&db.conn, 1).unwrap();
    let types: Vec<&str> = items.iter().map(|i| i.entity_type.as_str()).collect();
    assert!(types.contains(&"task"));
    assert!(types.contains(&"contract_payment"));
    assert!(types.contains(&"time_log"));
}

#[test]
fn purge_task_cascades_timelogs() {
    let db = TestDb::new();
    db.conn.execute(
        "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO tasks(project_id, title) VALUES(1, 'T')",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
         VALUES(1, 1, '2026-06-01', 8.0, 80000)",
        [],
    ).unwrap();
    soft_delete::soft_delete_task(&db.conn, 1).unwrap();
    purge_impl(&db.conn, "task", 1).unwrap();
    let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM tasks WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    let m: i64 = db.conn.query_row("SELECT COUNT(*) FROM time_logs WHERE task_id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(m, 0);
}
```

- [ ] **Step 5：修改 `commands/costs.rs::update_impl` 加 category↔company 防御**

替换 `update_impl`：

```rust
pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &CostEntryInput,
) -> AppResult<CostEntry> {
    validate(input)?;
    // verify category belongs to the project's company (defense in depth — M2-T5 originally accepted gap)
    let ok: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories cc
         JOIN projects p ON p.company_id = cc.company_id
         JOIN cost_entries ce ON ce.project_id = p.id
         WHERE ce.id = ?1 AND cc.id = ?2 AND cc.deleted_at IS NULL",
        [id, input.category_id],
        |r| r.get(0),
    )?;
    if ok == 0 {
        return Err(AppError::Validation(
            "科目与项目公司不匹配或科目不存在".into(),
        ));
    }
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
```

并在 `commands/costs.rs::tests` 追加：

```rust
#[test]
fn update_cross_company_category_rejected() {
    let db = TestDb::new();
    let e = create_impl(&db.conn, 1, &ce(50)).unwrap();
    // make a foreign company & category
    db.conn.execute("INSERT INTO companies(name) VALUES('Other')", []).unwrap();
    db.conn.execute(
        "INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(2, 'X', 0, 0)",
        [],
    ).unwrap();
    let mut bad = ce(50);
    bad.category_id = 2;
    let err = update_impl(&db.conn, e.id, &bad).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}
```

- [ ] **Step 6：在 `commands/projects.rs` 加 `get_project_financial_summary` 命令**

在文件底部（`delete_project` 命令之后）插入：

```rust
#[tauri::command]
pub fn get_project_financial_summary(
    state: tauri::State<AppState>,
    id: i64,
) -> AppResult<crate::domain::profit::ProjectFinancialSummary> {
    with_conn(&state, |c| {
        crate::domain::profit::project_financial_summary(c, id)
    })
}
```

- [ ] **Step 7：在 `lib.rs` invoke_handler 末尾追加新命令**

```rust
            commands::projects::get_project_financial_summary,
```

- [ ] **Step 8：跑测试**

```bash
cargo test 2>&1 | tail -5
```
预期：73 + 2 (trash) + 1 (costs update defense) = 76 passing。

- [ ] **Step 9：clippy + fmt**

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
全绿。

- [ ] **Step 10：Commit**

```bash
git add src-tauri/src
git commit -m "feat(trash+costs): 回收站扩展 4 类 + cost update 跨公司防御 + 财务命令"
```

- [ ] **Step 11：CHANGELOG**

`/changelog`：回收站现包含已删任务/收款/工时；`get_project_financial_summary` Tauri 命令暴露；`update_cost_entry` 与 `create_cost_entry` 一样校验跨公司科目。

---

## Task 9: 前端基础库扩展 + types/i18n/HoursInput/store reset 模式 + financial store

**Files:**
- Modify: `src/types/index.ts`（加 Member / Task / TimeLog / ContractPayment / ProjectFinancialSummary 等类型）
- Modify: `src/i18n/zh-CN.json`（加 member / payment / task / timelog / taskStatus 命名空间）
- Create: `src/lib/time.ts`（轻工具：今日 ISO 日期、hours 显示格式化）
- Create: `src/components/forms/HoursInput.tsx`
- Create: `src/stores/financial.ts`（按 projectId 缓存 ProjectFinancialSummary；T11/T12/T13 共用）
- Modify: `src/stores/auth.ts`（lock() 调用所有 store reset — M2 final review I-1）
- Modify: `src/stores/company.ts` / `categories.ts` / `projects.ts` / `costs.ts` / `trash.ts`（加 `reset` action）
- Modify: `components.json` / `src/components/ui/checkbox.tsx`（shadcn add checkbox）

**Interfaces:**
- Produces:
  - `todayIso(): string` — 返回 `YYYY-MM-DD`
  - `formatHours(h: number): string` — `8.0` → `"8 小时"`，`0.5` → `"30 分钟"`，`1.5` → `"1.5 小时"`
  - `<HoursInput value={hours} onChange={(hours: number) => ...} />` — 0-24 范围，0.25 精度允许
  - 所有现有 store 多一个 `reset` action；`useAuthStore.lock` 调用它们

- [ ] **Step 1：装 shadcn checkbox**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm dlx shadcn@latest add checkbox --yes
```

- [ ] **Step 2：写 `src/lib/time.ts`**

```typescript
export function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function formatHours(h: number): string {
  if (!Number.isFinite(h)) return "—";
  if (h === 0) return "0";
  if (h < 1) {
    const min = Math.round(h * 60);
    return `${min} 分钟`;
  }
  if (Number.isInteger(h)) return `${h} 小时`;
  return `${h.toFixed(2).replace(/\.?0+$/, "")} 小时`;
}
```

- [ ] **Step 3：写 `src/components/forms/HoursInput.tsx`**

```typescript
import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";

interface Props {
  value: number;
  onChange: (hours: number) => void;
  disabled?: boolean;
}

export function HoursInput({ value, onChange, disabled }: Props) {
  const [text, setText] = useState(value > 0 ? String(value) : "");

  useEffect(() => {
    setText(value > 0 ? String(value) : "");
  }, [value]);

  const commit = (raw: string) => {
    setText(raw);
    const n = Number(raw);
    if (Number.isFinite(n) && n >= 0 && n <= 24) {
      onChange(n);
    }
  };

  return (
    <div className="flex items-center gap-2">
      <Input
        inputMode="decimal"
        type="number"
        min="0"
        max="24"
        step="0.25"
        value={text}
        disabled={disabled}
        placeholder="8"
        onChange={(e) => commit(e.target.value)}
      />
      <span className="text-sm text-muted-foreground">小时</span>
    </div>
  );
}
```

- [ ] **Step 4：扩展 `src/types/index.ts`**

在文件末尾追加：

```typescript
export interface Member {
  id: number;
  company_id: number;
  name: string;
  role: string | null;
  daily_cost_cents: number;
  effective_from: string | null;
  is_active: boolean;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface MemberInput {
  name: string;
  role?: string | null;
  daily_cost_cents?: number | null;
  effective_from?: string | null;
  is_active?: boolean | null;
  notes?: string | null;
}

export interface ContractPayment {
  id: number;
  project_id: number;
  name: string;
  expected_amount_cents: number;
  expected_date: string | null;
  actual_amount_cents: number | null;
  actual_received_at: string | null;
  sort_order: number;
  notes: string | null;
}

export interface PaymentInput {
  name: string;
  expected_amount_cents: number;
  expected_date?: string | null;
  actual_amount_cents?: number | null;
  actual_received_at?: string | null;
  notes?: string | null;
}

export interface Task {
  id: number;
  project_id: number;
  title: string;
  description: string | null;
  assignee_id: number | null;
  status: string;
  estimated_hours: number | null;
  due_date: string | null;
  created_at: string;
  updated_at: string;
}

export interface TaskInput {
  title: string;
  description?: string | null;
  assignee_id?: number | null;
  status?: string | null;
  estimated_hours?: number | null;
  due_date?: string | null;
}

export interface TimeLog {
  id: number;
  task_id: number;
  member_id: number;
  work_date: string;
  hours: number;
  daily_cost_snapshot_cents: number;
  notes: string | null;
  created_at: string;
}

export interface TimeLogInput {
  task_id: number;
  member_id: number;
  work_date: string;
  hours: number;
  notes?: string | null;
}

export interface TimeLogUpdateInput {
  work_date: string;
  hours: number;
  notes?: string | null;
}

export interface ProjectFinancialSummary {
  revenue_tax_inclusive_cents: number;
  revenue_tax_exclusive_cents: number;
  tax_amount_cents: number;
  general_cost_cents: number;
  labor_cost_cents: number;
  total_cost_cents: number;
  gross_profit_cents: number;
  profit_rate: number;
  expected_payment_cents: number;
  actual_payment_cents: number;
  collection_rate: number;
}
```

- [ ] **Step 5：扩展 `src/i18n/zh-CN.json`**

在最后一个顶层对象之后（`status` 之后）追加：

```json
  "member": {
    "title": "成员",
    "create": "新建成员",
    "edit": "编辑成员",
    "delete": "删除",
    "deleteConfirm": "确认删除「{{name}}」？若有工时记录将无法删除，请改为归档。",
    "archive": "归档",
    "unarchive": "取消归档",
    "name": "姓名",
    "role": "角色 / 岗位",
    "dailyCost": "日成本（每人天）",
    "effectiveFrom": "生效日期",
    "isActive": "在职",
    "notes": "备注",
    "save": "保存",
    "empty": "暂无成员，新建第一个",
    "nameRequired": "姓名必填",
    "active": "在职",
    "inactive": "已归档"
  },
  "payment": {
    "title": "收款节点",
    "create": "新建收款节点",
    "edit": "编辑",
    "delete": "删除",
    "name": "名称",
    "expectedAmount": "预期金额",
    "expectedDate": "预期日期",
    "actualAmount": "实收金额",
    "actualReceivedAt": "实收日期",
    "notes": "备注",
    "markReceived": "标为已收",
    "save": "保存",
    "empty": "暂无收款节点",
    "nameRequired": "名称必填",
    "expectedLabel": "预期合计",
    "actualLabel": "实收合计",
    "collectionRate": "回款率"
  },
  "task": {
    "title": "任务",
    "create": "新建任务",
    "edit": "编辑",
    "delete": "删除",
    "name": "标题",
    "description": "描述",
    "assignee": "负责人",
    "status": "状态",
    "estimatedHours": "预估工时",
    "dueDate": "截止日期",
    "notes": "备注",
    "save": "保存",
    "empty": "暂无任务，新建第一个",
    "titleRequired": "标题必填",
    "filterByStatus": "按状态筛选",
    "allStatuses": "全部状态",
    "unassigned": "未指派",
    "logsCount": "{{n}} 条工时"
  },
  "taskStatus": {
    "todo": "待办",
    "in_progress": "进行中",
    "done": "已完成"
  },
  "timelog": {
    "title": "工时",
    "add": "录入工时",
    "edit": "编辑工时",
    "delete": "删除",
    "deleteConfirm": "确认删除这条工时？",
    "member": "成员",
    "workDate": "工作日期",
    "hours": "小时数",
    "notes": "备注",
    "save": "保存",
    "empty": "该任务尚无工时记录",
    "hoursRequired": "工时需在 0-24 之间",
    "dateRequired": "工作日期必填",
    "memberRequired": "请选择成员",
    "snapshotHint": "录入时锁定该成员的日成本快照，后续调薪不影响本条记录"
  },
  "financial": {
    "revenueInclusive": "含税收入",
    "revenueExclusive": "不含税收入",
    "tax": "税额",
    "generalCost": "一般成本",
    "laborCost": "人力成本",
    "totalCost": "总成本",
    "grossProfit": "毛利润",
    "profitRate": "利润率",
    "expectedPayment": "预期回款",
    "actualPayment": "实收",
    "collectionRate": "回款率"
  }
```

在 `nav` 对象里追加 `"members": "成员"` 项（M1 已有，但要确认存在）。

- [ ] **Step 5b：写 `src/stores/financial.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ProjectFinancialSummary } from "@/types";

interface S {
  byProject: Record<number, ProjectFinancialSummary>;
  refresh: (projectId: number) => Promise<void>;
  reset: () => void;
}

export const useFinancialStore = create<S>((set, get) => ({
  byProject: {},
  async refresh(projectId) {
    try {
      const f = await call<ProjectFinancialSummary>("get_project_financial_summary", { id: projectId });
      set({ byProject: { ...get().byProject, [projectId]: f } });
    } catch {
      // ignore — non-fatal; overview will show "—"
    }
  },
  reset() {
    set({ byProject: {} });
  },
}));
```

- [ ] **Step 6：在所有现有 store 加 `reset` action**

对 `src/stores/company.ts`、`categories.ts`、`projects.ts`、`costs.ts`、`trash.ts` 分别：

**company.ts** — 在 `CompanyState` 接口加 `reset: () => void`，state 加 `reset: () => set({ list: [], currentId: null, loaded: false })`

**categories.ts** — 加 `reset: () => void` 到 `S` 接口，加 `reset: () => set({ list: [], loadedForCompany: null })`

**projects.ts** — 加 `reset: () => void`，加 `reset: () => set({ list: [], loadedForCompany: null, statusFilter: null })`

**costs.ts** — 加 `reset: () => void`，加 `reset: () => set({ entriesByProject: {}, summaryByProject: {} })`

**trash.ts** — 加 `reset: () => void`，加 `reset: () => set({ items: [], loadedForCompany: null })`

每个 store 文件的具体改动模式：

```typescript
// 接口加一行
reset: () => void;

// 实现里加最后一项
async refresh() { ... },
// 或其他已有方法之后
reset() {
  set({ /* initial state values */ });
},
```

注意：zustand `set` 不需要 `async`，`reset` 是同步的。

- [ ] **Step 7：修改 `src/stores/auth.ts` 的 lock**

```typescript
import { useCompanyStore } from "./company";
import { useCategoriesStore } from "./categories";
import { useProjectsStore } from "./projects";
import { useCostsStore } from "./costs";
import { useTrashStore } from "./trash";
import { useFinancialStore } from "./financial";

// ...

async lock() {
  await call<void>("lock");
  // reset all entity stores so a re-unlock pulls fresh data
  useCompanyStore.getState().reset();
  useCategoriesStore.getState().reset();
  useProjectsStore.getState().reset();
  useCostsStore.getState().reset();
  useTrashStore.getState().reset();
  useFinancialStore.getState().reset();
  set({ status: "locked" });
},
```

- [ ] **Step 8：TS 编译 + build 验证**

```bash
pnpm tsc --noEmit
pnpm build
```
预期：0 errors / build 通过。

- [ ] **Step 9：Commit**

```bash
git add src/types src/i18n src/lib src/components src/stores components.json
git commit -m "feat(ui): m3 前端基础 + store reset 模式 + HoursInput + shadcn checkbox"
```

- [ ] **Step 10：CHANGELOG**

`/changelog`：M3 前端类型扩展（Member/Task/TimeLog/ContractPayment/FinancialSummary）；i18n 新增 5 个命名空间；HoursInput 受控组件；lock() 现重置所有 store（解决 M2 final review I-1）；shadcn Checkbox 补装。

---

## Task 10: 成员管理页（/members 路由）

**Files:**
- Create: `src/stores/members.ts`
- Create: `src/routes/members.tsx`
- Modify: `src/App.tsx`（注册 /members 真路由）
- Modify: `src/components/layout/Sidebar.tsx`（确保 nav `members` 项存在并指向 `/members`）

**Interfaces:**
- Produces:
  - `useMembersStore`：`{ list, loadedForCompany, loadFor(companyId), create(companyId, input), update(id, input), setActive(id, isActive), softDelete(id), reset() }`
- Consumes: Task 4 的 6 个 member 命令

- [ ] **Step 1：写 `src/stores/members.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Member, MemberInput } from "@/types";

interface S {
  list: Member[];
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  create: (companyId: number, input: MemberInput) => Promise<Member>;
  update: (id: number, input: MemberInput) => Promise<Member>;
  setActive: (id: number, isActive: boolean) => Promise<void>;
  softDelete: (id: number) => Promise<void>;
  reset: () => void;
}

export const useMembersStore = create<S>((set, get) => ({
  list: [],
  loadedForCompany: null,
  async loadFor(companyId) {
    const list = await call<Member[]>("list_members", { companyId });
    set({ list, loadedForCompany: companyId });
  },
  async create(companyId, input) {
    const m = await call<Member>("create_member", { companyId, input });
    set({ list: [m, ...get().list] });
    return m;
  },
  async update(id, input) {
    const m = await call<Member>("update_member", { id, input });
    set({ list: get().list.map((x) => (x.id === id ? m : x)) });
    return m;
  },
  async setActive(id, isActive) {
    const m = await call<Member>("set_member_active", { id, isActive });
    set({ list: get().list.map((x) => (x.id === id ? m : x)) });
  },
  async softDelete(id) {
    await call<void>("delete_member", { id });
    set({ list: get().list.filter((x) => x.id !== id) });
  },
  reset() {
    set({ list: [], loadedForCompany: null });
  },
}));
```

并把这个 store 加入 `useAuthStore.lock` 的 reset 链（修改 Task 9 Step 7 的代码 — 在 `useTrashStore.getState().reset()` 之后加一行）。**注意：Task 9 实现 lock reset 时不知道 useMembersStore 存在，所以这里要修一下 auth.ts**：

```typescript
import { useMembersStore } from "./members";
// ...
async lock() {
  await call<void>("lock");
  useCompanyStore.getState().reset();
  useCategoriesStore.getState().reset();
  useProjectsStore.getState().reset();
  useCostsStore.getState().reset();
  useTrashStore.getState().reset();
  useMembersStore.getState().reset();
  set({ status: "locked" });
},
```

- [ ] **Step 2：写 `src/routes/members.tsx`**

```typescript
import { useEffect, useState } from "react";
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
import { MoneyInput } from "@/components/forms/MoneyInput";
import { formatCNY } from "@/lib/money";
import { useCompanyStore } from "@/stores/company";
import { useMembersStore } from "@/stores/members";
import type { Member, MemberInput } from "@/types";

export default function MembersPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { list, loadedForCompany, loadFor, create, update, setActive, softDelete } =
    useMembersStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Member | null>(null);

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">请先选择公司</div>;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("member.title")}</h1>
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("member.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("member.create")}</DialogTitle></DialogHeader>
            <MemberForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(currentId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("member.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-2">
          {list.map((m) => (
            <Card key={m.id} className={m.is_active ? undefined : "opacity-60"}>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 py-3">
                <div className="space-y-1">
                  <CardTitle className="text-base flex items-center gap-2">
                    <span>{m.name}</span>
                    <Badge variant={m.is_active ? "secondary" : "outline"}>
                      {m.is_active ? t("member.active") : t("member.inactive")}
                    </Badge>
                  </CardTitle>
                  <div className="text-xs text-muted-foreground">
                    {m.role ?? "—"} · {formatCNY(m.daily_cost_cents)}/天
                    {m.effective_from && ` · ${t("member.effectiveFrom")} ${m.effective_from}`}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => setEditing(m)}>{t("member.edit")}</Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      try { await setActive(m.id, !m.is_active); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >
                    {m.is_active ? t("member.archive") : t("member.unarchive")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => {
                      if (!confirm(t("member.deleteConfirm", { name: m.name }))) return;
                      try { await softDelete(m.id); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}
                  >
                    {t("member.delete")}
                  </Button>
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("member.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <MemberForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function MemberForm({ initial, onSubmit, onCancel }: {
  initial?: Member;
  onSubmit: (input: MemberInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [role, setRole] = useState(initial?.role ?? "");
  const [dailyCost, setDailyCost] = useState(initial?.daily_cost_cents ?? 0);
  const [effectiveFrom, setEffectiveFrom] = useState(initial?.effective_from ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("member.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        role: role.trim() || null,
        daily_cost_cents: dailyCost,
        effective_from: effectiveFrom || null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <Label>{t("member.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </div>
      <div className="space-y-1">
        <Label>{t("member.role")}</Label>
        <Input value={role ?? ""} onChange={(e) => setRole(e.target.value)} />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <Label>{t("member.dailyCost")}</Label>
          <MoneyInput value={dailyCost} onChange={setDailyCost} />
        </div>
        <div className="space-y-1">
          <Label>{t("member.effectiveFrom")}</Label>
          <Input type="date" value={effectiveFrom ?? ""} onChange={(e) => setEffectiveFrom(e.target.value)} />
        </div>
      </div>
      <div className="space-y-1">
        <Label>{t("member.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} />
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("member.save")}</Button>
      </DialogFooter>
    </div>
  );
}
```

- [ ] **Step 3：注册 `/members` 路由**

修改 `src/App.tsx`：加 import `import MembersPage from "@/routes/members";`，并在 `<Route path="/" element={<AppLayout />}>` 块内加：

```typescript
<Route path="members" element={<MembersPage />} />
```

放在 `projects` 与 `categories` 之间。

- [ ] **Step 4：Sidebar 加 members 项**

修改 `src/components/layout/Sidebar.tsx` 的 ITEMS：

```typescript
import { LayoutDashboard, Building2, FolderKanban, Users, Tag, Trash2, Settings } from "lucide-react";

const ITEMS = [
  { to: "/dashboard", icon: LayoutDashboard, key: "nav.dashboard" as const },
  { to: "/projects", icon: FolderKanban, key: "nav.projects" as const },
  { to: "/members", icon: Users, key: "nav.members" as const },
  { to: "/categories", icon: Tag, key: "nav.categories" as const },
  { to: "/companies", icon: Building2, key: "nav.companies" as const },
  { to: "/trash", icon: Trash2, key: "nav.trash" as const },
  { to: "/settings", icon: Settings, key: "nav.settings" as const },
];
```

- [ ] **Step 5：TS + build 验证**

```bash
pnpm tsc --noEmit && pnpm build
```
0 errors。

- [ ] **Step 6：Commit + CHANGELOG**

```bash
git commit -m "feat(members): 成员管理页（在职/归档/删除阻断）"
```

`/changelog`：`/members` 路由 + 成员管理 UI（在职/归档 Badge、归档/取消归档按钮、有工时则拒绝删除并提示归档）。

---

## Task 11: 项目详情「收款」Tab

**Files:**
- Create: `src/stores/payments.ts`
- Modify: `src/routes/projects/detail.tsx`（激活 `payments` Tab，新增 `PaymentsPanel`）

**Interfaces:**
- Produces:
  - `usePaymentsStore`：`{ byProject, loadFor(projectId), create(projectId, input), update(id, input, projectId), markReceived(id, cents, date, projectId), softDelete(id, projectId), reset() }`
- Consumes: Task 5 的 6 个 payment 命令

- [ ] **Step 1：写 `src/stores/payments.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { ContractPayment, PaymentInput } from "@/types";

interface S {
  byProject: Record<number, ContractPayment[]>;
  loadFor: (projectId: number) => Promise<void>;
  create: (projectId: number, input: PaymentInput) => Promise<void>;
  update: (id: number, input: PaymentInput, projectId: number) => Promise<void>;
  markReceived: (id: number, actualAmountCents: number, actualReceivedAt: string, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

import { useFinancialStore } from "./financial";

export const usePaymentsStore = create<S>((set, get) => ({
  byProject: {},
  async loadFor(projectId) {
    const list = await call<ContractPayment[]>("list_payments", { projectId });
    set({ byProject: { ...get().byProject, [projectId]: list } });
  },
  async create(projectId, input) {
    await call<ContractPayment>("create_payment", { projectId, input });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async update(id, input, projectId) {
    await call<ContractPayment>("update_payment", { id, input });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async markReceived(id, actualAmountCents, actualReceivedAt, projectId) {
    await call<ContractPayment>("mark_payment_received", {
      id,
      actualAmountCents,
      actualReceivedAt,
    });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async softDelete(id, projectId) {
    await call<void>("delete_payment", { id });
    await get().loadFor(projectId);
    await useFinancialStore.getState().refresh(projectId);
  },
  reset() {
    set({ byProject: {} });
  },
}));
```

把 store 加入 lock reset 链：编辑 `src/stores/auth.ts` 加 `import { usePaymentsStore } from "./payments";` 与对应 `usePaymentsStore.getState().reset();`。

- [ ] **Step 2：修改 `src/routes/projects/detail.tsx`** — 激活 payments Tab

替换两处：

第一处，`<TabsList>` 中把 payments Tab 的 `disabled` 移除并去掉 `（M3）`：

```typescript
<TabsTrigger value="payments">收款</TabsTrigger>
```

第二处，在 `<TabsContent value="costs">` 之后追加：

```typescript
<TabsContent value="payments" className="mt-4">
  <PaymentsPanel projectId={project.id} />
</TabsContent>
```

在文件末尾追加 `PaymentsPanel` 组件 + `PaymentForm` 子组件：

```typescript
function PaymentsPanel({ projectId }: { projectId: number }) {
  const { t } = useTranslation();
  const { byProject, loadFor, create, update, markReceived, softDelete } = usePaymentsStore();
  const list = byProject[projectId] ?? [];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<ContractPayment | null>(null);
  const [marking, setMarking] = useState<ContractPayment | null>(null);

  useEffect(() => { loadFor(projectId); }, [projectId, loadFor]);

  const expectedTotal = list.reduce((s, p) => s + p.expected_amount_cents, 0);
  const actualTotal = list.reduce(
    (s, p) => s + (p.actual_received_at && p.actual_amount_cents != null ? p.actual_amount_cents : 0),
    0,
  );
  const rate = expectedTotal === 0 ? 0 : actualTotal / expectedTotal;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-3 gap-3">
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.expectedLabel")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{formatCNY(expectedTotal)}</CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.actualLabel")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{formatCNY(actualTotal)}</CardContent></Card>
        <Card><CardHeader><CardTitle className="text-sm">{t("payment.collectionRate")}</CardTitle></CardHeader>
          <CardContent className="text-2xl font-semibold">{(rate * 100).toFixed(2)}%</CardContent></Card>
      </div>

      <div className="flex justify-end">
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button>{t("payment.create")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("payment.create")}</DialogTitle></DialogHeader>
            <PaymentForm
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(projectId, input); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {list.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("payment.empty")}</CardContent></Card>
      ) : (
        <Card><CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("payment.name")}</TableHead>
                <TableHead className="w-28">{t("payment.expectedDate")}</TableHead>
                <TableHead className="text-right w-32">{t("payment.expectedAmount")}</TableHead>
                <TableHead className="w-28">{t("payment.actualReceivedAt")}</TableHead>
                <TableHead className="text-right w-32">{t("payment.actualAmount")}</TableHead>
                <TableHead className="w-44 text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.map((p) => (
                <TableRow key={p.id}>
                  <TableCell>{p.name}</TableCell>
                  <TableCell>{p.expected_date ?? "—"}</TableCell>
                  <TableCell className="text-right">{formatCNY(p.expected_amount_cents)}</TableCell>
                  <TableCell>{p.actual_received_at ?? "—"}</TableCell>
                  <TableCell className="text-right">
                    {p.actual_amount_cents != null ? formatCNY(p.actual_amount_cents) : "—"}
                  </TableCell>
                  <TableCell className="text-right">
                    {!p.actual_received_at && (
                      <Button size="sm" variant="ghost" onClick={() => setMarking(p)}>
                        {t("payment.markReceived")}
                      </Button>
                    )}
                    <Button size="sm" variant="ghost" onClick={() => setEditing(p)}>{t("payment.edit")}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={async () => {
                        if (!confirm("确认删除该收款节点？")) return;
                        try { await softDelete(p.id, projectId); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >
                      {t("payment.delete")}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent></Card>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("payment.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <PaymentForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!marking} onOpenChange={(o) => !o && setMarking(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("payment.markReceived")}</DialogTitle></DialogHeader>
          {marking && (
            <MarkReceivedForm
              initial={marking}
              onCancel={() => setMarking(null)}
              onSubmit={async (amount, date) => {
                try {
                  await markReceived(marking.id, amount, date, projectId);
                  setMarking(null);
                } catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function PaymentForm({ initial, onSubmit, onCancel }: {
  initial?: ContractPayment;
  onSubmit: (input: PaymentInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [expected, setExpected] = useState(initial?.expected_amount_cents ?? 0);
  const [expectedDate, setExpectedDate] = useState(initial?.expected_date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) return toast.error(t("payment.nameRequired"));
    setBusy(true);
    try {
      await onSubmit({
        name: name.trim(),
        expected_amount_cents: expected,
        expected_date: expectedDate || null,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("payment.name")}</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus /></div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("payment.expectedAmount")}</Label>
          <MoneyInput value={expected} onChange={setExpected} /></div>
        <div className="space-y-1"><Label>{t("payment.expectedDate")}</Label>
          <Input type="date" value={expectedDate ?? ""} onChange={(e) => setExpectedDate(e.target.value)} /></div>
      </div>
      <div className="space-y-1"><Label>{t("payment.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("payment.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function MarkReceivedForm({ initial, onSubmit, onCancel }: {
  initial: ContractPayment;
  onSubmit: (actualAmountCents: number, actualReceivedAt: string) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [amount, setAmount] = useState(initial.actual_amount_cents ?? initial.expected_amount_cents);
  const [date, setDate] = useState(initial.actual_received_at ?? "");
  const [busy, setBusy] = useState(false);

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("payment.actualAmount")}</Label>
        <MoneyInput value={amount} onChange={setAmount} /></div>
      <div className="space-y-1"><Label>{t("payment.actualReceivedAt")}</Label>
        <Input type="date" value={date ?? ""} onChange={(e) => setDate(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (!date) return toast.error("实收日期必填");
            setBusy(true);
            try { await onSubmit(amount, date); }
            finally { setBusy(false); }
          }}
        >确认</Button>
      </DialogFooter>
    </div>
  );
}
```

并把 `import` 区追加：

```typescript
import { usePaymentsStore } from "@/stores/payments";
import type { ContractPayment, PaymentInput } from "@/types";
```

- [ ] **Step 3：TS + build**

预期：0 errors。

- [ ] **Step 4：Commit + CHANGELOG**

```bash
git commit -m "feat(payments): 项目详情收款 tab + 标实收 + 回款率统计"
```

`/changelog`：项目详情新增「收款」Tab；包含预期/实收/回款率三个统计 Card、节点表格、新建/编辑/标实收/删除。

---

## Task 12: 项目详情「任务+工时」Tab

**Files:**
- Create: `src/stores/tasks.ts`
- Create: `src/stores/timelogs.ts`
- Modify: `src/routes/projects/detail.tsx`（激活 `tasks` Tab，新增 `TasksPanel` + `TimeLogsSection`）

**Interfaces:**
- Produces:
  - `useTasksStore`：`{ byProject, loadFor(projectId, statusFilter?), create(projectId, input), update(id, input, projectId), setStatus(id, status, projectId), softDelete(id, projectId), reset() }`
  - `useTimelogsStore`：`{ byTask, loadFor(taskId), create(input), update(id, input, taskId), softDelete(id, taskId), reset() }`
- Consumes: Task 6 + Task 7 后端命令

- [ ] **Step 1：写 `src/stores/tasks.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { Task, TaskInput } from "@/types";

interface S {
  byProject: Record<number, Task[]>;
  statusFilter: string | null;
  loadFor: (projectId: number, statusFilter?: string | null) => Promise<void>;
  create: (projectId: number, input: TaskInput) => Promise<Task>;
  update: (id: number, input: TaskInput, projectId: number) => Promise<Task>;
  setStatus: (id: number, status: string, projectId: number) => Promise<void>;
  softDelete: (id: number, projectId: number) => Promise<void>;
  reset: () => void;
}

export const useTasksStore = create<S>((set, get) => ({
  byProject: {},
  statusFilter: null,
  async loadFor(projectId, statusFilter = null) {
    const list = await call<Task[]>("list_tasks", { projectId, status: statusFilter });
    set({ byProject: { ...get().byProject, [projectId]: list }, statusFilter });
  },
  async create(projectId, input) {
    const t = await call<Task>("create_task", { projectId, input });
    await get().loadFor(projectId, get().statusFilter);
    return t;
  },
  async update(id, input, projectId) {
    const t = await call<Task>("update_task", { id, input });
    await get().loadFor(projectId, get().statusFilter);
    return t;
  },
  async setStatus(id, status, projectId) {
    await call<Task>("set_task_status", { id, status });
    await get().loadFor(projectId, get().statusFilter);
  },
  async softDelete(id, projectId) {
    await call<void>("delete_task", { id });
    await get().loadFor(projectId, get().statusFilter);
  },
  reset() {
    set({ byProject: {}, statusFilter: null });
  },
}));
```

- [ ] **Step 2：写 `src/stores/timelogs.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { TimeLog, TimeLogInput, TimeLogUpdateInput } from "@/types";

interface S {
  byTask: Record<number, TimeLog[]>;
  loadFor: (taskId: number) => Promise<void>;
  create: (input: TimeLogInput) => Promise<void>;
  update: (id: number, input: TimeLogUpdateInput, taskId: number) => Promise<void>;
  softDelete: (id: number, taskId: number) => Promise<void>;
  reset: () => void;
}

import { useFinancialStore } from "./financial";

interface S {
  byTask: Record<number, TimeLog[]>;
  loadFor: (taskId: number) => Promise<void>;
  create: (input: TimeLogInput, projectId: number) => Promise<void>;
  update: (id: number, input: TimeLogUpdateInput, taskId: number, projectId: number) => Promise<void>;
  softDelete: (id: number, taskId: number, projectId: number) => Promise<void>;
  reset: () => void;
}

export const useTimelogsStore = create<S>((set, get) => ({
  byTask: {},
  async loadFor(taskId) {
    const list = await call<TimeLog[]>("list_time_logs_by_task", { taskId });
    set({ byTask: { ...get().byTask, [taskId]: list } });
  },
  async create(input, projectId) {
    await call<TimeLog>("create_time_log", { input });
    await get().loadFor(input.task_id);
    await useFinancialStore.getState().refresh(projectId);
  },
  async update(id, input, taskId, projectId) {
    await call<TimeLog>("update_time_log", { id, input });
    await get().loadFor(taskId);
    await useFinancialStore.getState().refresh(projectId);
  },
  async softDelete(id, taskId, projectId) {
    await call<void>("delete_time_log", { id });
    await get().loadFor(taskId);
    await useFinancialStore.getState().refresh(projectId);
  },
  reset() {
    set({ byTask: {} });
  },
}));
```

注意：上面 `useTimelogsStore` 的 `S` 接口替换了原版（每个方法多一个 `projectId` 参数）。原 plan 顶部 Interfaces 节也需对应改：`create(input, projectId)`、`update(id, input, taskId, projectId)`、`softDelete(id, taskId, projectId)` — implementer 调用时记得传 projectId（detail.tsx 已经有 project.id）。

把这两个 store 加入 lock reset 链（修改 `src/stores/auth.ts`）。

- [ ] **Step 3：修改 `src/routes/projects/detail.tsx`** — 激活 tasks Tab

把 Tasks tab 的 disabled 移除：

```typescript
<TabsTrigger value="tasks">任务+工时</TabsTrigger>
```

在 `<TabsContent value="payments">` 之后加：

```typescript
<TabsContent value="tasks" className="mt-4">
  <TasksPanel projectId={project.id} companyId={project.company_id} />
</TabsContent>
```

在文件末尾追加 `TasksPanel`、`TaskForm`、`TimeLogsSection`、`TimeLogForm` 4 个组件。**注意：成员列表在「任务+工时」Tab 内需要，从 useMembersStore 拉取 active 成员；要在 TasksPanel 顶部加载**。

```typescript
function TasksPanel({ projectId, companyId }: { projectId: number; companyId: number }) {
  const { t } = useTranslation();
  const { byProject, statusFilter, loadFor, create, update, setStatus, softDelete } = useTasksStore();
  const tasks = byProject[projectId] ?? [];
  const { list: members, loadedForCompany: membersLoadedFor, loadFor: loadMembers } = useMembersStore();
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<Task | null>(null);
  const [openLogs, setOpenLogs] = useState<Task | null>(null);

  useEffect(() => { loadFor(projectId, null); }, [projectId, loadFor]);
  useEffect(() => {
    if (membersLoadedFor !== companyId) loadMembers(companyId);
  }, [companyId, membersLoadedFor, loadMembers]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
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

      {tasks.length === 0 ? (
        <Card><CardContent className="p-6 text-sm text-muted-foreground">{t("task.empty")}</CardContent></Card>
      ) : (
        <div className="grid gap-2">
          {tasks.map((tk) => {
            const assignee = members.find((m) => m.id === tk.assignee_id);
            return (
              <Card key={tk.id}>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 py-3">
                  <div className="space-y-1">
                    <CardTitle className="text-base flex items-center gap-2">
                      <span>{tk.title}</span>
                      <Badge variant="secondary">{t(`taskStatus.${tk.status}`)}</Badge>
                    </CardTitle>
                    <div className="text-xs text-muted-foreground">
                      {assignee?.name ?? t("task.unassigned")}
                      {tk.due_date && ` · 截止 ${tk.due_date}`}
                      {tk.estimated_hours != null && ` · 预估 ${tk.estimated_hours}h`}
                    </div>
                  </div>
                  <div className="flex gap-1">
                    <Select value={tk.status} onValueChange={async (v) => {
                      try { await setStatus(tk.id, v, projectId); }
                      catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                    }}>
                      <SelectTrigger className="w-32"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="todo">{t("taskStatus.todo")}</SelectItem>
                        <SelectItem value="in_progress">{t("taskStatus.in_progress")}</SelectItem>
                        <SelectItem value="done">{t("taskStatus.done")}</SelectItem>
                      </SelectContent>
                    </Select>
                    <Button size="sm" variant="ghost" onClick={() => setOpenLogs(tk)}>{t("timelog.title")}</Button>
                    <Button size="sm" variant="ghost" onClick={() => setEditing(tk)}>{t("task.edit")}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={async () => {
                        if (!confirm("确认删除该任务？关联工时将被一并软删。")) return;
                        try { await softDelete(tk.id, projectId); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >{t("task.delete")}</Button>
                  </div>
                </CardHeader>
              </Card>
            );
          })}
        </div>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("task.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <TaskForm
              members={members}
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, projectId); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!openLogs} onOpenChange={(o) => !o && setOpenLogs(null)}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{openLogs?.title ? `${openLogs.title} - ${t("timelog.title")}` : t("timelog.title")}</DialogTitle>
          </DialogHeader>
          {openLogs && <TimeLogsSection task={openLogs} members={members} />}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function TaskForm({ members, initial, onSubmit, onCancel }: {
  members: Member[];
  initial?: Task;
  onSubmit: (input: TaskInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [title, setTitle] = useState(initial?.title ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [assigneeId, setAssigneeId] = useState<string>(initial?.assignee_id ? String(initial.assignee_id) : "__none");
  const [status, setStatus] = useState(initial?.status ?? "todo");
  const [estHours, setEstHours] = useState(initial?.estimated_hours != null ? String(initial.estimated_hours) : "");
  const [dueDate, setDueDate] = useState(initial?.due_date ?? "");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!title.trim()) return toast.error(t("task.titleRequired"));
    setBusy(true);
    try {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || null,
        assignee_id: assigneeId === "__none" ? null : Number(assigneeId),
        status,
        estimated_hours: estHours === "" ? null : Number(estHours),
        due_date: dueDate || null,
      });
    } finally { setBusy(false); }
  };

  const active = members.filter((m) => m.is_active);

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("task.name")}</Label>
        <Input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus /></div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("task.assignee")}</Label>
          <Select value={assigneeId} onValueChange={setAssigneeId}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="__none">{t("task.unassigned")}</SelectItem>
              {active.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select></div>
        <div className="space-y-1"><Label>{t("task.status")}</Label>
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="todo">{t("taskStatus.todo")}</SelectItem>
              <SelectItem value="in_progress">{t("taskStatus.in_progress")}</SelectItem>
              <SelectItem value="done">{t("taskStatus.done")}</SelectItem>
            </SelectContent>
          </Select></div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("task.estimatedHours")}</Label>
          <Input type="number" min="0" step="0.5" value={estHours} onChange={(e) => setEstHours(e.target.value)} /></div>
        <div className="space-y-1"><Label>{t("task.dueDate")}</Label>
          <Input type="date" value={dueDate ?? ""} onChange={(e) => setDueDate(e.target.value)} /></div>
      </div>
      <div className="space-y-1"><Label>{t("task.description")}</Label>
        <Textarea value={description ?? ""} onChange={(e) => setDescription(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("task.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function TimeLogsSection({ task, members }: { task: Task; members: Member[] }) {
  const { t } = useTranslation();
  const { byTask, loadFor, create, update, softDelete } = useTimelogsStore();
  const logs = byTask[task.id] ?? [];
  const [openNew, setOpenNew] = useState(false);
  const [editing, setEditing] = useState<TimeLog | null>(null);

  useEffect(() => { loadFor(task.id); }, [task.id, loadFor]);

  const activeMembers = members.filter((m) => m.is_active);
  const findMemberName = (mid: number) =>
    members.find((m) => m.id === mid)?.name ?? `#${mid}`;

  return (
    <div className="space-y-3">
      <div className="text-xs text-muted-foreground">{t("timelog.snapshotHint")}</div>
      <div className="flex justify-end">
        <Dialog open={openNew} onOpenChange={setOpenNew}>
          <DialogTrigger asChild><Button size="sm">{t("timelog.add")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("timelog.add")}</DialogTitle></DialogHeader>
            <TimeLogForm
              taskId={task.id}
              members={activeMembers}
              onCancel={() => setOpenNew(false)}
              onSubmit={async (input) => {
                try { await create(input, task.project_id); setOpenNew(false); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {logs.length === 0 ? (
        <div className="p-6 text-sm text-muted-foreground text-center">{t("timelog.empty")}</div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-28">{t("timelog.workDate")}</TableHead>
              <TableHead className="w-28">{t("timelog.member")}</TableHead>
              <TableHead className="text-right w-20">{t("timelog.hours")}</TableHead>
              <TableHead className="text-right w-28">人力成本</TableHead>
              <TableHead>{t("timelog.notes")}</TableHead>
              <TableHead className="w-28 text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {logs.map((l) => {
              const cost = Math.round((l.hours / 8) * l.daily_cost_snapshot_cents);
              return (
                <TableRow key={l.id}>
                  <TableCell>{l.work_date}</TableCell>
                  <TableCell>{findMemberName(l.member_id)}</TableCell>
                  <TableCell className="text-right">{l.hours}</TableCell>
                  <TableCell className="text-right">{formatCNY(cost)}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{l.notes ?? ""}</TableCell>
                  <TableCell className="text-right">
                    <Button size="sm" variant="ghost" onClick={() => setEditing(l)}>{t("timelog.edit")}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={async () => {
                        if (!confirm(t("timelog.deleteConfirm"))) return;
                        try { await softDelete(l.id, task.id, task.project_id); }
                        catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
                      }}
                    >{t("timelog.delete")}</Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>{t("timelog.edit")}</DialogTitle></DialogHeader>
          {editing && (
            <TimeLogEditForm
              initial={editing}
              onCancel={() => setEditing(null)}
              onSubmit={async (input) => {
                try { await update(editing.id, input, task.id, task.project_id); setEditing(null); }
                catch (e: unknown) { toast.error(t("common.error", { msg: String(e) })); }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function TimeLogForm({ taskId, members, onSubmit, onCancel }: {
  taskId: number;
  members: Member[];
  onSubmit: (input: TimeLogInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [memberId, setMemberId] = useState(members[0]?.id ?? 0);
  const [date, setDate] = useState(todayIso());
  const [hours, setHours] = useState(8);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!memberId) return toast.error(t("timelog.memberRequired"));
    if (!date) return toast.error(t("timelog.dateRequired"));
    if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
    setBusy(true);
    try {
      await onSubmit({
        task_id: taskId,
        member_id: memberId,
        work_date: date,
        hours,
        notes: notes.trim() || null,
      });
    } finally { setBusy(false); }
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1"><Label>{t("timelog.member")}</Label>
          <Select value={String(memberId)} onValueChange={(v) => setMemberId(Number(v))}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {members.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select></div>
        <div className="space-y-1"><Label>{t("timelog.workDate")}</Label>
          <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
      </div>
      <div className="space-y-1"><Label>{t("timelog.hours")}</Label>
        <HoursInput value={hours} onChange={setHours} /></div>
      <div className="space-y-1"><Label>{t("timelog.notes")}</Label>
        <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button onClick={submit} disabled={busy}>{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}

function TimeLogEditForm({ initial, onSubmit, onCancel }: {
  initial: TimeLog;
  onSubmit: (input: TimeLogUpdateInput) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [date, setDate] = useState(initial.work_date);
  const [hours, setHours] = useState(initial.hours);
  const [notes, setNotes] = useState(initial.notes ?? "");
  const [busy, setBusy] = useState(false);

  return (
    <div className="space-y-3">
      <div className="space-y-1"><Label>{t("timelog.workDate")}</Label>
        <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
      <div className="space-y-1"><Label>{t("timelog.hours")}</Label>
        <HoursInput value={hours} onChange={setHours} /></div>
      <div className="space-y-1"><Label>{t("timelog.notes")}</Label>
        <Textarea value={notes ?? ""} onChange={(e) => setNotes(e.target.value)} /></div>
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>{t("common.cancel")}</Button>
        <Button
          disabled={busy}
          onClick={async () => {
            if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
            setBusy(true);
            try { await onSubmit({ work_date: date, hours, notes: notes.trim() || null }); }
            finally { setBusy(false); }
          }}
        >{t("timelog.save")}</Button>
      </DialogFooter>
    </div>
  );
}
```

并在 imports 区追加：

```typescript
import { useTasksStore } from "@/stores/tasks";
import { useTimelogsStore } from "@/stores/timelogs";
import { useMembersStore } from "@/stores/members";
import { HoursInput } from "@/components/forms/HoursInput";
import { todayIso } from "@/lib/time";
import type { Member, Task, TaskInput, TimeLog, TimeLogInput, TimeLogUpdateInput } from "@/types";
```

- [ ] **Step 4：TS + build**

预期：0 errors。

- [ ] **Step 5：Commit + CHANGELOG**

```bash
git commit -m "feat(tasks+timelogs): 任务+工时 tab + 工时弹窗 + 写时快照"
```

`/changelog`：项目详情新增「任务+工时」Tab；任务列表含状态切换、负责人、截止日期；点任务进入工时弹窗可录入/编辑/删除工时；工时显示当条人力成本（hours/8 × snapshot）。

---

## Task 13: 概览 Tab 升级为 FinancialPanel + 跨 Tab 自动刷新 + I-2 race 修复

**Files:**
- Modify: `src/routes/projects/detail.tsx`（替换 `OverviewPanel` 为 `FinancialPanel`，从 `useFinancialStore` 读；M2 `CostsPanel` 改为 mutation 后触发 financial refresh；修复 M2 final review I-2 公司切换 race）
- Modify: `src/stores/costs.ts`（M2 store —— mutation 后调 `useFinancialStore.refresh(projectId)`）

**Interfaces:**
- Produces: 概览 Tab 渲染 `ProjectFinancialSummary`（含 11 项财务指标），通过 `useFinancialStore` 接收 cost / payment / timelog 任一变动自动联动
- Consumes: T8 新增的 `get_project_financial_summary` 命令；T9 新增的 `useFinancialStore`；T9 新增的 `formatCNY` / i18n.financial.*

- [ ] **Step 1：在 detail.tsx 顶部加 imports**

```typescript
import { useFinancialStore } from "@/stores/financial";
import type { ProjectFinancialSummary } from "@/types";
```

- [ ] **Step 2：在 `ProjectDetailPage` 内订阅 financial store + 首次拉取**

在已有 useEffect 之后追加：

```typescript
const financial = useFinancialStore((s) => s.byProject[pid] ?? null);
const refreshFinancial = useFinancialStore((s) => s.refresh);

useEffect(() => {
  if (!Number.isNaN(pid)) refreshFinancial(pid);
}, [pid, refreshFinancial]);
```

- [ ] **Step 3：替换 `OverviewPanel` 为 `FinancialPanel`**

把原 `<OverviewPanel project={project} />` 改为：

```typescript
<FinancialPanel project={project} financial={financial} />
```

把 `function OverviewPanel` 整体替换为：

```typescript
function FinancialPanel({
  project,
  financial,
}: {
  project: Project;
  financial: ProjectFinancialSummary | null;
}) {
  const { t } = useTranslation();
  const formatRate = (r: number) => `${(r * 100).toFixed(2)}%`;
  return (
    <div className="space-y-4">
      {/* basic project info */}
      <div className="grid grid-cols-2 gap-3">
        <Card>
          <CardHeader><CardTitle className="text-sm">客户</CardTitle></CardHeader>
          <CardContent>{project.client_name ?? "—"}</CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">起止日期</CardTitle></CardHeader>
          <CardContent>
            {project.start_date ?? "—"} ~ {project.end_date ?? "—"}
          </CardContent>
        </Card>
      </div>

      {/* revenue / tax */}
      <div className="grid grid-cols-3 gap-3">
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.revenueInclusive")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.revenue_tax_inclusive_cents) : "—"}
            <div className="text-xs text-muted-foreground mt-1">
              税率 {(project.tax_rate * 100).toFixed(2)}% · {project.contract_amount_is_tax_inclusive ? "含税合同" : "不含税合同"}
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.revenueExclusive")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.revenue_tax_exclusive_cents) : "—"}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.tax")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.tax_amount_cents) : "—"}
          </CardContent>
        </Card>
      </div>

      {/* costs */}
      <div className="grid grid-cols-3 gap-3">
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.generalCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.general_cost_cents) : "—"}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.laborCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.labor_cost_cents) : "—"}
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.totalCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.total_cost_cents) : "—"}
          </CardContent>
        </Card>
      </div>

      {/* profit & collection */}
      <div className="grid grid-cols-2 gap-3">
        <Card className="border-primary">
          <CardHeader><CardTitle className="text-sm">{t("financial.grossProfit")}</CardTitle></CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {financial ? formatCNY(financial.gross_profit_cents) : "—"}
            </div>
            <div className="text-sm text-muted-foreground mt-1">
              {t("financial.profitRate")}：{financial ? formatRate(financial.profit_rate) : "—"}
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.collectionRate")}</CardTitle></CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {financial ? formatRate(financial.collection_rate) : "—"}
            </div>
            <div className="text-sm text-muted-foreground mt-1">
              {t("financial.actualPayment")}：{financial ? formatCNY(financial.actual_payment_cents) : "—"} /
              {" "}
              {t("financial.expectedPayment")}：{financial ? formatCNY(financial.expected_payment_cents) : "—"}
            </div>
          </CardContent>
        </Card>
      </div>

      {project.notes && (
        <Card>
          <CardHeader><CardTitle className="text-sm">备注</CardTitle></CardHeader>
          <CardContent className="whitespace-pre-wrap text-sm">{project.notes}</CardContent>
        </Card>
      )}
    </div>
  );
}
```

- [ ] **Step 4：修复 M2 final review I-2 — 公司切换时跳回项目列表**

在 `ProjectDetailPage` 加 effect：

```typescript
useEffect(() => {
  if (project && currentCompanyId != null && project.company_id !== currentCompanyId) {
    navigate("/projects", { replace: true });
  }
}, [project, currentCompanyId, navigate]);
```

放在 categories load effect 之后。

- [ ] **Step 4b：修改 M2 `src/stores/costs.ts` 让 mutation 触发 financial refresh**

在文件顶部加：

```typescript
import { useFinancialStore } from "./financial";
```

把 `create / update / remove` 三个方法尾部都加一行 `await useFinancialStore.getState().refresh(projectId);`。

最终形如：

```typescript
async create(projectId, input) {
  await call<CostEntry>("create_cost_entry", { projectId, input });
  await get().loadFor(projectId);
  await useFinancialStore.getState().refresh(projectId);
},
async update(id, input, projectId) {
  await call<CostEntry>("update_cost_entry", { id, input });
  await get().loadFor(projectId);
  await useFinancialStore.getState().refresh(projectId);
},
async remove(id, projectId) {
  await call<void>("delete_cost_entry", { id });
  await get().loadFor(projectId);
  await useFinancialStore.getState().refresh(projectId);
},
```

- [ ] **Step 5：TS + build**

预期：0 errors。

- [ ] **Step 6：Commit + CHANGELOG**

```bash
git commit -m "feat(projects): 详情概览升级为完整财务面板 + 跨 tab 自动刷新 + 公司切换防错"
```

`/changelog`：项目详情概览 Tab 改造成 FinancialPanel：含收入/税额/一般成本/人力成本/毛利润/利润率/回款率 7 大指标 + 客户/起止日期/备注；通过 `useFinancialStore` 在成本/收款/工时任一变动后自动刷新数字；切换当前公司若与正打开项目不一致则自动跳回 `/projects`（M2 final review I-2 修复）。

---

## Task 14: 回收站 UI 扩展 + 验收清单 + 全链路 closeout

**Files:**
- Modify: `src/routes/trash.tsx`（type label map 加 task / contract_payment / time_log）
- Create: `.superpowers/sdd/m3-acceptance.md`
- Modify: `CHANGELOG.md`（M3 milestone 总结）

**Interfaces:**
- Produces: 回收站显示 5 类实体类型；M3 验收清单覆盖全流程
- Consumes: 无新增

- [ ] **Step 1：修改 `src/routes/trash.tsx`**

把 `TYPE_LABEL` 常量改为：

```typescript
const TYPE_LABEL: Record<string, string> = {
  project: "项目",
  cost_entry: "成本",
  task: "任务",
  contract_payment: "收款",
  time_log: "工时",
};
```

无其他改动 — 其余逻辑（Restore / Purge / 表格渲染）都已适配通用 entity_type/id。

- [ ] **Step 2：跑 5 信号验证**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm tsc --noEmit
pnpm build
cd src-tauri && cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
cd ..
```
全绿。如有 clippy/fmt 失败立即修。

- [ ] **Step 3：写 `.superpowers/sdd/m3-acceptance.md`**

文件全文：

```markdown
# M3 手动验收清单

前置：M2 已通过验收。M3 完工后跑 `pnpm tauri dev`，按以下逐项核对。建议先删 `~/Library/Application Support/solo-cost/data.db` 干净重启，第一次启动日志应看到三次 `applied migration` 信息。

## 0. 数据迁移
- [ ] 日志依次出现 `applied migration 0001_init` / `0002_projects_costs` / `0003_people_contracts`
- [ ] 无 schema 错误

## 1. 成员管理
- [ ] `/members` 路由可访问，sidebar 显示"成员"项
- [ ] 新建成员"张三"，角色"前端"，日成本 ¥800，效力日 今天
- [ ] 列表显示 Badge "在职" + 日成本 ¥800.00/天 + 角色 + 生效日
- [ ] 点"归档" → Badge 变"已归档" 且行整体半透明；再点"取消归档"恢复
- [ ] 删除一个没工时的成员 → 列表移除；没有错误
- [ ] 后面 §5 录入工时后再回来删该成员 → toast「该成员有 N 条工时记录，请先归档...」

## 2. 合同收款节点
- [ ] 进入一个项目详情，切到「收款」Tab
- [ ] 顶部 3 个统计 Card：预期合计 ¥0 / 实收合计 ¥0 / 回款率 0%
- [ ] 新建预付款 ¥5,000 → 列表出现 + 预期合计变 ¥5,000
- [ ] 新建尾款 ¥5,000 → 预期合计 ¥10,000
- [ ] 点预付款"标为已收" → 弹窗填 ¥4,800 + 今天日期 → 确认 → 实收合计 ¥4,800 / 回款率 48%
- [ ] 编辑尾款金额 ¥6,000 → 预期合计变 ¥11,000
- [ ] 删除尾款 → 预期合计回到 ¥5,000 / 实收 ¥4,800 / 回款率 96%

## 3. 任务
- [ ] 切「任务+工时」Tab
- [ ] 状态筛选默认"全部状态"，列表空 → 空状态 Card
- [ ] 新建任务"做需求评审"，负责人 张三，预估 8h，截止 下周一 → 列表出现 + 状态 Badge"待办"
- [ ] 在卡片右上 Select 切到"进行中" → Badge 变化
- [ ] 状态筛选切到"已完成" → 列表空；切回"全部状态"恢复

## 4. 工时录入（写时快照）
- [ ] 点任务右上"工时"按钮 → 弹窗顶部提示「录入时锁定该成员的日成本快照」
- [ ] 录入：成员 张三，日期 今天，8 小时，备注"需求会"
- [ ] 表格出现一行，人力成本列显示 ¥800.00（= 8/8 × ¥800）
- [ ] 再录入 4 小时 → 人力成本 ¥400.00
- [ ] **关键：去 `/members` 把张三日成本改 ¥1,600 → 回到任务工时弹窗 → 两条历史工时人力成本仍是 ¥800/¥400**（快照不变）
- [ ] 再录入 8 小时 → 新工时人力成本 ¥1,600.00（用新日成本）
- [ ] 编辑历史工时改 hours = 6 → 表格人力成本变 ¥600.00（仍用原快照）
- [ ] 删除一条工时 → 列表少一行

## 5. 删除阻断
- [ ] 回到 `/members` 试图删除张三 → toast「该成员有 N 条工时记录，请先归档（设 is_active=0）」
- [ ] 删完工时再删张三 → 成功

## 6. 利润链（概览 Tab）
- [ ] 编辑项目，合同 ¥10,000、含税、税率 6%
- [ ] 切「概览」Tab：
  - [ ] 含税收入 ¥10,000.00
  - [ ] 不含税收入 ¥9,433.96
  - [ ] 税额 ¥566.04
  - [ ] 一般成本 ¥0（前面没录成本入金）
  - [ ] 人力成本 = §4 总和
  - [ ] 总成本 = 一般 + 人力
  - [ ] 毛利润 + 利润率（绿色 Card 高亮）
  - [ ] 回款率 = §2 实收 / 预期
- [ ] 切到「成本」Tab 录入一笔差旅 ¥500 → 回「概览」一般成本变 ¥500，总成本 +500，毛利润 -500

## 7. 级联软删 + 回收站
- [ ] 在 `/projects` 软删整个项目 → 项目消失
- [ ] 进 `/trash`：看到 5 类条目：
  - 项目
  - 成本（M2 已录的）
  - 任务
  - 收款（2 条）
  - 工时（§4 录的几条）
- [ ] 点项目「恢复」→ 回收站清空，项目和所有子表全部恢复
- [ ] 单独软删一个任务 → 回收站只显示该任务 + 它的工时；恢复后两者一同回来
- [ ] 软删一个任务后单独尝试恢复工时 → toast「任务已删除，请先恢复任务」

## 8. 多公司隔离
- [ ] 在公司 A 建项目+成员+任务+工时，切回公司 B → 都为空
- [ ] 工时录入时成员下拉只显示当前公司的 active 成员

## 9. 锁定/解锁后状态重置（M2 final review I-1）
- [ ] 在项目详情某 Tab 上点 Header"锁定"
- [ ] 解锁后再进同一项目 → 数据从 DB 重新拉（验证 store reset 生效，无残留缓存）

## 10. 公司切换防错（M2 final review I-2）
- [ ] 在公司 A 的某项目详情，header dropdown 切到公司 B → 应自动跳到 `/projects`

## 11. 回归
- [ ] M1/M2 功能（公司、科目、回收站、cost CRUD）全部仍可用
- [ ] sidebar 各项点击不报错
```

- [ ] **Step 4：Commit 验收清单（强制加文件以绕过 .superpowers/sdd/* gitignore）**

```bash
git add -f .superpowers/sdd/m3-acceptance.md
git commit -m "docs(m3): 验收清单 + 标记里程碑完工"
```

- [ ] **Step 5：M3 里程碑 CHANGELOG**

`/changelog` 写一条总结条目，例如：

> M3 完工：成员管理（含归档/有工时则拒删）+ 合同收款节点（预期/实收/回款率）+ 任务（todo/in_progress/done）+ 工时录入（写时锁定日成本快照）+ 完整财务面板（含税/不含税收入、税额、一般+人力成本、毛利润、利润率、回款率）。同时修复 M2 final review I-1（lock 时所有 store reset）与 I-2（切换公司时跳回项目列表）。

- [ ] **Step 6：Commit 修改的 trash.tsx + CHANGELOG**

```bash
git add src/routes/trash.tsx CHANGELOG.md
git commit -m "feat(trash): ui 显示 5 类已删实体类型 + m3 里程碑总结"
```

---

## Self-Review 结论（plan 提交前自检）

按 writing-plans skill 要求，对照 M3 范围与 spec 自检：

### 1. 覆盖范围

| Spec 项 | 任务 |
|--------|------|
| §3.2 members 表 | T1 schema + T4 CRUD + T10 UI |
| §3.2 contract_payments 表 | T1 schema + T5 CRUD + T11 UI |
| §3.2 tasks 表 | T1 schema + T6 CRUD + T12 UI |
| §3.2 time_logs 表（snapshot） | T1 schema + T7 CRUD（写时快照）+ T12 UI |
| §3.3 含税/不含税公式 | T3 ProjectFinancialSummary + T13 FinancialPanel |
| §3.3 人力成本聚合 + snapshot | T3 + T7 + T12 |
| §3.3 利润率 / 回款率 | T3 + T13 |
| §3.4 级联软删（项目→4 类子）| T2 |
| §3.4 任务→工时级联 | T2 |
| §3.4 成员有工时拒删 | T2 + T4 |
| §4.1 `/members` 路由 | T10 |
| §4.1 项目详情收款/任务+工时 Tab | T11 + T12 |
| §3.4 回收站整组恢复 | T2 + T8 |
| M2 final review I-1 store reset | T9 + T10 (members store) + T11/T12 (新 stores) |
| M2 final review I-2 切公司跳走 | T13 |
| M2 final review M-1 update_cost_entry 防御 | T8 |

### 2. 占位符扫描

- 无 "TBD/TODO/implement later"
- 每个 step 给了完整代码或确定动作
- T9 Step 6 「在 stores 加 reset action」给了具体改动模式，但因每个 store 形态略有不同，没逐字逐字给完整文件 — implementer 看现有 store 文件按 pattern 应用即可（implementer 已有先例：T7/T8 已类似处理）

### 3. 类型一致性

- Rust `Member.is_active: bool` ↔ TS `boolean`；DB INTEGER 0/1，Rust 读 `!=0`
- Rust `Task.estimated_hours: Option<f64>` ↔ TS `number | null`
- Rust `TimeLog.hours: f64` ↔ TS `number`；`daily_cost_snapshot_cents: i64` ↔ `number`
- Rust `ContractPayment.actual_amount_cents: Option<i64>` ↔ TS `number | null`
- 6 个新 store 方法签名前后端对齐（参数名 camelCase 转换）

### 4. 范围控制

- 未引入 M4 附件 / 备份 / 导出 / 任务总览 `/tasks` 跨项目页（spec 标注 v0.2 报表）
- 未做项目状态迁移合法性校验（M2 接受 M4 收口）
- 不增加新 npm 依赖（除 shadcn checkbox 一个）
- domain 层依然薄，没引 ORM

### 5. 风险点

- **T9 Step 7 + T10/T11/T12 store reset 链交错**：T9 实现 lock reset 时不知道后面新 store；T10/T11/T12 每个都要回到 auth.ts 追加一行 import + reset 调用。这是节奏问题，每个 implementer 应记得修。已在每个相关 task 显式提示。
- **T13 financial 跨 Tab 刷新（已采纳 zustand store 方案）**：T9 创建 `useFinancialStore`，T11 / T12 / T13（包括 M2 costs store 改造）在 mutation 成功后 `useFinancialStore.refresh(projectId)`。FinancialPanel 通过 `useFinancialStore((s) => s.byProject[pid])` 订阅，cost/payment/timelog 任一改动后概览 Tab 自动同步。无 prop drilling，无残留缓存。
- **T2 测试 sleep 复用**：M2 T2 已确认 sleep 模式可接受，M3 在同模块继续使用 datetime('now')，新测试不引入 sleep（因为没有依赖时间戳互不相同的 assertion）。

---

## Demoable End-State

完成 M3 全部 14 个 task 后，应能：

- 在公司 A 建项目「项目甲」合同 ¥10,000 含税 6%
- 在 `/members` 建张三（¥800/天）、李四（¥1,200/天）
- 在项目详情「任务+工时」Tab 建任务"做需求评审"指派张三
- 录工时：张三 8h，自动锁定 ¥800/天 snapshot
- 改张三日成本到 ¥1,600 → 历史人力成本不变
- 录新工时 8h → 用新 ¥1,600 snapshot
- 在「收款」Tab 建 ¥5,000 + ¥5,000 两节点，标第一笔实收 ¥4,800
- 在「概览」Tab 看到不含税收入 ¥9,433.96、税额 ¥566.04、人力成本（按 snapshot 累加）、毛利润、利润率、回款率 48%
- 软删整个项目 → 回收站显示 5 类条目；恢复后全部回来
- 切回公司 B → 项目/成员/任务/工时 列表空（隔离）
- 锁定再解锁 → 所有 store 重新拉

---
