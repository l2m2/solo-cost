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
