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
