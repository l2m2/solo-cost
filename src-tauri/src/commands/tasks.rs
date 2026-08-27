use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

const ALLOWED_STATUSES: [&str; 5] = ["todo", "in_progress", "paused", "done", "closed"];

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub status: String,
    pub estimated_hours: Option<f64>,
    pub actual_hours: f64,
    pub due_date: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub module_id: Option<i64>,
    pub external_ref: Option<String>,
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
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    pub module_id: Option<i64>,
    pub external_ref: Option<String>,
    // Optional source creation timestamp (zentao import); manual tasks fall back to now().
    #[serde(default)]
    pub created_at: Option<String>,
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
        actual_hours: row.get("actual_hours")?,
        due_date: row.get("due_date")?,
        started_at: row.get("started_at")?,
        completed_at: row.get("completed_at")?,
        module_id: row.get("module_id")?,
        external_ref: row.get("external_ref")?,
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

#[derive(Debug, Deserialize)]
pub struct StatusChange {
    pub to: String,
    #[serde(default)]
    pub occurred_at: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

fn status_label(s: &str) -> &str {
    match s {
        "todo" => "待办",
        "in_progress" => "进行中",
        "paused" => "已暂停",
        "done" => "已完成",
        "closed" => "已关闭",
        other => other,
    }
}

fn transition_allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("todo", "in_progress")
            | ("todo", "closed")
            | ("in_progress", "paused")
            | ("in_progress", "done")
            | ("in_progress", "closed")
            | ("paused", "in_progress")
            | ("paused", "done")
            | ("paused", "closed")
            | ("done", "closed")
    )
}

/// Trims the body and rejects blanks / oversized text. Shared by transitions
/// and by the note commands so both enforce the same limit.
pub(crate) fn normalized_body(body: Option<&str>) -> AppResult<Option<String>> {
    let Some(b) = body.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if b.chars().count() > 2000 {
        return Err(AppError::Validation("内容长度不能超过 2000 字".into()));
    }
    Ok(Some(b.to_string()))
}

/// Moves a task to a new status and records it on the timeline. Returns false
/// when the task already sits in the target status (no-op, no event written).
/// Every status change goes through here — a second write path that skipped the
/// event is exactly what makes a timeline untrustworthy.
fn apply_transition(conn: &Connection, id: i64, change: &StatusChange) -> AppResult<bool> {
    if !ALLOWED_STATUSES.contains(&change.to.as_str()) {
        return Err(AppError::Validation(format!("非法状态：{}", change.to)));
    }
    let from: String = conn
        .query_row(
            "SELECT status FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
            other => AppError::Db(other),
        })?;
    if from == change.to {
        return Ok(false);
    }
    if !transition_allowed(&from, &change.to) {
        return Err(AppError::Validation(format!(
            "任务不能从「{}」变为「{}」",
            status_label(&from),
            status_label(&change.to)
        )));
    }
    let body = normalized_body(change.body.as_deref())?;
    if change.to == "paused" && body.is_none() {
        return Err(AppError::Validation("暂停任务必须填写原因".into()));
    }
    conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![change.to, id],
    )?;
    // 'localtime': occurred_at is compared against timestamps typed into the
    // datetime-local picker (local wall-clock), so the fallback must be on the
    // same clock or a same-minute pause/note pair sorts hours apart.
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, body, occurred_at)
         VALUES(?1, 'status_change', ?2, ?3, ?4, COALESCE(?5, datetime('now','localtime')))",
        rusqlite::params![id, from, change.to, body, change.occurred_at.as_deref()],
    )?;
    Ok(true)
}

/// Seeds the timeline for a freshly created task. Mirrors the 0009 backfill so
/// imported tasks (which arrive with source dates already set) get the same
/// history as tasks that predate the events table.
fn seed_creation_events(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change', NULL, 'todo', created_at FROM tasks WHERE id = ?1",
        [id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change', 'todo', 'in_progress', started_at
         FROM tasks WHERE id = ?1 AND started_at IS NOT NULL",
        [id],
    )?;
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT id, 'status_change',
                CASE WHEN started_at IS NOT NULL THEN 'in_progress' ELSE 'todo' END,
                CASE WHEN status = 'closed' THEN 'closed' ELSE 'done' END,
                completed_at
         FROM tasks WHERE id = ?1 AND completed_at IS NOT NULL",
        [id],
    )?;
    // occurred_at uses MAX(created_at, started_at) rather than created_at alone:
    // a task can be 'done' with started_at set but completed_at NULL (zentao
    // import with a blank finish date), and created_at alone would place this
    // catch-all event before the in_progress event seeded above it, showing
    // the task as finished before it started. Kept identical to the 0009
    // migration's catch-all so imported and migrated tasks agree.
    conn.execute(
        "INSERT INTO task_events(task_id, kind, from_status, to_status, occurred_at)
         SELECT t.id, 'status_change', NULL, t.status,
                MAX(t.created_at, COALESCE(t.started_at, t.created_at))
         FROM tasks t
         WHERE t.id = ?1
           AND NOT EXISTS (
               SELECT 1 FROM task_events e WHERE e.task_id = t.id AND e.to_status = t.status
           )",
        [id],
    )?;
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
            "SELECT t.*,
                    COALESCE((SELECT SUM(hours) FROM time_logs
                              WHERE task_id = t.id AND deleted_at IS NULL), 0.0) AS actual_hours
             FROM tasks t
             WHERE t.project_id = ?1 AND t.status = ?2 AND t.deleted_at IS NULL
             ORDER BY t.id DESC",
            vec![project_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT t.*,
                    COALESCE((SELECT SUM(hours) FROM time_logs
                              WHERE task_id = t.id AND deleted_at IS NULL), 0.0) AS actual_hours
             FROM tasks t
             WHERE t.project_id = ?1 AND t.deleted_at IS NULL
             ORDER BY t.id DESC",
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
        "SELECT t.*,
                COALESCE((SELECT SUM(hours) FROM time_logs
                          WHERE task_id = t.id AND deleted_at IS NULL), 0.0) AS actual_hours
         FROM tasks t
         WHERE t.id = ?1 AND t.deleted_at IS NULL",
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
                           status, estimated_hours, due_date, started_at, completed_at,
                           module_id, external_ref, created_at)
         VALUES(?1, ?2, ?3, ?4, COALESCE(?5, 'todo'), ?6, ?7, ?8, ?9,
                ?10, ?11, COALESCE(?12, datetime('now')))",
        rusqlite::params![
            project_id,
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.status.as_deref(),
            input.estimated_hours,
            input.due_date.as_deref(),
            input.started_at.as_deref(),
            input.completed_at.as_deref(),
            input.module_id,
            input.external_ref.as_deref(),
            input.created_at.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    seed_creation_events(conn, id)?;
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &TaskInput,
    change: Option<&StatusChange>,
) -> AppResult<Task> {
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
    // Fields and the status change land in one transaction: the completion
    // dialog submits a timestamp, a description and the new status together.
    let tx = conn.unchecked_transaction()?;
    // input.status is deliberately not applied here. It survives only for
    // create_task / zentao import; routing every status change through
    // apply_transition is what keeps the timeline complete.
    let n = tx.execute(
        "UPDATE tasks SET
            title = ?1,
            description = ?2,
            assignee_id = ?3,
            estimated_hours = ?4,
            due_date = ?5,
            started_at = ?6,
            completed_at = ?7,
            module_id = ?8,
            updated_at = datetime('now')
         WHERE id = ?9 AND deleted_at IS NULL",
        rusqlite::params![
            input.title.trim(),
            input.description.as_deref(),
            input.assignee_id,
            input.estimated_hours,
            input.due_date.as_deref(),
            input.started_at.as_deref(),
            input.completed_at.as_deref(),
            input.module_id,
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "task", id });
    }
    if let Some(c) = change {
        apply_transition(&tx, id, c)?;
    }
    tx.commit()?;
    get_impl(conn, id)
}

pub(crate) fn set_status_impl(
    conn: &Connection,
    id: i64,
    change: &StatusChange,
) -> AppResult<Task> {
    let tx = conn.unchecked_transaction()?;
    apply_transition(&tx, id, change)?;
    tx.commit()?;
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
    change: Option<StatusChange>,
) -> AppResult<Task> {
    with_conn(&state, |c| update_impl(c, id, &input, change.as_ref()))
}
#[tauri::command]
pub fn set_task_status(
    state: tauri::State<AppState>,
    id: i64,
    change: StatusChange,
) -> AppResult<Task> {
    with_conn(&state, |c| set_status_impl(c, id, &change))
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
            started_at: None,
            completed_at: None,
            module_id: None,
            external_ref: None,
            created_at: None,
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

    fn change(to: &str) -> StatusChange {
        StatusChange { to: to.into(), occurred_at: None, body: None }
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let u = set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn illegal_transition_rejected() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        // todo -> done is not a legal edge; a task must be started first.
        let err = set_status_impl(&db.conn, t.id, &change("done")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn same_status_is_a_noop_without_event() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let before = event_count(&db.conn, t.id);
        set_status_impl(&db.conn, t.id, &change("todo")).unwrap();
        assert_eq!(event_count(&db.conn, t.id), before);
    }

    #[test]
    fn pause_without_body_rejected() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let err = set_status_impl(&db.conn, t.id, &change("paused")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn pause_with_body_writes_event() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let c = StatusChange {
            to: "paused".into(),
            occurred_at: Some("2026-03-01 10:00:00".into()),
            body: Some("等客户素材".into()),
        };
        let u = set_status_impl(&db.conn, t.id, &c).unwrap();
        assert_eq!(u.status, "paused");
        let (from, to, body, at): (String, String, String, String) = db
            .conn
            .query_row(
                "SELECT from_status, to_status, body, occurred_at FROM task_events
                 WHERE task_id = ?1 AND to_status = 'paused'",
                [t.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((from.as_str(), to.as_str(), body.as_str(), at.as_str()),
                   ("in_progress", "paused", "等客户素材", "2026-03-01 10:00:00"));
    }

    #[test]
    fn resume_from_paused_allowed() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        let c = StatusChange {
            to: "paused".into(),
            occurred_at: None,
            body: Some("等客户".into()),
        };
        set_status_impl(&db.conn, t.id, &c).unwrap();
        let u = set_status_impl(&db.conn, t.id, &change("in_progress")).unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn update_ignores_status_in_input() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let mut i = input("T");
        i.status = Some("done".into());
        let u = update_impl(&db.conn, t.id, &i, None).unwrap();
        assert_eq!(u.status, "todo");
    }

    #[test]
    fn update_with_change_applies_fields_and_status_together() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let mut i = input("T");
        i.description = Some("改过的描述".into());
        i.started_at = Some("2026-03-01 09:00:00".into());
        let c = StatusChange {
            to: "in_progress".into(),
            occurred_at: Some("2026-03-01 09:00:00".into()),
            body: None,
        };
        let u = update_impl(&db.conn, t.id, &i, Some(&c)).unwrap();
        assert_eq!(u.status, "in_progress");
        assert_eq!(u.description.as_deref(), Some("改过的描述"));
        assert_eq!(u.started_at.as_deref(), Some("2026-03-01 09:00:00"));
    }

    #[test]
    fn create_seeds_timeline() {
        let db = TestDb::new();
        let t = create_impl(&db.conn, 1, &input("T")).unwrap();
        let to: String = db
            .conn
            .query_row(
                "SELECT to_status FROM task_events WHERE task_id = ?1",
                [t.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(to, "todo");
    }

    #[test]
    fn create_with_source_dates_seeds_full_timeline() {
        // Mirrors a zentao import: the task arrives already finished.
        let db = TestDb::new();
        let mut i = input("T");
        i.status = Some("done".into());
        i.started_at = Some("2026-01-02 09:00:00".into());
        i.completed_at = Some("2026-01-05 18:00:00".into());
        i.created_at = Some("2026-01-01 08:00:00".into());
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(event_count(&db.conn, t.id), 3);
    }

    #[test]
    fn create_with_started_but_no_completed_orders_done_after_in_progress() {
        // Mirrors a zentao import whose 完成日期 was left blank: status is
        // 'done' but completed_at is NULL, so the third targeted insert in
        // seed_creation_events never fires and the catch-all has to produce
        // the 'done' event. Before the fix it fell back to created_at, which
        // sorts before the 'in_progress' event seeded from started_at.
        let db = TestDb::new();
        let mut i = input("T");
        i.status = Some("done".into());
        i.started_at = Some("2026-01-02 09:00:00".into());
        i.created_at = Some("2026-01-01 08:00:00".into());
        let t = create_impl(&db.conn, 1, &i).unwrap();

        let mut stmt = db
            .conn
            .prepare(
                "SELECT to_status, occurred_at FROM task_events
                 WHERE task_id = ?1 ORDER BY occurred_at ASC, id ASC",
            )
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([t.id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let statuses: Vec<&str> = rows.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(statuses, vec!["todo", "in_progress", "done"]);

        let in_progress_at = &rows[1].1;
        let done_at = &rows[2].1;
        assert!(
            done_at.as_str() >= in_progress_at.as_str(),
            "done ({done_at}) must not be ordered before in_progress ({in_progress_at})"
        );
    }

    fn event_count(conn: &Connection, task_id: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND deleted_at IS NULL",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
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
        let updated = update_impl(&db.conn, t.id, &u, None).unwrap();
        assert_eq!(updated.module_id, None);
    }

    #[test]
    fn create_task_persists_external_ref() {
        let db = TestDb::new();
        let mut i = input("T");
        i.external_ref = Some("zentao:368".into());
        let t = create_impl(&db.conn, 1, &i).unwrap();
        assert_eq!(t.external_ref.as_deref(), Some("zentao:368"));
    }

    #[test]
    fn external_ref_unique_index_rejects_duplicate_in_same_project() {
        let db = TestDb::new();
        let mut i = input("T1");
        i.external_ref = Some("zentao:368".into());
        create_impl(&db.conn, 1, &i).unwrap();
        // second insert into same project with same external_ref → SQLite UNIQUE violation
        let mut j = input("T2");
        j.external_ref = Some("zentao:368".into());
        let err = create_impl(&db.conn, 1, &j).unwrap_err();
        assert!(matches!(err, AppError::Db(_)));
    }

    #[test]
    fn external_ref_unique_index_allows_same_id_across_projects() {
        let db = TestDb::new();
        db.conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P2')", []).unwrap();
        let mut i = input("T1");
        i.external_ref = Some("zentao:368".into());
        create_impl(&db.conn, 1, &i).unwrap();
        let mut j = input("T2");
        j.external_ref = Some("zentao:368".into());
        // project 2 can hold the same external_ref
        create_impl(&db.conn, 2, &j).unwrap();
    }
}
