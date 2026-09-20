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

// Internal helpers accept a path directly to keep filesystem handling independent of Tauri.
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
