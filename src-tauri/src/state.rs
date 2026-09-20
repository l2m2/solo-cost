use rusqlite::Connection;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

#[derive(Default)]
pub struct McpRuntimeStatus {
    pub running: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub conn: Mutex<Option<Connection>>,
    pub mcp: Mutex<McpRuntimeStatus>,
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

    pub fn database_unlocked(&self) -> AppResult<bool> {
        self.conn
            .lock()
            .map(|guard| guard.is_some())
            .map_err(|_| AppError::Internal("database state lock poisoned".into()))
    }
}
