use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TrashItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub deleted_at: String,
    pub project_id: Option<i64>,
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<TrashItem>> {
    let mut out = Vec::new();

    // soft-deleted projects in this company
    let mut sp = conn.prepare(
        "SELECT id, name, deleted_at FROM projects
         WHERE company_id = ?1 AND deleted_at IS NOT NULL
         ORDER BY deleted_at DESC",
    )?;
    let rows = sp.query_map([company_id], |r| {
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "project".into(),
            name: r.get::<_, String>(1)?,
            deleted_at: r.get::<_, String>(2)?,
            project_id: None,
        })
    })?;
    for r in rows {
        out.push(r?);
    }

    // soft-deleted cost entries whose project belongs to this company
    let mut sc = conn.prepare(
        "SELECT ce.id, ce.project_id, ce.amount_cents, ce.description, ce.deleted_at
         FROM cost_entries ce
         JOIN projects p ON p.id = ce.project_id
         WHERE p.company_id = ?1 AND ce.deleted_at IS NOT NULL
         ORDER BY ce.deleted_at DESC",
    )?;
    let rows = sc.query_map([company_id], |r| {
        let pid: i64 = r.get(1)?;
        let amt: i64 = r.get(2)?;
        let desc: Option<String> = r.get(3)?;
        let yuan = amt as f64 / 100.0;
        let name = match desc {
            Some(d) if !d.is_empty() => format!("成本 ¥{:.2} ({d})", yuan),
            _ => format!("成本 ¥{:.2}", yuan),
        };
        Ok(TrashItem {
            id: r.get::<_, i64>(0)?,
            entity_type: "cost_entry".into(),
            name,
            deleted_at: r.get::<_, String>(4)?,
            project_id: Some(pid),
        })
    })?;
    for r in rows {
        out.push(r?);
    }

    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}

pub(crate) fn restore_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    match entity_type {
        "project" => soft_delete::restore_project(conn, id),
        "cost_entry" => soft_delete::restore_cost_entry(conn, id),
        other => Err(AppError::Validation(format!("未知实体类型：{other}"))),
    }
}

pub(crate) fn purge_impl(conn: &Connection, entity_type: &str, id: i64) -> AppResult<()> {
    let table = match entity_type {
        "project" => "projects",
        "cost_entry" => "cost_entries",
        other => return Err(AppError::Validation(format!("未知实体类型：{other}"))),
    };
    let tx = conn.unchecked_transaction()?;
    if entity_type == "project" {
        // physically delete cost_entries first to respect FK
        tx.execute("DELETE FROM cost_entries WHERE project_id = ?1", [id])?;
    }
    let n = tx.execute(
        &format!("DELETE FROM {table} WHERE id = ?1 AND deleted_at IS NOT NULL"),
        [id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "trash_item",
            id,
        });
    }
    tx.commit()?;
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
pub fn list_trash(state: tauri::State<AppState>, company_id: i64) -> AppResult<Vec<TrashItem>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn restore_trash_item(
    state: tauri::State<AppState>,
    entity_type: String,
    id: i64,
) -> AppResult<()> {
    with_conn(&state, |c| restore_impl(c, &entity_type, id))
}
#[tauri::command]
pub fn purge_trash_item(
    state: tauri::State<AppState>,
    entity_type: String,
    id: i64,
) -> AppResult<()> {
    with_conn(&state, |c| purge_impl(c, &entity_type, id))
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
            conn.execute(
                "INSERT INTO cost_categories(company_id, name, is_system, sort_order) VALUES(1, 'X', 1, 0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cost_entries(project_id, category_id, incurred_at, amount_cents)
                 VALUES(1, 1, '2026-06-01', 200)",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }
    }

    #[test]
    fn list_returns_soft_deleted() {
        let db = TestDb::new();
        soft_delete::soft_delete_cost_entry(&db.conn, 1).unwrap();
        let items = list_impl(&db.conn, 1).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].entity_type, "cost_entry");
        assert_eq!(items[0].project_id, Some(1));
    }

    #[test]
    fn restore_brings_back() {
        let db = TestDb::new();
        soft_delete::soft_delete_cost_entry(&db.conn, 1).unwrap();
        restore_impl(&db.conn, "cost_entry", 1).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }

    #[test]
    fn purge_project_cascades_physical_delete() {
        let db = TestDb::new();
        soft_delete::soft_delete_project(&db.conn, 1).unwrap();
        purge_impl(&db.conn, "project", 1).unwrap();
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM projects WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 0);
        let n2: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM cost_entries WHERE project_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn unknown_entity_type_validation_error() {
        let db = TestDb::new();
        let err = restore_impl(&db.conn, "task", 1).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
