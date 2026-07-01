use crate::domain::soft_delete;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub id: i64,
    pub company_id: i64,
    pub name: String,
    pub role: Option<String>,
    pub daily_cost_cents: i64,
    pub effective_from: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct MemberInput {
    pub name: String,
    pub role: Option<String>,
    pub daily_cost_cents: Option<i64>,
    pub effective_from: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

fn row_to_member(row: &rusqlite::Row) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        name: row.get("name")?,
        role: row.get("role")?,
        daily_cost_cents: row.get("daily_cost_cents")?,
        effective_from: row.get("effective_from")?,
        is_active: row.get::<_, i64>("is_active")? != 0,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate(input: &MemberInput) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation("成员名长度必须在 1–80 之间".into()));
    }
    if let Some(d) = input.daily_cost_cents {
        if d < 0 {
            return Err(AppError::Validation("日成本不能为负".into()));
        }
    }
    Ok(())
}

pub(crate) fn list_impl(conn: &Connection, company_id: i64) -> AppResult<Vec<Member>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM members
         WHERE company_id = ?1 AND deleted_at IS NULL
         ORDER BY is_active DESC, id DESC",
    )?;
    let rows = stmt.query_map([company_id], row_to_member)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn get_impl(conn: &Connection, id: i64) -> AppResult<Member> {
    conn.query_row(
        "SELECT * FROM members WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        row_to_member,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "member",
            id,
        },
        other => AppError::Db(other),
    })
}

pub(crate) fn create_impl(
    conn: &Connection,
    company_id: i64,
    input: &MemberInput,
) -> AppResult<Member> {
    validate(input)?;
    conn.execute(
        "INSERT INTO members(company_id, name, role, daily_cost_cents,
                             effective_from, is_active, notes)
         VALUES(?1, ?2, ?3, COALESCE(?4, 0), ?5, COALESCE(?6, 1), ?7)",
        rusqlite::params![
            company_id,
            input.name.trim(),
            input.role.as_deref(),
            input.daily_cost_cents,
            input.effective_from.as_deref(),
            input.is_active.map(|b| b as i64),
            input.notes.as_deref(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_impl(conn, id)
}

pub(crate) fn update_impl(conn: &Connection, id: i64, input: &MemberInput) -> AppResult<Member> {
    validate(input)?;
    let n = conn.execute(
        "UPDATE members SET
            name = ?1,
            role = ?2,
            daily_cost_cents = COALESCE(?3, daily_cost_cents),
            effective_from = ?4,
            is_active = COALESCE(?5, is_active),
            notes = ?6,
            updated_at = datetime('now')
         WHERE id = ?7 AND deleted_at IS NULL",
        rusqlite::params![
            input.name.trim(),
            input.role.as_deref(),
            input.daily_cost_cents,
            input.effective_from.as_deref(),
            input.is_active.map(|b| b as i64),
            input.notes.as_deref(),
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "member",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn set_active_impl(conn: &Connection, id: i64, is_active: bool) -> AppResult<Member> {
    let n = conn.execute(
        "UPDATE members SET is_active = ?1, updated_at = datetime('now')
         WHERE id = ?2 AND deleted_at IS NULL",
        rusqlite::params![is_active as i64, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound {
            entity: "member",
            id,
        });
    }
    get_impl(conn, id)
}

pub(crate) fn delete_impl(conn: &Connection, id: i64) -> AppResult<()> {
    soft_delete::soft_delete_member(conn, id)
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
pub fn list_members(state: tauri::State<AppState>, company_id: i64) -> AppResult<Vec<Member>> {
    with_conn(&state, |c| list_impl(c, company_id))
}
#[tauri::command]
pub fn get_member(state: tauri::State<AppState>, id: i64) -> AppResult<Member> {
    with_conn(&state, |c| get_impl(c, id))
}
#[tauri::command]
pub fn create_member(
    state: tauri::State<AppState>,
    company_id: i64,
    input: MemberInput,
) -> AppResult<Member> {
    with_conn(&state, |c| create_impl(c, company_id, &input))
}
#[tauri::command]
pub fn update_member(
    state: tauri::State<AppState>,
    id: i64,
    input: MemberInput,
) -> AppResult<Member> {
    with_conn(&state, |c| update_impl(c, id, &input))
}
#[tauri::command]
pub fn set_member_active(
    state: tauri::State<AppState>,
    id: i64,
    is_active: bool,
) -> AppResult<Member> {
    with_conn(&state, |c| set_active_impl(c, id, is_active))
}
#[tauri::command]
pub fn delete_member(state: tauri::State<AppState>, id: i64) -> AppResult<()> {
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

    fn input(name: &str) -> MemberInput {
        MemberInput {
            name: name.into(),
            role: None,
            daily_cost_cents: None,
            effective_from: None,
            is_active: None,
            notes: None,
        }
    }

    #[test]
    fn create_with_defaults_is_active() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("张三")).unwrap();
        assert!(m.is_active);
        assert_eq!(m.daily_cost_cents, 0);
    }

    #[test]
    fn validate_negative_daily_cost() {
        let db = TestDb::new();
        let mut i = input("X");
        i.daily_cost_cents = Some(-1);
        let err = create_impl(&db.conn, 1, &i).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn set_active_toggles() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("A")).unwrap();
        let inactive = set_active_impl(&db.conn, m.id, false).unwrap();
        assert!(!inactive.is_active);
        let active = set_active_impl(&db.conn, m.id, true).unwrap();
        assert!(active.is_active);
    }

    #[test]
    fn delete_member_with_active_logs_blocked() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("M")).unwrap();
        // create project + task + time_log referencing this member
        db.conn
            .execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", [])
            .unwrap();
        db.conn
            .execute("INSERT INTO tasks(project_id, title) VALUES(1, 'T')", [])
            .unwrap();
        db.conn.execute(
            "INSERT INTO time_logs(task_id, member_id, work_date, hours, daily_cost_snapshot_cents)
             VALUES(1, ?1, '2026-06-01', 8.0, 80000)",
            [m.id],
        ).unwrap();
        let err = delete_impl(&db.conn, m.id).unwrap_err();
        assert!(matches!(err, AppError::DeleteBlocked(_)));
    }

    #[test]
    fn delete_member_without_logs_succeeds() {
        let db = TestDb::new();
        let m = create_impl(&db.conn, 1, &input("M")).unwrap();
        delete_impl(&db.conn, m.id).unwrap();
        assert!(list_impl(&db.conn, 1).unwrap().is_empty());
    }
}
