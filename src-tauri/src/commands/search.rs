use crate::error::AppResult;
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;

const DEFAULT_LIMIT: u32 = 8;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// "project" or "task".
    pub kind: String,
    pub id: i64,
    pub title: String,
    /// Client name for a project hit, owning project name for a task hit.
    pub subtitle: Option<String>,
    /// Equals `id` for a project hit; the owning project for a task hit.
    pub project_id: i64,
}

/// Escape the LIKE wildcards so a user typing "%" does not match every row.
/// Pairs with `ESCAPE '\'` in every query below.
fn escape_like(raw: &str) -> String {
    raw.replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_")
}

pub(crate) fn search_impl(
    conn: &Connection,
    company_id: i64,
    query: &str,
    limit: u32,
) -> AppResult<Vec<SearchHit>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let escaped = escape_like(trimmed);
    let contains = format!("%{escaped}%");
    let prefix = format!("{escaped}%");

    let mut hits = Vec::new();

    let mut stmt = conn.prepare(
        r"SELECT p.id, p.name, c.name AS client_name
          FROM projects p
          LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
          WHERE p.company_id = ?1
            AND p.deleted_at IS NULL
            AND (p.name LIKE ?2 ESCAPE '\' OR c.name LIKE ?2 ESCAPE '\')
          ORDER BY CASE WHEN p.name LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
                   p.updated_at DESC
          LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![company_id, contains, prefix, limit],
        |row| {
            let id: i64 = row.get("id")?;
            Ok(SearchHit {
                kind: "project".into(),
                id,
                title: row.get("name")?,
                subtitle: row.get("client_name")?,
                project_id: id,
            })
        },
    )?;
    for r in rows {
        hits.push(r?);
    }

    let mut stmt = conn.prepare(
        r"SELECT t.id, t.title, t.project_id, p.name AS project_name
          FROM tasks t
          JOIN projects p ON p.id = t.project_id
          WHERE p.company_id = ?1
            AND t.deleted_at IS NULL
            AND p.deleted_at IS NULL
            AND t.title LIKE ?2 ESCAPE '\'
          ORDER BY CASE WHEN t.title LIKE ?3 ESCAPE '\' THEN 0 ELSE 1 END,
                   t.updated_at DESC
          LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![company_id, contains, prefix, limit],
        |row| {
            Ok(SearchHit {
                kind: "task".into(),
                id: row.get("id")?,
                title: row.get("title")?,
                subtitle: row.get("project_name")?,
                project_id: row.get("project_id")?,
            })
        },
    )?;
    for r in rows {
        hits.push(r?);
    }

    Ok(hits)
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(crate::error::AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn search(
    state: tauri::State<AppState>,
    company_id: i64,
    query: String,
    limit: Option<u32>,
) -> AppResult<Vec<SearchHit>> {
    with_conn(&state, |c| {
        search_impl(c, company_id, &query, limit.unwrap_or(DEFAULT_LIMIT))
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
            conn.execute("INSERT INTO companies(name) VALUES('Other')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO clients(company_id, name) VALUES(1, '官网科技')",
                [],
            )
            .unwrap();
            Self { conn, _dir: dir }
        }

        fn project(&self, company_id: i64, name: &str, client_id: Option<i64>) -> i64 {
            self.conn
                .execute(
                    "INSERT INTO projects(company_id, name, client_id) VALUES(?1, ?2, ?3)",
                    rusqlite::params![company_id, name, client_id],
                )
                .unwrap();
            self.conn.last_insert_rowid()
        }

        fn task(&self, project_id: i64, title: &str) -> i64 {
            self.conn
                .execute(
                    "INSERT INTO tasks(project_id, title) VALUES(?1, ?2)",
                    rusqlite::params![project_id, title],
                )
                .unwrap();
            self.conn.last_insert_rowid()
        }

        fn soft_delete(&self, table: &str, id: i64) {
            self.conn
                .execute(
                    &format!("UPDATE {table} SET deleted_at = datetime('now') WHERE id = ?1"),
                    rusqlite::params![id],
                )
                .unwrap();
        }
    }

    fn titles(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.title.as_str()).collect()
    }

    #[test]
    fn finds_project_by_name() {
        let db = TestDb::new();
        db.project(1, "官网改版", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(titles(&hits), vec!["官网改版"]);
        assert_eq!(hits[0].kind, "project");
        assert_eq!(hits[0].project_id, hits[0].id);
    }

    #[test]
    fn finds_project_by_client_name() {
        let db = TestDb::new();
        let p = db.project(1, "内部系统", Some(1));
        let hits = search_impl(&db.conn, 1, "官网科技", 8).unwrap();
        assert_eq!(titles(&hits), vec!["内部系统"]);
        assert_eq!(hits[0].id, p);
        assert_eq!(hits[0].subtitle.as_deref(), Some("官网科技"));
    }

    #[test]
    fn finds_project_without_client() {
        // LEFT JOIN, not JOIN: a project with no client must still be findable.
        let db = TestDb::new();
        db.project(1, "无客户项目", None);
        let hits = search_impl(&db.conn, 1, "无客户", 8).unwrap();
        assert_eq!(titles(&hits), vec!["无客户项目"]);
        assert_eq!(hits[0].subtitle, None);
    }

    #[test]
    fn finds_task_by_title_with_project_subtitle() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        let t = db.task(p, "首页切图");
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert_eq!(titles(&hits), vec!["首页切图"]);
        assert_eq!(hits[0].kind, "task");
        assert_eq!(hits[0].id, t);
        assert_eq!(hits[0].project_id, p);
        assert_eq!(hits[0].subtitle.as_deref(), Some("官网改版"));
    }

    #[test]
    fn projects_come_before_tasks() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        db.task(p, "官网切图");
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(hits[0].kind, "project");
        assert_eq!(hits[1].kind, "task");
    }

    #[test]
    fn scopes_to_company() {
        let db = TestDb::new();
        db.project(2, "别家的官网", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_soft_deleted_project() {
        let db = TestDb::new();
        let p = db.project(1, "官网改版", None);
        db.soft_delete("projects", p);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_soft_deleted_task() {
        let db = TestDb::new();
        let p = db.project(1, "某项目", None);
        let t = db.task(p, "首页切图");
        db.soft_delete("tasks", t);
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn excludes_task_whose_project_is_soft_deleted() {
        let db = TestDb::new();
        let p = db.project(1, "某项目", None);
        db.task(p, "首页切图");
        db.soft_delete("projects", p);
        let hits = search_impl(&db.conn, 1, "切图", 8).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn escapes_percent_wildcard() {
        // Without ESCAPE, a bare "%" would match every row.
        let db = TestDb::new();
        db.project(1, "普通项目", None);
        db.project(1, "折扣 100% 项目", None);
        let hits = search_impl(&db.conn, 1, "%", 8).unwrap();
        assert_eq!(titles(&hits), vec!["折扣 100% 项目"]);
    }

    #[test]
    fn escapes_underscore_wildcard() {
        let db = TestDb::new();
        db.project(1, "ab", None);
        db.project(1, "a_b", None);
        let hits = search_impl(&db.conn, 1, "a_b", 8).unwrap();
        assert_eq!(titles(&hits), vec!["a_b"]);
    }

    #[test]
    fn escapes_backslash() {
        let db = TestDb::new();
        db.project(1, r"路径\测试", None);
        let hits = search_impl(&db.conn, 1, r"\", 8).unwrap();
        assert_eq!(titles(&hits), vec![r"路径\测试"]);
    }

    #[test]
    fn prefix_matches_rank_first() {
        let db = TestDb::new();
        db.project(1, "改版官网", None);
        db.project(1, "官网改版", None);
        let hits = search_impl(&db.conn, 1, "官网", 8).unwrap();
        assert_eq!(titles(&hits), vec!["官网改版", "改版官网"]);
    }

    #[test]
    fn blank_query_returns_empty() {
        let db = TestDb::new();
        db.project(1, "官网改版", None);
        assert!(search_impl(&db.conn, 1, "", 8).unwrap().is_empty());
        assert!(search_impl(&db.conn, 1, "   ", 8).unwrap().is_empty());
    }

    #[test]
    fn limit_applies_per_kind() {
        let db = TestDb::new();
        let p = db.project(1, "官网 A", None);
        db.project(1, "官网 B", None);
        db.project(1, "官网 C", None);
        db.task(p, "官网任务一");
        db.task(p, "官网任务二");
        db.task(p, "官网任务三");
        let hits = search_impl(&db.conn, 1, "官网", 2).unwrap();
        assert_eq!(hits.iter().filter(|h| h.kind == "project").count(), 2);
        assert_eq!(hits.iter().filter(|h| h.kind == "task").count(), 2);
    }
}
