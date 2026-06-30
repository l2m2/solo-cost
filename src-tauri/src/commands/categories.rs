use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

const PRESET_NAMES: [&str; 9] = [
    "外包成本",
    "硬件采购",
    "服务器与SaaS",
    "差旅",
    "办公耗材",
    "市场推广",
    "税费与手续费",
    "培训与资料",
    "其它",
];

#[derive(Debug, Clone, Serialize)]
pub struct CostCategory {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub is_system: bool,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
pub struct CostCategoryInput {
    pub name: String,
}

fn row_to_category(row: &rusqlite::Row) -> rusqlite::Result<CostCategory> {
    Ok(CostCategory {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        is_system: row.get::<_, i64>("is_system")? != 0,
        sort_order: row.get("sort_order")?,
    })
}

fn validate(input: &CostCategoryInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::Validation("科目名长度必须在 1–40 之间".into()));
    }
    Ok(())
}

pub(crate) fn ensure_presets(conn: &Connection, company_id: i64) -> AppResult<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_categories WHERE company_id = ?1 AND deleted_at IS NULL",
        [company_id],
        |r| r.get(0),
    )?;
    if count > 0 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    for (i, name) in PRESET_NAMES.iter().enumerate() {
        tx.execute(
            "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
             VALUES(?1, ?2, 1, ?3)",
            rusqlite::params![company_id, name, i as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<CostCategory>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cost_categories
         WHERE company_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([company_id], row_to_category)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &CostCategoryInput,
) -> AppResult<CostCategory> {
    validate(input)?;
    // pick sort_order = max + 1
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM cost_categories WHERE company_id = ?1",
        [company_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO cost_categories(company_id, name, is_system, sort_order)
         VALUES(?1, ?2, 0, ?3)",
        rusqlite::params![company_id, input.name.trim(), next_order],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(
    conn: &Connection,
    id: i64,
    input: &CostCategoryInput,
) -> AppResult<CostCategory> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE cost_categories SET name = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![input.name.trim(), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "cost_category",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT is_system, deleted_at FROM cost_categories WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let (is_system, already_deleted) = match row {
        Some(x) => x,
        None => {
            return Err(AppError::NotFound {
                entity: "cost_category",
                id,
            })
        }
    };
    if is_system == 1 {
        return Err(AppError::DeleteBlocked("预设科目不可删除".into()));
    }
    if already_deleted.is_some() {
        return Ok(()); // idempotent
    }
    let used: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cost_entries WHERE category_id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    if used > 0 {
        return Err(AppError::DeleteBlocked(format!(
            "该科目下还有 {used} 条成本记录，请先迁移或删除"
        )));
    }
    let ts: String = conn.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
    conn.execute(
        "UPDATE cost_categories SET deleted_at = ?1 WHERE id = ?2",
        rusqlite::params![ts, id],
    )?;
    Ok(())
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<CostCategory> {
    conn.query_row(
        "SELECT * FROM cost_categories WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_category,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "cost_category",
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
pub fn list_categories(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<Vec<CostCategory>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn create_category(
    state: tauri::State<AppState>,
    company_id: i64,
    input: CostCategoryInput,
) -> AppResult<CostCategory> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_category(
    state: tauri::State<AppState>,
    id: i64,
    input: CostCategoryInput,
) -> AppResult<CostCategory> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn delete_category(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
    with_conn(&state, |c| delete_impl(c, id))
}
#[tauri::command]
pub fn seed_preset_categories_if_empty(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<Vec<CostCategory>> {
    with_conn(&state, |c| {
        ensure_presets(c, company_id)?;
        list_impl(c, company_id)
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

    #[test]
    fn seed_creates_nine_preset_categories() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        assert_eq!(list.len(), 9);
        assert!(list.iter().all(|c| c.is_system));
        assert_eq!(list[0].name, "外包成本");
    }

    #[test]
    fn seed_is_idempotent() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        ensure_presets(&db.conn, 1).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 9);
    }

    #[test]
    fn create_custom_category() {
        let db = TestDb::new();
        let c = create_impl(
            &db.conn,
            1,
            &CostCategoryInput {
                name: "广告投放".into(),
            },
        )
        .unwrap();
        assert!(!c.is_system);
        assert_eq!(c.name, "广告投放");
    }

    #[test]
    fn delete_system_blocked() {
        let db = TestDb::new();
        ensure_presets(&db.conn, 1).unwrap();
        let list = list_impl(&db.conn, 1).unwrap();
        let err = delete_impl(&db.conn, list[0].id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_in_use_blocked() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        db.conn
            .execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
             VALUES(1, ?1, '2026-06-01', 100)",
                [cat.id],
            )
            .unwrap();
        let err = delete_impl(&db.conn, cat.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_unused_custom_succeeds() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        delete_impl(&db.conn, cat.id).unwrap();
        assert_eq!(list_impl(&db.conn, 1).unwrap().len(), 0);
    }

    #[test]
    fn update_renames() {
        let db = TestDb::new();
        let cat = create_impl(&db.conn, 1, &CostCategoryInput { name: "X".into() }).unwrap();
        let new = update_impl(&db.conn, cat.id, &CostCategoryInput { name: "Y".into() }).unwrap();
        assert_eq!(new.name, "Y");
    }
}
