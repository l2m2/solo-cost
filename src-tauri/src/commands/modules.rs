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
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
                .unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn input(name: &str) -> ModuleInput {
        ModuleInput { name: name.into(), sort_order: None }
    }

    #[test]
    fn create_defaults_sort_order_to_max_plus_one() {
        let db = TestDb::new();
        let a = create_impl(&db.conn, 1, &input("A")).unwrap();
        let b = create_impl(&db.conn, 1, &input("B")).unwrap();
        let c = create_impl(&db.conn, 1, &input("C")).unwrap();
        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);
        assert_eq!(c.sort_order, 2);
    }

    #[test]
    fn create_persists_name_and_project() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("前端")).unwrap();
        assert_eq!(m.name, "前端");
        assert_eq!(m.project_id, 1);
    }

    #[test]
    fn update_can_rename() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        let u = update_impl(&db.conn, m.id, &ModuleInput { name: "Y".into(), sort_order: None }).unwrap();
        assert_eq!(u.name, "Y");
        assert_eq!(u.sort_order, m.sort_order);
    }

    #[test]
    fn update_can_reorder() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        let u = update_impl(&db.conn, m.id, &ModuleInput { name: "X".into(), sort_order: Some(9) }).unwrap();
        assert_eq!(u.sort_order, 9);
    }

    #[test]
    fn list_orders_by_sort_order_then_id() {
        let db = TestDb::new();
        // insert with explicit sort_order to make ordering deterministic
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'B', 1)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'A', 1)", []).unwrap();
        db.conn.execute("INSERT INTO modules(project_id, name, sort_order) VALUES(1, 'C', 0)", []).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["C", "B", "A"]);
    }

    #[test]
    fn list_excludes_soft_deleted() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn delete_blocks_when_task_attached() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, module_id) VALUES(1, 'T', ?1)",
            [m.id],
        ).unwrap();
        let err = delete_impl(&db.conn, m.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_succeeds_when_no_tasks() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("X")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn validate_rejects_empty_name() {
        let db = TestDb::new();
        let err = create_impl(&db.conn, 1, &input("")).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_rejects_too_long_name() {
        let db = TestDb::new();
        let long = "x".repeat(41);
        let err = create_impl(&db.conn, 1, &input(&long)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
