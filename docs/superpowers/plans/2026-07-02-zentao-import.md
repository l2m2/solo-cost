# 禅道 CSV 导入 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户在项目详情「任务+工时」tab 顶部点「从禅道 CSV 导入」按钮 → 选文件 → 走 4 步 wizard（成员映射、模块映射、确认、报告）→ 幂等地把禅道任务 + 工时导入到当前项目。

**Architecture:** 后端一张迁移 0006 给 `tasks` 加 `external_ref TEXT` + 唯一部分索引；新 `commands/zentao_import.rs` 用 `encoding_rs` 探测 UTF-8/GBK、`csv` 解析，两条 IPC `preview_zentao_csv` / `execute_zentao_import`；execute 每行独立事务、任务与 timelog 同 tx，出错记入 failed 继续下一行。前端 5 步 wizard 用一个新组件 `ZentaoImportDialog`；成员与模块映射用 shadcn `<Table compact>` + `<Select>`；不持久化映射；结束展示一次性总结 Dialog。

**Tech Stack:** Rust (rusqlite / tauri v2 command / csv / encoding_rs) + React 19 + TypeScript + Vite + Tailwind + shadcn/radix + zustand + i18next + `@tauri-apps/plugin-dialog`（已装）。

## Global Constraints

- 迁移编号沿用现有序列：新增 `0006_tasks_external_ref.sql`（当前 `MIGRATIONS` 到 `0005_modules`，`current_version` 断言 5；bump 到 6）。
- `tasks.external_ref TEXT NULL`；唯一部分索引 `(project_id, external_ref) WHERE external_ref IS NOT NULL AND deleted_at IS NULL`。
- 编码探测：UTF-8 优先，失败尝试 GBK；都失败 → `AppError::Validation("不支持的编码，请另存为 UTF-8")`。
- CSV 必要列：`编号 / 任务名称 / 任务状态`；缺任一 → `AppError::Validation("CSV 缺少必要列: <名>")`。可选列缺失 → 静默按空处理。
- 状态映射（见 spec §5）：已关闭+关闭原因=已完成 → done；已完成 → done；进行中/已激活 → in_progress；已暂停/未开始 → todo；已取消 / 已关闭+关闭原因≠已完成 → 整行跳过（`status=None`）；其他未识别 → todo（防御性 fallback）。
- 名字取哪一列：优先 `由谁完成`；空则用 `指派给`（`"Closed"` 视为空）；再空用 `由谁创建`。
- 模块解析：从 `所属模块` 剥去 `(#\d+)` 后取路径末段（叶子名）；等于根 `/(#0)` / `/` 或空 → `None`。
- work_date：`实际开始[0..10]` → 空则 `实际完成`（若已是 `YYYY-MM-DD`）→ 再空 `创建日期[0..10]`。
- 幂等：命中 `external_ref` 已在库 → 整行跳过（不导任务、不导 timelog）。
- 每行独立事务；任务和 timelog 同 tx；任一失败 → 整行 rollback + 记入 `failed`，下一行继续。
- `failed` 列表报告限制显示前 100 条。
- 不持久化成员映射 / 模块映射（每次导入重新填）。
- Wizard 结束触发前端 `useTasksStore.loadFor` + `useModulesStore.loadFor` + `useModuleStatsStore.refresh` + `useFinancialStore.refresh`。
- Commit 规范：Conventional Commits + 中文 subject，≤ 72 字符；body 说明为什么这么改。

---

## File Structure

**新增**：
- `src-tauri/migrations/0006_tasks_external_ref.sql`
- `src-tauri/src/commands/zentao_import.rs`
- `src/components/zentao-import/ZentaoImportDialog.tsx`
- `docs/superpowers/plans/2026-07-02-zentao-import.md`（本文件）

**修改**：
- `src-tauri/Cargo.toml` — 加 `csv` 与 `encoding_rs` 依赖
- `src-tauri/src/commands/tasks.rs` — `Task/TaskInput` 扩 `external_ref`；`row_to_task`；`create_impl` INSERT 带上；1 持久化测试
- `src-tauri/src/commands/mod.rs` — `pub mod zentao_import;`
- `src-tauri/src/lib.rs` — 注入 2 个新 IPC handler
- `src-tauri/src/db/migrations.rs` — MIGRATIONS 追加 + version 5→6
- `src/types/index.ts` — `Task` 扩 `external_ref`；`TaskInput` 扩 `external_ref`；4 个新接口 + 2 个 union type
- `src/i18n/zh-CN.json` — `zentaoImport.*` 一整块 + `common.{next,back,done}` 若缺
- `src/routes/projects/detail.tsx` — `TasksPanel` 顶部加「从禅道 CSV 导入」按钮 + 挂 `ZentaoImportDialog`
- `CHANGELOG.md` — Unreleased/Added 段追加

**不改**：
- `src/stores/tasks.ts` 结构（透传 `TaskInput` 已足够，`external_ref` 从 UI 侧永远传空）
- `src/stores/modules.ts` / `moduleStats.ts` / `financial.ts` — 只在 wizard 完成回调里 refresh

---

## Task 1: Backend — 迁移 0006 + `tasks.external_ref`

**Files:**
- Create: `src-tauri/migrations/0006_tasks_external_ref.sql`
- Modify: `src-tauri/src/commands/tasks.rs`
- Modify: `src-tauri/src/db/migrations.rs`

**Interfaces:**
- Consumes: 现有 `tasks` 表 + `Task/TaskInput` 结构（Task 2 → Modules-T1..T2 的 module_id 已在）。
- Produces（Task 2/3 消费）:
  - `tasks.external_ref TEXT NULL` 列 + 唯一部分索引
  - `Task` 字段 `external_ref: Option<String>`
  - `TaskInput` 字段 `external_ref: Option<String>`

- [ ] **Step 1.1: 写迁移文件**

Create `src-tauri/migrations/0006_tasks_external_ref.sql`：

```sql
-- Tasks external ref for zentao / future CSV imports
ALTER TABLE tasks ADD COLUMN external_ref TEXT;
CREATE UNIQUE INDEX idx_tasks_external_ref
    ON tasks(project_id, external_ref)
    WHERE external_ref IS NOT NULL AND deleted_at IS NULL;
```

（不写 `BEGIN;/COMMIT;` —— `db/migrations.rs::run` 已用 `unchecked_transaction()` 包起来。）

- [ ] **Step 1.2: 注册迁移**

编辑 `src-tauri/src/db/migrations.rs`：

- 在 `MIGRATIONS` 数组 `("0005_modules", ...)` 之后追加：

  ```rust
      (
          "0006_tasks_external_ref",
          include_str!("../../migrations/0006_tasks_external_ref.sql"),
      ),
  ```

- 将 `#[test] fn fresh_db_runs_all_migrations` 里的 `assert_eq!(v, 5);` 改为 `assert_eq!(v, 6);`
- 将 `#[test] fn run_is_idempotent` 里的 `assert_eq!(current_version(&conn).unwrap(), 5);` 改为 `assert_eq!(current_version(&conn).unwrap(), 6);`

- [ ] **Step 1.3: 扩 `Task` / `TaskInput`**

编辑 `src-tauri/src/commands/tasks.rs`：

- `pub struct Task` 里，在 `pub module_id: Option<i64>,` 之后追加：

  ```rust
      pub external_ref: Option<String>,
  ```

- `pub struct TaskInput` 里，在 `pub module_id: Option<i64>,` 之后追加：

  ```rust
      pub external_ref: Option<String>,
  ```

- [ ] **Step 1.4: `row_to_task` 加字段**

在 `fn row_to_task` 中，`module_id: row.get("module_id")?,` 之后追加：

```rust
        external_ref: row.get("external_ref")?,
```

- [ ] **Step 1.5: `create_impl` INSERT 带上 external_ref**

把 `create_impl` 的 INSERT 语句与参数改成（新增 `external_ref` 列作 `?9`）：

```rust
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, assignee_id,
                           status, estimated_hours, due_date, module_id, external_ref)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7, ?8, ?9)",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.module_id,
            input.external_ref.as_deref(),
        ],
    )?;
```

- [ ] **Step 1.6: 同步测试辅助 `input()`**

在 `fn input(...)` 里，`module_id: None,` 之后追加：

```rust
        external_ref: None,
```

- [ ] **Step 1.7: 加持久化测试**

在 `#[cfg(test)] mod tests { ... }` 末尾追加：

```rust
    #[test]
    fn create_task_persists_external_ref() {
        let db = TestDb::new();
        let mut i = input("T");
        i.external_ref = Some("zentao:368".into());
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(t.external_ref.as_deref(), Some("zentao:368"));
    }

    #[test]
    fn external_ref_unique_index_rejects_duplicate_in_same_project() {
        let db = TestDb::new();
        let mut i = input("T1");
        i.external_ref = Some("zentao:368".into());
        create_impl(&db.conn, 1, &i).unwrap();
        // second insert into same project with same external_ref → SQLite UNIQUE violation
        let mut j = input("T2");
        j.external_ref = Some("zentao:368".into());
        let err = create_impl(&db.conn, 1, &j).unwrap_err();
        assert!(matches!(err, AppError::Db(_)));
    }

    #[test]
    fn external_ref_unique_index_allows_same_id_across_projects() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P2')", []).unwrap();
        let mut i = input("T1");
        i.external_ref = Some("zentao:368".into());
        create_impl(&db.conn, 1, &i).unwrap();
        let mut j = input("T2");
        j.external_ref = Some("zentao:368".into());
        // project 2 can hold the same external_ref
        create_impl(&db.conn, 2, &j).unwrap();
    }
```

- [ ] **Step 1.8: 跑测试**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test commands::tasks 2>&1 | tail -15
cargo test db::migrations 2>&1 | tail -10
cargo test 2>&1 | grep -E "test result:" | head
```

Expected：全部 PASS；db::migrations 断言 version = 6。

- [ ] **Step 1.9: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/migrations/0006_tasks_external_ref.sql \
        src-tauri/src/commands/tasks.rs \
        src-tauri/src/db/migrations.rs
git commit -m "$(cat <<'EOF'
feat(tasks): 加 external_ref 字段与唯一部分索引

迁移 0006：tasks 增 external_ref TEXT + partial UNIQUE index
(project_id, external_ref) WHERE external_ref IS NOT NULL AND
deleted_at IS NULL。Task/TaskInput 扩同名字段；create_impl
INSERT 带上；3 个测试覆盖持久化、同项目重复拒绝、跨项目允许
同名（含 external_ref='zentao:368' 场景）。
EOF
)"
```

---

## Task 2: Rust deps + `zentao_import.rs` 解析器（含 16 解析测试）

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/commands/zentao_import.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Consumes: `tasks.external_ref` 列（Task 1）、`AppError::{Validation, Db}`、`AppResult`。
- Produces（Task 3 消费）:
  - `pub struct ImportPreview`、`ImportReport`、`PreSkipStats`、`SkipCounts`、`FailedRow`（后续 IPC 返回）
  - `pub enum MemberChoice { UseMember { member_id: i64 }, Unassigned, SkipRow }`
  - `pub enum ModuleChoice { UseModule { module_id: i64 }, CreateWithName { name: String }, Unassigned }`
  - Internal `struct ParsedRow` + `fn parse_all(bytes: &[u8]) -> AppResult<Vec<ParsedRow>>`
  - Public helpers `fn detect_and_decode(bytes: &[u8]) -> Option<String>`（Task 3 preview/execute 共用）

- [ ] **Step 2.1: 加 crate 依赖**

编辑 `src-tauri/Cargo.toml`——在 `[dependencies]` 段追加：

```toml
csv = "1"
encoding_rs = "0.8"
```

- [ ] **Step 2.2: 注册 module**

编辑 `src-tauri/src/commands/mod.rs`——按字母序追加：

```rust
pub mod zentao_import;
```

（位置：`pub mod trash;` 之后。）

- [ ] **Step 2.3: 创建 `zentao_import.rs` 骨架（types + parser 空壳）**

Create `src-tauri/src/commands/zentao_import.rs`：

```rust
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── DTO structs shared by preview / execute ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub total_rows: u32,
    pub member_names: Vec<String>,
    pub module_names: Vec<String>,
    pub pre_skip: PreSkipStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreSkipStats {
    pub cancelled: u32,
    pub already_imported: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemberChoice {
    UseMember { member_id: i64 },
    Unassigned,
    SkipRow,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleChoice {
    UseModule { module_id: i64 },
    CreateWithName { name: String },
    Unassigned,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub imported_tasks: u32,
    pub imported_timelogs: u32,
    pub skipped: SkipCounts,
    pub failed: Vec<FailedRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SkipCounts {
    pub cancelled: u32,
    pub already_imported: u32,
    pub member_skipped: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedRow {
    pub row_no: u32,
    pub zentao_id: String,
    pub error: String,
}

// ─── Internal parser output ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct ParsedRow {
    pub row_no: u32,
    pub zentao_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub assignee_name: Option<String>,
    pub module_name: Option<String>,
    pub estimated_hours: Option<f64>,
    pub consumed_hours: f64,
    pub work_date: Option<String>,
    pub due_date: Option<String>,
}

// ─── Encoding detection ──────────────────────────────────────────────────

pub(crate) fn detect_and_decode(bytes: &[u8]) -> Option<String> {
    // Prefer strict UTF-8 (strip BOM if present)
    let stripped = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Ok(s) = std::str::from_utf8(stripped) {
        return Some(s.to_string());
    }
    // Fall back to GBK
    let (cow, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if had_errors {
        None
    } else {
        Some(cow.into_owned())
    }
}

// ─── Status mapping ──────────────────────────────────────────────────────

pub(crate) fn map_status(zentao_status: &str, close_reason: &str) -> Option<String> {
    match zentao_status.trim() {
        "已关闭" => {
            if close_reason.trim() == "已完成" {
                Some("done".into())
            } else {
                None // cancelled / duplicate / etc → skip whole row
            }
        }
        "已完成" => Some("done".into()),
        "进行中" | "已激活" => Some("in_progress".into()),
        "已暂停" | "未开始" => Some("todo".into()),
        "已取消" => None,
        _ => Some("todo".into()), // defensive fallback
    }
}

// ─── Assignee fallback ───────────────────────────────────────────────────

pub(crate) fn pick_assignee(completer: &str, assigned: &str, creator: &str) -> Option<String> {
    let completer = completer.trim();
    if !completer.is_empty() {
        return Some(completer.into());
    }
    let assigned = assigned.trim();
    if !assigned.is_empty() && assigned != "Closed" {
        return Some(assigned.into());
    }
    let creator = creator.trim();
    if !creator.is_empty() {
        return Some(creator.into());
    }
    None
}

// ─── Module leaf extraction ──────────────────────────────────────────────

pub(crate) fn extract_module_leaf(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Strip trailing "(#\d+)"
    let without_id: &str = match s.rfind('(') {
        Some(i) if s[i..].starts_with("(#") && s.ends_with(')') => &s[..i],
        _ => s,
    };
    let path = without_id.trim().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return None;
    }
    // Take last segment
    let leaf = path.rsplit('/').next().unwrap_or("").trim();
    if leaf.is_empty() {
        None
    } else {
        Some(leaf.to_string())
    }
}

// ─── Work date fallback ──────────────────────────────────────────────────

pub(crate) fn pick_work_date(actual_start: &str, actual_end: &str, created_at: &str) -> Option<String> {
    fn take_date_prefix(s: &str) -> Option<String> {
        let t = s.trim();
        if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
            Some(t[0..10].into())
        } else if t.is_empty() {
            None
        } else {
            None
        }
    }
    if let Some(d) = take_date_prefix(actual_start) {
        return Some(d);
    }
    if let Some(d) = take_date_prefix(actual_end) {
        return Some(d);
    }
    take_date_prefix(created_at)
}

// ─── Parser core ─────────────────────────────────────────────────────────

pub(crate) fn parse_all(bytes: &[u8]) -> AppResult<Vec<ParsedRow>> {
    let text = detect_and_decode(bytes)
        .ok_or_else(|| AppError::Validation("不支持的编码，请另存为 UTF-8".into()))?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr.headers()
        .map_err(|e| AppError::Validation(format!("CSV 头解析失败: {e}")))?
        .clone();

    let col_index: HashMap<&str, usize> = headers.iter().enumerate()
        .map(|(i, h)| (h.trim(), i))
        .collect();

    let required = ["编号", "任务名称", "任务状态"];
    for name in required {
        if !col_index.contains_key(name) {
            return Err(AppError::Validation(format!("CSV 缺少必要列: {name}")));
        }
    }

    fn get<'a>(rec: &'a csv::StringRecord, idx: Option<&usize>) -> &'a str {
        idx.and_then(|&i| rec.get(i)).unwrap_or("")
    }

    let mut out = Vec::new();
    for (row_no0, rec) in rdr.records().enumerate() {
        let row_no = (row_no0 as u32) + 1; // 1-indexed data row (after header)
        let rec = match rec {
            Ok(r) => r,
            Err(_) => continue, // silently skip malformed rows
        };
        if rec.iter().all(|f| f.trim().is_empty()) {
            continue; // silently skip blank rows
        }

        let zentao_num = get(&rec, col_index.get("编号")).trim();
        if zentao_num.is_empty() {
            continue; // silently skip rows without id (e.g. legend footer)
        }
        let title = get(&rec, col_index.get("任务名称")).trim().to_string();
        if title.is_empty() {
            continue; // silently skip rows without title
        }

        let status = map_status(
            get(&rec, col_index.get("任务状态")),
            get(&rec, col_index.get("关闭原因")),
        );

        let assignee_name = pick_assignee(
            get(&rec, col_index.get("由谁完成")),
            get(&rec, col_index.get("指派给")),
            get(&rec, col_index.get("由谁创建")),
        );

        let module_name = extract_module_leaf(get(&rec, col_index.get("所属模块")));

        fn strip_h_parse(s: &str) -> Option<f64> {
            let t = s.trim();
            let stripped = t.strip_suffix('h').unwrap_or(t).trim();
            if stripped.is_empty() { None } else { stripped.parse::<f64>().ok() }
        }

        let estimated_hours = strip_h_parse(get(&rec, col_index.get("最初预计")));
        let consumed_hours = strip_h_parse(get(&rec, col_index.get("总计消耗"))).unwrap_or(0.0);

        let work_date = pick_work_date(
            get(&rec, col_index.get("实际开始")),
            get(&rec, col_index.get("实际完成")),
            get(&rec, col_index.get("创建日期")),
        );

        let due_date = {
            let d = get(&rec, col_index.get("截止日期")).trim();
            if d.is_empty() { None } else { Some(d.to_string()) }
        };

        let description = {
            let d = get(&rec, col_index.get("任务描述")).trim();
            if d.is_empty() { None } else { Some(d.to_string()) }
        };

        out.push(ParsedRow {
            row_no,
            zentao_id: format!("zentao:{zentao_num}"),
            title,
            description,
            status,
            assignee_name,
            module_name,
            estimated_hours,
            consumed_hours,
            work_date,
            due_date,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Encoding tests ──────────────────────────────────────────

    #[test]
    fn detects_utf8() {
        let s = "编号,任务名称,任务状态\n1,做事,已完成";
        let out = detect_and_decode(s.as_bytes()).unwrap();
        assert!(out.contains("任务名称"));
    }

    #[test]
    fn detects_utf8_with_bom() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice("编号,任务名称,任务状态\n1,做事,已完成".as_bytes());
        let out = detect_and_decode(&bytes).unwrap();
        assert!(out.starts_with("编号"));
    }

    #[test]
    fn detects_gbk() {
        // Encode a small header line as GBK using encoding_rs (the same crate used at runtime)
        let (bytes, _, _) = encoding_rs::GBK.encode("编号,任务名称,任务状态\n1,做事,已完成");
        let out = detect_and_decode(&bytes).unwrap();
        assert!(out.contains("任务名称"));
    }

    #[test]
    fn rejects_missing_required_columns() {
        let s = "任务名称,任务状态\nA,已完成";
        let err = parse_all(s.as_bytes()).unwrap_err();
        assert!(matches!(err, AppError::Validation(msg) if msg.contains("编号")));
    }

    // ─── Status mapping tests ────────────────────────────────────

    #[test]
    fn parse_status_closed_done_maps_to_done() {
        assert_eq!(map_status("已关闭", "已完成"), Some("done".into()));
    }

    #[test]
    fn parse_status_done_maps_to_done() {
        assert_eq!(map_status("已完成", ""), Some("done".into()));
    }

    #[test]
    fn parse_status_in_progress_maps_to_in_progress() {
        assert_eq!(map_status("进行中", ""), Some("in_progress".into()));
        assert_eq!(map_status("已激活", ""), Some("in_progress".into()));
    }

    #[test]
    fn parse_status_paused_maps_to_todo() {
        assert_eq!(map_status("已暂停", ""), Some("todo".into()));
    }

    #[test]
    fn parse_status_wait_maps_to_todo() {
        assert_eq!(map_status("未开始", ""), Some("todo".into()));
    }

    #[test]
    fn parse_status_cancelled_yields_none() {
        assert_eq!(map_status("已取消", ""), None);
    }

    #[test]
    fn parse_status_closed_non_done_yields_none() {
        assert_eq!(map_status("已关闭", "已取消"), None);
    }

    #[test]
    fn parse_status_unknown_falls_back_to_todo() {
        assert_eq!(map_status("foo", ""), Some("todo".into()));
    }

    // ─── Assignee fallback tests ─────────────────────────────────

    #[test]
    fn parse_assignee_completer_first() {
        assert_eq!(pick_assignee("李黎明", "Closed", "他人"), Some("李黎明".into()));
    }

    #[test]
    fn parse_assignee_falls_back_to_assigned_when_completer_empty() {
        assert_eq!(pick_assignee("", "小王", "创建人"), Some("小王".into()));
    }

    #[test]
    fn parse_assignee_treats_closed_sentinel_as_empty() {
        assert_eq!(pick_assignee("", "Closed", "创建人"), Some("创建人".into()));
    }

    #[test]
    fn parse_assignee_returns_none_when_all_empty() {
        assert_eq!(pick_assignee("", "Closed", ""), None);
    }

    // ─── Module leaf extraction tests ────────────────────────────

    #[test]
    fn parse_module_leaf_from_nested_path() {
        assert_eq!(extract_module_leaf("/前端/表单(#8)"), Some("表单".into()));
    }

    #[test]
    fn parse_module_leaf_from_single_level() {
        assert_eq!(extract_module_leaf("/前端(#5)"), Some("前端".into()));
    }

    #[test]
    fn parse_module_root_yields_none() {
        assert_eq!(extract_module_leaf("/(#0)"), None);
        assert_eq!(extract_module_leaf("/"), None);
        assert_eq!(extract_module_leaf(""), None);
    }

    // ─── Work date fallback ──────────────────────────────────────

    #[test]
    fn parse_workdate_falls_back_from_start_to_end_to_created() {
        assert_eq!(pick_work_date("2026-06-28 08:44:00", "", ""), Some("2026-06-28".into()));
        assert_eq!(pick_work_date("", "2026-06-27", ""), Some("2026-06-27".into()));
        assert_eq!(pick_work_date("", "", "2026-06-01 10:00:00"), Some("2026-06-01".into()));
        assert_eq!(pick_work_date("", "", ""), None);
    }

    // ─── End-to-end parse_all smoke ──────────────────────────────

    #[test]
    fn parse_all_smoke_from_sample_csv_shape() {
        let s = "编号,所属项目,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                 368,a005-2(#25),现场实施 20260628,已关闭,已完成,8h,8h,2026-06-28 08:44:00,李黎明,Closed,李黎明,/(#0)\n\
                 367,a005-2(#25),重写串口通信,已关闭,已完成,4h,4h,2026-06-26 22:00:00,李黎明,Closed,李黎明,/(#0)\n";
        let rows = parse_all(s.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].zentao_id, "zentao:368");
        assert_eq!(rows[0].title, "现场实施 20260628");
        assert_eq!(rows[0].status.as_deref(), Some("done"));
        assert_eq!(rows[0].assignee_name.as_deref(), Some("李黎明"));
        assert_eq!(rows[0].module_name, None);
        assert!((rows[0].estimated_hours.unwrap() - 8.0).abs() < 1e-9);
        assert!((rows[0].consumed_hours - 8.0).abs() < 1e-9);
        assert_eq!(rows[0].work_date.as_deref(), Some("2026-06-28"));
    }
}
```

- [ ] **Step 2.4: 跑测试验证**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test commands::zentao_import 2>&1 | tail -30
```

Expected：全部 21 个测试 PASS（3 encoding + 1 缺列拒绝 + 8 status + 4 assignee + 3 module + 1 workdate + 1 parse_all smoke）。

- [ ] **Step 2.5: 全库回归**

Run: `cargo test 2>&1 | grep -E "test result:" | head`

Expected：PASS，测试数 = 之前基线 + 21（Task 2）+ 3（Task 1）。

- [ ] **Step 2.6: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/Cargo.toml \
        src-tauri/Cargo.lock \
        src-tauri/src/commands/zentao_import.rs \
        src-tauri/src/commands/mod.rs
git commit -m "$(cat <<'EOF'
feat(zentao_import): 添加 CSV 解析器与状态/成员/模块规则

依赖 encoding_rs（UTF-8/GBK 探测）+ csv。zentao_import.rs 定义
DTO 与内部 ParsedRow，实现 detect_and_decode / map_status /
pick_assignee / extract_module_leaf / pick_work_date / parse_all。
21 个测试覆盖编码、缺列拒绝、状态映射 8 分支（含 fallback）、
成员回退、模块叶子提取、工作日期回退、端到端 smoke。
EOF
)"
```

---

## Task 3: `preview_zentao_csv` + `execute_zentao_import` IPC + 执行测试

**Files:**
- Modify: `src-tauri/src/commands/zentao_import.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 的 `Task.external_ref`、Task 2 的 parse_all + 类型；`commands::tasks::create_impl`、`commands::timelogs::create_impl`、`commands::modules::create_impl`、`commands::modules::ModuleInput`。
- Produces（Task 4 消费）: 两条 IPC：
  - `preview_zentao_csv(project_id, file_path) → ImportPreview`
  - `execute_zentao_import(project_id, file_path, member_mapping, module_mapping) → ImportReport`

- [ ] **Step 3.1: 追加 IPC 与执行逻辑到 `zentao_import.rs`**

在 `zentao_import.rs` 末尾（`#[cfg(test)] mod tests` 之前）追加：

```rust
// ─── IPC handlers ────────────────────────────────────────────────────────

use crate::state::AppState;
use crate::commands::modules::{self, ModuleInput};
use crate::commands::tasks::{self, TaskInput};
use crate::commands::timelogs::{self, TimeLogInput};
use rusqlite::Connection;

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

fn read_file(file_path: &str) -> AppResult<Vec<u8>> {
    std::fs::read(file_path).map_err(|e| AppError::Validation(format!("无法读取文件: {e}")))
}

fn dedupe_ordered(items: impl IntoIterator<Item = String>) -> Vec<String> {
    // Preserve first-seen order, dedupe by string
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for s in items {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

pub(crate) fn preview_impl(
    conn: &Connection,
    project_id: i64,
    file_path: &str,
) -> AppResult<ImportPreview> {
    let bytes = read_file(file_path)?;
    let rows = parse_all(&bytes)?;
    let total_rows = rows.len() as u32;
    let cancelled = rows.iter().filter(|r| r.status.is_none()).count() as u32;

    // Count already-imported: rows whose external_ref already lives in the project
    let mut already_imported: u32 = 0;
    for r in &rows {
        let hit: Option<i64> = conn.query_row(
            "SELECT 1 FROM tasks WHERE project_id = ?1 AND external_ref = ?2 AND deleted_at IS NULL",
            rusqlite::params![project_id, r.zentao_id],
            |row| row.get(0),
        ).ok();
        if hit.is_some() {
            already_imported += 1;
        }
    }

    let member_names = dedupe_ordered(
        rows.iter().filter_map(|r| r.assignee_name.clone()),
    );
    let module_names = dedupe_ordered(
        rows.iter().filter_map(|r| r.module_name.clone()),
    );

    Ok(ImportPreview {
        total_rows,
        member_names,
        module_names,
        pre_skip: PreSkipStats { cancelled, already_imported },
    })
}

pub(crate) fn execute_impl(
    conn: &Connection,
    project_id: i64,
    file_path: &str,
    member_mapping: &HashMap<String, MemberChoice>,
    module_mapping: &HashMap<String, ModuleChoice>,
) -> AppResult<ImportReport> {
    let bytes = read_file(file_path)?;
    let rows = parse_all(&bytes)?;

    let mut imported_tasks: u32 = 0;
    let mut imported_timelogs: u32 = 0;
    let mut skipped = SkipCounts::default();
    let mut failed: Vec<FailedRow> = Vec::new();
    let mut created_module_cache: HashMap<String, i64> = HashMap::new();

    for row in rows {
        // 1) cancelled?
        if row.status.is_none() {
            skipped.cancelled += 1;
            continue;
        }

        // 2) already imported?
        let hit: Option<i64> = conn.query_row(
            "SELECT 1 FROM tasks WHERE project_id = ?1 AND external_ref = ?2 AND deleted_at IS NULL",
            rusqlite::params![project_id, row.zentao_id],
            |r| r.get(0),
        ).ok();
        if hit.is_some() {
            skipped.already_imported += 1;
            continue;
        }

        // 3) member mapping
        let assignee_key = row.assignee_name.clone().unwrap_or_default();
        let assignee_id: Option<i64> = match member_mapping.get(&assignee_key) {
            Some(MemberChoice::SkipRow) => {
                skipped.member_skipped += 1;
                continue;
            }
            Some(MemberChoice::UseMember { member_id }) => Some(*member_id),
            Some(MemberChoice::Unassigned) | None => None,
        };

        // 4) module mapping (may create on the fly, cached across rows)
        let module_key = row.module_name.clone().unwrap_or_default();
        let module_id: Option<i64> = match module_mapping.get(&module_key) {
            Some(ModuleChoice::UseModule { module_id }) => Some(*module_id),
            Some(ModuleChoice::CreateWithName { name }) => {
                if let Some(&id) = created_module_cache.get(name) {
                    Some(id)
                } else {
                    match modules::create_impl(
                        conn,
                        project_id,
                        &ModuleInput { name: name.clone(), sort_order: None },
                    ) {
                        Ok(m) => {
                            created_module_cache.insert(name.clone(), m.id);
                            Some(m.id)
                        }
                        Err(e) => {
                            failed.push(FailedRow {
                                row_no: row.row_no,
                                zentao_id: row.zentao_id.clone(),
                                error: format!("module: {e}"),
                            });
                            continue;
                        }
                    }
                }
            }
            Some(ModuleChoice::Unassigned) | None => None,
        };

        // 5) per-row transaction: task + optional timelog
        let tx = match conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => {
                failed.push(FailedRow {
                    row_no: row.row_no,
                    zentao_id: row.zentao_id.clone(),
                    error: format!("tx: {e}"),
                });
                continue;
            }
        };

        let task_input = TaskInput {
            title: row.title.clone(),
            description: row.description.clone(),
            assignee_id,
            status: row.status.clone(),
            estimated_hours: row.estimated_hours,
            due_date: row.due_date.clone(),
            module_id,
            external_ref: Some(row.zentao_id.clone()),
        };
        let task = match tasks::create_impl(&tx, project_id, &task_input) {
            Ok(t) => t,
            Err(e) => {
                failed.push(FailedRow {
                    row_no: row.row_no,
                    zentao_id: row.zentao_id.clone(),
                    error: format!("task: {e}"),
                });
                let _ = tx.rollback();
                continue;
            }
        };

        // 6) optional timelog
        if row.consumed_hours > 0.0 {
            if let (Some(mid), Some(wd)) = (assignee_id, row.work_date.clone()) {
                let tl_input = TimeLogInput {
                    task_id: task.id,
                    member_id: mid,
                    work_date: wd,
                    hours: row.consumed_hours,
                    notes: None,
                };
                match timelogs::create_impl(&tx, &tl_input) {
                    Ok(_) => imported_timelogs += 1,
                    Err(e) => {
                        failed.push(FailedRow {
                            row_no: row.row_no,
                            zentao_id: row.zentao_id.clone(),
                            error: format!("timelog: {e}"),
                        });
                        let _ = tx.rollback();
                        continue;
                    }
                }
            }
        }

        if let Err(e) = tx.commit() {
            failed.push(FailedRow {
                row_no: row.row_no,
                zentao_id: row.zentao_id.clone(),
                error: format!("commit: {e}"),
            });
            continue;
        }
        imported_tasks += 1;
    }

    // Cap failed list at 100 to avoid gigantic reports
    if failed.len() > 100 {
        failed.truncate(100);
    }

    Ok(ImportReport {
        imported_tasks,
        imported_timelogs,
        skipped,
        failed,
    })
}

#[tauri::command]
pub fn preview_zentao_csv(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
) -> AppResult<ImportPreview> {
    with_conn(&state, |c| preview_impl(c, project_id, &file_path))
}

#[tauri::command]
pub fn execute_zentao_import(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
    member_mapping: HashMap<String, MemberChoice>,
    module_mapping: HashMap<String, ModuleChoice>,
) -> AppResult<ImportReport> {
    with_conn(&state, |c| {
        execute_impl(c, project_id, &file_path, &member_mapping, &module_mapping)
    })
}
```

- [ ] **Step 3.2: 注册 IPC handlers**

编辑 `src-tauri/src/lib.rs`——在 `invoke_handler!` 数组里、`commands::modules::get_module_labor_stats,` 之后追加：

```rust
            commands::zentao_import::preview_zentao_csv,
            commands::zentao_import::execute_zentao_import,
```

- [ ] **Step 3.3: 追加执行测试**

在 `zentao_import.rs` 的 `#[cfg(test)] mod tests { ... }` 末尾追加：

```rust
    // ─── Execution / preview tests ───────────────────────────────

    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};
    use std::path::PathBuf;

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
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, '李黎明', 80000)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn write_csv(dir: &TempDir, name: &str, body: &str) -> PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    fn one_row_csv() -> &'static str {
        "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
         368,现场实施,已关闭,已完成,8h,8h,2026-06-28 08:44:00,李黎明,Closed,李黎明,/(#0)\n"
    }

    fn mapping_use_member(name: &str, member_id: i64) -> HashMap<String, MemberChoice> {
        let mut m = HashMap::new();
        m.insert(name.into(), MemberChoice::UseMember { member_id });
        m
    }

    #[test]
    fn preview_counts_total_and_already_imported() {
        let db = TestDb::new();
        // Seed one existing task with external_ref=zentao:368
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, external_ref) VALUES(1, 'seed', 'zentao:368')",
            [],
        ).unwrap();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let out = preview_impl(&db.conn, 1, path.to_str().unwrap()).unwrap();
        assert_eq!(out.total_rows, 1);
        assert_eq!(out.pre_skip.already_imported, 1);
        assert_eq!(out.pre_skip.cancelled, 0);
    }

    #[test]
    fn preview_collects_member_names() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let out = preview_impl(&db.conn, 1, path.to_str().unwrap()).unwrap();
        assert_eq!(out.member_names, vec!["李黎明"]);
        assert!(out.module_names.is_empty());
    }

    #[test]
    fn execute_creates_task_with_external_ref() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 1);
        // verify DB
        let ref_val: String = db.conn.query_row(
            "SELECT external_ref FROM tasks WHERE project_id = 1",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(ref_val, "zentao:368");
    }

    #[test]
    fn execute_skips_timelog_when_hours_zero() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   369,零工时,已关闭,已完成,0h,0h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 0);
    }

    #[test]
    fn execute_skips_timelog_when_member_unassigned() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mut mapping = HashMap::new();
        mapping.insert("李黎明".into(), MemberChoice::Unassigned);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 0);
    }

    #[test]
    fn execute_skips_row_when_member_skip_row() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mut mapping = HashMap::new();
        mapping.insert("李黎明".into(), MemberChoice::SkipRow);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.member_skipped, 1);
    }

    #[test]
    fn execute_skips_row_when_status_cancelled() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   370,取消的任务,已取消,,0h,0h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.cancelled, 1);
    }

    #[test]
    fn execute_skips_row_when_external_ref_already_imported() {
        let db = TestDb::new();
        // Seed
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, external_ref) VALUES(1, 'seed', 'zentao:368')",
            [],
        ).unwrap();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.already_imported, 1);
    }

    #[test]
    fn execute_creates_module_on_the_fly() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   371,前端任务,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/前端(#5)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let mut mmap = HashMap::new();
        mmap.insert("前端".into(), ModuleChoice::CreateWithName { name: "前端".into() });
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &mmap).unwrap();
        assert_eq!(out.imported_tasks, 1);
        // Verify a module named "前端" was created under project 1
        let module_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM modules WHERE project_id = 1 AND name = '前端' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(module_count, 1);
    }

    #[test]
    fn execute_reuses_created_module_across_rows() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   372,前端任务1,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/前端(#5)\n\
                   373,前端任务2,已关闭,已完成,2h,2h,2026-06-29,李黎明,Closed,李黎明,/前端(#5)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let mut mmap = HashMap::new();
        mmap.insert("前端".into(), ModuleChoice::CreateWithName { name: "前端".into() });
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &mmap).unwrap();
        assert_eq!(out.imported_tasks, 2);
        let module_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM modules WHERE project_id = 1 AND name = '前端' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(module_count, 1);
    }

    #[test]
    fn execute_records_failure_and_continues() {
        let db = TestDb::new();
        // Row 1 fine; row 2 has invalid hours (>24); row 3 fine
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   374,ok1,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n\
                   375,bad,已关闭,已完成,4h,99h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n\
                   376,ok2,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 2);
        assert_eq!(out.failed.len(), 1);
        assert_eq!(out.failed[0].zentao_id, "zentao:375");
    }
```

- [ ] **Step 3.4: 跑执行测试**

Run:
```bash
source ~/.cargo/env
cd /Users/l2m2/workspace/l2m2/solo-cost/src-tauri
cargo test commands::zentao_import 2>&1 | tail -30
```

Expected：全部 PASS（新增 11 个执行测试，加上之前 21 = 32 个 zentao_import 测试）。

- [ ] **Step 3.5: 全库回归**

Run: `cargo test 2>&1 | grep -E "test result:" | head`

Expected：PASS，所有测试。

- [ ] **Step 3.6: Commit**

```bash
cd /Users/l2m2/workspace/l2m2/solo-cost
git add src-tauri/src/commands/zentao_import.rs src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(zentao_import): preview + execute IPC 与执行流程

preview_impl 收集 member/module 去重名单与预跳过计数；execute_impl
按行处理：cancelled/已导入/成员 SkipRow 走跳过路径；CreateWithName
模块首次调 modules::create_impl 后本次缓存；task 与 timelog 同一
tx，失败 rollback 记入 failed 继续；failed 上限 100。注册两条
IPC。新增 11 个执行测试覆盖创建/跳过/失败继续/模块复用。
EOF
)"
```

---

## Task 4: Frontend — TS 类型 + i18n

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/i18n/zh-CN.json`

**Interfaces:**
- Consumes: Task 1/2/3 后端结构。
- Produces（Task 5/6 消费）:
  - `Task` 扩 `external_ref: string | null`
  - `TaskInput` 扩 `external_ref?: string | null`
  - `ImportPreview`、`MemberChoice`（tagged union）、`ModuleChoice`（tagged union）、`ImportReport`
  - i18n keys `zentaoImport.*` + `common.{next,back,done}`

- [ ] **Step 4.1: 扩展 TS 类型**

编辑 `src/types/index.ts`：

- 在 `interface Task { ... }` 里，`module_id: number | null;` 之后追加：

  ```ts
    external_ref: string | null;
  ```

- 在 `interface TaskInput { ... }` 里，`module_id?: number | null;` 之后追加：

  ```ts
    external_ref?: string | null;
  ```

- 在文件末尾追加：

  ```ts
  export interface ImportPreview {
    total_rows: number;
    member_names: string[];
    module_names: string[];
    pre_skip: { cancelled: number; already_imported: number };
  }

  export type MemberChoice =
    | { kind: "use_member"; member_id: number }
    | { kind: "unassigned" }
    | { kind: "skip_row" };

  export type ModuleChoice =
    | { kind: "use_module"; module_id: number }
    | { kind: "create_with_name"; name: string }
    | { kind: "unassigned" };

  export interface ImportReport {
    imported_tasks: number;
    imported_timelogs: number;
    skipped: {
      cancelled: number;
      already_imported: number;
      member_skipped: number;
    };
    failed: { row_no: number; zentao_id: string; error: string }[];
  }
  ```

- [ ] **Step 4.2: 加 i18n keys**

编辑 `src/i18n/zh-CN.json`：

- 在顶层（`module` 对象之后即可）新增：

  ```json
    "zentaoImport": {
      "title": "从禅道 CSV 导入",
      "chooseFile": "选择 CSV 文件",
      "reselect": "重选文件",
      "step": {
        "file": "选文件",
        "members": "成员",
        "modules": "模块",
        "confirm": "确认",
        "report": "报告"
      },
      "preview": {
        "summary": "共 {{total}} 行；已在库 {{already}}，已取消 {{cancelled}}",
        "willImport": "将导入 {{n}} 个任务（其中 {{logs}} 个带工时）",
        "willSkip": "跳过 {{n}} 个（已存在 {{already}} / 已取消 {{cancelled}} / 成员选择跳过 {{member}}）"
      },
      "member": {
        "column": "CSV 名字",
        "mapTo": "映射到",
        "unassigned": "未指派",
        "skipRow": "跳过含此人的行"
      },
      "module": {
        "column": "CSV 模块名",
        "mapTo": "映射到",
        "createWith": "新建「{{name}}」",
        "unassigned": "未分类"
      },
      "report": {
        "title": "导入完成",
        "imported": "已导入 {{tasks}} 个任务、{{logs}} 条工时",
        "skipped": "跳过 {{n}} 个（已存在 {{already}} / 已取消 {{cancelled}} / 成员跳过 {{member}}）",
        "failedTitle": "失败 {{n}} 条：",
        "failedItem": "[第 {{row}} 行] {{ref}}: {{err}}"
      },
      "action": {
        "start": "开始导入",
        "importing": "导入中…"
      },
      "error": {
        "parseFailed": "CSV 解析失败：{{msg}}",
        "executeFailed": "导入执行失败：{{msg}}"
      }
    },
  ```

- 在 `"common"` 对象内追加（若 `next`/`back`/`done` 已存在则不重复）：

  ```json
      "next": "下一步",
      "back": "上一步",
      "done": "完成",
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
feat(types+i18n): 禅道 CSV 导入类型与文案

Task 扩 external_ref；新增 ImportPreview / MemberChoice /
ModuleChoice / ImportReport（tagged union 与后端 serde 一致）。
zh-CN.json 新增 zentaoImport.* 与 common.{next,back,done}。
EOF
)"
```

---

## Task 5: Frontend — `ZentaoImportDialog` 组件

**Files:**
- Create: `src/components/zentao-import/ZentaoImportDialog.tsx`

**Interfaces:**
- Consumes: Task 4 类型 + i18n；`useMembersStore`（读 `list`）、`useModulesStore`（读 `byProject`）；`@tauri-apps/plugin-dialog::open`；`@/lib/ipc::call`；stores for post-import refresh: `useTasksStore` / `useModulesStore` / `useModuleStatsStore` / `useFinancialStore`。
- Produces（Task 6 消费）: 组件 `ZentaoImportDialog(props: { projectId, companyId, open, onOpenChange })`。

- [ ] **Step 5.1: 创建组件文件**

Create `src/components/zentao-import/ZentaoImportDialog.tsx`：

```tsx
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { call } from "@/lib/ipc";
import { useMembersStore } from "@/stores/members";
import { useModulesStore } from "@/stores/modules";
import { useModuleStatsStore } from "@/stores/moduleStats";
import { useTasksStore } from "@/stores/tasks";
import { useFinancialStore } from "@/stores/financial";
import type {
  ImportPreview, ImportReport, MemberChoice, ModuleChoice,
} from "@/types";

type Step = 1 | 2 | 3 | 4 | 5;

export default function ZentaoImportDialog({
  projectId,
  companyId,
  open,
  onOpenChange,
}: {
  projectId: number;
  companyId: number;
  open: boolean;
  onOpenChange: (v: boolean) => void;
}) {
  const { t } = useTranslation();
  const { list: members, loadedForCompany: membersLoadedFor, loadFor: loadMembers } = useMembersStore();
  const { byProject: modulesByProject, loadedForProject: modulesLoadedFor, loadFor: loadModules } = useModulesStore();
  const modules = modulesByProject[projectId] ?? [];

  const [step, setStep] = useState<Step>(1);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [memberMapping, setMemberMapping] = useState<Record<string, MemberChoice>>({});
  const [moduleMapping, setModuleMapping] = useState<Record<string, ModuleChoice>>({});
  const [report, setReport] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);

  // Reset on open
  useEffect(() => {
    if (open) {
      setStep(1);
      setFilePath(null);
      setPreview(null);
      setMemberMapping({});
      setModuleMapping({});
      setReport(null);
      setBusy(false);
    }
  }, [open]);

  // Ensure members + modules are loaded for the mapping steps
  useEffect(() => {
    if (open && membersLoadedFor !== companyId) loadMembers(companyId);
  }, [open, companyId, membersLoadedFor, loadMembers]);
  useEffect(() => {
    if (open && !modulesLoadedFor[projectId]) loadModules(projectId);
  }, [open, projectId, modulesLoadedFor, loadModules]);

  const activeMembers = members.filter((m) => m.is_active);

  const goPreview = async () => {
    try {
      const p = await openFilePicker({
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (typeof p !== "string") return; // user cancelled
      setFilePath(p);
      setBusy(true);
      const res = await call<ImportPreview>("preview_zentao_csv", {
        projectId, filePath: p,
      });
      setPreview(res);
      // Pre-fill mappings with defaults
      const mm: Record<string, MemberChoice> = {};
      for (const name of res.member_names) {
        const match = activeMembers.find((m) => m.name === name);
        mm[name] = match
          ? { kind: "use_member", member_id: match.id }
          : { kind: "unassigned" };
      }
      const modmap: Record<string, ModuleChoice> = {};
      for (const name of res.module_names) {
        const match = modules.find((m) => m.name === name);
        modmap[name] = match
          ? { kind: "use_module", module_id: match.id }
          : { kind: "create_with_name", name };
      }
      setMemberMapping(mm);
      setModuleMapping(modmap);
      // Skip empty mapping steps
      if (res.member_names.length === 0 && res.module_names.length === 0) setStep(4);
      else if (res.member_names.length === 0) setStep(3);
      else setStep(2);
    } catch (e: unknown) {
      toast.error(t("zentaoImport.error.parseFailed", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const execute = async () => {
    if (!filePath) return;
    setBusy(true);
    try {
      const res = await call<ImportReport>("execute_zentao_import", {
        projectId,
        filePath,
        memberMapping,
        moduleMapping,
      });
      setReport(res);
      setStep(5);
    } catch (e: unknown) {
      toast.error(t("zentaoImport.error.executeFailed", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const finish = async () => {
    // Refresh downstream views
    await useTasksStore.getState().loadFor(projectId, null);
    await useModulesStore.getState().loadFor(projectId);
    await useModuleStatsStore.getState().refresh(projectId);
    await useFinancialStore.getState().refresh(projectId);
    onOpenChange(false);
  };

  const willImportCount = (() => {
    if (!preview) return 0;
    const memberSkipped = preview.member_names.filter(
      (n) => memberMapping[n]?.kind === "skip_row",
    ).length;
    // approximate: preview rows minus cancelled minus already_imported minus rows with SkipRow assignee
    // Note: this is a coarse estimate; report will be authoritative.
    return preview.total_rows
      - preview.pre_skip.cancelled
      - preview.pre_skip.already_imported
      - memberSkipped;
  })();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>{t("zentaoImport.title")}</DialogTitle>
        </DialogHeader>
        <div className="text-xs text-muted-foreground">
          {t("zentaoImport.step.file")} → {t("zentaoImport.step.members")} → {t("zentaoImport.step.modules")} → {t("zentaoImport.step.confirm")} → {t("zentaoImport.step.report")}
        </div>

        {step === 1 && (
          <div className="space-y-3">
            <Button onClick={goPreview} disabled={busy}>{t("zentaoImport.chooseFile")}</Button>
            {preview && filePath && (
              <div className="text-sm text-muted-foreground">
                {filePath}<br />
                {t("zentaoImport.preview.summary", {
                  total: preview.total_rows,
                  already: preview.pre_skip.already_imported,
                  cancelled: preview.pre_skip.cancelled,
                })}
              </div>
            )}
          </div>
        )}

        {step === 2 && preview && (
          <div className="space-y-3">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("zentaoImport.member.column")}</TableHead>
                  <TableHead>{t("zentaoImport.member.mapTo")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.member_names.map((name) => {
                  const cur = memberMapping[name];
                  const value =
                    cur?.kind === "use_member" ? String(cur.member_id)
                    : cur?.kind === "skip_row" ? "__skip"
                    : "__unassigned";
                  return (
                    <TableRow key={name}>
                      <TableCell>{name}</TableCell>
                      <TableCell>
                        <Select
                          value={value}
                          onValueChange={(v) => {
                            let choice: MemberChoice;
                            if (v === "__unassigned") choice = { kind: "unassigned" };
                            else if (v === "__skip") choice = { kind: "skip_row" };
                            else choice = { kind: "use_member", member_id: Number(v) };
                            setMemberMapping({ ...memberMapping, [name]: choice });
                          }}
                        >
                          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="__unassigned">{t("zentaoImport.member.unassigned")}</SelectItem>
                            <SelectItem value="__skip">{t("zentaoImport.member.skipRow")}</SelectItem>
                            {activeMembers.map((m) => (
                              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}

        {step === 3 && preview && (
          <div className="space-y-3">
            <Table compact>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("zentaoImport.module.column")}</TableHead>
                  <TableHead>{t("zentaoImport.module.mapTo")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.module_names.map((name) => {
                  const cur = moduleMapping[name];
                  const value =
                    cur?.kind === "use_module" ? String(cur.module_id)
                    : cur?.kind === "create_with_name" ? "__create"
                    : "__unassigned";
                  return (
                    <TableRow key={name}>
                      <TableCell>{name}</TableCell>
                      <TableCell>
                        <Select
                          value={value}
                          onValueChange={(v) => {
                            let choice: ModuleChoice;
                            if (v === "__unassigned") choice = { kind: "unassigned" };
                            else if (v === "__create") choice = { kind: "create_with_name", name };
                            else choice = { kind: "use_module", module_id: Number(v) };
                            setModuleMapping({ ...moduleMapping, [name]: choice });
                          }}
                        >
                          <SelectTrigger className="w-56"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="__unassigned">{t("zentaoImport.module.unassigned")}</SelectItem>
                            <SelectItem value="__create">{t("zentaoImport.module.createWith", { name })}</SelectItem>
                            {modules.map((m) => (
                              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}

        {step === 4 && preview && (
          <Card>
            <CardContent className="p-6 space-y-2 text-sm">
              <div>{t("zentaoImport.preview.willImport", { n: willImportCount, logs: willImportCount })}</div>
              <div className="text-muted-foreground">
                {t("zentaoImport.preview.willSkip", {
                  n: preview.pre_skip.cancelled + preview.pre_skip.already_imported
                     + preview.member_names.filter((n) => memberMapping[n]?.kind === "skip_row").length,
                  already: preview.pre_skip.already_imported,
                  cancelled: preview.pre_skip.cancelled,
                  member: preview.member_names.filter((n) => memberMapping[n]?.kind === "skip_row").length,
                })}
              </div>
            </CardContent>
          </Card>
        )}

        {step === 5 && report && (
          <div className="space-y-3">
            <div className="font-medium">{t("zentaoImport.report.title")}</div>
            <div className="text-sm">
              {t("zentaoImport.report.imported", { tasks: report.imported_tasks, logs: report.imported_timelogs })}
            </div>
            <div className="text-sm text-muted-foreground">
              {t("zentaoImport.report.skipped", {
                n: report.skipped.cancelled + report.skipped.already_imported + report.skipped.member_skipped,
                already: report.skipped.already_imported,
                cancelled: report.skipped.cancelled,
                member: report.skipped.member_skipped,
              })}
            </div>
            {report.failed.length > 0 && (
              <div className="rounded border border-destructive/40 p-3 space-y-1">
                <div className="text-sm text-destructive">
                  {t("zentaoImport.report.failedTitle", { n: report.failed.length })}
                </div>
                <div className="max-h-40 overflow-y-auto space-y-1">
                  {report.failed.map((f) => (
                    <div key={`${f.row_no}-${f.zentao_id}`} className="text-xs">
                      {t("zentaoImport.report.failedItem", { row: f.row_no, ref: f.zentao_id, err: f.error })}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        <DialogFooter>
          {step > 1 && step < 5 && (
            <Button variant="outline" onClick={() => setStep((s) => (s - 1) as Step)} disabled={busy}>
              {t("common.back")}
            </Button>
          )}
          {step === 1 && preview && (
            <Button
              onClick={() => {
                if (preview.member_names.length === 0 && preview.module_names.length === 0) setStep(4);
                else if (preview.member_names.length === 0) setStep(3);
                else setStep(2);
              }}
              disabled={busy}
            >
              {t("common.next")}
            </Button>
          )}
          {step === 2 && (
            <Button onClick={() => setStep(preview!.module_names.length === 0 ? 4 : 3)} disabled={busy}>
              {t("common.next")}
            </Button>
          )}
          {step === 3 && (
            <Button onClick={() => setStep(4)} disabled={busy}>
              {t("common.next")}
            </Button>
          )}
          {step === 4 && (
            <Button onClick={execute} disabled={busy}>
              {busy ? t("zentaoImport.action.importing") : t("zentaoImport.action.start")}
            </Button>
          )}
          {step === 5 && (
            <Button onClick={finish}>{t("common.done")}</Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 5.2: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
```

Expected：`TSC_EXIT=0`。

- [ ] **Step 5.3: Commit**

```bash
git add src/components/zentao-import/ZentaoImportDialog.tsx
git commit -m "$(cat <<'EOF'
feat(zentao-import): 5 步 wizard 组件

选文件（tauri-plugin-dialog）→ preview → 成员映射（默认按同名
匹配 active member）→ 模块映射（默认按同名匹配已有 module，
否则默认"新建"）→ 确认摘要 → 执行 → 报告；结束时 refresh
tasks/modules/moduleStats/financial。空映射步骤自动跳过。
EOF
)"
```

---

## Task 6: Frontend — TasksPanel 按钮 + 挂 Dialog

**Files:**
- Modify: `src/routes/projects/detail.tsx`

**Interfaces:**
- Consumes: Task 5 组件 `ZentaoImportDialog`。
- Produces: 无。

- [ ] **Step 6.1: 加 state 与 import**

编辑 `src/routes/projects/detail.tsx`：

- 在顶部 import 追加：

  ```tsx
  import ZentaoImportDialog from "@/components/zentao-import/ZentaoImportDialog";
  ```

- 在 `TasksPanel` 组件顶部（`const [openManageModules, setOpenManageModules] = useState(false);` 之后）追加：

  ```tsx
    const [openZentaoImport, setOpenZentaoImport] = useState(false);
  ```

- [ ] **Step 6.2: 顶部工具栏加按钮**

在 `TasksPanel` 顶部工具栏 `<div className="flex items-center gap-2">` 内（在「管理模块」按钮之后）追加：

```tsx
          <Button variant="outline" onClick={() => setOpenZentaoImport(true)}>
            {t("zentaoImport.title")}
          </Button>
```

- [ ] **Step 6.3: 挂 Dialog**

在 `TasksPanel` 返回 JSX 的最后（在 `ManageModules Dialog` 之后、外层 `</div>` 之前）追加：

```tsx
      <ZentaoImportDialog
        projectId={projectId}
        companyId={companyId}
        open={openZentaoImport}
        onOpenChange={setOpenZentaoImport}
      />
```

- [ ] **Step 6.4: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
cd /Users/l2m2/workspace/l2m2/solo-cost
pnpm exec tsc -b
echo "TSC_EXIT=$?"
```

Expected：`TSC_EXIT=0`。

- [ ] **Step 6.5: 人工验收**

启动 `pnpm tauri dev`，用 `~/Downloads/a005-2-全部任务.csv` 走一遍：

1. 打开一个项目详情 →「任务+工时」tab → 点「从禅道 CSV 导入」→ 弹 Dialog
2. Step 1：点「选择 CSV 文件」→ 系统对话框选 CSV → preview 显示「共 5 行；已在库 0，已取消 0」
3. Step 2：成员映射表格显示「李黎明」一行；下拉自动匹配到 solo-cost 里的「李黎明」（若已建）；下一步
4. Step 3（若无模块）跳过
5. Step 4：摘要「将导入 5 个任务（其中 5 个带工时），跳过 0 个」→ 「开始导入」
6. Step 5：报告「已导入 5 个任务、5 条工时」→ 「完成」
7. Dialog 关闭；任务列表、模块统计卡（若有）、财务面板都反映了新数据
8. 二次导入同一份 CSV：Step 1 显示「已在库 5」；Step 4 摘要「将导入 0 个」；Step 5 报告「已导入 0、跳过 5 (5 已存在)」

- [ ] **Step 6.6: Commit**

```bash
git add src/routes/projects/detail.tsx
git commit -m "$(cat <<'EOF'
feat(projects): TasksPanel 加「从禅道 CSV 导入」入口

顶部工具栏「管理模块」按钮之后加 outline 按钮，点击开
ZentaoImportDialog，projectId/companyId 传入。
EOF
)"
```

---

## Task 7: CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:** 无

- [ ] **Step 7.1: 加条目**

编辑 `CHANGELOG.md`——在 `## Unreleased → ### Added` 段的第一条之前插入：

```markdown
- 禅道 CSV 导入：项目详情「任务+工时」tab 顶部新增「从禅道 CSV 导入」按钮，5 步 wizard 完成成员/模块映射与幂等导入（`tasks.external_ref = "zentao:<编号>"` 命中则整行跳过），支持 UTF-8 与 GBK 编码
```

- [ ] **Step 7.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录禅道 CSV 导入条目"
```

---

## 完成后

- [ ] 全库 `cargo test` PASS（旧基线 + Task 1 `+3` + Task 2 `+21` + Task 3 `+11` = 增加 35 条 Rust 测试）
- [ ] `pnpm exec tsc -b` EXIT=0
- [ ] `pnpm tauri dev` 手工回归 Task 6.5 的 8 条验收清单
- [ ] 询问用户是否推送 origin/main
