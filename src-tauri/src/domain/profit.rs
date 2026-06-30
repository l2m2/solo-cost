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
    let (contract, inclusive, rate): (i64, i64, f64) = conn
        .query_row(
            "SELECT contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate
             FROM projects WHERE id = ?1 AND deleted_at IS NULL",
            [project_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
                entity: "project",
                id: project_id,
            },
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
                 VALUES(1, '差旅', 1, 0), (1, '硬件', 1, 1)",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }
    }

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
