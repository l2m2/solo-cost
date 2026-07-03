use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Client {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub contact_name: Option<String>,
    pub contact_info: Option<String>,
    pub tax_id: Option<String>,
    pub legal_name: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ClientInput {
    pub name: String,
    pub contact_name: Option<String>,
    pub contact_info: Option<String>,
    pub tax_id: Option<String>,
    pub legal_name: Option<String>,
    pub notes: Option<String>,
}

fn row_to_client(row: &rusqlite::Row) -> rusqlite::Result<Client> {
    Ok(Client {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        contact_name: row.get("contact_name")?,
        contact_info: row.get("contact_info")?,
        tax_id: row.get("tax_id")?,
        legal_name: row.get("legal_name")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &ClientInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::Validation("客户名长度必须在 1–120 之间".into()));
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<Client>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM clients
         WHERE company_id = ?1 AND deleted_at IS NULL
         ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([company_id], row_to_client)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Client> {
    conn.query_row(
        "SELECT * FROM clients WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_client,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "client",
            id,
        },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &ClientInput,
) -> AppResult<Client> {
    validate(input)?;
    conn.execute(
        "INSERT INTO clients(company_id, name, contact_name, contact_info,
                             tax_id, legal_name, notes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.contact_name.as_deref(),
            input.contact_info.as_deref(),
            input.tax_id.as_deref(),
            input.legal_name.as_deref(),
            input.notes.as_deref(),
        ],
    )
    .map_err(map_unique_name_error)?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &ClientInput) -> AppResult<Client> {
    validate(input)?;
    let n = conn
        .execute(
            "UPDATE clients SET
                name = ?1,
                contact_name = ?2,
                contact_info = ?3,
                tax_id = ?4,
                legal_name = ?5,
                notes = ?6,
                updated_at = datetime('now')
             WHERE id = ?7 AND deleted_at IS NULL",
            rusqlite::params![
                input.name.trim(),
                input.contact_name.as_deref(),
                input.contact_info.as_deref(),
                input.tax_id.as_deref(),
                input.legal_name.as_deref(),
                input.notes.as_deref(),
                id,
            ],
        )
        .map_err(map_unique_name_error)?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "client",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let ref_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects
         WHERE client_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if ref_count > 0 {
        return Err(AppError::DeleteBlocked(format!(
            "该客户被 {ref_count} 个项目引用，请先在项目中改绑或清空"
        )));
    }
    let n = conn.execute(
        "UPDATE clients SET deleted_at = datetime('now')
         WHERE id = ?1 AND deleted_at IS NULL",
        [id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "client",
            id,
        });
    }
    Ok(())
}

fn map_unique_name_error(e: rusqlite::Error) -> rusqlite::Error {
    // Callers convert to AppError via `?`; we intercept the unique index violation to give
    // a friendlier message. The unique index is (company_id, lower(name)) so any duplicate
    // within the same company hits this branch.
    if let rusqlite::Error::SqliteFailure(_, Some(ref msg)) = e {
        if msg.contains("idx_clients_company_name_unique") {
            return rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ffi::ErrorCode::ConstraintViolation,
                    extended_code: 2067,
                },
                Some("客户名在当前公司下已存在".to_string()),
            );
        }
    }
    e
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
pub fn list_clients(state: tauri::State<AppState>, company_id: i64) -> AppResult<Vec<Client>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn get_client(state: tauri::State<AppState>, id: i64) -> AppResult<Client> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_client(
    state: tauri::State<AppState>,
    company_id: i64,
    input: ClientInput,
) -> AppResult<Client> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_client(
    state: tauri::State<AppState>,
    id: i64,
    input: ClientInput,
) -> AppResult<Client> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_client(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> ClientInput {
        ClientInput {
            name: name.into(),
            contact_name: None,
            contact_info: None,
            tax_id: None,
            legal_name: None,
            notes: None,
        }
    }

    #[test]
    fn create_and_get() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &input("Acme")).unwrap();
        assert_eq!(c.name, "Acme");
        assert_eq!(get_impl(&db.conn, c.id).unwrap().id, c.id);
    }

    #[test]
    fn validate_empty_name() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &input("  ")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn duplicate_name_case_insensitive_rejected() {
        let db = TestDb::new();
        create_impl(&db.conn, 1, &input("Acme")).unwrap();
        let err = create_impl(&db.conn, 1, &input("ACME")).unwrap_err();
        assert!(matches!(err, AppError::Db(_)));
    }

    #[test]
    fn same_name_in_different_company_ok() {
        let db = TestDb::new();
        db.conn
            .execute("INSERT INTO companies(name) VALUES('Co2')", [])
            .unwrap();
        create_impl(&db.conn, 1, &input("Acme")).unwrap();
        create_impl(&db.conn, 2, &input("Acme")).unwrap();
    }

    #[test]
    fn delete_blocked_when_referenced_by_project() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &input("Acme")).unwrap();
        db.conn
            .execute(
                "INSERT INTO projects(company_id, name, client_id) VALUES(1, 'P', ?1)",
                [c.id],
            )
            .unwrap();
        let err = delete_impl(&db.conn, c.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_ok_when_no_active_reference() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &input("Acme")).unwrap();
        delete_impl(&db.conn, c.id).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }

    #[test]
    fn delete_ok_when_referencing_project_is_soft_deleted() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &input("Acme")).unwrap();
        db.conn
            .execute(
                "INSERT INTO projects(company_id, name, client_id, deleted_at)
                 VALUES(1, 'P', ?1, datetime('now'))",
                [c.id],
            )
            .unwrap();
        delete_impl(&db.conn, c.id).unwrap();
    }

    #[test]
    fn update_persists_all_fields() {
        let db = TestDb::new();
        let c = create_impl(&db.conn, 1, &input("Acme")).unwrap();
        let mut i = input("Acme Corp");
        i.contact_name = Some("张三".into());
        i.contact_info = Some("13800000000".into());
        i.tax_id = Some("91330000000000000X".into());
        i.legal_name = Some("Acme 有限公司".into());
        i.notes = Some("VIP".into());
        let updated = update_impl(&db.conn, c.id, &i).unwrap();
        assert_eq!(updated.name, "Acme Corp");
        assert_eq!(updated.contact_name.as_deref(), Some("张三"));
        assert_eq!(updated.tax_id.as_deref(), Some("91330000000000000X"));
    }
}
