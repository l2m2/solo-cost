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
    for r in rows { out.push(r?); }
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
        return Err(AppError::Validation("科目与项目公司不匹配或科目不存在".into()));
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
    // Note: update does not re-check category↔company match; original create validated this.
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
        return Err(AppError::NotFound { entity: "cost_entry", id });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_cost_entry(conn, id)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<CostEntry> {
    conn.query_row(
        "SELECT * FROM cost_entries WHERE id = ?1 AND deleted_at IS NULL",
        [id], row_to_entry,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "cost_entry", id },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};

    struct TestDb { conn: Connection, _dir: TempDir }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
                 VALUES(1, '差旅', 1, 0)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn ce(amount: i64) -> CostEntryInput {
        CostEntryInput {
            category_id: 1,
            incurred_at: "2026-06-15".into(),
            amount_cents: amount,
            description: Some("交通".into()),
            notes: None,
        }
    }

    #[test]
    fn create_and_list() {
        let db = TestDb::new();
        let e = create_impl(&db.conn, 1, &ce(12345)).unwrap();
        assert_eq!(e.amount_cents, 12345);
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn negative_amount_rejected() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &ce(-1)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn category_company_mismatch_rejected() {
        let db = TestDb::new();
        // Create a second company and a category under it.
        db.conn.execute("INSERT INTO companies(name) VALUES('Other')", []).unwrap();
        db.conn.execute(
            "INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(2, 'X', 0, 0)",
            [],
        ).unwrap();
        let mut bad = ce(100);
        bad.category_id = 2;
        let err = create_impl(&db.conn, 1, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_soft_only() {
        let db = TestDb::new();
        let e = create_impl(&db.conn, 1, &ce(50)).unwrap();
        delete_impl(&db.conn, e.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
        // Row still exists with deleted_at set.
        let dt: Option<String> = db.conn.query_row(
            "SELECT deleted_at FROM cost_entries WHERE id = ?1",
            [e.id], |r| r.get(0),
        ).unwrap();
        assert!(dt.is_some());
    }
}
