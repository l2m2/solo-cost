use crate::domain::profit::{project_cost_summary, ProjectCostSummary};
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CostEntry {
    pub id: i64,
    pub project_id: i64,
    pub category_id: i64,
    pub incurred_at: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CostEntryInput {
    pub category_id: i64,
    pub incurred_at: String,
    pub amount_cents: i64,
    pub description: Option<String>,
    pub notes: Option<String>,
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<CostEntry> {
    Ok(CostEntry {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        category_id: row.get("category_id")?,
        incurred_at: row.get("incurred_at")?,
        amount_cents: row.get("amount_cents")?,
        description: row.get("description")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
    })
}

fn validate(input: &CostEntryInput) -> AppResult<()> {
    if input.amount_cents < 0 {
        return Err(AppError::Validation("金额不能为负".into()));
    }
    if input.incurred_at.trim().is_empty() {
        return Err(AppError::Validation("发生日期必填".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<CostEntry>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cost_entries
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY incurred_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([project_id], row_to_entry)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &CostEntryInput,
) -> AppResult<CostEntry> {
    validate(input)?;
    // Verify category belongs to the same company as the project (defense in depth).
    let ok: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories cc
         JOIN projects p ON p.company_id = cc.company_id
         WHERE p.id = ?1 AND cc.id = ?2 AND cc.deleted_at IS NULL",
        [project_id, input.category_id],
        |r| r.get(0),
    )?;
    if ok == 0 {
        return Err(AppError::Validation(
            "科目与项目公司不匹配或科目不存在".into(),
        ));
    }
    conn.execute(
        "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents, description, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            project_id,
            input.category_id,
            input.incurred_at.trim(),
            input.amount_cents,
            input.description.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &CostEntryInput,
) -> AppResult<CostEntry> {
    validate(input)?;
    // verify category belongs to the project's company (defense in depth — M2-T5 originally accepted gap)
    let ok: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories cc
         JOIN projects p ON p.company_id = cc.company_id
         JOIN cost_entries ce ON ce.project_id = p.id
         WHERE ce.id = ?1 AND cc.id = ?2 AND cc.deleted_at IS NULL",
        [id, input.category_id],
        |r| r.get(0),
    )?;
    if ok == 0 {
        return Err(AppError::Validation(
            "科目与项目公司不匹配或科目不存在".into(),
        ));
    }
    let n = conn.execute(
        "UPDATE cost_entries SET
            category_id = ?1,
            incurred_at = ?2,
            amount_cents = ?3,
            description = ?4,
            notes = ?5
         WHERE id = ?6 AND deleted_at IS NULL",
        rusqlite::params![
            input.category_id,
            input.incurred_at.trim(),
            input.amount_cents,
            input.description.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "cost_entry",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_cost_entry(conn, id)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<CostEntry> {
    conn.query_row(
        "SELECT * FROM cost_entries WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_entry,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "cost_entry",
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
pub fn list_cost_entries(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<CostEntry>> {
    with_conn(&state, |c| list_impl(c, project_id))
}

#[tauri::command]
pub fn create_cost_entry(
    state: tauri::State<AppState>,
    project_id: i64,
    input: CostEntryInput,
) -> AppResult<CostEntry> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}

#[tauri::command]
pub fn update_cost_entry(
    state: tauri::State<AppState>,
    id: i64,
    input: CostEntryInput,
) -> AppResult<CostEntry> {
    with_conn(&state, |c| update_impl(c, id, &input))
}

#[tauri::command]
pub fn delete_cost_entry(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

#[tauri::command]
pub fn get_project_cost_summary(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<ProjectCostSummary> {
    with_conn(&state, |c| project_cost_summary(c, project_id))
}
