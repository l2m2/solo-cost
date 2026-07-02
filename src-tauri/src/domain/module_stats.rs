use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModuleLaborStat {
    pub module_id: Option<i64>,
    pub module_name: Option<String>,
    pub hours: f64,
    pub cost_cents: i64,
}

pub fn labor_by_module(
    conn: &Connection,
    project_id: i64,
) -> AppResult<Vec<ModuleLaborStat>> {
    // Coalesce tasks pointing at soft-deleted modules into the "unassigned"
    // bucket: the LEFT JOIN yields m.id = NULL for both truly-unassigned tasks
    // and orphans, so grouping by t.module_id alone would split them into two
    // rows both rendered as "未分类". The CASE folds orphans into NULL.
    let mut stmt = conn.prepare(
        "SELECT CASE WHEN m.id IS NOT NULL THEN t.module_id ELSE NULL END AS module_id,
                m.name AS module_name,
                COALESCE(SUM(tl.hours), 0.0) AS hours,
                COALESCE(CAST(SUM(ROUND(tl.hours / 8.0 * tl.daily_cost_snapshot_cents)) AS INTEGER), 0) AS cost
         FROM tasks t
         LEFT JOIN modules m
                ON m.id = t.module_id AND m.deleted_at IS NULL
         LEFT JOIN time_logs tl
                ON tl.task_id = t.id AND tl.deleted_at IS NULL
         WHERE t.project_id = ?1 AND t.deleted_at IS NULL
         GROUP BY CASE WHEN m.id IS NOT NULL THEN t.module_id ELSE NULL END, m.name
         HAVING hours > 0
         ORDER BY m.sort_order ASC NULLS LAST, m.id ASC",
    )?;
    let rows = stmt.query_map([project_id], |r| {
        Ok(ModuleLaborStat {
            module_id: r.get(0)?,
            module_name: r.get(1)?,
            hours: r.get(2)?,
            cost_cents: r.get::<_, i64>(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
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
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn add_task(conn: &Connection, module_id: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO tasks(project_id, title, module_id) VALUES(1, 'T', ?1)",
            [module_id],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn add_log(conn: &Connection, task_id: i64, hours: f64) {
        add_log_with_date(conn, task_id, hours, "2026-06-01");
    }

    fn add_log_with_date(conn: &Connection, task_id: i64, hours: f64, work_date: &str) {
        conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(?1, 1, ?3, ?2, 80000)",
            rusqlite::params![task_id, hours, work_date],
        ).unwrap();
    }

    #[test]
    fn labor_by_module_empty_project_returns_empty() {
        let db = TestDb::new();
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn labor_by_module_unassigned_bucket_only() {
        let db = TestDb::new();
        let tid = add_task(&db.conn, None);
        add_log(&db.conn, tid, 8.0);
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].module_id, None);
        assert_eq!(out[0].module_name, None);
        assert!((out[0].hours - 8.0).abs() < 1e-9);
        // 8h / 8 * 80000 = 80000
        assert_eq!(out[0].cost_cents, 80_000);
    }

    #[test]
    fn labor_by_module_mixes_named_and_unassigned() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '后端', 1)", []).unwrap();
        let fe = add_task(&db.conn, Some(1));
        let be = add_task(&db.conn, Some(2));
        let na = add_task(&db.conn, None);
        add_log(&db.conn, fe, 20.0);
        // Split 30 hours across two days to respect 24h constraint
        add_log_with_date(&db.conn, be, 16.0, "2026-06-01");
        add_log_with_date(&db.conn, be, 14.0, "2026-06-02");
        add_log(&db.conn, na, 8.0);
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 3);
        // ORDER BY m.sort_order ASC NULLS LAST → 前端 (0) / 后端 (1) / 未分类 (NULL)
        assert_eq!(out[0].module_name, Some("前端".into()));
        assert!((out[0].hours - 20.0).abs() < 1e-9);
        assert_eq!(out[0].cost_cents, 200_000);
        assert_eq!(out[1].module_name, Some("后端".into()));
        assert_eq!(out[1].cost_cents, 300_000);
        assert_eq!(out[2].module_id, None);
        assert_eq!(out[2].cost_cents, 80_000);
    }

    #[test]
    fn labor_by_module_excludes_soft_deleted_tasks_and_logs() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        let t1 = add_task(&db.conn, Some(1));
        let t2 = add_task(&db.conn, Some(1));
        add_log(&db.conn, t1, 8.0);
        add_log(&db.conn, t2, 8.0);
        // soft-delete one log AND one task
        db.conn.execute("UPDATE time_logs SET deleted_at = datetime('now') WHERE id = 1", []).unwrap();
        db.conn.execute("UPDATE tasks SET deleted_at = datetime('now') WHERE id = ?1", [t2]).unwrap();
        let out = labor_by_module(&db.conn, 1).unwrap();
        // t1 has 0h left (its only log was deleted), t2 fully deleted → nothing above HAVING hours > 0
        assert!(out.is_empty());
    }

    #[test]
    fn labor_by_module_collapses_orphans_with_unassigned() {
        // Regression for I1: tasks that still point at a soft-deleted module
        // must collapse into the single "未分类" row, not spawn a second one.
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        // orphan: task holds module_id=1 but the module is soft-deleted below
        let orphan = add_task(&db.conn, Some(1));
        let na = add_task(&db.conn, None);
        add_log(&db.conn, orphan, 4.0);
        add_log(&db.conn, na, 4.0);
        db.conn.execute("UPDATE modules SET deleted_at = datetime('now') WHERE id = 1", []).unwrap();
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 1, "orphan + unassigned should be one row, got {out:?}");
        assert_eq!(out[0].module_id, None);
        assert_eq!(out[0].module_name, None);
        assert!((out[0].hours - 8.0).abs() < 1e-9);
        assert_eq!(out[0].cost_cents, 80_000);
    }

    #[test]
    fn labor_by_module_uses_snapshot_daily_cost() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)", []).unwrap();
        let tid = add_task(&db.conn, Some(1));
        // insert time_log with EXPLICIT snapshot ≠ current member.daily_cost_cents
        db.conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(?1, 1, '2026-06-01', 8.0, 60000)",
            [tid],
        ).unwrap();
        // change member daily cost afterwards; snapshot should NOT be affected
        db.conn.execute("UPDATE members SET daily_cost_cents = 999999 WHERE id = 1", []).unwrap();
        let out = labor_by_module(&db.conn, 1).unwrap();
        assert_eq!(out.len(), 1);
        // 8h / 8 * 60000 = 60_000
        assert_eq!(out[0].cost_cents, 60_000);
    }
}
