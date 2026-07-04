use crate::domain::dashboard::{company_dashboard, DashboardSummary};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

#[tauri::command]
pub fn get_dashboard(
    state: tauri::State<AppState>,
    company_id: i64,
) -> AppResult<DashboardSummary> {
    with_conn(&state, |c| {
        let today: String = c.query_row("SELECT date('now','localtime')", [], |r| r.get(0))?;
        company_dashboard(c, company_id, &today)
    })
}
