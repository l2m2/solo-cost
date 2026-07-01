use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct TimeLog {
    pub id: i64,
    pub task_id: i64,
    pub member_id: i64,
    pub work_date: String,
    pub hours: f64,
    pub daily_cost_snapshot_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TimeLogInput {
    pub task_id: i64,
    pub member_id: i64,
    pub work_date: String,
    pub hours: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimeLogUpdateInput {
    pub work_date: String,
    pub hours: f64,
    pub notes: Option<String>,
}

fn row_to_log(row: &rusqlite::Row) -> rusqlite::Result<TimeLog> {
    Ok(TimeLog {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        member_id: row.get("member_id")?,
        work_date: row.get("work_date")?,
        hours: row.get("hours")?,
        daily_cost_snapshot_cents: row.get("daily_cost_snapshot_cents")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
    })
}

fn validate_hours(hours: f64) -> AppResult<()> {
    if !(0.0..=24.0).contains(&hours) {
        return Err(AppError::Validation("工时需在 [0, 24] 之间".into()));
    }
    Ok(())
}

fn validate_date(date: &str) -> AppResult<()> {
    if date.trim().is_empty() {
        return Err(AppError::Validation("工作日期必填".into()));
    }
    Ok(())
}

pub(crate) fn list_by_task_impl(conn: &Connection, task_id: i64) -> AppResult<Vec<TimeLog>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM time_logs
         WHERE task_id = ?1 AND deleted_at IS NULL
         ORDER BY work_date DESC, id DESC",
    )?;
    let rows = stmt.query_map([task_id], row_to_log)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn list_by_project_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<TimeLog>> {
    let mut stmt = conn.prepare(
        "SELECT tl.* FROM time_logs tl
         JOIN tasks t ON t.id = tl.task_id
         WHERE t.project_id = ?1 AND tl.deleted_at IS NULL AND t.deleted_at IS NULL
         ORDER BY tl.work_date DESC, tl.id DESC",
    )?;
    let rows = stmt.query_map([project_id], row_to_log)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn create_impl(conn: &Connection, input: &TimeLogInput) -> AppResult<TimeLog> {
    validate_hours(input.hours)?;
    validate_date(&input.work_date)?;
    // Verify task is active and load member's current daily_cost_cents.
    // The JOIN ensures task and member share the same company_id.
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT m.daily_cost_cents, p.company_id
             FROM members m JOIN projects p ON p.company_id = m.company_id
             JOIN tasks t ON t.project_id = p.id
             WHERE t.id = ?1 AND m.id = ?2 AND t.deleted_at IS NULL AND m.deleted_at IS NULL",
            [input.task_id, input.member_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let (snapshot, _company_id) = match row {
        Some(t) => t,
        None => {
            return Err(AppError::Validation(
                "任务与成员公司不一致或资源不存在".into(),
            ));
        }
    };
    conn.execute(
        "INSERT INTO time_logs(task_id, member_id, work_date, hours,
                               daily_cost_snapshot_cents, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            input.task_id,
            input.member_id,
            input.work_date.trim(),
            input.hours,
            snapshot,
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TimeLogUpdateInput,
) -> AppResult<TimeLog> {
    validate_hours(input.hours)?;
    validate_date(&input.work_date)?;
    let n = conn.execute(
        "UPDATE time_logs SET
            work_date = ?1,
            hours = ?2,
            notes = ?3
         WHERE id = ?4 AND deleted_at IS NULL",
        rusqlite::params![
            input.work_date.trim(),
            input.hours,
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "time_log",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_time_log(conn, id)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<TimeLog> {
    conn.query_row(
        "SELECT * FROM time_logs WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_log,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "time_log",
            id,
        },
        other => AppError::Db(other),
    })
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn list_time_logs_by_task(
    state: tauri::State<AppState>,
    task_id: i64,
) -> AppResult<Vec<TimeLog>> {
    with_conn(&state, |c| list_by_task_impl(c, task_id))
}

#[tauri::command]
pub fn list_time_logs_by_project(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<TimeLog>> {
    with_conn(&state, |c| list_by_project_impl(c, project_id))
}

#[tauri::command]
pub fn create_time_log(state: tauri::State<AppState>, input: TimeLogInput) -> AppResult<TimeLog> {
    with_conn(&state, |c| create_impl(c, &input))
}

#[tauri::command]
pub fn update_time_log(
    state: tauri::State<AppState>,
    id: i64,
    input: TimeLogUpdateInput,
) -> AppResult<TimeLog> {
    with_conn(&state, |c| update_impl(c, id, &input))
}

#[tauri::command]
pub fn delete_time_log(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            conn.execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input() -> TimeLogInput {
        TimeLogInput {
            task_id: 1,
            member_id: 1,
            work_date: "2026-06-01".into(),
            hours: 8.0,
            notes: None,
        }
    }

    #[test]
    fn create_snapshots_member_daily_cost() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        assert_eq!(log.daily_cost_snapshot_cents, 80000);
    }

    #[test]
    fn snapshot_does_not_change_when_member_repriced() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        db.conn
            .execute(
                "UPDATE members SET daily_cost_cents = 999999 WHERE id = 1",
                [],
            )
            .unwrap();
        let refetched = get_impl(&db.conn, log.id).unwrap();
        assert_eq!(refetched.daily_cost_snapshot_cents, 80000);
    }

    #[test]
    fn update_only_hours_date_notes() {
        let db = TestDb::new();
        let log = create_impl(&db.conn, &input()).unwrap();
        let updated = update_impl(
            &db.conn,
            log.id,
            &TimeLogUpdateInput {
                work_date: "2026-06-02".into(),
                hours: 4.0,
                notes: Some("延期".into()),
            },
        )
        .unwrap();
        assert_eq!(updated.work_date, "2026-06-02");
        assert_eq!(updated.hours, 4.0);
        assert_eq!(updated.member_id, 1); // unchanged
        assert_eq!(updated.task_id, 1); // unchanged
        assert_eq!(updated.daily_cost_snapshot_cents, 80000); // unchanged
    }

    #[test]
    fn hours_out_of_range_rejected() {
        let db = TestDb::new();
        let mut bad = input();
        bad.hours = 25.0;
        let err = create_impl(&db.conn, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn cross_company_task_member_rejected() {
        let db = TestDb::new();
        db.conn
            .execute("INSERT INTO companies(name) VALUES('Other')", [])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(2, 'Foreign', 60000)",
                [],
            )
            .unwrap();
        let mut bad = input();
        bad.member_id = 2;
        let err = create_impl(&db.conn, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
