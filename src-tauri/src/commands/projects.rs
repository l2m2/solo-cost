use crate::commands::categories;
use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const ALLOWED_STATUSES: [&str; 6] = [
    "negotiating",
    "pending",
    "in_progress",
    "delivered",
    "settled",
    "cancelled",
];

const ALLOWED_COMMISSION_MODES: [&str; 3] = ["none", "rate", "fixed"];

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub client_id: Option<i64>,
    /// Read-only convenience: populated via LEFT JOIN clients. Not settable on write.
    pub client_name: Option<String>,
    pub status: String,
    pub is_archived: bool,
    pub cancelled_from: Option<String>,
    pub contract_amount_cents: i64,
    pub contract_amount_is_tax_inclusive: bool,
    pub tax_rate: f64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
    pub commission_mode: String,
    pub commission_rate: Option<f64>,
    pub commission_amount_cents: Option<i64>,
    pub commission_settled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub client_id: Option<i64>,
    pub status: Option<String>,
    pub contract_amount_cents: Option<i64>,
    pub contract_amount_is_tax_inclusive: Option<bool>,
    pub tax_rate: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
    pub commission_mode: Option<String>,
    pub commission_rate: Option<f64>,
    pub commission_amount_cents: Option<i64>,
    pub commission_settled: Option<bool>,
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        client_id: row.get("client_id")?,
        client_name: row.get("client_name")?,
        status: row.get("status")?,
        is_archived: row.get("is_archived")?,
        cancelled_from: row.get("cancelled_from")?,
        contract_amount_cents: row.get("contract_amount_cents")?,
        contract_amount_is_tax_inclusive: row.get::<_, i64>("contract_amount_is_tax_inclusive")?
            != 0,
        tax_rate: row.get("tax_rate")?,
        start_date: row.get("start_date")?,
        end_date: row.get("end_date")?,
        actual_delivered_at: row.get("actual_delivered_at")?,
        notes: row.get("notes")?,
        commission_mode: row.get("commission_mode")?,
        commission_rate: row.get("commission_rate")?,
        commission_amount_cents: row.get("commission_amount_cents")?,
        commission_settled: row.get::<_, i64>("commission_settled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &ProjectInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::Validation("项目名长度必须在 1–120 之间".into()));
    }
    if let Some(ref s) = input.status {
        if !ALLOWED_STATUSES.contains(&s.as_str()) {
            return Err(AppError::Validation(format!("非法状态：{s}")));
        }
    }
    if let Some(rate) = input.tax_rate {
        if !(0.0..1.0).contains(&rate) {
            return Err(AppError::Validation("税率必须在 [0, 1) 之间".into()));
        }
    }
    if let Some(amt) = input.contract_amount_cents {
        if amt < 0 {
            return Err(AppError::Validation("合同金额不能为负".into()));
        }
    }
    if let Some(ref m) = input.commission_mode {
        if !ALLOWED_COMMISSION_MODES.contains(&m.as_str()) {
            return Err(AppError::Validation(format!("非法提成模式：{m}")));
        }
        match m.as_str() {
            "rate" => {
                let r = input.commission_rate.unwrap_or(0.0);
                if !(0.0..=1.0).contains(&r) {
                    return Err(AppError::Validation("提成率必须在 [0, 1] 之间".into()));
                }
            }
            "fixed" => {
                let a = input.commission_amount_cents.unwrap_or(0);
                if a < 0 {
                    return Err(AppError::Validation("固定提成金额不能为负".into()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn list_impl(
    conn: &Connection,
    company_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Project>> {
    let (sql, params): (&str, Vec<rusqlite::types::Value>) = match status {
        Some(s) => (
            "SELECT p.*, c.name AS client_name
             FROM projects p LEFT JOIN clients c ON c.id = p.client_id
             WHERE p.company_id = ?1 AND ((?2 = 'archived' AND p.is_archived = 1) OR (p.is_archived = 0 AND p.status = ?2)) AND p.deleted_at IS NULL
             ORDER BY p.id DESC",
            vec![company_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT p.*, c.name AS client_name
             FROM projects p LEFT JOIN clients c ON c.id = p.client_id
             WHERE p.company_id = ?1 AND p.deleted_at IS NULL
             ORDER BY p.id DESC",
            vec![company_id.into()],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_project)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Project> {
    conn.query_row(
        "SELECT p.*, c.name AS client_name
         FROM projects p LEFT JOIN clients c ON c.id = p.client_id
         WHERE p.id = ?1 AND p.deleted_at IS NULL",
        [id],
        row_to_project,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "project",
            id,
        },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &ProjectInput,
) -> AppResult<Project> {
    validate(input)?;
    // ensure presets exist for this company so the first cost entry has at least one category
    categories::ensure_presets(conn, company_id)?;
    conn.execute(
        "INSERT INTO projects(
            company_id, name, client_id, status,
            contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
            start_date, end_date, actual_delivered_at, notes,
            commission_mode, commission_rate, commission_amount_cents, commission_settled
         ) VALUES(
            ?1, ?2, ?3, COALESCE(?4, 'pending'),
            COALESCE(?5, 0), COALESCE(?6, 1), COALESCE(?7, 0.06),
            ?8, ?9, ?10, ?11,
            COALESCE(?12, 'none'), ?13, ?14, COALESCE(?15, 0)
         )",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.client_id,
            input.status.as_deref(),
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
            input.commission_mode.as_deref(),
            input.commission_rate,
            input.commission_amount_cents,
            input.commission_settled.map(|b| b as i64),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ProjectInput) -> AppResult<Project> {
    validate(input)?;
    let tx = conn.unchecked_transaction()?;
    if let Some(status) = input.status.as_deref() {
        if get_impl(&tx, id)?.status != status {
            change_lifecycle(&tx, id, status, None, None)?;
        }
    }
    let n = tx.execute(
        "UPDATE projects SET
            name = ?1,
            client_id = ?2,
            contract_amount_cents = COALESCE(?3, contract_amount_cents),
            contract_amount_is_tax_inclusive = COALESCE(?4, contract_amount_is_tax_inclusive),
            tax_rate = COALESCE(?5, tax_rate),
            start_date = ?6,
            end_date = ?7,
            actual_delivered_at = ?8,
            notes = ?9,
            commission_mode = COALESCE(?10, commission_mode),
            commission_rate = ?11,
            commission_amount_cents = ?12,
            commission_settled = COALESCE(?13, commission_settled),
            updated_at = datetime('now')
         WHERE id = ?14 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.client_id,
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
            input.commission_mode.as_deref(),
            input.commission_rate,
            input.commission_amount_cents,
            input.commission_settled.map(|b| b as i64),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "project",
            id,
        });
    }
    let project = get_impl(&tx, id)?;
    tx.commit()?;
    Ok(project)
}

fn change_lifecycle(
    conn: &Connection,
    id: i64,
    action: &str,
    target: Option<&str>,
    reason: Option<&str>,
) -> AppResult<Project> {
    let project = get_impl(conn, id)?;
    let current = project.status.as_str();
    let invalid = || AppError::Validation("当前项目状态不允许此操作".into());
    let is_archive = action == "archive";
    let next = match action {
        "archive" if !project.is_archived => current,
        "unarchive" if project.is_archived && current == "unknown" => target
            .filter(|s| ALLOWED_STATUSES.contains(s))
            .ok_or_else(invalid)?,
        "unarchive" if project.is_archived => current,
        _ if project.is_archived => return Err(invalid()),
        "restore" if current == "cancelled" => project
            .cancelled_from
            .as_deref()
            .or(target.filter(|s| matches!(*s, "negotiating" | "pending" | "in_progress")))
            .ok_or_else(invalid)?,
        "pending" if current == "negotiating" => action,
        "negotiating" if current == "pending" => action,
        "in_progress" if matches!(current, "negotiating" | "pending" | "delivered") => action,
        "cancelled" if matches!(current, "negotiating" | "pending" | "in_progress") => action,
        "delivered" if matches!(current, "in_progress" | "settled") => action,
        "settled" if current == "delivered" => action,
        "settled" if current == "in_progress" => {
            let summary = crate::domain::profit::project_financial_summary(conn, id)?;
            if summary.revenue_tax_inclusive_cents <= 0
                || summary.actual_payment_cents < summary.revenue_tax_inclusive_cents
            {
                return Err(invalid());
            }
            action
        }
        _ => return Err(invalid()),
    };
    let cancelled_from = match action {
        "cancelled" => Some(current),
        "archive" | "unarchive" => project.cancelled_from.as_deref(),
        _ => None,
    };
    conn.execute(
        "UPDATE projects SET status=?1, is_archived=?2, cancelled_from=?3,
                  updated_at=datetime('now') WHERE id=?4",
        rusqlite::params![next, is_archive, cancelled_from, id],
    )?;
    conn.execute(
        "INSERT INTO project_status_events(project_id,action,from_status,to_status,reason)
                  VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![
            id,
            action,
            current,
            next,
            reason.map(str::trim).filter(|s| !s.is_empty())
        ],
    )?;
    get_impl(conn, id)
}

#[derive(Serialize)]
pub struct StatusEvent {
    id: i64,
    action: String,
    from_status: Option<String>,
    to_status: String,
    reason: Option<String>,
    created_at: String,
}

#[tauri::command]
pub fn list_project_status_events(
    state: tauri::State<AppState>,
    id: i64,
) -> AppResult<Vec<StatusEvent>> {
    with_conn(&state, |conn| {
        get_impl(conn, id)?;
        let mut stmt = conn.prepare(
            "SELECT id, action, from_status, to_status, reason, created_at
            FROM project_status_events WHERE project_id=?1 ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok(StatusEvent {
                id: r.get(0)?,
                action: r.get(1)?,
                from_status: r.get(2)?,
                to_status: r.get(3)?,
                reason: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    })
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_project(conn, id)
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
pub fn list_projects(
    state: tauri::State<AppState>,
    company_id: i64,
    status: Option<String>,
) -> AppResult<Vec<Project>> {
    with_conn(&state, |c| list_impl(c, company_id, status.as_deref()))
}
#[tauri::command]
pub fn get_project(state: tauri::State<AppState>, id: i64) -> AppResult<Project> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_project(
    state: tauri::State<AppState>,
    company_id: i64,
    input: ProjectInput,
) -> AppResult<Project> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_project(
    state: tauri::State<AppState>,
    id: i64,
    input: ProjectInput,
) -> AppResult<Project> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_project_status(
    state: tauri::State<AppState>,
    id: i64,
    status: String,
    target: Option<String>,
    reason: Option<String>,
) -> AppResult<Project> {
    with_conn(&state, |conn| {
        let tx = conn.unchecked_transaction()?;
        let project = change_lifecycle(&tx, id, &status, target.as_deref(), reason.as_deref())?;
        tx.commit()?;
        Ok(project)
    })
}
#[tauri::command]
pub fn delete_project(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}

#[tauri::command]
pub fn get_project_financial_summary(
    state: tauri::State<AppState>,
    id: i64,
) -> AppResult<crate::domain::profit::ProjectFinancialSummary> {
    with_conn(&state, |c| {
        crate::domain::profit::project_financial_summary(c, id)
    })
}
