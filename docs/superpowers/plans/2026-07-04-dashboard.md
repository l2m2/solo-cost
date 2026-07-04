# 仪表盘（Dashboard）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把空壳仪表盘做成公司级「生意一览」：合同/已收两套口径的钱大盘、到手净收入、按年到手（叠加已收对照）、项目状态分布、应收提醒、按到手排行，分 Tab 展示。

**Architecture:** 后端新增纯函数 `domain/dashboard.rs::company_dashboard` 一次性聚合整个公司数据，命令 `get_dashboard` 暴露；前端新增 store 拉取，重写 `routes/dashboard.tsx` 用 Tabs + 轻量 CSS 条形展示，不引入图表依赖。

**Tech Stack:** Rust + rusqlite（SQLCipher）、React + TypeScript、zustand、shadcn UI、i18next。

## Global Constraints

- 金额一律以「分」(i64/number) 在后端计算，前端仅用 `formatCNY` 格式化。
- 不新增任何依赖（图表用 CSS `div` 宽度条形）。
- 代码注释用英文；i18n 文案用中文，放 `dashboard.*` 命名空间。
- 「到手」= 不含税收入 − 提成 − 非人力成本（人力成本不扣）。
- 金额纳入**全部项目**（不按状态排除）。
- 提成口径与 `profit.rs` 一致：rate = 实收(含税)×率；fixed = 已入账才算（潜在口径除外，潜在口径 fixed 只要配置了金额就算）。
- Commit 信息遵循 Conventional Commits，结尾加 `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`。

参考规格：`docs/superpowers/specs/2026-07-04-dashboard-design.md`。

---

### Task 1: 后端聚合域函数 `domain/dashboard.rs`

**Files:**
- Create: `src-tauri/src/domain/dashboard.rs`
- Modify: `src-tauri/src/domain/mod.rs`（加 `pub mod dashboard;`）
- Test: 同文件 `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub struct DashboardSummary { contract_total_inclusive_cents: i64, revenue_exclusive_cents: i64, commission_potential_cents: i64, general_cost_cents: i64, net_potential_cents: i64, received_inclusive_cents: i64, received_exclusive_cents: i64, outstanding_cents: i64, commission_realized_cents: i64, net_realized_cents: i64, by_year: Vec<YearRow>, by_status: Vec<StatusRow>, receivables: Vec<ReceivableRow>, receivables_outstanding_cents: i64, top_clients: Vec<RankRow>, top_projects: Vec<RankRow> }`
  - `pub struct YearRow { year: i32, received_exclusive_cents: i64, general_cost_cents: i64, commission_cents: i64, net_cents: i64 }`
  - `pub struct StatusRow { status: String, count: i64, contract_inclusive_cents: i64 }`
  - `pub struct ReceivableRow { project_id: i64, project_name: String, name: String, expected_amount_cents: i64, expected_date: String, bucket: String }`
  - `pub struct RankRow { id: i64, name: String, net_cents: i64, received_inclusive_cents: i64 }`
  - `pub fn company_dashboard(conn: &Connection, company_id: i64, today: &str) -> AppResult<DashboardSummary>`

- [ ] **Step 1: 注册模块**

Modify `src-tauri/src/domain/mod.rs` — 按字母序插入：

```rust
pub mod backup;
pub mod dashboard;
pub mod module_stats;
pub mod profit;
pub mod soft_delete;
```

- [ ] **Step 2: 写失败测试**

Create `src-tauri/src/domain/dashboard.rs`，先只写测试骨架（结构与函数尚未定义，编译应失败）：

```rust
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

// (implementation added in Step 4)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb {
        conn: Connection,
        _dir: TempDir,
    }

    // company 1；clients: 1='Alpha'；
    // project 1 'PA' client1 in_progress 含税¥10,000 税0 提成none
    // project 2 'PB' 无client settled 不含税¥5,000 税0 提成rate 10%
    fn seed(conn: &Connection) {
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO clients(company_id, name) VALUES(1, 'Alpha')", []).unwrap();
        conn.execute(
            "INSERT INTO projects(company_id, name, client_id, status,
                 contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
                 commission_mode)
             VALUES(1, 'PA', 1, 'in_progress', 1000000, 1, 0.0, 'none')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO projects(company_id, name, client_id, status,
                 contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
                 commission_mode, commission_rate)
             VALUES(1, 'PB', NULL, 'settled', 500000, 0, 0.0, 'rate', 0.10)",
            [],
        ).unwrap();
        // cost categories + entries: PA 2025 ¥100, 2026 ¥200
        conn.execute("INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(1,'杂',1,0)", []).unwrap();
        conn.execute(
            "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(1, 1, '2025-03-01', 10000), (1, 1, '2026-03-01', 20000)",
            [],
        ).unwrap();
        // payments: PA received 2025 ¥4000 + 2026 ¥3000, plus 1 unpaid overdue node ¥3000
        conn.execute(
            "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                 actual_amount_cents, actual_received_at, expected_date)
             VALUES(1,'一期',400000,400000,'2025-05-01',NULL),
                    (1,'二期',300000,300000,'2026-02-01',NULL),
                    (1,'尾款',300000,NULL,NULL,'2026-06-01')",
            [],
        ).unwrap();
        // PB received 2026 ¥5000
        conn.execute(
            "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                 actual_amount_cents, actual_received_at, expected_date)
             VALUES(2,'全款',500000,500000,'2026-04-01',NULL)",
            [],
        ).unwrap();
    }

    fn new_db() -> TestDb {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        seed(&conn);
        TestDb { conn, _dir: dir }
    }

    #[test]
    fn dashboard_totals_and_buckets() {
        let db = new_db();
        let d = company_dashboard(&db.conn, 1, "2026-07-04").unwrap();

        // contract scope
        assert_eq!(d.contract_total_inclusive_cents, 1_500_000);
        assert_eq!(d.revenue_exclusive_cents, 1_500_000);
        assert_eq!(d.commission_potential_cents, 50_000); // PB 500000*0.10
        assert_eq!(d.general_cost_cents, 30_000);
        assert_eq!(d.net_potential_cents, 1_420_000);

        // received scope
        assert_eq!(d.received_inclusive_cents, 1_200_000);
        assert_eq!(d.received_exclusive_cents, 1_200_000);
        assert_eq!(d.outstanding_cents, 300_000);
        assert_eq!(d.commission_realized_cents, 50_000);
        assert_eq!(d.net_realized_cents, 1_120_000);

        // by_year: 2025 net 390_000, 2026 net 730_000
        assert_eq!(d.by_year.len(), 2);
        assert_eq!(d.by_year[0].year, 2025);
        assert_eq!(d.by_year[0].net_cents, 390_000);
        assert_eq!(d.by_year[0].received_exclusive_cents, 400_000);
        assert_eq!(d.by_year[1].year, 2026);
        assert_eq!(d.by_year[1].received_exclusive_cents, 800_000);
        assert_eq!(d.by_year[1].commission_cents, 50_000);
        assert_eq!(d.by_year[1].net_cents, 730_000);

        // by_status ordered
        assert_eq!(d.by_status.len(), 2);
        assert_eq!(d.by_status[0].status, "in_progress");
        assert_eq!(d.by_status[0].count, 1);
        assert_eq!(d.by_status[0].contract_inclusive_cents, 1_000_000);
        assert_eq!(d.by_status[1].status, "settled");

        // receivables: 1 overdue node ¥3000
        assert_eq!(d.receivables.len(), 1);
        assert_eq!(d.receivables[0].bucket, "overdue");
        assert_eq!(d.receivables_outstanding_cents, 300_000);

        // rankings by net desc: PA net 670_000 > PB net 450_000
        assert_eq!(d.top_projects[0].name, "PA");
        assert_eq!(d.top_projects[0].net_cents, 670_000);
        assert_eq!(d.top_projects[1].name, "PB");
        assert_eq!(d.top_clients[0].name, "Alpha");
        assert_eq!(d.top_clients[0].net_cents, 670_000);
        assert_eq!(d.top_clients[1].name, "未分配");
    }

    #[test]
    fn empty_company_all_zero() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        let d = company_dashboard(&conn, 1, "2026-07-04").unwrap();
        assert_eq!(d.contract_total_inclusive_cents, 0);
        assert_eq!(d.net_realized_cents, 0);
        assert!(d.by_year.is_empty());
        assert!(d.receivables.is_empty());
        assert!(d.top_projects.is_empty());
    }

    #[test]
    fn receivable_bucket_soon_and_future() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1,'P')", []).unwrap();
        conn.execute(
            "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                 actual_received_at, expected_date)
             VALUES(1,'逾期',100,NULL,'2026-06-01'),
                    (1,'近期',100,NULL,'2026-07-20'),
                    (1,'未来',100,NULL,'2026-12-01')",
            [],
        ).unwrap();
        let d = company_dashboard(&conn, 1, "2026-07-04").unwrap();
        assert_eq!(d.receivables.len(), 3);
        // sorted asc by date
        assert_eq!(d.receivables[0].bucket, "overdue");
        assert_eq!(d.receivables[1].bucket, "soon");   // within +30d (<=2026-08-03)
        assert_eq!(d.receivables[2].bucket, "future");
    }
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib dashboard 2>&1 | tail -20`
Expected: 编译失败 —「cannot find function `company_dashboard`」等。

- [ ] **Step 4: 写最小实现**

在 `src-tauri/src/domain/dashboard.rs` 顶部（测试模块之前）加入完整实现：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct YearRow {
    pub year: i32,
    pub received_exclusive_cents: i64,
    pub general_cost_cents: i64,
    pub commission_cents: i64,
    pub net_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusRow {
    pub status: String,
    pub count: i64,
    pub contract_inclusive_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceivableRow {
    pub project_id: i64,
    pub project_name: String,
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: String,
    pub bucket: String, // "overdue" | "soon" | "future"
}

#[derive(Debug, Clone, Serialize)]
pub struct RankRow {
    pub id: i64,
    pub name: String,
    pub net_cents: i64,
    pub received_inclusive_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummary {
    pub contract_total_inclusive_cents: i64,
    pub revenue_exclusive_cents: i64,
    pub commission_potential_cents: i64,
    pub general_cost_cents: i64,
    pub net_potential_cents: i64,
    pub received_inclusive_cents: i64,
    pub received_exclusive_cents: i64,
    pub outstanding_cents: i64,
    pub commission_realized_cents: i64,
    pub net_realized_cents: i64,
    pub by_year: Vec<YearRow>,
    pub by_status: Vec<StatusRow>,
    pub receivables: Vec<ReceivableRow>,
    pub receivables_outstanding_cents: i64,
    pub top_clients: Vec<RankRow>,
    pub top_projects: Vec<RankRow>,
}

const STATUS_ORDER: [&str; 6] = [
    "negotiating", "pending", "in_progress", "delivered", "settled", "archived",
];

struct Proj {
    id: i64,
    name: String,
    client_id: Option<i64>,
    client_name: Option<String>,
    status: String,
    contract: i64,
    inclusive: bool,
    rate: f64,
    comm_mode: String,
    comm_rate: Option<f64>,
    comm_amount: Option<i64>,
    comm_settled: bool,
}

pub fn company_dashboard(
    conn: &Connection,
    company_id: i64,
    today: &str,
) -> AppResult<DashboardSummary> {
    // 1) load projects (LEFT JOIN clients for name)
    let mut pstmt = conn.prepare(
        "SELECT p.id, p.name, p.client_id, c.name, p.status,
                p.contract_amount_cents, p.contract_amount_is_tax_inclusive, p.tax_rate,
                p.commission_mode, p.commission_rate, p.commission_amount_cents, p.commission_settled
         FROM projects p
         LEFT JOIN clients c ON c.id = p.client_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL
         ORDER BY p.id",
    )?;
    let projects: Vec<Proj> = pstmt
        .query_map([company_id], |r| {
            Ok(Proj {
                id: r.get(0)?,
                name: r.get(1)?,
                client_id: r.get(2)?,
                client_name: r.get(3)?,
                status: r.get(4)?,
                contract: r.get(5)?,
                inclusive: r.get::<_, i64>(6)? != 0,
                rate: r.get(7)?,
                comm_mode: r.get(8)?,
                comm_rate: r.get(9)?,
                comm_amount: r.get(10)?,
                comm_settled: r.get::<_, i64>(11)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // +30d cutoff for "soon"
    let soon_cutoff: String =
        conn.query_row("SELECT date(?1, '+30 days')", [today], |r| r.get(0))?;

    // company-wide non-labor cost by year
    let mut cost_by_year: HashMap<i32, i64> = HashMap::new();
    let mut cystmt = conn.prepare(
        "SELECT CAST(substr(ce.incurred_at,1,4) AS INTEGER) AS y, COALESCE(SUM(ce.amount_cents),0)
         FROM cost_entries ce JOIN projects p ON p.id = ce.project_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND ce.deleted_at IS NULL
         GROUP BY y",
    )?;
    for row in cystmt.query_map([company_id], |r| Ok((r.get::<_, i64>(0)? as i32, r.get::<_, i64>(1)?)))? {
        let (y, amt) = row?;
        cost_by_year.insert(y, amt);
    }

    let mut out = DashboardSummary {
        contract_total_inclusive_cents: 0,
        revenue_exclusive_cents: 0,
        commission_potential_cents: 0,
        general_cost_cents: 0,
        net_potential_cents: 0,
        received_inclusive_cents: 0,
        received_exclusive_cents: 0,
        outstanding_cents: 0,
        commission_realized_cents: 0,
        net_realized_cents: 0,
        by_year: Vec::new(),
        by_status: Vec::new(),
        receivables: Vec::new(),
        receivables_outstanding_cents: 0,
        top_clients: Vec::new(),
        top_projects: Vec::new(),
    };

    let mut year_recv_exc: HashMap<i32, i64> = HashMap::new();
    let mut year_commission: HashMap<i32, i64> = HashMap::new();
    let mut year_seen: HashMap<i32, ()> = HashMap::new();
    let mut status_count: HashMap<String, i64> = HashMap::new();
    let mut status_inc: HashMap<String, i64> = HashMap::new();
    let mut client_net: HashMap<i64, i64> = HashMap::new();
    let mut client_recv: HashMap<i64, i64> = HashMap::new();
    let mut client_name: HashMap<i64, String> = HashMap::new();
    let mut project_ranks: Vec<RankRow> = Vec::new();

    let mut paystmt = conn.prepare(
        "SELECT actual_amount_cents, actual_received_at
         FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL AND actual_received_at IS NOT NULL",
    )?;
    let mut coststmt = conn.prepare(
        "SELECT COALESCE(SUM(amount_cents),0) FROM cost_entries
         WHERE project_id = ?1 AND deleted_at IS NULL",
    )?;

    for p in &projects {
        let one_plus = 1.0 + p.rate;
        let inc = if p.inclusive { p.contract } else { (p.contract as f64 * one_plus).round() as i64 };
        let exc = if p.inclusive { (p.contract as f64 / one_plus).round() as i64 } else { p.contract };
        let general: i64 = coststmt.query_row([p.id], |r| r.get(0))?;

        let pays: Vec<(i64, String)> = paystmt
            .query_map([p.id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let recv_inc: i64 = pays.iter().map(|(a, _)| *a).sum();
        let recv_exc: i64 = (recv_inc as f64 / one_plus).round() as i64;

        let comm_potential = match p.comm_mode.as_str() {
            "rate" => (inc as f64 * p.comm_rate.unwrap_or(0.0)).round() as i64,
            "fixed" => p.comm_amount.unwrap_or(0),
            _ => 0,
        };
        let comm_realized = match p.comm_mode.as_str() {
            "rate" => (recv_inc as f64 * p.comm_rate.unwrap_or(0.0)).round() as i64,
            "fixed" => if p.comm_settled { p.comm_amount.unwrap_or(0) } else { 0 },
            _ => 0,
        };

        out.contract_total_inclusive_cents += inc;
        out.revenue_exclusive_cents += exc;
        out.commission_potential_cents += comm_potential;
        out.general_cost_cents += general;
        out.received_inclusive_cents += recv_inc;
        out.received_exclusive_cents += recv_exc;
        out.commission_realized_cents += comm_realized;

        *status_count.entry(p.status.clone()).or_insert(0) += 1;
        *status_inc.entry(p.status.clone()).or_insert(0) += inc;

        // per-year received for this project
        let mut proj_year_inc: HashMap<i32, i64> = HashMap::new();
        for (amt, date) in &pays {
            if date.len() >= 4 {
                if let Ok(y) = date[0..4].parse::<i32>() {
                    *proj_year_inc.entry(y).or_insert(0) += *amt;
                }
            }
        }
        for (y, y_inc) in &proj_year_inc {
            let y_exc = (*y_inc as f64 / one_plus).round() as i64;
            let y_comm = match p.comm_mode.as_str() {
                "rate" => (*y_inc as f64 * p.comm_rate.unwrap_or(0.0)).round() as i64,
                "fixed" => {
                    let fixed = if p.comm_settled { p.comm_amount.unwrap_or(0) } else { 0 };
                    if recv_inc > 0 {
                        ((*y_inc as f64 / recv_inc as f64) * fixed as f64).round() as i64
                    } else { 0 }
                }
                _ => 0,
            };
            *year_recv_exc.entry(*y).or_insert(0) += y_exc;
            *year_commission.entry(*y).or_insert(0) += y_comm;
            year_seen.insert(*y, ());
        }

        let proj_net = recv_exc - comm_realized - general;
        project_ranks.push(RankRow { id: p.id, name: p.name.clone(), net_cents: proj_net, received_inclusive_cents: recv_inc });
        let ckey = p.client_id.unwrap_or(0);
        *client_net.entry(ckey).or_insert(0) += proj_net;
        *client_recv.entry(ckey).or_insert(0) += recv_inc;
        client_name.entry(ckey).or_insert_with(|| p.client_name.clone().unwrap_or_else(|| "未分配".to_string()));
    }

    out.net_potential_cents = out.revenue_exclusive_cents - out.commission_potential_cents - out.general_cost_cents;
    out.net_realized_cents = out.received_exclusive_cents - out.commission_realized_cents - out.general_cost_cents;
    out.outstanding_cents = out.contract_total_inclusive_cents - out.received_inclusive_cents;

    // by_year: union of years from received and cost
    let mut years: Vec<i32> = year_seen.keys().chain(cost_by_year.keys()).cloned().collect();
    years.sort_unstable();
    years.dedup();
    for y in years {
        let recv_exc = *year_recv_exc.get(&y).unwrap_or(&0);
        let gcost = *cost_by_year.get(&y).unwrap_or(&0);
        let comm = *year_commission.get(&y).unwrap_or(&0);
        out.by_year.push(YearRow {
            year: y,
            received_exclusive_cents: recv_exc,
            general_cost_cents: gcost,
            commission_cents: comm,
            net_cents: recv_exc - gcost - comm,
        });
    }

    // by_status in fixed order
    for st in STATUS_ORDER {
        if let Some(cnt) = status_count.get(st) {
            out.by_status.push(StatusRow {
                status: st.to_string(),
                count: *cnt,
                contract_inclusive_cents: *status_inc.get(st).unwrap_or(&0),
            });
        }
    }

    // receivables
    let mut rstmt = conn.prepare(
        "SELECT cp.project_id, p.name, cp.name, cp.expected_amount_cents, cp.expected_date
         FROM contract_payments cp JOIN projects p ON p.id = cp.project_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND cp.deleted_at IS NULL
           AND cp.actual_received_at IS NULL AND cp.expected_date IS NOT NULL
         ORDER BY cp.expected_date ASC",
    )?;
    let rows = rstmt.query_map([company_id], |r| {
        Ok((
            r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?, r.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (pid, pname, name, amt, edate) = row?;
        let bucket = if edate.as_str() < today {
            "overdue"
        } else if edate.as_str() <= soon_cutoff.as_str() {
            "soon"
        } else {
            "future"
        };
        out.receivables_outstanding_cents += amt;
        out.receivables.push(ReceivableRow {
            project_id: pid, project_name: pname, name,
            expected_amount_cents: amt, expected_date: edate, bucket: bucket.to_string(),
        });
    }

    // rankings: net desc, top 5
    project_ranks.sort_by(|a, b| b.net_cents.cmp(&a.net_cents));
    out.top_projects = project_ranks.into_iter().take(5).collect();

    let mut clients: Vec<RankRow> = client_net
        .iter()
        .map(|(k, net)| RankRow {
            id: *k,
            name: client_name.get(k).cloned().unwrap_or_default(),
            net_cents: *net,
            received_inclusive_cents: *client_recv.get(k).copied().unwrap_or(0),
        })
        .collect();
    clients.sort_by(|a, b| b.net_cents.cmp(&a.net_cents));
    out.top_clients = clients.into_iter().take(5).collect();

    Ok(out)
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib dashboard 2>&1 | tail -20`
Expected: `dashboard_totals_and_buckets`、`empty_company_all_zero`、`receivable_bucket_soon_and_future` 全部 PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/domain/dashboard.rs src-tauri/src/domain/mod.rs
git commit -m "$(cat <<'MSG'
feat(dashboard): 公司级聚合域函数

一次遍历公司项目算出合同/已收两口径、到手净收入、按年到手、状态分布、
应收提醒与按到手排行，纯函数带单测

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
MSG
)"
```

---

### Task 2: 后端命令 `get_dashboard`

**Files:**
- Create: `src-tauri/src/commands/dashboard.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod dashboard;`）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Interfaces:**
- Consumes: `crate::domain::dashboard::{company_dashboard, DashboardSummary}`
- Produces: `#[tauri::command] pub fn get_dashboard(state, company_id: i64) -> AppResult<DashboardSummary>`（JS 侧传 `{ companyId }`）

- [ ] **Step 1: 写命令文件**

Create `src-tauri/src/commands/dashboard.rs`：

```rust
use crate::domain::dashboard::{company_dashboard, DashboardSummary};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn get_dashboard(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<DashboardSummary> {
    with_conn(&state, |c| {
        let today: String = c.query_row("SELECT date('now','localtime')", [], |r| r.get(0))?;
        company_dashboard(c, company_id, &today)
    })
}
```

- [ ] **Step 2: 注册模块**

Modify `src-tauri/src/commands/mod.rs` — 在 `pub mod costs;` 之后加：

```rust
pub mod dashboard;
```

- [ ] **Step 3: 注册命令到 invoke_handler**

Modify `src-tauri/src/lib.rs` — 在 `commands::costs::get_project_cost_summary,` 那一行之后插入：

```rust
            commands::dashboard::get_dashboard,
```

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo build 2>&1 | tail -8`
Expected: `Finished`，无错误。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/dashboard.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "$(cat <<'MSG'
feat(dashboard): get_dashboard 命令

命令层取本地当天日期后调用聚合域函数，返回公司级仪表盘数据

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
MSG
)"
```

---

### Task 3: 前端类型 + store + 锁定重置

**Files:**
- Modify: `src/types/index.ts`
- Create: `src/stores/dashboard.ts`
- Modify: `src/stores/auth.ts`

**Interfaces:**
- Produces:
  - `DashboardSummary` / `DashYearRow` / `DashStatusRow` / `DashReceivableRow` / `RankRow`（TS 接口，字段与 Task 1 的 Rust struct 一一对应）
  - `useDashboardStore` with `{ data: DashboardSummary | null, loadedForCompany: number | null, loadFor(companyId), reset() }`

- [ ] **Step 1: 加类型**

Modify `src/types/index.ts` — 文件末尾追加：

```ts
export interface DashYearRow {
  year: number;
  received_exclusive_cents: number;
  general_cost_cents: number;
  commission_cents: number;
  net_cents: number;
}
export interface DashStatusRow {
  status: string;
  count: number;
  contract_inclusive_cents: number;
}
export interface DashReceivableRow {
  project_id: number;
  project_name: string;
  name: string;
  expected_amount_cents: number;
  expected_date: string;
  bucket: string; // "overdue" | "soon" | "future"
}
export interface RankRow {
  id: number;
  name: string;
  net_cents: number;
  received_inclusive_cents: number;
}
export interface DashboardSummary {
  contract_total_inclusive_cents: number;
  revenue_exclusive_cents: number;
  commission_potential_cents: number;
  general_cost_cents: number;
  net_potential_cents: number;
  received_inclusive_cents: number;
  received_exclusive_cents: number;
  outstanding_cents: number;
  commission_realized_cents: number;
  net_realized_cents: number;
  by_year: DashYearRow[];
  by_status: DashStatusRow[];
  receivables: DashReceivableRow[];
  receivables_outstanding_cents: number;
  top_clients: RankRow[];
  top_projects: RankRow[];
}
```

- [ ] **Step 2: 建 store**

Create `src/stores/dashboard.ts`：

```ts
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { DashboardSummary } from "@/types";

interface S {
  data: DashboardSummary | null;
  loadedForCompany: number | null;
  loadFor: (companyId: number) => Promise<void>;
  reset: () => void;
}

export const useDashboardStore = create<S>((set) => ({
  data: null,
  loadedForCompany: null,
  async loadFor(companyId) {
    try {
      const d = await call<DashboardSummary>("get_dashboard", { companyId });
      set({ data: d, loadedForCompany: companyId });
    } catch {
      // non-fatal; page shows loading/empty
      set({ data: null, loadedForCompany: companyId });
    }
  },
  reset() {
    set({ data: null, loadedForCompany: null });
  },
}));
```

- [ ] **Step 3: 锁定时重置**

Modify `src/stores/auth.ts` — 在 import 段加：

```ts
import { useDashboardStore } from "./dashboard";
```

并在 `lock()` 的 reset 序列里（`useBackupStore.getState().reset();` 之后）加：

```ts
    useDashboardStore.getState().reset();
```

- [ ] **Step 4: 类型检查**

Run: `pnpm tsc --noEmit 2>&1 | tail -5`
Expected: 无输出（通过）。

- [ ] **Step 5: Commit**

```bash
git add src/types/index.ts src/stores/dashboard.ts src/stores/auth.ts
git commit -m "$(cat <<'MSG'
feat(dashboard): 前端类型与 store

新增 DashboardSummary 等类型及按公司缓存的 dashboard store，锁定时重置

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
MSG
)"
```

---

### Task 4: 前端仪表盘页面 + i18n

**Files:**
- Modify: `src/routes/dashboard.tsx`（整页重写）
- Modify: `src/i18n/zh-CN.json`（加 `dashboard.*`）

**Interfaces:**
- Consumes: `useDashboardStore`、`DashboardSummary`/`RankRow`、`formatCNY`、`statusLabel`、shadcn `Card`/`Tabs`/`Table`/`Badge`。

- [ ] **Step 1: 加 i18n 文案**

Modify `src/i18n/zh-CN.json` — 在顶层对象里（例如 `financial` 段之后）插入：

```json
  "dashboard": {
    "tabOverview": "总览",
    "tabRanking": "分布 & 排行",
    "tabReceivables": "应收",
    "contractScope": "合同口径（全量潜在）",
    "receivedScope": "已收口径（真实落袋）",
    "contractTotal": "合同总额(含税)",
    "revenueExclusive": "不含税收入",
    "netPotential": "潜在到手",
    "received": "已收(含税)",
    "outstanding": "未收/应收",
    "netRealized": "已收到手",
    "netFormula": "不含税 − 提成 − 非人力成本",
    "netByYear": "按年到手（叠加已收对照）",
    "netLabel": "到手",
    "receivedLabel": "已收",
    "statusDist": "项目状态分布",
    "topClients": "客户排行 Top5（按到手）",
    "topProjects": "项目排行 Top5（按到手）",
    "name": "名称",
    "receivablesOutstanding": "合计未收",
    "dueDate": "预期日期",
    "project": "项目",
    "status": "状态",
    "noReceivables": "没有待收款节点",
    "empty": "暂无数据",
    "loading": "加载中…",
    "selectCompany": "请先选择公司",
    "bucket": {
      "overdue": "已逾期",
      "soon": "近30天",
      "future": "未来"
    }
  },
```

> 注意：确保插入位置前一段结尾有逗号、JSON 合法。

- [ ] **Step 2: 重写页面**

Replace 全文 `src/routes/dashboard.tsx`：

```tsx
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { useCompanyStore } from "@/stores/company";
import { useDashboardStore } from "@/stores/dashboard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { formatCNY } from "@/lib/money";
import { statusLabel } from "@/lib/status";
import type { RankRow } from "@/types";

const BUCKET_CLASS: Record<string, string> = {
  overdue: "bg-red-100 text-red-700",
  soon: "bg-amber-100 text-amber-700",
  future: "bg-slate-100 text-slate-600",
};

function Kpi({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <Card>
      <CardContent className="p-4 space-y-1">
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="text-lg font-semibold">{value}</div>
        {sub && <div className="text-xs text-muted-foreground">{sub}</div>}
      </CardContent>
    </Card>
  );
}

function RankCard({ title, rows, t }: { title: string; rows: RankRow[]; t: TFunction }) {
  return (
    <Card>
      <CardHeader><CardTitle className="text-sm">{title}</CardTitle></CardHeader>
      <CardContent className="p-0">
        <Table compact>
          <TableHeader>
            <TableRow>
              <TableHead>{t("dashboard.name")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.netLabel")}</TableHead>
              <TableHead className="text-right w-28">{t("dashboard.receivedLabel")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow><TableCell colSpan={3} className="p-4 text-sm text-muted-foreground">{t("dashboard.empty")}</TableCell></TableRow>
            ) : rows.map((r) => (
              <TableRow key={r.id}>
                <TableCell>{r.name}</TableCell>
                <TableCell className="text-right">{formatCNY(r.net_cents)}</TableCell>
                <TableCell className="text-right text-muted-foreground">{formatCNY(r.received_inclusive_cents)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const currentId = useCompanyStore((s) => s.currentId);
  const { data, loadedForCompany, loadFor } = useDashboardStore();

  useEffect(() => {
    if (currentId != null && loadedForCompany !== currentId) loadFor(currentId);
  }, [currentId, loadedForCompany, loadFor]);

  if (currentId == null) {
    return <div className="text-sm text-muted-foreground">{t("dashboard.selectCompany")}</div>;
  }
  if (!data) {
    return <div className="text-sm text-muted-foreground">{t("dashboard.loading")}</div>;
  }

  const maxYear = Math.max(1, ...data.by_year.map((y) => Math.max(y.net_cents, y.received_exclusive_cents)));
  const maxStatus = Math.max(1, ...data.by_status.map((s) => s.contract_inclusive_cents));
  const pct = (v: number, max: number) => `${Math.max(0, Math.min(100, (v / max) * 100))}%`;

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("nav.dashboard")}</h1>
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">{t("dashboard.tabOverview")}</TabsTrigger>
          <TabsTrigger value="ranking">{t("dashboard.tabRanking")}</TabsTrigger>
          <TabsTrigger value="receivables">{t("dashboard.tabReceivables")}</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-4">
          <div>
            <div className="mb-2 text-sm font-medium">{t("dashboard.contractScope")}</div>
            <div className="grid grid-cols-3 gap-3">
              <Kpi label={t("dashboard.contractTotal")} value={formatCNY(data.contract_total_inclusive_cents)} />
              <Kpi label={t("dashboard.revenueExclusive")} value={formatCNY(data.revenue_exclusive_cents)} />
              <Kpi label={t("dashboard.netPotential")} value={formatCNY(data.net_potential_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <div>
            <div className="mb-2 text-sm font-medium">{t("dashboard.receivedScope")}</div>
            <div className="grid grid-cols-3 gap-3">
              <Kpi label={t("dashboard.received")} value={formatCNY(data.received_inclusive_cents)} />
              <Kpi label={t("dashboard.outstanding")} value={formatCNY(data.outstanding_cents)} />
              <Kpi label={t("dashboard.netRealized")} value={formatCNY(data.net_realized_cents)} sub={t("dashboard.netFormula")} />
            </div>
          </div>
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("dashboard.netByYear")}</CardTitle></CardHeader>
            <CardContent className="space-y-3">
              {data.by_year.length === 0 ? (
                <div className="text-sm text-muted-foreground">{t("dashboard.empty")}</div>
              ) : data.by_year.map((y) => (
                <div key={y.year} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{y.year}</span>
                    <span className="text-muted-foreground">
                      {t("dashboard.netLabel")} {formatCNY(y.net_cents)} · {t("dashboard.receivedLabel")} {formatCNY(y.received_exclusive_cents)}
                    </span>
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-slate-300" style={{ width: pct(y.received_exclusive_cents, maxYear) }} />
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-emerald-500" style={{ width: pct(y.net_cents, maxYear) }} />
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="ranking" className="space-y-4">
          <Card>
            <CardHeader><CardTitle className="text-sm">{t("dashboard.statusDist")}</CardTitle></CardHeader>
            <CardContent className="space-y-2">
              {data.by_status.length === 0 ? (
                <div className="text-sm text-muted-foreground">{t("dashboard.empty")}</div>
              ) : data.by_status.map((s) => (
                <div key={s.status} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{statusLabel(s.status)} · {s.count}</span>
                    <span className="text-muted-foreground">{formatCNY(s.contract_inclusive_cents)}</span>
                  </div>
                  <div className="h-2 overflow-hidden rounded bg-muted">
                    <div className="h-full bg-sky-400" style={{ width: pct(s.contract_inclusive_cents, maxStatus) }} />
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
          <div className="grid grid-cols-2 gap-4">
            <RankCard title={t("dashboard.topClients")} rows={data.top_clients} t={t} />
            <RankCard title={t("dashboard.topProjects")} rows={data.top_projects} t={t} />
          </div>
        </TabsContent>

        <TabsContent value="receivables" className="space-y-4">
          <Kpi label={t("dashboard.receivablesOutstanding")} value={formatCNY(data.receivables_outstanding_cents)} />
          <Card>
            <CardContent className="p-0">
              <Table compact>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-28">{t("dashboard.dueDate")}</TableHead>
                    <TableHead>{t("dashboard.project")}</TableHead>
                    <TableHead>{t("payment.name")}</TableHead>
                    <TableHead className="text-right w-32">{t("payment.expectedAmount")}</TableHead>
                    <TableHead className="w-20">{t("dashboard.status")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.receivables.length === 0 ? (
                    <TableRow><TableCell colSpan={5} className="p-4 text-sm text-muted-foreground">{t("dashboard.noReceivables")}</TableCell></TableRow>
                  ) : data.receivables.map((r, i) => (
                    <TableRow key={i}>
                      <TableCell className="whitespace-nowrap">{r.expected_date}</TableCell>
                      <TableCell>{r.project_name}</TableCell>
                      <TableCell className="text-muted-foreground">{r.name}</TableCell>
                      <TableCell className="text-right">{formatCNY(r.expected_amount_cents)}</TableCell>
                      <TableCell>
                        <Badge variant="secondary" className={BUCKET_CLASS[r.bucket] ?? ""}>
                          {t(`dashboard.bucket.${r.bucket}`)}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
```

- [ ] **Step 3: 类型检查 + 构建**

Run: `pnpm tsc --noEmit 2>&1 | tail -5 && pnpm build 2>&1 | tail -4`
Expected: tsc 无输出；vite `built in ...`。

- [ ] **Step 4: Commit**

```bash
git add src/routes/dashboard.tsx src/i18n/zh-CN.json
git commit -m "$(cat <<'MSG'
feat(dashboard): 仪表盘页面

分总览/分布排行/应收三 Tab：钱大盘两口径 KPI、按年到手叠加已收对照、
状态分布、客户/项目按到手 Top5、应收提醒（逾期红/近30天黄），CSS 条形无新依赖

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
MSG
)"
```

---

## Self-Review

**Spec coverage：**
- 合同/已收两口径 KPI → Task 1 totals + Task 4 overview ✔
- 到手（不含税−提成−非人力成本）→ Task 1 `net_potential`/`net_realized` ✔
- 按年到手（叠加已收对照）→ Task 1 `by_year` + Task 4 双条形 ✔
- 项目状态分布 → Task 1 `by_status` + Task 4 ✔
- 应收提醒（逾期/近30天/未来）→ Task 1 `receivables` + Task 4 ✔
- 按到手排行（客户/项目 Top5）→ Task 1 `top_clients`/`top_projects` + Task 4 ✔
- 全部项目纳入、无图表依赖、锁定重置、公司切换重载 → Task 1/3/4 ✔
- 分 Tab → Task 4 ✔

**Placeholder scan：** 无 TBD/TODO；所有步骤含完整代码与命令。

**Type consistency：** Rust struct 字段（`*_cents`、`by_year`/`by_status`/`receivables`/`top_clients`/`top_projects`、`RankRow{id,name,net_cents,received_inclusive_cents}`）与 TS 接口逐一对应；命令名 `get_dashboard`、参数 `company_id`↔`companyId` 一致；store 方法 `loadFor`/`reset`/`loadedForCompany` 前后一致。
