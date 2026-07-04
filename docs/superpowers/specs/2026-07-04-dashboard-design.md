# 仪表盘（Dashboard）设计

日期：2026-07-04
状态：待评审

## 目标

把当前只显示公司名的空壳仪表盘，做成「个人开发者的生意一览」。核心是站在个人开发者视角看钱——人力成本其实是自己的收入，所以除了标准毛利，还要一个「到手」口径（不含税收入 − 销售提成 − 非人力成本）。

面向单一用户（老板即开发者），只读展示，数据来自当前公司下的全部项目。

## 范围与口径决策（已与用户确认）

- 金额汇总纳入**全部项目**（不按状态排除）。
- 大盘同时给**合同口径**（全量潜在）和**已收口径**（真实落袋）两套数字。
- 「到手」= **不含税收入 − 提成 − 非人力成本**（人力成本不扣，算作个人收入）。
- 「按年到手」按**到账年份**归集，成本与提成**同年扣**。
- 排行按**到手**降序。
- 按年图**叠加「已收(不含税)」做对照**。
- Widget **全上**；一屏放不下就**分 Tab**。

## 指标定义（精确公式）

每个项目先归一化：

- `含税额 inc` = `is_tax_inclusive ? contract_amount : contract_amount × (1 + tax_rate)`
- `不含税额 exc` = `inc / (1 + tax_rate)`（等于 `profit.rs` 的 `revenue_tax_exclusive`）

### 合同口径（全部项目求和）

| 指标 | 公式 |
|------|------|
| 合同总额(含税) | Σ `inc` |
| 不含税收入 | Σ `exc` |
| 潜在提成 | rate: `inc × commission_rate`；fixed: `commission_amount`（已配置即计，不看 settled）；none: 0 |
| 非人力成本 | Σ `cost_entries.amount`（未软删，全部） |
| **潜在到手** | 不含税收入 − 潜在提成 − 非人力成本 |

### 已收口径

| 指标 | 公式 |
|------|------|
| 已收(含税) | Σ 已到账收款节点 `actual_amount`（`actual_received_at IS NOT NULL`） |
| 已收(不含税) | 按各项目税率折算求和：Σ_项目 (该项目已收含税 / (1+tax_rate)) |
| 未收/应收 | 合同总额(含税) − 已收(含税) |
| 已入账提成 | rate: `已收含税 × commission_rate`；fixed: `commission_settled ? commission_amount : 0`（与 `profit.rs` 一致） |
| **已收到手** | 已收(不含税) − 已入账提成 − 非人力成本(全部已发生) |

> 注：已收到手扣的是「已发生的全部非人力成本」，回款早期可能为负，属正常（表示尚未回本）。

### 按年到手（按到账年份分桶）

对每个已到账收款节点，取 `actual_received_at` 的年份 Y：

- 当年实收(含税) `+= actual_amount`
- 当年实收(不含税) `+= actual_amount / (1 + 该项目 tax_rate)`
- 当年非人力成本 = Σ `cost_entries`（`incurred_at` 落在 Y 年）
- 当年提成：
  - rate: `当年实收含税 × commission_rate`
  - fixed: 按当年实收占该项目**总实收**的比例摊：`(该项目当年实收 / 该项目总实收) × (settled ? amount : 0)`；总实收为 0 时不摊
  - none: 0
- **当年到手** = 当年实收(不含税) − 当年非人力成本 − 当年提成

按年图每年展示两个值：**到手**（主）与 **已收(不含税)**（对照）。

### 项目状态分布

各 `status`（商务洽谈/待启动/进行中/已交付待结款/已结款/已归档）：项目数 `count` + 合同额(含税)合计 Σ`inc`。展示全部状态（含归档），与金额纳入范围无关。

### 应收提醒

未到账收款节点（`actual_received_at IS NULL` 且 `expected_date` 非空），按 `expected_date` 相对今天分三档：

- `overdue`：`expected_date < today`（标红）
- `soon`：`today ≤ expected_date ≤ today+30d`（标黄）
- `future`：其余

顶部合计未收 = Σ 未到账节点 `expected_amount`。列表按 `expected_date` 升序，逾期在前。

### 排行（按到手降序，Top 5）

- **客户排行**：按客户聚合各自项目的「已收到手」，降序取前 5。`client_id` 为空的项目归「未分配」。
- **项目排行**：按项目「已收到手」降序取前 5。

每行同时显示到手与已收(含税)供参照。

## 后端设计

新增纯函数 `domain/dashboard.rs`：

```
pub fn company_dashboard(conn, company_id, today: &str) -> AppResult<DashboardSummary>
```

`today` 由命令层用 SQL `date('now','localtime')` 取，便于单测注入固定日期。

```
struct DashboardSummary {
  // 合同口径
  contract_total_inclusive_cents: i64,
  revenue_exclusive_cents: i64,
  commission_potential_cents: i64,
  general_cost_cents: i64,
  net_potential_cents: i64,
  // 已收口径
  received_inclusive_cents: i64,
  received_exclusive_cents: i64,
  outstanding_cents: i64,
  commission_realized_cents: i64,
  net_realized_cents: i64,
  // 明细
  by_year: Vec<YearRow>,          // 按年份升序
  by_status: Vec<StatusRow>,      // 固定状态顺序
  receivables: Vec<ReceivableRow>,
  receivables_outstanding_cents: i64,
  top_clients: Vec<RankRow>,      // 到手降序 ≤5
  top_projects: Vec<RankRow>,     // 到手降序 ≤5
}
struct YearRow { year: i32, received_exclusive_cents, general_cost_cents, commission_cents, net_cents }
struct StatusRow { status: String, count: i64, contract_inclusive_cents: i64 }
struct ReceivableRow { project_id, project_name, name, expected_amount_cents, expected_date, bucket: String }
struct RankRow { id: i64, name: String, net_cents: i64, received_inclusive_cents: i64 }
```

实现方式：查询当前公司未软删项目及其收款节点、成本，按项目遍历一次累加各口径与年份桶，复用 `profit.rs` 已有的归一化/提成逻辑思路（不强行共享代码，避免耦合）。命令层 `commands/dashboard.rs` 暴露 `get_dashboard(company_id) -> DashboardSummary`，在 `lib.rs` 注册。

单测（仿 `profit.rs`）：空公司全 0；单项目合同/已收/到手正确；跨年分桶；rate/fixed 提成摊分；应收分档（注入固定 today）；排行截断与排序。

## 前端设计

- `stores/dashboard.ts`：按 `company_id` 缓存 `DashboardSummary`，切换公司/锁定时 reset（与其他 store 一致）。
- 重写 `routes/dashboard.tsx`，分 Tab（放不下就分，预期会分）：
  - **总览**：钱大盘 KPI（合同口径 + 已收口径两组卡）+ 按年到手（到手/已收对照条形）。
  - **分布 & 排行**：项目状态分布 + 客户 Top5 + 项目 Top5。
  - **应收**：应收提醒列表（逾期红/近 30 天黄）。
- 图形用轻量 CSS 条形（`div` 宽度按比例），**不引入图表依赖**。
- 金额统一用现有 `formatCNY`；文案入 `i18n` 的 `dashboard.*` 命名空间。

## 边界与约定

- 公司无项目：所有数字为 0，各列表为空，展示友好空态。
- 项目税率为 0：折算 `/(1+0)` 即含税=不含税，正常。
- 收款节点无 `expected_date`：不进应收提醒列表（无法判断到期）。
- 到手可能为负：如实展示，不裁剪为 0。
- 金额一律以「分」为单位在后端计算，前端只格式化。

## 不做（YAGNI）

- 不做自定义时间范围筛选、导出、图表库、跨公司汇总。
- 不做实时刷新；进入页面加载一次，公司切换重载。
