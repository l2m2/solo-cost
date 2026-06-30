use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const ALLOWED_STATUSES: [&str; 3] = ["todo", "in_progress", "done"];

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub status: String,
    pub estimated_hours: Option<f64>,
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskInput {
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub status: Option<String>,
    pub estimated_hours: Option<f64>,
    pub due_date: Option<String>,
}

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        assignee_id: row.get("assignee_id")?,
        status: row.get("status")?,
        estimated_hours: row.get("estimated_hours")?,
        due_date: row.get("due_date")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &TaskInput) -> AppResult<()> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(AppError::Validation("任务标题长度必须在 1–120 之间".into()));
    }
    if let Some(s) = &input.status {
        if !ALLOWED_STATUSES.contains(&s.as_str()) {
            return Err(AppError::Validation(format!("非法状态：{s}")));
        }
    }
    if let Some(h) = input.estimated_hours {
        if !(0.0..=9999.0).contains(&h) {
            return Err(AppError::Validation("预估工时需在 [0, 9999] 之间".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(
    conn: &Connection,
    project_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Task>> {
    let (sql, params): (&str, Vec<rusqlite::types::Value>) = match status {
        Some(s) => (
            "SELECT * FROM tasks
             WHERE project_id = ?1 AND status = ?2 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![project_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT * FROM tasks
             WHERE project_id = ?1 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![project_id.into()],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Task> {
    conn.query_row(
        "SELECT * FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_task,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &TaskInput,
) -> AppResult<Task> {
    validate(input)?;
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, assignee_id,
                           status, estimated_hours, due_date)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7)",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TaskInput,
) -> AppResult<Task> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            status = COALESCE(?4, status),
            estimated_hours = ?5,
            due_date = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    get_impl(conn, id)
}

pub(crate) fn set_status_impl(conn: &Connection, id: i64, status: &str) -> AppResult<Task> {
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(AppError::Validation(format!("非法状态：{status}")));
    }
    let n = conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![status, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_task(conn, id)
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
pub fn list_tasks(
    state: tauri::State<AppState>,
    project_id: i64,
    status: Option<String>,
) -> AppResult<Vec<Task>> {
    with_conn(&state, |c| list_impl(c, project_id, status.as_deref()))
}
#[tauri::command]
pub fn get_task(state: tauri::State<AppState>, id: i64) -> AppResult<Task> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_task(
    state: tauri::State<AppState>,
    project_id: i64,
    input: TaskInput,
) -> AppResult<Task> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_task(
    state: tauri::State<AppState>,
    id: i64,
    input: TaskInput,
) -> AppResult<Task> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_task_status(
    state: tauri::State<AppState>,
    id: i64,
    status: String,
) -> AppResult<Task> {
    with_conn(&state, |c| set_status_impl(c, id, &status))
}
#[tauri::command]
pub fn delete_task(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input(title: &str) -> TaskInput {
        TaskInput {
            title: title.into(),
            description: None,
            assignee_id: None,
            status: None,
            estimated_hours: None,
            due_date: None,
        }
    }

    #[test]
    fn create_defaults_status_todo() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        assert_eq!(t.status, "todo");
    }

    #[test]
    fn validate_bad_status() {
        let db = TestDb::new();
        let mut i = input("T");
        i.status = Some("foo".into());
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let db = TestDb::new();
        let mut a = input("A"); a.status = Some("todo".into());
        let mut b = input("B"); b.status = Some("done".into());
        create_impl(&db.conn, 1, &a).unwrap();
        create_impl(&db.conn, 1, &b).unwrap();
        assert_eq!(list_impl(&db.conn, 1, Some("done")).unwrap().len(), 1);
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let u = set_status_impl(&db.conn, t.id, "in_progress").unwrap();
        assert_eq!(u.status, "in_progress");
    }
}
