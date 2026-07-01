use crate::domain::backup;
use crate::domain::backup::BackupInfo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

const KEEP: usize = 7;

#[derive(Debug, Clone, Serialize)]
pub struct BackupStatus {
    pub last_backup_at: Option<String>,
    pub auto_count: usize,
    pub should_auto_backup_now: bool,
}

fn data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {e}")))
}

fn db_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("data.db"))
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

fn timestamped_filename(conn: &Connection) -> AppResult<String> {
    // format: auto_YYYYMMDD_HHmmss.db
    let stamp: String =
        conn.query_row("SELECT strftime('%Y%m%d_%H%M%S', 'now')", [], |r| r.get(0))?;
    Ok(format!("auto_{stamp}.db"))
}

#[tauri::command]
pub fn list_backups(app: tauri::AppHandle) -> AppResult<Vec<BackupInfo>> {
    let data = data_dir(&app)?;
    backup::list_auto_backups(&data)
}

#[tauri::command]
pub fn create_backup_now(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<BackupInfo> {
    let data = data_dir(&app)?;
    let src = db_path(&app)?;
    let dir = backup::backup_dir(&data);
    let (fname, dst) = {
        let guard = state.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or(AppError::Locked)?;
        let fname = timestamped_filename(conn)?;
        let dst = dir.join(&fname);
        backup::copy_encrypted_db(conn, &src, &dst)?;
        (fname, dst)
    };
    backup::rotate_auto_backups(&data, KEEP)?;
    // Look up the created BackupInfo (list is sorted DESC → first).
    let list = backup::list_auto_backups(&data)?;
    list.into_iter()
        .find(|b| b.file_name == fname && b.absolute_path == dst.to_string_lossy())
        .ok_or_else(|| AppError::Backup("created backup not found after rotation".into()))
}

#[tauri::command]
pub fn maybe_run_auto_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<Option<BackupInfo>> {
    let now: Option<String> = with_conn(&state, |c| {
        Ok(Some(c.query_row("SELECT datetime('now')", [], |r| {
            r.get::<_, String>(0)
        })?))
    })?;
    let now = now.unwrap();
    let due = with_conn(&state, |c| backup::should_auto_backup(c, &now))?;
    if !due {
        return Ok(None);
    }
    Ok(Some(create_backup_now(app, state)?))
}

#[tauri::command]
pub fn export_plaintext_backup(state: tauri::State<AppState>, dst_path: String) -> AppResult<()> {
    let dst = PathBuf::from(dst_path);
    with_conn(&state, |c| backup::export_plaintext(c, &dst))
}

#[tauri::command]
pub fn get_backup_status(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<BackupStatus> {
    let data = data_dir(&app)?;
    let list = backup::list_auto_backups(&data)?;
    let (last, due) = with_conn(&state, |c| {
        let last = backup::last_backup_at(c)?;
        let now: String = c.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
        let due = backup::should_auto_backup(c, &now)?;
        Ok((last, due))
    })?;
    Ok(BackupStatus {
        last_backup_at: last,
        auto_count: list.len(),
        should_auto_backup_now: due,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::tempdir;

    #[test]
    fn timestamped_filename_matches_pattern() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("data.db"), "s").unwrap();
        let name = timestamped_filename(&conn).unwrap();
        assert!(name.starts_with("auto_"));
        assert!(name.ends_with(".db"));
        // total length: auto_ (5) + 15 (YYYYMMDD_HHmmss) + .db (3) = 23
        assert_eq!(name.len(), 23);
    }
}
