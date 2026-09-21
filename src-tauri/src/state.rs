use rusqlite::Connection;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

#[derive(Default)]
pub struct AppState {
    pub conn: Mutex<Option<Connection>>,
}

impl AppState {
    pub fn with_conn<R>(
        &self,
        f: impl FnOnce(&Connection) -> AppResult<R>,
    ) -> AppResult<R> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| AppError::Internal("database state lock poisoned".into()))?;
        let conn = guard.as_ref().ok_or(AppError::Locked)?;
        f(conn)
    }
}
