use crate::commands::tasks::normalized_body;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    pub id: i64,
    pub task_id: i64,
    pub kind: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub body: Option<String>,
    pub occurred_at: String,
    pub created_at: String,
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<TaskEvent> {
    Ok(TaskEvent {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        kind: row.get("kind")?,
        from_status: row.get("from_status")?,
        to_status: row.get("to_status")?,
        body: row.get("body")?,
        occurred_at: row.get("occurred_at")?,
        created_at: row.get("created_at")?,
    })
}

fn get_impl(conn: &Connection, id: i64) -> AppResult<TaskEvent> {
    conn.query_row(
        "SELECT * FROM task_events WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_event,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "task_event",
            id,
        },
        other => AppError::Db(other),
    })
}

/// Notes are editable content; status_change rows are a record of what happened
/// and must stay immutable, so every mutation checks the kind first.
fn ensure_note(conn: &Connection, id: i64) -> AppResult<()> {
    let kind = get_impl(conn, id)?.kind;
    if kind != "note" {
        return Err(AppError::Validation("状态变更记录不可修改或删除".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, task_id: i64) -> AppResult<Vec<TaskEvent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM task_events
         WHERE task_id = ?1 AND deleted_at IS NULL
         ORDER BY occurred_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([task_id], row_to_event)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn create_note_impl(
    conn: &Connection,
    task_id: i64,
    body: &str,
    occurred_at: Option<&str>,
) -> AppResult<TaskEvent> {
    let body = normalized_body(Some(body))?
        .ok_or_else(|| AppError::Validation("备注内容不能为空".into()))?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        [task_id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound {
            entity: "task",
            id: task_id,
        });
    }
    // 'localtime': occurred_at is compared against timestamps typed into the
    // datetime-local picker (local wall-clock), so the fallback must be on the
    // same clock or a same-minute pause/note pair sorts hours apart.
    conn.execute(
        "INSERT INTO task_events(task_id, kind, body, occurred_at)
         VALUES(?1, 'note', ?2, COALESCE(?3, datetime('now','localtime')))",
        rusqlite::params![task_id, body, occurred_at],
    )?;
    get_impl(conn, conn.last_insert_rowid())
}

pub(crate) fn update_note_impl(conn: &Connection, id: i64, body: &str) -> AppResult<TaskEvent> {
    ensure_note(conn, id)?;
    let body = normalized_body(Some(body))?
        .ok_or_else(|| AppError::Validation("备注内容不能为空".into()))?;
    conn.execute(
        "UPDATE task_events SET body = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![body, id],
    )?;
    get_impl(conn, id)
}

pub(crate) fn delete_note_impl(conn: &Connection, id: i64) -> AppResult<()> {
    ensure_note(conn, id)?;
    conn.execute(
        "UPDATE task_events SET deleted_at = datetime('now')
         WHERE id = ?1 AND deleted_at IS NULL",
        [id],
    )?;
    Ok(())
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
pub fn list_task_events(state: tauri::State<AppState>, task_id: i64) -> AppResult<Vec<TaskEvent>> {
    with_conn(&state, |c| list_impl(c, task_id))
}
#[tauri::command]
pub fn create_task_note(
    state: tauri::State<AppState>,
    task_id: i64,
    body: String,
    occurred_at: Option<String>,
) -> AppResult<TaskEvent> {
    with_conn(&state, |c| {
        create_note_impl(c, task_id, &body, occurred_at.as_deref())
    })
}
#[tauri::command]
pub fn update_task_note(
    state: tauri::State<AppState>,
    id: i64,
    body: String,
) -> AppResult<TaskEvent> {
    with_conn(&state, |c| update_note_impl(c, id, &body))
}
#[tauri::command]
pub fn delete_task_note(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_note_impl(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use crate::commands::tasks::{create_impl as create_task_impl, TaskInput};
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
            create_task_impl(&conn, 1, &task_input("T")).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn task_input(title: &str) -> TaskInput {
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
    fn create_note_persists_and_lists() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "客户说下周再定", None).unwrap();
        assert_eq!(e.kind, "note");
        assert_eq!(e.body.as_deref(), Some("客户说下周再定"));
        let all = list_impl(&db.conn, 1).unwrap();
        assert!(all.iter().any(|x| x.id == e.id));
    }

    #[test]
    fn create_note_rejects_blank_body() {
        let db = TestDb::new();
        let err = create_note_impl(&db.conn, 1, "   ", None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn create_note_rejects_oversized_body() {
        let db = TestDb::new();
        let long = "字".repeat(2001);
        let err = create_note_impl(&db.conn, 1, &long, None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn create_note_on_missing_task_rejected() {
        let db = TestDb::new();
        let err = create_note_impl(&db.conn, 999, "x", None).unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[test]
    fn list_returns_events_in_occurred_order() {
        let db = TestDb::new();
        create_note_impl(&db.conn, 1, "第二条", Some("2026-03-02 10:00:00")).unwrap();
        create_note_impl(&db.conn, 1, "第一条", Some("2026-03-01 10:00:00")).unwrap();
        let all = list_impl(&db.conn, 1).unwrap();
        let notes: Vec<&str> = all
            .iter()
            .filter(|e| e.kind == "note")
            .map(|e| e.body.as_deref().unwrap())
            .collect();
        assert_eq!(notes, vec!["第一条", "第二条"]);
    }

    #[test]
    fn update_note_changes_body() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "旧的", None).unwrap();
        let u = update_note_impl(&db.conn, e.id, "新的").unwrap();
        assert_eq!(u.body.as_deref(), Some("新的"));
    }

    #[test]
    fn update_status_event_rejected() {
        // The seeded creation event is a status_change; it is a record, not content.
        let db = TestDb::new();
        let all = list_impl(&db.conn, 1).unwrap();
        let seeded = all.iter().find(|e| e.kind == "status_change").unwrap();
        let err = update_note_impl(&db.conn, seeded.id, "改不了").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_status_event_rejected() {
        let db = TestDb::new();
        let all = list_impl(&db.conn, 1).unwrap();
        let seeded = all.iter().find(|e| e.kind == "status_change").unwrap();
        let err = delete_note_impl(&db.conn, seeded.id).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_note_hides_it_from_list() {
        let db = TestDb::new();
        let e = create_note_impl(&db.conn, 1, "删掉我", None).unwrap();
        delete_note_impl(&db.conn, e.id).unwrap();
        let all = list_impl(&db.conn, 1).unwrap();
        assert!(!all.iter().any(|x| x.id == e.id));
    }
}
