use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ContractPayment {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: Option<String>,
    pub actual_amount_cents: Option<i64>,
    pub actual_received_at: Option<String>,
    pub sort_order: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentInput {
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: Option<String>,
    pub actual_amount_cents: Option<i64>,
    pub actual_received_at: Option<String>,
    pub notes: Option<String>,
}

fn row_to_payment(row: &rusqlite::Row) -> rusqlite::Result<ContractPayment> {
    Ok(ContractPayment {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        expected_amount_cents: row.get("expected_amount_cents")?,
        expected_date: row.get("expected_date")?,
        actual_amount_cents: row.get("actual_amount_cents")?,
        actual_received_at: row.get("actual_received_at")?,
        sort_order: row.get("sort_order")?,
        notes: row.get("notes")?,
    })
}

fn validate(input: &PaymentInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::Validation(
            "收款节点名长度必须在 1–60 之间".into(),
        ));
    }
    if input.expected_amount_cents < 0 {
        return Err(AppError::Validation("预期金额不能为负".into()));
    }
    if let Some(a) = input.actual_amount_cents {
        if a < 0 {
            return Err(AppError::Validation("实收金额不能为负".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, project_id: i64) -> AppResult<Vec<ContractPayment>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([project_id], row_to_payment)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<ContractPayment> {
    conn.query_row(
        "SELECT * FROM contract_payments WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_payment,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "contract_payment",
            id,
        },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    project_id: i64,
    input: &PaymentInput,
) -> AppResult<ContractPayment> {
    validate(input)?;
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM contract_payments WHERE project_id = ?1",
        [project_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                                       expected_date, actual_amount_cents,
                                       actual_received_at, sort_order, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            project_id,
            input.name.trim(),
            input.expected_amount_cents,
            input.expected_date.as_deref(),
            input.actual_amount_cents,
            input.actual_received_at.as_deref(),
            next_order,
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &PaymentInput,
) -> AppResult<ContractPayment> {
    validate(input)?;
    // Cheap existence check so we return NotFound rather than falling through to a 0-row UPDATE
    // that would only surface as NotFound after the invalid data was already validated. Guards
    // the "wrong error type for missing entity" pattern (M2 review F2 carry-over).
    let _existing = get_impl(conn, id)?;
    let n = conn.execute(
        "UPDATE contract_payments SET
            name = ?1,
            expected_amount_cents = ?2,
            expected_date = ?3,
            actual_amount_cents = ?4,
            actual_received_at = ?5,
            notes = ?6
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.expected_amount_cents,
            input.expected_date.as_deref(),
            input.actual_amount_cents,
            input.actual_received_at.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "contract_payment",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn mark_received_impl(
    conn: &Connection,
    id: i64,
    actual_amount_cents: i64,
    actual_received_at: &str,
) -> AppResult<ContractPayment> {
    if actual_amount_cents < 0 {
        return Err(AppError::Validation("实收金额不能为负".into()));
    }
    if actual_received_at.trim().is_empty() {
        return Err(AppError::Validation("实收日期必填".into()));
    }
    let n = conn.execute(
        "UPDATE contract_payments SET
            actual_amount_cents = ?1,
            actual_received_at = ?2
         WHERE id = ?3 AND deleted_at IS NULL",
        rusqlite::params![actual_amount_cents, actual_received_at.trim(), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "contract_payment",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_payment(conn, id)
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
pub fn list_payments(
    state: tauri::State<AppState>,
    project_id: i64,
) -> AppResult<Vec<ContractPayment>> {
    with_conn(&state, |c| list_impl(c, project_id))
}
#[tauri::command]
pub fn get_payment(state: tauri::State<AppState>, id: i64) -> AppResult<ContractPayment> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_payment(
    state: tauri::State<AppState>,
    project_id: i64,
    input: PaymentInput,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| create_impl(c, project_id, &input))
}
#[tauri::command]
pub fn update_payment(
    state: tauri::State<AppState>,
    id: i64,
    input: PaymentInput,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn mark_payment_received(
    state: tauri::State<AppState>,
    id: i64,
    actual_amount_cents: i64,
    actual_received_at: String,
) -> AppResult<ContractPayment> {
    with_conn(&state, |c| {
        mark_received_impl(c, id, actual_amount_cents, &actual_received_at)
    })
}
#[tauri::command]
pub fn delete_payment(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}
