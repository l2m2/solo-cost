# 项目销售提成 设计文档

- **创建日期**：2026-07-02
- **作者**：l2m2
- **状态**：设计已定稿，待实现
- **关联主设计**：[2026-06-29-solo-cost-design.md](2026-06-29-solo-cost-design.md)

---

## 1. 背景与目标

现有项目财务模型（`domain/profit.rs::project_financial_summary`）只把成本拆成两类：

- `general_cost_cents` = `SUM(cost_entries.amount_cents)`
- `labor_cost_cents` = Σ `hours / 8 × daily_cost_snapshot_cents`

**缺口**：多数项目附带销售提成，但目前没有专门的字段/公式承载，用户只能作为通用成本条目手工录入，且金额需要自己按合同或回款算。

**目标**：让项目直接持有提成配置，`total_cost` 派生时自动包含提成金额，且支持每个项目独立选择计算方式。

## 2. 用户诉求

来源：l2m2 的真实业务口径，2026-07-02 对齐。

- **大多数项目**：按合同含税金额约定提成率；但**实际入账金额随回款推进**（不是签合同就一次性入账）。
- **少数项目**：谈定一个具体的提成金额（固定值），用户自己决定什么时候入账。
- **其他项目**：不算提成。
- 不需要挂到某个成员或销售人——只关心金额本身。

## 3. 领域模型

### 3.1 三种模式

| `commission_mode` | 语义 | 计算 |
|---|---|---|
| `none` | 不算提成（默认） | `commission = 0` |
| `rate` | 按已回款含税额 × 提成率 | `commission = actual_payment_cents × commission_rate` |
| `fixed` | 固定金额 + 手工入账开关 | `commission = commission_settled ? commission_amount_cents : 0` |

**说明**：
- `rate` 模式下，提成随收款节点实收进度累计，不是签合同就锁定；不需要用户手动确认入账。
- `fixed` 模式下，何时"计入 total_cost"由用户自己控制（勾选 `commission_settled`），因为固定金额和收款进度可以完全解耦。

### 3.2 与现有成本模型的关系

- 提成**不写入** `cost_entries` 表，而是作为 `projects` 的独立字段派生。
- 好处：无需在项目字段变化时同步生成/删除 `cost_entries` 行，避免数据一致性问题。
- 代价：按科目汇总里不会出现提成分类——通过财务面板单独一张卡展示。

### 3.3 计算基数细节

`rate` 模式的基数是**当前已实际回款（含税）**，来源 SQL 与现有 `actual_payment_cents` 一致：

```sql
SELECT COALESCE(SUM(actual_amount_cents), 0)
FROM contract_payments
WHERE project_id = ?1 AND deleted_at IS NULL
  AND actual_received_at IS NOT NULL;
```

即 `contract_payments` 中 `actual_received_at` 非空且未软删除的行的 `actual_amount_cents` 之和。

## 4. 数据库迁移

新增文件：`src-tauri/migrations/0004_projects_commission.sql`

```sql
-- M+: project commission fields
BEGIN;

ALTER TABLE projects ADD COLUMN commission_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE projects ADD COLUMN commission_rate REAL;
ALTER TABLE projects ADD COLUMN commission_amount_cents INTEGER;
ALTER TABLE projects ADD COLUMN commission_settled INTEGER NOT NULL DEFAULT 0;

COMMIT;
```

**兼容性**：
- 所有旧行自动落入 `commission_mode = 'none'`，`commission_cents` 计算结果为 0，财务面板行为不变。
- 不加 CHECK 约束（SQLite ALTER TABLE 添加 CHECK 麻烦），改由 Rust 层白名单校验。

## 5. 后端变更

### 5.1 `Project` 结构体（`commands/projects.rs`）

新增 4 个字段（对应表列）：

```rust
pub struct Project {
    // ...existing...
    pub commission_mode: String,               // "none" | "rate" | "fixed"
    pub commission_rate: Option<f64>,          // 0..1
    pub commission_amount_cents: Option<i64>,  // ≥ 0
    pub commission_settled: bool,
}
```

`ProjectInput` 同样新增这 4 个字段作为可选/默认 `none`。

### 5.2 校验（`commands/projects.rs::validate`）

在现有校验函数里追加：

```rust
match input.commission_mode.as_str() {
    "none" => { /* ok */ }
    "rate" => {
        let r = input.commission_rate.unwrap_or(0.0);
        if !(0.0..=1.0).contains(&r) {
            return Err(AppError::Validation(
                "提成率必须在 0–1 之间".into(),
            ));
        }
    }
    "fixed" => {
        let a = input.commission_amount_cents.unwrap_or(0);
        if a < 0 {
            return Err(AppError::Validation(
                "固定提成金额必须 ≥ 0".into(),
            ));
        }
    }
    _ => return Err(AppError::Validation("提成模式不合法".into())),
}
```

对非当前模式的字段，后端**不清零**——保留用户先前填的值，UI 层负责在提交时清理（见 §6.4）。

### 5.3 `project_financial_summary`（`domain/profit.rs`）

- `ProjectFinancialSummary` 新增字段：

  ```rust
  pub commission_cents: i64,
  ```

- 把现有 `SELECT contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate FROM projects` 扩展成也读 `commission_mode, commission_rate, commission_amount_cents, commission_settled`（一次查询、同一行返回）。变量命名区分：现有 `rate: f64` 是税率，新增 `comm_rate: Option<f64>`、`comm_mode: String`、`comm_amount: Option<i64>`、`comm_settled: bool`（`INTEGER != 0`）。
- 在计算完 `actual` 之后追加：

  ```rust
  let commission = match comm_mode.as_str() {
      "rate"  => (actual as f64 * comm_rate.unwrap_or(0.0)).round() as i64,
      "fixed" => if comm_settled { comm_amount.unwrap_or(0) } else { 0 },
      _       => 0, // "none" and any other
  };

  let total_cost = general + labor + commission;
  ```

  其余（`gross`、`profit_rate`）沿用现有公式——`total_cost` 已包含 commission，毛利/利润率无需改。

### 5.4 单元测试

在 `domain/profit.rs` 的 `#[cfg(test)] mod tests` 中新增：

| 用例 | 断言 |
|---|---|
| `commission_mode_none_yields_zero` | 现有 `financial_summary_full_calculation` 里若显式设 `mode='none'`，commission=0，total_cost 不变 |
| `commission_rate_scales_with_received` | rate=0.05，actual=500,000 → commission=25,000；actual=0 → 0 |
| `commission_fixed_unsettled_ignored` | mode=fixed, amount=100,000, settled=0 → commission=0 |
| `commission_fixed_settled_counted` | mode=fixed, amount=100,000, settled=1 → commission=100,000 |
| `commission_rate_null_defaults_zero` | mode=rate 但 commission_rate IS NULL → 防御性返回 0 |

## 6. 前端变更

### 6.1 类型定义（`src/types/`）

`Project` 与 `ProjectInput` 同步新增 4 个字段，类型与后端一致。

### 6.2 Store（`src/stores/projects.ts`）

无结构性改动：`update`/`create` 已直接透传整个 `ProjectInput`，字段扩展后自动兼容。

### 6.3 项目表单（`src/routes/projects/list.tsx::ProjectForm`）

在合同区块下方加"销售提成"区块：

```
┌ 销售提成 ─────────────────────────┐
│  模式  [不算 ▾]                     │
│  ── mode='rate' 时：                │
│  提成率 [___] %                     │
│  ── mode='fixed' 时：               │
│  提成金额 [___]   ☐ 已入账          │
└─────────────────────────────────────┘
```

**交互规则**：
- 模式切换时，把另外两栏的表单 state 清零，避免脏值：
  - 切到 `none` → rate/amount/settled 都清零
  - 切到 `rate` → amount/settled 清零；rate 若原值 null 则 default 0
  - 切到 `fixed` → rate 清零；amount 若原值 null 则 default 0，settled 默认 false
- 提交时按当前模式只发送相关字段值，其余显式传 `null` / `false`。

### 6.4 项目详情财务面板（`routes/projects/detail.tsx::FinancialPanel`）

- **`mode !== 'none'`** 时，把原来的 `grid-cols-3` 成本卡片行扩成 `grid-cols-4`，顺序为「一般成本 / 人力成本 / 销售提成 / 总成本」；`mode === 'none'` 保持原 3 列布局。提成卡样例：

  ```
  ┌──────────────────────────────┐
  │  销售提成                     │
  │  ¥1,234.56                    │
  │  已回款 × 3.00%               │  ← mode=rate 副文
  │  固定 ¥5,000 · 未入账          │  ← mode=fixed 副文
  └──────────────────────────────┘
  ```

- 总成本卡直接展示 `financial.total_cost_cents`——后端已合并，不需要前端加算。
- `mode='none'` 时**不渲染**这张卡，避免视觉噪声。

### 6.5 项目列表

暂不显示提成信息。理由：列表以合同额、状态为主，加入提成会稀释信息密度；用户想看请点入详情。

## 7. i18n

`src/i18n/{zh,en}.ts` 新增 key：

| key | zh | en |
|---|---|---|
| `project.commissionSection` | 销售提成 | Sales commission |
| `project.commissionMode` | 提成模式 | Commission mode |
| `commissionMode.none` | 不算 | None |
| `commissionMode.rate` | 按回款比率 | Rate on received |
| `commissionMode.fixed` | 固定金额 | Fixed amount |
| `project.commissionRate` | 提成率 | Commission rate |
| `project.commissionAmount` | 提成金额 | Commission amount |
| `project.commissionSettled` | 已入账 | Settled |
| `financial.commission` | 销售提成 | Sales commission |
| `financial.commissionRateFootnote` | 已回款 × {{rate}} | Received × {{rate}} |
| `financial.commissionFixedSettled` | 固定 {{amount}} · 已入账 | Fixed {{amount}} · settled |
| `financial.commissionFixedUnsettled` | 固定 {{amount}} · 未入账 | Fixed {{amount}} · unsettled |

## 8. 非目标 / 明确不做

以下场景本次不覆盖，保留给后续：

- 挂到某个成员 / 销售人（多销售人分成）
- 阶梯提成（按回款额区间不同费率）
- 与单个 `contract_payments` 节点一一绑定
- 提成的历史记账流水（本设计里 rate 模式随回款动态派生，不产生流水）
- 已入账的固定提成显示"入账日期"

## 9. 验收标准

- [ ] 创建项目时选择 `mode='rate'`，rate=5%，添加 100 元实收 → 财务面板"销售提成"卡显示 ¥5.00；总成本增加 500 分
- [ ] 创建项目时选择 `mode='fixed'`，amount=200，settled=0 → 提成 ¥0；勾选已入账 → 提成 ¥200
- [ ] 老项目（迁移前）打开财务面板：无提成卡，`total_cost` 与旧版本一致
- [ ] 提成模式切换后，非当前模式的字段在下次编辑打开时呈现清零状态
- [ ] rate 模式 rate=1.5 或负数 → 后端拒绝并给出"提成率必须在 0–1 之间"提示
- [ ] Rust `cargo test` 5 个新测试全部通过

## 10. 影响面清单

| 层 | 文件 | 变更类型 |
|---|---|---|
| DB | `src-tauri/migrations/0004_projects_commission.sql` | 新建 |
| Rust | `src-tauri/src/commands/projects.rs` | 结构体扩字段 + 校验 + SELECT/INSERT/UPDATE 语句 |
| Rust | `src-tauri/src/domain/profit.rs` | 计算 + 结构体扩字段 + 5 个新测试 |
| TS | `src/types/*.ts` | `Project` / `ProjectInput` 扩字段 |
| TSX | `src/routes/projects/list.tsx` | `ProjectForm` 加提成区块 |
| TSX | `src/routes/projects/detail.tsx` | `FinancialPanel` 加提成卡 |
| i18n | `src/i18n/zh.ts` / `en.ts` | 新增文案 keys |
