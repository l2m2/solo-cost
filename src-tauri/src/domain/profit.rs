use crate::error::AppResult;
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
