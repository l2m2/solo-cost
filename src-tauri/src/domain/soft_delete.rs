use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use rusqlite::OptionalExtension;

fn now_iso(conn: &Connection) -> AppResult<String> {
    let s: String = conn.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
    Ok(s)
}

pub fn soft_delete_project(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE projects SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "project",
            id,
        });
    }
    tx.execute(
        "UPDATE cost_entries SET deleted_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore_project(conn: &Connection, id: i64) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let ts: Option<String> = tx
        .query_row("SELECT deleted_at FROM projects WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
                entity: "project",
                id,
            },
            other => AppError::Db(other),
        })?;
    let ts = match ts {
        Some(t) => t,
        None => return Ok(()), // already active, no-op
    };
    tx.execute("UPDATE projects SET deleted_at = NULL WHERE id = ?1", [id])?;
    tx.execute(
        "UPDATE cost_entries SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn soft_delete_cost_entry(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE cost_entries SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "cost_entry",
            id,
        });
    }
    Ok(())
}

pub fn restore_cost_entry(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT ce.project_id, p.deleted_at
         FROM cost_entries ce JOIN projects p ON p.id = ce.project_id
         WHERE ce.id = ?1",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let (_project_id, project_deleted_at) = match row {
        Some(t) => t,
        None => {
            return Err(AppError::NotFound {
                entity: "cost_entry",
                id,
            })
        }
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("项目已删除，请先恢复项目".into()));
    }
    conn.execute(
        "UPDATE cost_entries SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
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
            // create one company and one project + two cost entries for fixtures
            conn.execute("INSERT INTO companies(name) VALUES('C')", [])
                .unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system) VALUES(1, '差旅', 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-01', 12345)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-02', 6789)",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn deleted_at(conn: &Connection, table: &str, id: i64) -> Option<String> {
        conn.query_row(
            &format!("SELECT deleted_at FROM {table} WHERE id = ?1"),
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
    }

    #[test]
    fn project_delete_cascades_to_cost_entries_with_same_timestamp() {
        let db = TestDb::new();
        soft_delete_project(&db.conn, 1).unwrap();
        let pt = deleted_at(&db.conn, "projects", 1).unwrap();
        let c1 = deleted_at(&db.conn, "cost_entries", 1).unwrap();
        let c2 = deleted_at(&db.conn, "cost_entries", 2).unwrap();
        assert_eq!(pt, c1);
        assert_eq!(pt, c2);
    }

    #[test]
    fn restore_project_only_restores_entries_with_matching_timestamp() {
        let db = TestDb::new();
        // independently delete entry 2 first (different timestamp)
        soft_delete_cost_entry(&db.conn, 2).unwrap();
        let entry2_deleted_at = deleted_at(&db.conn, "cost_entries", 2).unwrap();

        // ensure project delete uses a distinct timestamp
        std::thread::sleep(std::time::Duration::from_millis(1100));
        soft_delete_project(&db.conn, 1).unwrap();
        let project_ts = deleted_at(&db.conn, "projects", 1).unwrap();
        assert_ne!(project_ts, entry2_deleted_at);

        // restore project: entry 1 (matched the cascade) is restored, entry 2 (pre-deleted) stays deleted
        restore_project(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "projects", 1).is_none());
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_none());
        assert_eq!(
            deleted_at(&db.conn, "cost_entries", 2).unwrap(),
            entry2_deleted_at
        );
    }

    #[test]
    fn restore_cost_entry_under_deleted_project_blocked() {
        let db = TestDb::new();
        soft_delete_project(&db.conn, 1).unwrap();
        let err = restore_cost_entry(&db.conn, 1).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn soft_delete_then_restore_single_cost_entry_when_project_alive() {
        let db = TestDb::new();
        soft_delete_cost_entry(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_some());
        restore_cost_entry(&db.conn, 1).unwrap();
        assert!(deleted_at(&db.conn, "cost_entries", 1).is_none());
    }
}
