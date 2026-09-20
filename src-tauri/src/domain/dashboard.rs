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
pub struct DashTaskRow {
    pub task_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub title: String,
    pub assignee_name: Option<String>,
    pub status: String,
    pub due_date: Option<String>,
    pub overdue: bool,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub estimated_hours: Option<f64>,
    pub actual_hours: f64,
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
    pub todo_tasks: Vec<DashTaskRow>,
    pub todo_task_count: i64,
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
        todo_tasks: Vec::new(),
        todo_task_count: 0,
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

    // todo tasks: all non-closed tasks across the company (the dedicated "待办"
    // tab filters and paginates them client-side). Active tasks (todo /
    // in_progress) come first, then paused, then done, each bucket ordered by
    // soonest due first and undated last. A paused task is blocked on something
    // external, so like done tasks it is never flagged overdue even past its due
    // date. todo_task_count equals the returned row count now that the list is
    // uncapped.
    out.todo_task_count = conn.query_row(
        "SELECT COUNT(*)
         FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND t.deleted_at IS NULL
           AND t.status != 'closed'",
        [company_id],
        |r| r.get(0),
    )?;
    let mut tstmt = conn.prepare(
        "SELECT t.id, t.project_id, p.name, t.title, m.name, t.status, t.due_date,
                t.started_at, t.completed_at, t.estimated_hours,
                COALESCE((SELECT SUM(hours) FROM time_logs
                          WHERE task_id = t.id AND deleted_at IS NULL), 0.0) AS actual_hours
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         LEFT JOIN members m ON m.id = t.assignee_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND t.deleted_at IS NULL
           AND t.status != 'closed'
         ORDER BY (t.status = 'done'), (t.status = 'paused'),
                  (t.due_date IS NULL), t.due_date ASC, t.id ASC",
    )?;
    let task_rows = tstmt.query_map([company_id], |r| {
        let status: String = r.get(5)?;
        let due_date: Option<String> = r.get(6)?;
        // A paused task is blocked on something external; flagging it overdue
        // blames the wrong party.
        let overdue = status != "done"
            && status != "paused"
            && due_date.as_deref().is_some_and(|d| d < today);
        Ok(DashTaskRow {
            task_id: r.get(0)?,
            project_id: r.get(1)?,
            project_name: r.get(2)?,
            title: r.get(3)?,
            assignee_name: r.get(4)?,
            status,
            due_date,
            overdue,
            started_at: r.get(7)?,
            completed_at: r.get(8)?,
            estimated_hours: r.get(9)?,
            actual_hours: r.get(10)?,
        })
    })?;
    out.todo_tasks = task_rows.collect::<rusqlite::Result<_>>()?;

    Ok(out)
}
