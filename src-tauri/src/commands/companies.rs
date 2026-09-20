use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Company {
    pub id: i64,
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub default_tax_rate: f64,
    pub currency_code: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CompanyInput {
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub default_tax_rate: Option<f64>,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

fn row_to_company(row: &rusqlite::Row) -> rusqlite::Result<Company> {
    Ok(Company {
        id: row.get("id")?,
        name: row.get("name")?,
        legal_name: row.get("legal_name")?,
        tax_id: row.get("tax_id")?,
        default_tax_rate: row.get("default_tax_rate")?,
        currency_code: row.get("currency_code")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &CompanyInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation("公司名长度必须在 1–80 之间".into()));
    }
    if let Some(rate) = input.default_tax_rate {
        if !(0.0..1.0).contains(&rate) {
            return Err(AppError::Validation("税率必须在 [0, 1) 之间".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection) -> AppResult<Vec<Company>> {
    let mut stmt =
        conn.prepare("SELECT * FROM companies WHERE deleted_at IS NULL ORDER BY id DESC")?;
    let rows = stmt.query_map([], row_to_company)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Company> {
    conn.query_row(
        "SELECT * FROM companies WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_company,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "company",
            id,
        },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(conn: &Connection, input: &CompanyInput) -> AppResult<Company> {
    validate(input)?;
    conn.execute(
        "INSERT INTO companies(name, legal_name, tax_id, default_tax_rate, currency_code, notes)
         VALUES(?1, ?2, ?3, COALESCE(?4, 0.06), COALESCE(?5, 'CNY'), ?6)",
        rusqlite::params![
            input.name.trim(),
            input.legal_name.as_deref(),
            input.tax_id.as_deref(),
            input.default_tax_rate,
            input.currency_code.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &CompanyInput) -> AppResult<Company> {
    validate(input)?;
    let affected = conn.execute(
        "UPDATE companies SET
            name = ?1,
            legal_name = ?2,
            tax_id = ?3,
            default_tax_rate = COALESCE(?4, default_tax_rate),
            currency_code = COALESCE(?5, currency_code),
            notes = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.legal_name.as_deref(),
            input.tax_id.as_deref(),
            input.default_tax_rate,
            input.currency_code.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound {
            entity: "company",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn get_current_impl(conn: &Connection) -> AppResult<Option<i64>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'current_company_id'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(row.and_then(|s| s.parse::<i64>().ok()))
}

pub(crate) fn set_current_impl(conn: &Connection, id: i64) -> AppResult<()> {
    // Verify the company exists and is not deleted before setting it as current.
    let _ = get_impl(conn, id)?;
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('current_company_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [id.to_string()],
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
pub fn list_companies(state: tauri::State<AppState>) -> AppResult<Vec<Company>> {
    with_conn(&state, list_impl)
}
#[tauri::command]
pub fn get_company(state: tauri::State<AppState>, id: i64) -> AppResult<Company> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_company(state: tauri::State<AppState>, input: CompanyInput) -> AppResult<Company> {
    with_conn(&state, |c| create_impl(c, &input))
}
#[tauri::command]
pub fn update_company(
    state: tauri::State<AppState>,
    id: i64,
    input: CompanyInput,
) -> AppResult<Company> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn get_current_company_id(state: tauri::State<AppState>) -> AppResult<Option<i64>> {
    with_conn(&state, get_current_impl)
}
#[tauri::command]
pub fn set_current_company(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| set_current_impl(c, id))
}
