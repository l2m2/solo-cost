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
    "archived",
];

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub client_name: Option<String>,
    pub status: String,
    pub contract_amount_cents: i64,
    pub contract_amount_is_tax_inclusive: bool,
    pub tax_rate: f64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub client_name: Option<String>,
    pub status: Option<String>,
    pub contract_amount_cents: Option<i64>,
    pub contract_amount_is_tax_inclusive: Option<bool>,
    pub tax_rate: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub actual_delivered_at: Option<String>,
    pub notes: Option<String>,
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        client_name: row.get("client_name")?,
        status: row.get("status")?,
        contract_amount_cents: row.get("contract_amount_cents")?,
        contract_amount_is_tax_inclusive: row.get::<_, i64>("contract_amount_is_tax_inclusive")?
            != 0,
        tax_rate: row.get("tax_rate")?,
        start_date: row.get("start_date")?,
        end_date: row.get("end_date")?,
        actual_delivered_at: row.get("actual_delivered_at")?,
        notes: row.get("notes")?,
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
    Ok(())
}

pub(crate) fn list_impl(
    conn: &Connection,
    company_id: i64,
    status: Option<&str>,
) -> AppResult<Vec<Project>> {
    let (sql, params): (&str, Vec<rusqlite::types::Value>) = match status {
        Some(s) => (
            "SELECT * FROM projects
             WHERE company_id = ?1 AND status = ?2 AND deleted_at IS NULL
             ORDER BY id DESC",
            vec![company_id.into(), s.to_string().into()],
        ),
        None => (
            "SELECT * FROM projects
             WHERE company_id = ?1 AND deleted_at IS NULL
             ORDER BY id DESC",
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
        "SELECT * FROM projects WHERE id = ?1 AND deleted_at IS NULL",
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
            company_id, name, client_name, status,
            contract_amount_cents, contract_amount_is_tax_inclusive, tax_rate,
            start_date, end_date, actual_delivered_at, notes
         ) VALUES(
            ?1, ?2, ?3, COALESCE(?4, 'pending'),
            COALESCE(?5, 0), COALESCE(?6, 1), COALESCE(?7, 0.06),
            ?8, ?9, ?10, ?11
         )",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.client_name.as_deref(),
            input.status.as_deref(),
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ProjectInput) -> AppResult<Project> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE projects SET
            name = ?1,
            client_name = ?2,
            status = COALESCE(?3, status),
            contract_amount_cents = COALESCE(?4, contract_amount_cents),
            contract_amount_is_tax_inclusive = COALESCE(?5, contract_amount_is_tax_inclusive),
            tax_rate = COALESCE(?6, tax_rate),
            start_date = ?7,
            end_date = ?8,
            actual_delivered_at = ?9,
            notes = ?10,
            updated_at = datetime('now')
         WHERE id = ?11 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.client_name.as_deref(),
            input.status.as_deref(),
            input.contract_amount_cents,
            input.contract_amount_is_tax_inclusive.map(|b| b as i64),
            input.tax_rate,
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.actual_delivered_at.as_deref(),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "project",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn set_status_impl(conn: &Connection, id: i64, status: &str) -> AppResult<Project> {
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(AppError::Validation(format!("非法状态：{status}")));
    }
    let n = conn.execute(
        "UPDATE projects SET status = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![status, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "project",
            id,
        });
    }
    get_impl(conn, id)
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
) -> AppResult<Project> {
    with_conn(&state, |c| set_status_impl(c, id, &status))
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
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            name: name.into(),
            client_name: None,
            status: None,
            contract_amount_cents: None,
            contract_amount_is_tax_inclusive: None,
            tax_rate: None,
            start_date: None,
            end_date: None,
            actual_delivered_at: None,
            notes: None,
        }
    }

    #[test]
    fn create_with_defaults_status_pending() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        assert_eq!(p.status, "pending");
        assert_eq!(p.contract_amount_cents, 0);
        assert!(p.contract_amount_is_tax_inclusive);
        assert!((p.tax_rate - 0.06).abs() < 1e-9);
    }

    #[test]
    fn create_seeds_categories_for_company() {
        let db = TestDb::new();
        create_impl(&db.conn, 1, &input("P")).unwrap();
        let n: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM cost_categories WHERE company_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 9);
    }

    #[test]
    fn validate_empty_name() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &input("")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_bad_status() {
        let db = TestDb::new();
        let mut i = input("P");
        i.status = Some("foo".into());
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let db = TestDb::new();
        let mut a = input("A");
        a.status = Some("in_progress".into());
        let mut b = input("B");
        b.status = Some("delivered".into());
        create_impl(&db.conn, 1, &a).unwrap();
        create_impl(&db.conn, 1, &b).unwrap();
        assert_eq!(list_impl(&db.conn, 1, None).unwrap().len(), 2);
        assert_eq!(list_impl(&db.conn, 1, Some("delivered")).unwrap().len(), 1);
    }

    #[test]
    fn set_status_changes_state() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        let u = set_status_impl(&db.conn, p.id, "in_progress").unwrap();
        assert_eq!(u.status, "in_progress");
    }

    #[test]
    fn delete_cascades_to_cost_entries() {
        let db = TestDb::new();
        let p = create_impl(&db.conn, 1, &input("P")).unwrap();
        let cat_id: i64 = db
            .conn
            .query_row(
                "SELECT id FROM cost_categories WHERE company_id = 1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(?1, ?2, '2026-06-01', 100)",
                [p.id, cat_id],
            )
            .unwrap();
        delete_impl(&db.conn, p.id).unwrap();
        // project gone from active list
        assert_eq!(list_impl(&db.conn, 1, None).unwrap().len(), 0);
        // cost entry soft deleted too
        let entry_del: Option<String> = db
            .conn
            .query_row(
                "SELECT deleted_at FROM cost_entries WHERE project_id = ?1",
                [p.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(entry_del.is_some());
    }
}
