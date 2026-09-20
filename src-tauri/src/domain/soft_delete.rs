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
    tx.execute(
        "UPDATE contract_payments SET deleted_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    // time_logs cascade through tasks: capture which tasks were active before tagging tasks
    tx.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE deleted_at IS NULL
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?2 AND deleted_at IS NULL)",
        rusqlite::params![ts, id],
    )?;
    // task_events cascade through tasks too; must run before the tasks UPDATE
    // below, since it selects tasks via deleted_at IS NULL.
    tx.execute(
        "UPDATE task_events SET deleted_at = ?1
         WHERE deleted_at IS NULL
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?2 AND deleted_at IS NULL)",
        rusqlite::params![ts, id],
    )?;
    tx.execute(
        "UPDATE tasks SET deleted_at = ?1
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
    tx.execute(
        "UPDATE contract_payments SET deleted_at = NULL
         WHERE project_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE time_logs SET deleted_at = NULL
         WHERE deleted_at = ?2
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE task_events SET deleted_at = NULL
         WHERE deleted_at = ?2
           AND task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE tasks SET deleted_at = NULL
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

pub fn soft_delete_task(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    tx.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE task_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.execute(
        "UPDATE task_events SET deleted_at = ?1
         WHERE task_id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore_task(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT t.deleted_at IS NOT NULL, p.deleted_at
             FROM tasks t JOIN projects p ON p.id = t.project_id
             WHERE t.id = ?1",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let (_task_is_deleted, project_deleted_at) = match row {
        Some(t) => t,
        None => return Err(AppError::NotFound { entity: "task", id }),
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("项目已删除，请先恢复项目".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let ts: Option<String> =
        tx.query_row("SELECT deleted_at FROM tasks WHERE id = ?1", [id], |r| {
            r.get(0)
        })?;
    let ts = match ts {
        Some(t) => t,
        None => return Ok(()),
    };
    tx.execute("UPDATE tasks SET deleted_at = NULL WHERE id = ?1", [id])?;
    tx.execute(
        "UPDATE time_logs SET deleted_at = NULL
         WHERE task_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.execute(
        "UPDATE task_events SET deleted_at = NULL
         WHERE task_id = ?1 AND deleted_at = ?2",
        rusqlite::params![id, ts],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn soft_delete_payment(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE contract_payments SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "contract_payment",
            id,
        });
    }
    Ok(())
}

pub fn restore_payment(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT p.deleted_at
             FROM contract_payments cp JOIN projects p ON p.id = cp.project_id
             WHERE cp.id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let project_deleted_at = match row {
        Some(t) => t,
        None => {
            return Err(AppError::NotFound {
                entity: "contract_payment",
                id,
            })
        }
    };
    if project_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("项目已删除，请先恢复项目".into()));
    }
    conn.execute(
        "UPDATE contract_payments SET deleted_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn soft_delete_time_log(conn: &Connection, id: i64) -> AppResult<()> {
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE time_logs SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "time_log",
            id,
        });
    }
    Ok(())
}

pub fn restore_time_log(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT t.deleted_at
             FROM time_logs tl JOIN tasks t ON t.id = tl.task_id
             WHERE tl.id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let task_deleted_at = match row {
        Some(t) => t,
        None => {
            return Err(AppError::NotFound {
                entity: "time_log",
                id,
            })
        }
    };
    if task_deleted_at.is_some() {
        return Err(AppError::DeleteBlocked("任务已删除，请先恢复任务".into()));
    }
    conn.execute("UPDATE time_logs SET deleted_at = NULL WHERE id = ?1", [id])?;
    Ok(())
}

pub fn soft_delete_member(conn: &Connection, id: i64) -> AppResult<()> {
    let active_logs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM time_logs WHERE member_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if active_logs > 0 {
        return Err(AppError::DeleteBlocked(format!(
            "该成员有 {active_logs} 条工时记录，请先归档（设 is_active=0）"
        )));
    }
    let ts = now_iso(conn)?;
    let n = conn.execute(
        "UPDATE members SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![ts, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "member",
            id,
        });
    }
    Ok(())
}
