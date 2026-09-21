use crate::domain::chatgpt_report::{self, ChatGptReportInput};
use crate::error::AppResult;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct ExportChatGptReportInput {
    pub company_id: i64,
    pub start_date: String,
    pub end_date: String,
    pub dst_path: String,
}

#[derive(Debug, Serialize)]
pub struct ExportChatGptReportResult {
    pub absolute_path: String,
}

#[tauri::command]
pub fn export_chatgpt_report(
    state: tauri::State<AppState>,
    input: ExportChatGptReportInput,
) -> AppResult<ExportChatGptReportResult> {
    let destination = PathBuf::from(&input.dst_path);
    let report = state.with_conn(|conn| {
        chatgpt_report::build(
            conn,
            &ChatGptReportInput {
                company_id: input.company_id,
                start_date: input.start_date,
                end_date: input.end_date,
            },
        )
    })?;
    std::fs::write(&destination, report)?;
    Ok(ExportChatGptReportResult {
        absolute_path: destination.to_string_lossy().into_owned(),
    })
}
