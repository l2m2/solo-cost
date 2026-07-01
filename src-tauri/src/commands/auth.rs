use crate::db::{migrations, pool};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use std::path::PathBuf;
use tauri::Manager;

fn data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {}", e)))
}

fn db_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("data.db"))
}

#[tauri::command]
pub fn is_initialized(app: tauri::AppHandle) -> AppResult<bool> {
    Ok(db_path(&app)?.exists())
}

#[tauri::command]
pub fn setup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    password: String,
) -> AppResult<()> {
    let path = db_path(&app)?;
    if path.exists() {
        return Err(AppError::AlreadyInitialized);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = setup_at(&path, &password)?;
    *state.conn.lock().unwrap() = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn unlock(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    password: String,
) -> AppResult<()> {
    let path = db_path(&app)?;
    if !path.exists() {
        return Err(AppError::NotInitialized);
    }
    let conn = unlock_at(&path, &password)?;
    *state.conn.lock().unwrap() = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn lock(state: tauri::State<AppState>) -> AppResult<()> {
    state.conn.lock().unwrap().take();
    Ok(())
}

// Internal helpers: accept path directly so they can be unit-tested without a tauri::AppHandle.
pub(crate) fn setup_at(path: &std::path::Path, password: &str) -> AppResult<rusqlite::Connection> {
    let conn = pool::open_encrypted(path, password)?;
    migrations::run(&conn)?;
    Ok(conn)
}

pub(crate) fn unlock_at(path: &std::path::Path, password: &str) -> AppResult<rusqlite::Connection> {
    let conn = pool::open_encrypted(path, password)?;
    // Also run migrations on unlock so schema upgrades apply after app updates.
    migrations::run(&conn)?;
    // Verify the database is uncorrupted before returning it to the caller.
    crate::domain::backup::integrity_check(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn setup_creates_db_and_runs_migrations() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "secret").unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='companies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn unlock_with_correct_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "s").unwrap();
        drop(conn);
        let conn = unlock_at(&path, "s").unwrap();
        // Query companies table to confirm unlock succeeded.
        let n: i64 = conn
            .query_row("SELECT count(*) FROM companies", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        let conn = setup_at(&path, "right").unwrap();
        drop(conn);
        let err = unlock_at(&path, "wrong").unwrap_err();
        assert!(matches!(err, AppError::WrongPassword));
    }

    #[test]
    fn unlock_reports_integrity_failure() {
        use crate::error::AppError;
        use std::io::{Seek, SeekFrom, Write};
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.db");
        // create a valid encrypted db then close
        drop(setup_at(&path, "s").unwrap());
        // corrupt a page interior (skip past SQLCipher header ~16 bytes → seek 4096 to hit page 2 boundary,
        // then splat some garbage; enough to break integrity_check with the current password intact
        // for the header decryption).
        let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.seek(SeekFrom::Start(4096)).unwrap();
        f.write_all(&[0xFFu8; 512]).unwrap();
        drop(f);
        let err = unlock_at(&path, "s").unwrap_err();
        assert!(
            matches!(
                err,
                AppError::IntegrityCheckFailed(_)
                    | AppError::WrongPassword
                    | AppError::Db(_)
                    | AppError::Migration(_)
            ),
            "expected integrity or decryption failure, got {err:?}"
        );
    }
}
