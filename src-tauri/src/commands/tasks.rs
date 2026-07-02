use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
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
    pub module_id: Option<i64>,
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
    pub module_id: Option<i64>,
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
        module_id: row.get("module_id")?,
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

fn validate_module_belongs_to_project(
    conn: &Connection,
    module_id: Option<i64>,
    task_project_id: i64,
) -> AppResult<()> {
    let Some(mid) = module_id else { return Ok(()); };
    let pid: Option<i64> = conn
        .query_row(
            "SELECT project_id FROM modules WHERE id = ?1 AND deleted_at IS NULL",
            [mid],
            |r| r.get(0),
        )
        .optional()?;
    match pid {
        Some(p) if p == task_project_id => Ok(()),
        _ => Err(AppError::Validation("模块不属于当前项目".into())),
    }
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
    validate_module_belongs_to_project(conn, input.module_id, project_id)?;
    if let Some(assignee_id) = input.assignee_id {
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM members m
             JOIN projects p ON p.company_id = m.company_id
             WHERE p.id = ?1 AND m.id = ?2 AND m.deleted_at IS NULL",
            [project_id, assignee_id],
            |r| r.get(0),
        )?;
        if ok == 0 {
            return Err(AppError::Validation(
                "负责人不属该项目所在公司或已归档/删除".into(),
            ));
        }
    }
    conn.execute(
        "INSERT INTO tasks(project_id, title, description, assignee_id,
                           status, estimated_hours, due_date, module_id)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7, ?8)",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.module_id,
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &TaskInput) -> AppResult<Task> {
    validate(input)?;
    let project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
            other => AppError::Db(other),
        })?;
    validate_module_belongs_to_project(conn, input.module_id, project_id)?;
    if let Some(assignee_id) = input.assignee_id {
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM members m
             JOIN projects p ON p.company_id = m.company_id
             WHERE p.id = ?1 AND m.id = ?2 AND m.deleted_at IS NULL",
            [project_id, assignee_id],
            |r| r.get(0),
        )?;
        if ok == 0 {
            return Err(AppError::Validation(
                "负责人不属该项目所在公司或已归档/删除".into(),
            ));
        }
    }
    let n = conn.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            status = COALESCE(?4, status),
            estimated_hours = ?5,
            due_date = ?6,
            module_id = ?7,
            updated_at = datetime('now')
         WHERE id = ?8 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.module_id,
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
pub fn update_task(state: tauri::State<AppState>, id: i64, input: TaskInput) -> AppResult<Task> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_task_status(state: tauri::State<AppState>, id: i64, status: String) -> AppResult<Task> {
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
            module_id: None,
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
        let mut a = input("A");
        a.status = Some("todo".into());
        let mut b = input("B");
        b.status = Some("done".into());
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

    #[test]
    fn create_with_cross_company_assignee_rejected() {
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
        let mut i = input("T");
        i.assignee_id = Some(1);
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn create_with_own_company_assignee_ok() {
        let db = TestDb::new();
        db.conn
            .execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
                [],
            )
            .unwrap();
        let mut i = input("T");
        i.assignee_id = Some(1);
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(t.assignee_id, Some(1));
    }

    #[test]
    fn create_task_with_module_persists_module_id() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(t.module_id, Some(1));
    }

    #[test]
    fn create_task_rejects_module_from_other_project() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P2')", []).unwrap();
        // module belongs to project 2
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(2, 'X', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        // create task under project 1 with module of project 2 → Validation
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn update_task_can_clear_module_to_null() {
        let db = TestDb::new();
        db.conn.execute(
            "INSERT INTO modules(project_id, name, sort_order) VALUES(1, '前端', 0)",
            [],
        ).unwrap();
        let mut i = input("T");
        i.module_id = Some(1);
        let t = create_impl(&db.conn, 1, &i).unwrap();
        let mut u = input("T");
        u.module_id = None;
        let updated = update_impl(&db.conn, t.id, &u).unwrap();
        assert_eq!(updated.module_id, None);
    }
}
