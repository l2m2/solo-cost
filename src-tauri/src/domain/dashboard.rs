use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct YearReceiptRow {
    pub project_id: i64,
    pub project_name: String,
    pub name: String, // payment node name
    pub amount_inclusive_cents: i64,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct YearProjectRow {
    pub project_id: i64,
    pub project_name: String,
    pub received_inclusive_cents: i64,
    pub received_exclusive_cents: i64,
    pub general_cost_cents: i64,
    pub commission_cents: i64,
    pub net_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct YearRow {
    pub year: i32,
    pub received_inclusive_cents: i64,
    pub received_exclusive_cents: i64,
    pub general_cost_cents: i64,
    pub commission_cents: i64,
    pub net_cents: i64,
    pub projects: Vec<YearProjectRow>,
    pub receipts: Vec<YearReceiptRow>,
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
    pub client_name: String,
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

    // company-wide non-labor cost by (project, year), for the per-project year breakdown
    let mut cost_by_proj: HashMap<i64, HashMap<i32, i64>> = HashMap::new();
    let mut cpstmt = conn.prepare(
        "SELECT ce.project_id, CAST(substr(ce.incurred_at,1,4) AS INTEGER) AS y,
                COALESCE(SUM(ce.amount_cents),0)
         FROM cost_entries ce JOIN projects p ON p.id = ce.project_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND ce.deleted_at IS NULL
         GROUP BY ce.project_id, y",
    )?;
    for row in cpstmt.query_map([company_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as i32, r.get::<_, i64>(2)?))
    })? {
        let (pid, y, amt) = row?;
        cost_by_proj.entry(pid).or_default().insert(y, amt);
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

    let mut year_recv_inc: HashMap<i32, i64> = HashMap::new();
    let mut year_recv_exc: HashMap<i32, i64> = HashMap::new();
    let mut year_commission: HashMap<i32, i64> = HashMap::new();
    let mut year_receipts: HashMap<i32, Vec<YearReceiptRow>> = HashMap::new();
    let mut year_projects: HashMap<i32, Vec<YearProjectRow>> = HashMap::new();
    let mut year_seen: HashSet<i32> = HashSet::new();
    let mut status_count: HashMap<String, i64> = HashMap::new();
    let mut status_inc: HashMap<String, i64> = HashMap::new();
    let mut client_net: HashMap<i64, i64> = HashMap::new();
    let mut client_recv: HashMap<i64, i64> = HashMap::new();
    let mut client_name: HashMap<i64, String> = HashMap::new();
    let mut project_ranks: Vec<RankRow> = Vec::new();

    let mut paystmt = conn.prepare(
        "SELECT name, actual_amount_cents, actual_received_at
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

        // (node_name, amount_inclusive, received_at)
        let pays: Vec<(String, i64, String)> = paystmt
            .query_map([p.id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let recv_inc: i64 = pays.iter().map(|(_, a, _)| *a).sum();
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
        for (node_name, amt, date) in &pays {
            if date.len() >= 4 {
                if let Ok(y) = date[0..4].parse::<i32>() {
                    *proj_year_inc.entry(y).or_insert(0) += *amt;
                    year_receipts.entry(y).or_default().push(YearReceiptRow {
                        project_id: p.id,
                        project_name: p.name.clone(),
                        name: node_name.clone(),
                        amount_inclusive_cents: *amt,
                        received_at: date.clone(),
                    });
                }
            }
        }
        // Years this project touches: received years ∪ years with non-labor cost.
        let proj_costs = cost_by_proj.get(&p.id);
        let mut proj_years: HashSet<i32> = proj_year_inc.keys().copied().collect();
        if let Some(pc) = proj_costs {
            for y in pc.keys() { proj_years.insert(*y); }
        }
        for y in &proj_years {
            let y_inc = proj_year_inc.get(y).copied().unwrap_or(0);
            let y_exc = (y_inc as f64 / one_plus).round() as i64;
            let y_comm = match p.comm_mode.as_str() {
                "rate" => (y_inc as f64 * p.comm_rate.unwrap_or(0.0)).round() as i64,
                "fixed" => {
                    let fixed = if p.comm_settled { p.comm_amount.unwrap_or(0) } else { 0 };
                    if recv_inc > 0 {
                        ((y_inc as f64 / recv_inc as f64) * fixed as f64).round() as i64
                    } else { 0 }
                }
                _ => 0,
            };
            let y_cost = proj_costs.and_then(|pc| pc.get(y)).copied().unwrap_or(0);
            // year-level totals
            *year_recv_inc.entry(*y).or_insert(0) += y_inc;
            *year_recv_exc.entry(*y).or_insert(0) += y_exc;
            *year_commission.entry(*y).or_insert(0) += y_comm;
            year_seen.insert(*y);
            // per-project-in-year breakdown
            year_projects.entry(*y).or_default().push(YearProjectRow {
                project_id: p.id,
                project_name: p.name.clone(),
                received_inclusive_cents: y_inc,
                received_exclusive_cents: y_exc,
                general_cost_cents: y_cost,
                commission_cents: y_comm,
                net_cents: y_exc - y_cost - y_comm,
            });
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
    let mut years: Vec<i32> = year_seen.iter().chain(cost_by_year.keys()).cloned().collect();
    years.sort_unstable();
    years.dedup();
    for y in years {
        let recv_exc = *year_recv_exc.get(&y).unwrap_or(&0);
        let gcost = *cost_by_year.get(&y).unwrap_or(&0);
        let comm = *year_commission.get(&y).unwrap_or(&0);
        let mut receipts = year_receipts.remove(&y).unwrap_or_default();
        receipts.sort_by(|a, b| a.received_at.cmp(&b.received_at));
        let mut projects_y = year_projects.remove(&y).unwrap_or_default();
        projects_y.sort_by(|a, b| b.net_cents.cmp(&a.net_cents));
        out.by_year.push(YearRow {
            year: y,
            received_inclusive_cents: year_recv_inc.remove(&y).unwrap_or(0),
            received_exclusive_cents: recv_exc,
            general_cost_cents: gcost,
            commission_cents: comm,
            net_cents: recv_exc - gcost - comm,
            projects: projects_y,
            receipts,
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
        "SELECT cp.project_id, p.name, COALESCE(c.name, ''), cp.name, cp.expected_amount_cents, cp.expected_date
         FROM contract_payments cp
         JOIN projects p ON p.id = cp.project_id
         LEFT JOIN clients c ON c.id = p.client_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND cp.deleted_at IS NULL
           AND cp.actual_received_at IS NULL AND cp.expected_date IS NOT NULL
         ORDER BY cp.expected_date ASC",
    )?;
    let rows = rstmt.query_map([company_id], |r| {
        Ok((
            r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
            r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?,
        ))
    })?;
    for row in rows {
        let (pid, pname, cname, name, amt, edate) = row?;
        let bucket = if edate.as_str() < today {
            "overdue"
        } else if edate.as_str() <= soon_cutoff.as_str() {
            "soon"
        } else {
            "future"
        };
        out.receivables_outstanding_cents += amt;
        out.receivables.push(ReceivableRow {
            project_id: pid, project_name: pname, client_name: cname, name,
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
            received_inclusive_cents: client_recv.get(k).copied().unwrap_or(0),
        })
        .collect();
    clients.sort_by(|a, b| b.net_cents.cmp(&a.net_cents));
    out.top_clients = clients.into_iter().take(5).collect();

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

        // by_year receipts: node-level breakdown, sorted by received_at
        assert_eq!(d.by_year[0].receipts.len(), 1);
        assert_eq!(d.by_year[0].receipts[0].name, "一期");
        assert_eq!(d.by_year[0].receipts[0].amount_inclusive_cents, 400_000);
        assert_eq!(d.by_year[1].receipts.len(), 2);
        assert_eq!(d.by_year[1].receipts[0].name, "二期"); // 2026-02-01 before 2026-04-01
        assert_eq!(d.by_year[1].receipts[1].name, "全款");
        let y2026_sum: i64 = d.by_year[1].receipts.iter().map(|r| r.amount_inclusive_cents).sum();
        assert_eq!(y2026_sum, 800_000); // matches year received (含税)

        // by_year per-project breakdown (sorted by net desc)
        assert_eq!(d.by_year[0].projects.len(), 1); // 2025: only PA
        assert_eq!(d.by_year[0].projects[0].project_name, "PA");
        assert_eq!(d.by_year[0].projects[0].received_inclusive_cents, 400_000); // same as inc (tax 0)
        assert_eq!(d.by_year[0].projects[0].net_cents, 390_000);
        assert_eq!(d.by_year[0].projects[0].commission_cents, 0);
        assert_eq!(d.by_year[1].projects.len(), 2); // 2026: PB then PA
        assert_eq!(d.by_year[1].projects[0].project_name, "PB");
        assert_eq!(d.by_year[1].projects[0].received_inclusive_cents, 500_000);
        assert_eq!(d.by_year[1].projects[0].commission_cents, 50_000);
        assert_eq!(d.by_year[1].projects[0].net_cents, 450_000);
        assert_eq!(d.by_year[1].projects[1].project_name, "PA");
        assert_eq!(d.by_year[1].projects[1].received_inclusive_cents, 300_000);
        assert_eq!(d.by_year[1].projects[1].net_cents, 280_000);
        // per-project net & commission sum to the year totals
        let net_sum: i64 = d.by_year[1].projects.iter().map(|p| p.net_cents).sum();
        assert_eq!(net_sum, d.by_year[1].net_cents);
        let comm_sum: i64 = d.by_year[1].projects.iter().map(|p| p.commission_cents).sum();
        assert_eq!(comm_sum, d.by_year[1].commission_cents);
        assert_eq!(d.by_year[1].received_inclusive_cents, 800_000); // year-level inc total

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

    #[test]
    fn fixed_commission_prorated_across_years() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        // fixed + settled commission ¥1000, tax 0, contract 含税 ¥10000
        conn.execute(
            "INSERT INTO projects(company_id, name, status,
                 contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
                 commission_mode, commission_amount_cents, commission_settled)
             VALUES(1, 'PF', 'settled', 1000000, 1, 0.0, 'fixed', 100000, 1)",
            [],
        ).unwrap();
        // receipts: 2025 ¥6000, 2026 ¥4000 (total ¥10000)
        conn.execute(
            "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                 actual_amount_cents, actual_received_at)
             VALUES(1,'一期',600000,600000,'2025-05-01'),
                    (1,'二期',400000,400000,'2026-05-01')",
            [],
        ).unwrap();
        let d = company_dashboard(&conn, 1, "2026-07-04").unwrap();
        assert_eq!(d.commission_realized_cents, 100_000);
        assert_eq!(d.by_year.len(), 2);
        assert_eq!(d.by_year[0].year, 2025);
        assert_eq!(d.by_year[0].commission_cents, 60_000); // 600000/1000000 * 100000
        assert_eq!(d.by_year[1].year, 2026);
        assert_eq!(d.by_year[1].commission_cents, 40_000); // 400000/1000000 * 100000
        // per-year commission sums to realized
        let sum: i64 = d.by_year.iter().map(|y| y.commission_cents).sum();
        assert_eq!(sum, d.commission_realized_cents);
    }
}
