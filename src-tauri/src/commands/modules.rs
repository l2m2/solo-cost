use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Module {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ModuleInput {
    pub name: String,
    pub sort_order: Option<i64>,
}

fn row_to_module(row: &rusqlite::Row) -> rusqlite::Result<Module> {
    Ok(Module {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &ModuleInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::Validation("模块名长度必须在 1–40 之间".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<Module>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM modules
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([project_id], row_to_module)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Module> {
    conn.query_row(
        "SELECT * FROM modules WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_module,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "module", id },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &ModuleInput,
) -> AppResult<Module> {
    validate(input)?;
    let next_order: i64 = match input.sort_order {
        Some(n) => n,
        None => conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM modules
             WHERE project_id = ?1 AND deleted_at IS NULL",
            [project_id],
            |r| r.get(0),
        )?,
    };
    conn.execute(
        "INSERT INTO modules(project_id, name, sort_order)
         VALUES(?1, ?2, ?3)",
        rusqlite::params![project_id, input.name.trim(), next_order],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ModuleInput) -> AppResult<Module> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE modules SET
            name = ?1,
            sort_order = COALESCE(?2, sort_order),
            updated_at = datetime('now')
         WHERE id = ?3 AND deleted_at IS NULL",
        rusqlite::params![input.name.trim(), input.sort_order, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound { entity: "module", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT deleted_at FROM modules WHERE id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;
    let already_deleted = match row {
        Some(x) => x,
        None => return Err(AppError::NotFound { entity: "module", id }),
    };
    if already_deleted.is_some() {
        return Ok(()); // idempotent
    }
    let attached: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE module_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if attached > 0 {
        return Err(AppError::DeleteBlocked(
            "模块下还有任务，请先删除或转移".into(),
        ));
    }
    conn.execute(
        "UPDATE modules SET deleted_at = datetime('now') WHERE id = ?1",
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
pub fn list_modules(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<Module>> {
    with_conn(&state, |c| list_impl(c, project_id))
}
#[tauri::command]
pub fn create_module(
    state: tauri::State<AppState>,
    project_id: i64,
    input: ModuleInput,
) -> AppResult<Module> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_module(
    state: tauri::State<AppState>,
    id: i64,
    input: ModuleInput,
) -> AppResult<Module> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_module(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

#[tauri::command]
pub fn get_module_labor_stats(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<crate::domain::module_stats::ModuleLaborStat>> {
    with_conn(&state, |c| crate::domain::module_stats::labor_by_module(c, project_id))
}
