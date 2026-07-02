# 项目销售提成 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给项目加"销售提成"能力：`none` / `rate`（按已回款含税额 × 提成率）/ `fixed`（固定金额 + 手工入账开关），后端自动汇入 `total_cost`，前端在项目表单和财务面板暴露。

**Architecture:** 提成配置作为 `projects` 表的独立字段（4 列），不落 `cost_entries`；`domain/profit.rs::project_financial_summary` 在读取项目行时一并读出并派生 `commission_cents` 加入 `total_cost`。前端 `ProjectForm` 加提成区块（模式切换清脏值），`FinancialPanel` 在 `mode !== 'none'` 时将成本卡片行 3 列扩为 4 列并插入"销售提成"卡。

**Tech Stack:** Rust (rusqlite / tauri v2 command) + React 19 + TypeScript + Vite + Tailwind + shadcn/radix + i18next。

## Global Constraints

- 迁移编号沿用现有序列：新增 `0004_projects_commission.sql`。
- 前端零改动 store 结构：`useProjectsStore` 已经透传整个 `ProjectInput`，字段扩展即用。
- 现有 `ProjectFinancialSummary.total_cost_cents` 语义扩为 `general + labor + commission`，`gross`/`profit_rate` 公式不变。
- i18n 只维护 `src/i18n/zh-CN.json`（当前项目未启用英文）。
- Commit 规范：Conventional Commits + 中文 subject，≤ 72 字符。

---

## File Structure

**新增**：
- `src-tauri/migrations/0004_projects_commission.sql` — 4 列迁移
- `docs/superpowers/plans/2026-07-02-project-commission.md`（本文件）

**修改**：
- `src-tauri/src/commands/projects.rs` — `Project` / `ProjectInput` 扩字段、`row_to_project`、`validate`、`create_impl`、`update_impl`、单元测试
- `src-tauri/src/domain/profit.rs` — `ProjectFinancialSummary` 加字段、SELECT 扩字段、commission 计算、5 个新测试
- `src/types/index.ts` — `Project` / `ProjectInput` / `ProjectFinancialSummary` 扩字段
- `src/i18n/zh-CN.json` — 新增 `project.commissionSection` 等 keys
- `src/routes/projects/list.tsx` — `ProjectForm` 加提成区块 + submit 映射
- `src/routes/projects/detail.tsx` — `FinancialPanel` 加提成卡与自适应布局

**不改**：
- `src/stores/projects.ts`（已透传，字段扩展自动兼容）
- `src/stores/financial.ts`（同上）

---

## Task 1: Backend — 数据库迁移 + Project 结构与 CRUD

**Files:**
- Create: `src-tauri/migrations/0004_projects_commission.sql`
- Modify: `src-tauri/src/commands/projects.rs`

**Interfaces:**
- Consumes: 现有 `Project`/`ProjectInput`（`commands/projects.rs:17-47`）、`AppError::Validation`（`error.rs`）
- Produces:
  - `Project` 新增字段：
    ```rust
    pub commission_mode: String,               // "none" | "rate" | "fixed"
    pub commission_rate: Option<f64>,          // 0..1
    pub commission_amount_cents: Option<i64>,  // ≥ 0
    pub commission_settled: bool,
    ```
  - `ProjectInput` 新增同名 4 个 `Option<...>` 字段
  - Task 2 依赖以上字段名和 `commission_mode` 白名单常量

- [ ] **Step 1.1: 写迁移文件**

Create `src-tauri/migrations/0004_projects_commission.sql`：

```sql
-- Project sales commission fields
BEGIN;

ALTER TABLE projects ADD COLUMN commission_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE projects ADD COLUMN commission_rate REAL;
ALTER TABLE projects ADD COLUMN commission_amount_cents INTEGER;
ALTER TABLE projects ADD COLUMN commission_settled INTEGER NOT NULL DEFAULT 0;

COMMIT;
```

- [ ] **Step 1.2: 扩展 `Project` 与 `ProjectInput` 结构体**

在 `src-tauri/src/commands/projects.rs` 中，找到 `pub struct Project`（第 17-33 行），在 `updated_at: String` 前追加：

```rust
    pub commission_mode: String,
    pub commission_rate: Option<f64>,
    pub commission_amount_cents: Option<i64>,
    pub commission_settled: bool,
```

紧接着在 `pub struct ProjectInput`（第 35-47 行）的 `notes` 之后追加：

```rust
    pub commission_mode: Option<String>,
    pub commission_rate: Option<f64>,
    pub commission_amount_cents: Option<i64>,
    pub commission_settled: Option<bool>,
```

- [ ] **Step 1.3: 扩展 `row_to_project`**

在 `row_to_project`（第 49-67 行）的返回结构末尾（`updated_at` 后）追加：

```rust
        commission_mode: row.get("commission_mode")?,
        commission_rate: row.get("commission_rate")?,
        commission_amount_cents: row.get("commission_amount_cents")?,
        commission_settled: row.get::<_, i64>("commission_settled")? != 0,
```

- [ ] **Step 1.4: 加提成白名单常量与校验**

在文件顶部 `ALLOWED_STATUSES` 常量下方追加：

```rust
const ALLOWED_COMMISSION_MODES: [&str; 3] = ["none", "rate", "fixed"];
```

在 `fn validate` 末尾（`Ok(())` 之前）追加：

```rust
    if let Some(ref m) = input.commission_mode {
        if !ALLOWED_COMMISSION_MODES.contains(&m.as_str()) {
            return Err(AppError::Validation(format!("非法提成模式：{m}")));
        }
        match m.as_str() {
            "rate" => {
                let r = input.commission_rate.unwrap_or(0.0);
                if !(0.0..=1.0).contains(&r) {
                    return Err(AppError::Validation(
                        "提成率必须在 [0, 1] 之间".into(),
                    ));
                }
            }
            "fixed" => {
                let a = input.commission_amount_cents.unwrap_or(0);
                if a < 0 {
                    return Err(AppError::Validation(
                        "固定提成金额不能为负".into(),
                    ));
                }
            }
            _ => {}
        }
    }
```

- [ ] **Step 1.5: 扩展 INSERT（`create_impl`）**

把 `create_impl` 里的 `INSERT INTO projects(...)` 参数与 SQL 替换为下面版本：

```rust
    conn.execute(
        "INSERT INTO projects(
            company_id, name, client_name, status,
            contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
            start_date, end_date, actual_delivered_at, notes,
            commission_mode, commission_rate, commission_amount_cents, commission_settled
         ) VALUES(
            ?1, ?2, ?3, COALESCE(?4, 'pending'),
            COALESCE(?5, 0), COALESCE(?6, 1), COALESCE(?7, 0.06),
            ?8, ?9, ?10, ?11,
            COALESCE(?12, 'none'), ?13, ?14, COALESCE(?15, 0)
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
            input.commission_mode.as_deref(),
            input.commission_rate,
            input.commission_amount_cents,
            input.commission_settled.map(|b| b as i64),
        ],
    )?;
```

- [ ] **Step 1.6: 扩展 UPDATE（`update_impl`）**

把 `update_impl` 里的 `UPDATE projects SET ...` 语句替换为：

```rust
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
            commission_mode = COALESCE(?11, commission_mode),
            commission_rate = ?12,
            commission_amount_cents = ?13,
            commission_settled = COALESCE(?14, commission_settled),
            updated_at = datetime('now')
         WHERE id = ?15 AND deleted_at IS NULL",
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
            input.commission_mode.as_deref(),
            input.commission_rate,
            input.commission_amount_cents,
            input.commission_settled.map(|b| b as i64),
            id,
        ],
    )?;
```

说明：`commission_rate` 与 `commission_amount_cents` 允许被显式清空（None 直接写 NULL），因为切换模式时前端会传 None；`commission_mode` 与 `commission_settled` 用 COALESCE 保留旧值，允许部分更新（例如 detail 页面单独改状态时不必再传提成字段）。

- [ ] **Step 1.7: 同步测试辅助 `input()`**

在 `#[cfg(test)] mod tests` 的 `fn input(name: &str) -> ProjectInput` 末尾（`notes: None` 之后）追加：

```rust
        commission_mode: None,
        commission_rate: None,
        commission_amount_cents: None,
        commission_settled: None,
```

- [ ] **Step 1.8: 写失败测试**

在 `#[cfg(test)] mod tests` 末尾追加：

```rust
    #[test]
    fn create_defaults_commission_mode_none() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        assert_eq!(p.commission_mode, "none");
        assert!(p.commission_rate.is_none());
        assert!(p.commission_amount_cents.is_none());
        assert!(!p.commission_settled);
    }

    #[test]
    fn create_persists_commission_rate() {
        let db = TestDb::new();
        let mut i = input("P");
        i.commission_mode = Some("rate".into());
        i.commission_rate = Some(0.05);
        let p = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(p.commission_mode, "rate");
        assert!((p.commission_rate.unwrap() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn create_persists_commission_fixed_with_settled() {
        let db = TestDb::new();
        let mut i = input("P");
        i.commission_mode = Some("fixed".into());
        i.commission_amount_cents = Some(200_000);
        i.commission_settled = Some(true);
        let p = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(p.commission_mode, "fixed");
        assert_eq!(p.commission_amount_cents, Some(200_000));
        assert!(p.commission_settled);
    }

    #[test]
    fn validate_rejects_out_of_range_rate() {
        let db = TestDb::new();
        let mut i = input("P");
        i.commission_mode = Some("rate".into());
        i.commission_rate = Some(1.5);
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_rejects_bad_commission_mode() {
        let db = TestDb::new();
        let mut i = input("P");
        i.commission_mode = Some("percentage".into());
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
```

- [ ] **Step 1.9: 跑测试验证失败**

Run: `cd src-tauri && cargo test -p solo-cost-lib commands::projects::tests 2>&1 | tail -30`

Expected: 编译失败（`commission_mode` 字段还没有）或 5 个新测试全部 FAIL 与老的持久化测试全通过。

- [ ] **Step 1.10: 跑测试验证通过**

前面 Step 1.1–1.7 已经完成实现。现在再次执行：

Run: `cd src-tauri && cargo test -p solo-cost-lib commands::projects::tests 2>&1 | tail -20`

Expected: 全部 PASS（包含新增 5 个 + 原 7 个）。

- [ ] **Step 1.11: Commit**

```bash
git add src-tauri/migrations/0004_projects_commission.sql src-tauri/src/commands/projects.rs
git commit -m "$(cat <<'EOF'
feat(projects): 新增销售提成 4 个字段与校验

在 projects 表加入 commission_mode/commission_rate/
commission_amount_cents/commission_settled，扩展 CRUD 与
输入校验（rate ∈ [0,1]、fixed amount ≥ 0），旧行默认 none。
EOF
)"
```

---

## Task 2: Backend — `profit.rs` 计算提成

**Files:**
- Modify: `src-tauri/src/domain/profit.rs`

**Interfaces:**
- Consumes: Task 1 产生的 `projects` 表 4 列
- Produces:
  - `ProjectFinancialSummary` 新增字段：
    ```rust
    pub commission_cents: i64,
    ```
  - `total_cost_cents` 语义扩为 `general + labor + commission`

- [ ] **Step 2.1: 扩展 `ProjectFinancialSummary`**

在 `src-tauri/src/domain/profit.rs` 的 `pub struct ProjectFinancialSummary { ... }`（约第 51-64 行）里，在 `total_cost_cents: i64,` 后立即插入：

```rust
    pub commission_cents: i64,
```

- [ ] **Step 2.2: 扩展 SELECT，加载提成字段**

把 `project_financial_summary` 顶部的 `query_row`（约第 71-84 行）替换为：

```rust
    let (contract, inclusive, rate, comm_mode, comm_rate, comm_amount, comm_settled): (
        i64,
        i64,
        f64,
        String,
        Option<f64>,
        Option<i64>,
        i64,
    ) = conn
        .query_row(
            "SELECT contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
                    commission_mode, commission_rate, commission_amount_cents, commission_settled
             FROM projects WHERE id = ?1 AND deleted_at IS NULL",
            [project_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
                entity: "project",
                id: project_id,
            },
            other => AppError::Db(other),
        })?;
    let comm_settled = comm_settled != 0;
```

- [ ] **Step 2.3: 派生 `commission` 并合入 `total_cost`**

在计算完 `let actual: i64 = ...`（约第 135-141 行）之后、`let collection_rate = ...` 之前插入：

```rust
    // sales commission — depends on mode
    let commission = match comm_mode.as_str() {
        "rate" => {
            let r = comm_rate.unwrap_or(0.0);
            (actual as f64 * r).round() as i64
        }
        "fixed" => {
            if comm_settled {
                comm_amount.unwrap_or(0)
            } else {
                0
            }
        }
        _ => 0, // "none" 与任何异常值
    };
```

然后把上面已经算出的 `let total_cost = general + labor;`（约第 120 行）修改为：

```rust
    let total_cost = general + labor + commission;
```

- [ ] **Step 2.4: 返回结构里带上 `commission_cents`**

把返回块（`Ok(ProjectFinancialSummary { ... })`, 约第 148-161 行）中 `total_cost_cents: total_cost,` 后插入：

```rust
        commission_cents: commission,
```

- [ ] **Step 2.5: 写失败测试**

在 `#[cfg(test)] mod tests` 里追加下面 5 个测试（放在 `financial_summary_excludes_soft_deleted` 之后）：

```rust
    #[test]
    fn commission_mode_none_yields_zero() {
        let db = TestDb::new();
        make_full_fixture(&db.conn);
        // fixture 未设 commission_mode，DB 默认为 'none'
        let s = project_financial_summary(&db.conn, 1).unwrap();
        assert_eq!(s.commission_cents, 0);
        // total_cost = 210_000（与原 full 用例一致）
        assert_eq!(s.total_cost_cents, 210_000);
    }

    #[test]
    fn commission_rate_scales_with_received() {
        let db = TestDb::new();
        make_full_fixture(&db.conn);
        db.conn
            .execute(
                "UPDATE projects SET commission_mode='rate', commission_rate=0.05
                 WHERE id = 1",
                [],
            )
            .unwrap();
        let s = project_financial_summary(&db.conn, 1).unwrap();
        // 已回款 500_000 × 5% = 25_000
        assert_eq!(s.commission_cents, 25_000);
        // total_cost = general(50_000) + labor(160_000) + commission(25_000)
        assert_eq!(s.total_cost_cents, 235_000);
    }

    #[test]
    fn commission_fixed_unsettled_ignored() {
        let db = TestDb::new();
        make_full_fixture(&db.conn);
        db.conn
            .execute(
                "UPDATE projects SET commission_mode='fixed',
                    commission_amount_cents=100000, commission_settled=0
                 WHERE id = 1",
                [],
            )
            .unwrap();
        let s = project_financial_summary(&db.conn, 1).unwrap();
        assert_eq!(s.commission_cents, 0);
        assert_eq!(s.total_cost_cents, 210_000);
    }

    #[test]
    fn commission_fixed_settled_counted() {
        let db = TestDb::new();
        make_full_fixture(&db.conn);
        db.conn
            .execute(
                "UPDATE projects SET commission_mode='fixed',
                    commission_amount_cents=100000, commission_settled=1
                 WHERE id = 1",
                [],
            )
            .unwrap();
        let s = project_financial_summary(&db.conn, 1).unwrap();
        assert_eq!(s.commission_cents, 100_000);
        assert_eq!(s.total_cost_cents, 310_000);
    }

    #[test]
    fn commission_rate_null_defaults_zero() {
        let db = TestDb::new();
        make_full_fixture(&db.conn);
        db.conn
            .execute(
                "UPDATE projects SET commission_mode='rate', commission_rate=NULL
                 WHERE id = 1",
                [],
            )
            .unwrap();
        let s = project_financial_summary(&db.conn, 1).unwrap();
        assert_eq!(s.commission_cents, 0);
    }
```

- [ ] **Step 2.6: 跑测试验证通过**

Run: `cd src-tauri && cargo test -p solo-cost-lib domain::profit::tests 2>&1 | tail -30`

Expected: 全部 PASS（含新增 5 个）。老的 `financial_summary_full_calculation` 因为 `total_cost_cents` 含 commission 但 fixture 里 mode='none' → commission=0，断言值不变，保持 PASS。

- [ ] **Step 2.7: 全库回归**

Run: `cd src-tauri && cargo test 2>&1 | tail -15`

Expected: 全部 PASS。

- [ ] **Step 2.8: Commit**

```bash
git add src-tauri/src/domain/profit.rs
git commit -m "$(cat <<'EOF'
feat(profit): 计算销售提成并合入 total_cost

project_financial_summary 增加 commission_cents 字段，rate
模式取 已回款(含税) × 提成率，fixed 模式在勾选已入账时计入。
毛利与利润率公式沿用现有逻辑（total_cost 已含提成）。
EOF
)"
```

---

## Task 3: Frontend — 类型 + i18n 文案

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/i18n/zh-CN.json`

**Interfaces:**
- Consumes: Task 1 后端字段
- Produces:
  - `Project` / `ProjectInput` / `ProjectFinancialSummary` 扩字段（Task 4/5 依赖）
  - i18n keys（Task 4/5 依赖）

- [ ] **Step 3.1: 扩展 TypeScript 类型**

在 `src/types/index.ts` 的 `interface Project`（约第 34-49 行）里，在 `notes` 之后（`created_at` 之前）插入：

```ts
  commission_mode: string;
  commission_rate: number | null;
  commission_amount_cents: number | null;
  commission_settled: boolean;
```

在 `interface ProjectInput`（约第 51-62 行）的 `notes` 之后追加：

```ts
  commission_mode?: string | null;
  commission_rate?: number | null;
  commission_amount_cents?: number | null;
  commission_settled?: boolean | null;
```

在 `interface ProjectFinancialSummary`（约第 192-204 行）的 `total_cost_cents` 之后追加：

```ts
  commission_cents: number;
```

- [ ] **Step 3.2: 加 i18n keys**

编辑 `src/i18n/zh-CN.json`：

- 在 `"project"` 对象（现有 keys 如 `client`、`contractAmount` 等）里追加：

```json
    "commissionSection": "销售提成",
    "commissionMode": "提成模式",
    "commissionRate": "提成率",
    "commissionAmount": "提成金额",
    "commissionSettled": "已入账",
```

- 在同文件顶层新增或扩充 `"commissionMode"` 对象（如已存在则并入）：

```json
  "commissionMode": {
    "none": "不算",
    "rate": "按回款比率",
    "fixed": "固定金额"
  },
```

- 在 `"financial"` 对象里追加：

```json
    "commission": "销售提成",
    "commissionRateFootnote": "已回款 × {{rate}}",
    "commissionFixedSettled": "固定 {{amount}} · 已入账",
    "commissionFixedUnsettled": "固定 {{amount}} · 未入账",
```

- [ ] **Step 3.3: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
pnpm exec tsc -b
echo "EXIT=$?"
```

Expected: `EXIT=0`。

- [ ] **Step 3.4: JSON 语法检查**

Run:
```bash
jq . src/i18n/zh-CN.json > /dev/null && echo OK
```

Expected: `OK`。

- [ ] **Step 3.5: Commit**

```bash
git add src/types/index.ts src/i18n/zh-CN.json
git commit -m "$(cat <<'EOF'
feat(types+i18n): 项目提成类型与文案

types 扩 Project/ProjectInput/ProjectFinancialSummary 提成字段，
zh-CN.json 加 project.commissionSection/commissionMode/
commissionRate 等 keys 与 financial.commission* 文案。
EOF
)"
```

---

## Task 4: Frontend — 项目表单加提成区块

**Files:**
- Modify: `src/routes/projects/list.tsx`（`ProjectForm` 组件）

**Interfaces:**
- Consumes: Task 3 的 `ProjectInput` 与 i18n keys
- Produces: 项目创建/编辑时携带提成字段的 `ProjectInput`

- [ ] **Step 4.1: 扩展 `ProjectForm` 内部 state**

在 `ProjectForm`（约第 144-181 行）里的 `const [notes, setNotes] = useState(...)` 后追加 4 行 state：

```tsx
  const [commissionMode, setCommissionMode] = useState(initial?.commission_mode ?? "none");
  const [commissionRate, setCommissionRate] = useState(
    initial?.commission_rate != null ? String(initial.commission_rate) : ""
  );
  const [commissionAmount, setCommissionAmount] = useState(initial?.commission_amount_cents ?? 0);
  const [commissionSettled, setCommissionSettled] = useState(initial?.commission_settled ?? false);
```

- [ ] **Step 4.2: submit 时按模式过滤字段**

把 `submit` 内 `onSubmit({ ... })` 调用改为在既有属性基础上追加提成映射：

```tsx
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
        commission_mode: commissionMode,
        commission_rate:
          commissionMode === "rate"
            ? (commissionRate === "" ? 0 : Number(commissionRate))
            : null,
        commission_amount_cents:
          commissionMode === "fixed" ? commissionAmount : null,
        commission_settled:
          commissionMode === "fixed" ? commissionSettled : false,
      });
```

- [ ] **Step 4.3: 引入 Checkbox 组件（若未导入）**

在 `list.tsx` 顶部的组件导入区确认或追加：

```tsx
import { Checkbox } from "@/components/ui/checkbox";
```

（组件已随 shadcn 存在于 `src/components/ui/`，若真不存在则改用 `<input type="checkbox">` 兜底，见 Step 4.4 备注。）

- [ ] **Step 4.4: 在合同区块下方加提成 UI**

在 `ProjectForm` 返回 JSX 的 `备注 Textarea` 之前插入：

```tsx
      <div className="space-y-2 rounded border p-3">
        <div className="text-sm font-medium">{t("project.commissionSection")}</div>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1">
            <Label>{t("project.commissionMode")}</Label>
            <Select value={commissionMode} onValueChange={setCommissionMode}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">{t("commissionMode.none")}</SelectItem>
                <SelectItem value="rate">{t("commissionMode.rate")}</SelectItem>
                <SelectItem value="fixed">{t("commissionMode.fixed")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {commissionMode === "rate" && (
            <div className="space-y-1">
              <Label>{t("project.commissionRate")}</Label>
              <Input
                type="number"
                step="0.01"
                min="0"
                max="1"
                value={commissionRate}
                onChange={(e) => setCommissionRate(e.target.value)}
                placeholder="0.05"
              />
            </div>
          )}
          {commissionMode === "fixed" && (
            <div className="space-y-1">
              <Label>{t("project.commissionAmount")}</Label>
              <MoneyInput value={commissionAmount} onChange={setCommissionAmount} />
            </div>
          )}
        </div>
        {commissionMode === "fixed" && (
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={commissionSettled}
              onCheckedChange={(v) => setCommissionSettled(!!v)}
            />
            {t("project.commissionSettled")}
          </label>
        )}
      </div>
```

**兜底**：如果 `@/components/ui/checkbox` 不存在，把 `<Checkbox ...>` 改为：

```tsx
<input
  type="checkbox"
  checked={commissionSettled}
  onChange={(e) => setCommissionSettled(e.target.checked)}
/>
```

- [ ] **Step 4.5: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
pnpm exec tsc -b
echo "EXIT=$?"
```

Expected: `EXIT=0`。

- [ ] **Step 4.6: 人工验收**

启动 `pnpm tauri dev`，逐条验证：

1. 打开"新建项目"对话框 → 提成区块默认 "不算"，无 rate/amount 输入。
2. 切到"按回款比率" → 出现提成率输入；切到"固定金额" → 出现金额输入 + "已入账" 复选框；切回"不算" → 输入全部收起。
3. 保存一个 rate=0.05 的项目 → 重新编辑该项目 → 提成模式回显 `rate`，提成率回显 `0.05`。
4. 保存一个 fixed amount=200 已入账 的项目 → 重新编辑 → 模式回显 `fixed`，金额 `¥2.00`，已入账勾选。

- [ ] **Step 4.7: Commit**

```bash
git add src/routes/projects/list.tsx
git commit -m "$(cat <<'EOF'
feat(projects): 项目表单加销售提成区块

模式三选一（不算/按回款比率/固定金额），fixed 模式带
已入账开关；模式切换时对应输入条件渲染，提交按模式
只发送相关字段，其余置 null。
EOF
)"
```

---

## Task 5: Frontend — 财务面板提成卡与自适应布局

**Files:**
- Modify: `src/routes/projects/detail.tsx`（`FinancialPanel` 组件）

**Interfaces:**
- Consumes: Task 2 的 `ProjectFinancialSummary.commission_cents`、Task 3 的 i18n keys
- Produces: UI only

- [ ] **Step 5.1: 定位并修改成本卡片行**

打开 `src/routes/projects/detail.tsx`，在 `FinancialPanel` 内部找到"costs"注释下的 `<div className="grid grid-cols-3 gap-3">` 块（当前包含"一般成本 / 人力成本 / 总成本"三张卡）。把该块整个替换为：

```tsx
      {/* costs */}
      <div
        className={`grid gap-3 ${project.commission_mode !== "none" ? "grid-cols-4" : "grid-cols-3"}`}
      >
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
        {project.commission_mode !== "none" && (
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("financial.commission")}</CardTitle></CardHeader>
            <CardContent className="text-xl font-semibold">
              {financial ? formatCNY(financial.commission_cents) : "—"}
              <div className="text-xs text-muted-foreground mt-1">
                {project.commission_mode === "rate" &&
                  t("financial.commissionRateFootnote", {
                    rate: `${((project.commission_rate ?? 0) * 100).toFixed(2)}%`,
                  })}
                {project.commission_mode === "fixed" &&
                  (project.commission_settled
                    ? t("financial.commissionFixedSettled", {
                        amount: formatCNY(project.commission_amount_cents ?? 0),
                      })
                    : t("financial.commissionFixedUnsettled", {
                        amount: formatCNY(project.commission_amount_cents ?? 0),
                      }))}
              </div>
            </CardContent>
          </Card>
        )}
        <Card>
          <CardHeader><CardTitle className="text-sm">{t("financial.totalCost")}</CardTitle></CardHeader>
          <CardContent className="text-xl font-semibold">
            {financial ? formatCNY(financial.total_cost_cents) : "—"}
          </CardContent>
        </Card>
      </div>
```

说明：`total_cost_cents` 直接来自后端（已含 commission），前端不做二次加算。

- [ ] **Step 5.2: 类型检查**

Run:
```bash
export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh" && nvm use default >/dev/null 2>&1
pnpm exec tsc -b
echo "EXIT=$?"
```

Expected: `EXIT=0`。

- [ ] **Step 5.3: 人工验收**

在运行中的 `pnpm tauri dev`：

1. 打开一个 `mode='none'` 的旧项目详情：成本行仍是 3 列，无提成卡；总成本值与旧行为一致。
2. 打开一个 `mode='rate'` rate=0.05 且已回款 100 元 的项目：成本行 4 列，提成卡显示 ¥5.00，副文 "已回款 × 5.00%"；总成本 = 一般 + 人力 + ¥5.00。
3. 打开一个 `mode='fixed'` amount=200 settled=false 的项目：提成卡 ¥0.00，副文 "固定 ¥2.00 · 未入账"；总成本不含提成。
4. 把上一项目的 settled 勾选后再进入详情：提成卡 ¥2.00，副文 "固定 ¥2.00 · 已入账"，总成本相应增加 200 分。
5. 项目列表页面样式与之前一致（未加提成信息）。

- [ ] **Step 5.4: Commit**

```bash
git add src/routes/projects/detail.tsx
git commit -m "$(cat <<'EOF'
feat(projects): 财务面板加销售提成卡与自适应布局

mode!==none 时成本卡片行由 3 列扩为 4 列并插入销售提成卡，
rate 模式副文显示"已回款 × 提成率"，fixed 模式显示金额与
入账状态；mode=none 沿用 3 列布局不影响旧项目。
EOF
)"
```

---

## Task 6: 更新 CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:** 无

- [ ] **Step 6.1: 加条目**

在 `CHANGELOG.md` 的 `## Unreleased → ### Added` 段末尾追加：

```markdown
- 项目支持销售提成配置：`不算` / `按已回款含税额 × 提成率` / `固定金额 + 手工入账` 三种模式，后端自动计入 `total_cost`，项目详情财务面板独立展示"销售提成"卡
```

- [ ] **Step 6.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): 记录项目销售提成条目"
```

---

## 完成后

- [ ] 全局 `cargo test` + `pnpm exec tsc -b` 通过
- [ ] `pnpm tauri dev` 手工回归 Task 4/5 的 4+5 项验收清单
- [ ] 询问用户是否合并到主分支 / 推送
