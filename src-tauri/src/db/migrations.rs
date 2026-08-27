use crate::error::{AppError, AppResult};
use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../../migrations/0001_init.sql")),
    (
        "0002_projects_costs",
        include_str!("../../migrations/0002_projects_costs.sql"),
    ),
    (
        "0003_people_contracts",
        include_str!("../../migrations/0003_people_contracts.sql"),
    ),
    (
        "0004_projects_commission",
        include_str!("../../migrations/0004_projects_commission.sql"),
    ),
    (
        "0005_modules",
        include_str!("../../migrations/0005_modules.sql"),
    ),
    (
        "0006_tasks_external_ref",
        include_str!("../../migrations/0006_tasks_external_ref.sql"),
    ),
    (
        "0007_clients",
        include_str!("../../migrations/0007_clients.sql"),
    ),
    (
        "0008_tasks_status_dates",
        include_str!("../../migrations/0008_tasks_status_dates.sql"),
    ),
    (
        "0009_task_events",
        include_str!("../../migrations/0009_task_events.sql"),
    ),
];

pub fn run(conn: &Connection) -> AppResult<()> {
    ensure_meta_table(conn)?;
    let current = current_version(conn)?;
    // Some migrations rebuild a table that others reference (e.g. changing a CHECK
    // constraint requires dropping & recreating the table). SQLite refuses to drop a
    // table whose rows are still referenced while foreign keys are enforced, and the
    // pragma is a no-op inside a transaction — so disable enforcement for the whole
    // migration pass and restore it afterward. Each migration stays atomic via its
    // own transaction.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = apply_pending(conn, current);
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}

fn apply_pending(conn: &Connection, current: i64) -> AppResult<()> {
    for (idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let target = (idx + 1) as i64;
        if target <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)
            .map_err(|e| AppError::Migration(format!("{}: {}", name, e)))?;
        // Upsert schema_version so it survives even if the SQL already inserted it.
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [target.to_string()],
        )?;
        tx.commit()?;
        tracing::info!("applied migration {}", name);
    }
    Ok(())
}

fn ensure_meta_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
         );",
    )?;
    Ok(())
}

fn current_version(conn: &Connection) -> AppResult<i64> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    match row {
        Some(s) => s
            .parse::<i64>()
            .map_err(|e| AppError::Migration(format!("bad schema_version: {}", e))),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_in_memory_for_test;

    #[test]
    fn fresh_db_runs_all_migrations() {
        let conn = open_in_memory_for_test("p").unwrap();
        run(&conn).unwrap();

        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='companies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        let v = current_version(&conn).unwrap();
        assert_eq!(v, 9);
    }

    #[test]
    fn run_is_idempotent() {
        let conn = open_in_memory_for_test("p").unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap(); // second run should not error
        assert_eq!(current_version(&conn).unwrap(), 9);
    }

    #[test]
    fn migration_0009_allows_paused_status() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title, status) VALUES(1, 'T', 'paused')",
            [],
        )
        .unwrap();
        let s: String = conn
            .query_row("SELECT status FROM tasks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(s, "paused");
    }

    #[test]
    fn migration_0009_backfills_history_for_existing_tasks() {
        // Simulate a pre-0009 database: apply every migration except the last,
        // insert a finished task, then let 0009 run and check the timeline.
        let conn = Connection::open_in_memory().unwrap();
        ensure_meta_table(&conn).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        let upto = (MIGRATIONS.len() - 1) as i64;
        for (idx, (_, sql)) in MIGRATIONS.iter().enumerate() {
            if (idx as i64) >= upto {
                break;
            }
            conn.execute_batch(sql).unwrap();
        }
        conn.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [upto.to_string()],
        )
        .unwrap();
        conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
        conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(project_id, title, status, started_at, completed_at, created_at)
             VALUES(1, 'T', 'done', '2026-01-02 09:00:00', '2026-01-05 18:00:00',
                    '2026-01-01 08:00:00')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT to_status, occurred_at FROM task_events
                 WHERE task_id = 1 ORDER BY occurred_at ASC",
            )
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            rows,
            vec![
                ("todo".to_string(), "2026-01-01 08:00:00".to_string()),
                ("in_progress".to_string(), "2026-01-02 09:00:00".to_string()),
                ("done".to_string(), "2026-01-05 18:00:00".to_string()),
            ]
        );
    }
}
