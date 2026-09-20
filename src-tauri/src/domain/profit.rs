use crate::error::{AppError, AppResult};
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
    Ok(ProjectCostSummary {
        total_cents: total,
        by_category,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectFinancialSummary {
    pub revenue_tax_inclusive_cents: i64,
    pub revenue_tax_exclusive_cents: i64,
    pub tax_amount_cents: i64,
    pub general_cost_cents: i64,
    pub labor_cost_cents: i64,
    pub total_cost_cents: i64,
    pub commission_cents: i64,
    pub gross_profit_cents: i64,
    pub profit_rate: f64,
    pub expected_payment_cents: i64,
    pub actual_payment_cents: i64,
    pub collection_rate: f64,
}

pub fn project_financial_summary(
    conn: &Connection,
    project_id: i64,
) -> AppResult<ProjectFinancialSummary> {
    // load project core
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

    let labor = crate::domain::income::project_labor_income(conn, project_id)?;

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

    let total_cost = general + labor + commission;
    let gross = revenue_exc - total_cost;
    let profit_rate = if revenue_exc == 0 {
        0.0
    } else {
        gross as f64 / revenue_exc as f64
    };
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
        commission_cents: commission,
        gross_profit_cents: gross,
        profit_rate,
        expected_payment_cents: expected,
        actual_payment_cents: actual,
        collection_rate,
    })
}
