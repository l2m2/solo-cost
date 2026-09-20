use crate::error::{AppError, AppResult};
use crate::mcp::server::MCP_ADDRESS;
use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct McpStatus {
    pub address: &'static str,
    pub running: bool,
    pub database_unlocked: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_mcp_status(state: tauri::State<AppState>) -> AppResult<McpStatus> {
    let database_unlocked = state.database_unlocked()?;
    let status = state
        .mcp
        .lock()
        .map_err(|_| AppError::Internal("MCP status lock poisoned".into()))?;
    Ok(McpStatus {
        address: MCP_ADDRESS,
        running: status.running,
        database_unlocked,
        error: status.error.clone(),
    })
}

