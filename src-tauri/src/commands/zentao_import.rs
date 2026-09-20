use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── DTO structs shared by preview / execute ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub total_rows: u32,
    pub member_names: Vec<String>,
    pub module_names: Vec<String>,
    pub pre_skip: PreSkipStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreSkipStats {
    pub cancelled: u32,
    pub already_imported: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemberChoice {
    UseMember { member_id: i64 },
    Unassigned,
    SkipRow,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleChoice {
    UseModule { module_id: i64 },
    CreateWithName { name: String },
    Unassigned,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub imported_tasks: u32,
    pub imported_timelogs: u32,
    pub skipped: SkipCounts,
    pub failed: Vec<FailedRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SkipCounts {
    pub cancelled: u32,
    pub already_imported: u32,
    pub member_skipped: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedRow {
    pub row_no: u32,
    pub zentao_id: String,
    pub error: String,
}

// ─── Internal parser output ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct ParsedRow {
    pub row_no: u32,
    pub zentao_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub assignee_name: Option<String>,
    pub module_name: Option<String>,
    pub estimated_hours: Option<f64>,
    pub consumed_hours: f64,
    pub work_date: Option<String>,
    pub due_date: Option<String>,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

// ─── Encoding detection ──────────────────────────────────────────────────

pub(crate) fn detect_and_decode(bytes: &[u8]) -> Option<String> {
    // Prefer strict UTF-8 (strip BOM if present)
    let stripped = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Ok(s) = std::str::from_utf8(stripped) {
        return Some(s.to_string());
    }
    // Fall back to GBK
    let (cow, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if had_errors {
        None
    } else {
        Some(cow.into_owned())
    }
}

// ─── Status mapping ──────────────────────────────────────────────────────

pub(crate) fn map_status(zentao_status: &str, close_reason: &str) -> Option<String> {
    match zentao_status.trim() {
        "已关闭" => {
            if close_reason.trim() == "已完成" {
                Some("closed".into()) // done AND archived
            } else {
                None // cancelled / duplicate / etc → skip whole row
            }
        }
        "已完成" => Some("done".into()), // done, not yet closed
        "进行中" | "已激活" => Some("in_progress".into()),
        "已暂停" | "未开始" => Some("todo".into()),
        "已取消" => None,
        _ => Some("todo".into()), // defensive fallback
    }
}

// ─── Assignee fallback ───────────────────────────────────────────────────

pub(crate) fn pick_assignee(completer: &str, assigned: &str, creator: &str) -> Option<String> {
    let completer = completer.trim();
    if !completer.is_empty() {
        return Some(completer.into());
    }
    let assigned = assigned.trim();
    if !assigned.is_empty() && assigned != "Closed" {
        return Some(assigned.into());
    }
    let creator = creator.trim();
    if !creator.is_empty() {
        return Some(creator.into());
    }
    None
}

// ─── Module leaf extraction ──────────────────────────────────────────────

pub(crate) fn extract_module_leaf(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Strip trailing "(#\d+)"
    let without_id: &str = match s.rfind('(') {
        Some(i) if s[i..].starts_with("(#") && s.ends_with(')') => &s[..i],
        _ => s,
    };
    let path = without_id.trim().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return None;
    }
    // Take last segment
    let leaf = path.rsplit('/').next().unwrap_or("").trim();
    if leaf.is_empty() {
        None
    } else {
        Some(leaf.to_string())
    }
}

// ─── Work date fallback ──────────────────────────────────────────────────

// Take the leading "YYYY-MM-DD" of a zentao datetime string (empty / malformed → None).
pub(crate) fn take_date_prefix(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
        Some(t[0..10].into())
    } else {
        None
    }
}

pub(crate) fn pick_work_date(actual_start: &str, actual_end: &str, created_at: &str) -> Option<String> {
    if let Some(d) = take_date_prefix(actual_start) {
        return Some(d);
    }
    if let Some(d) = take_date_prefix(actual_end) {
        return Some(d);
    }
    take_date_prefix(created_at)
}

// ─── Parser core ─────────────────────────────────────────────────────────

pub(crate) fn parse_all(bytes: &[u8]) -> AppResult<Vec<ParsedRow>> {
    let text = detect_and_decode(bytes)
        .ok_or_else(|| AppError::Validation("不支持的编码，请另存为 UTF-8".into()))?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr.headers()
        .map_err(|e| AppError::Validation(format!("CSV 头解析失败: {e}")))?
        .clone();

    let col_index: HashMap<&str, usize> = headers.iter().enumerate()
        .map(|(i, h)| (h.trim(), i))
        .collect();

    let required = ["编号", "任务名称", "任务状态"];
    for name in required {
        if !col_index.contains_key(name) {
            return Err(AppError::Validation(format!("CSV 缺少必要列: {name}")));
        }
    }

    fn get<'a>(rec: &'a csv::StringRecord, idx: Option<&usize>) -> &'a str {
        idx.and_then(|&i| rec.get(i)).unwrap_or("")
    }

    let mut out = Vec::new();
    for (row_no0, rec) in rdr.records().enumerate() {
        let row_no = (row_no0 as u32) + 1; // 1-indexed data row (after header)
        let rec = match rec {
            Ok(r) => r,
            Err(_) => continue, // silently skip malformed rows
        };
        if rec.iter().all(|f| f.trim().is_empty()) {
            continue; // silently skip blank rows
        }

        let zentao_num = get(&rec, col_index.get("编号")).trim();
        if zentao_num.is_empty() {
            continue; // silently skip rows without id (e.g. legend footer)
        }
        let title = get(&rec, col_index.get("任务名称")).trim().to_string();
        if title.is_empty() {
            continue; // silently skip rows without title
        }

        let status = map_status(
            get(&rec, col_index.get("任务状态")),
            get(&rec, col_index.get("关闭原因")),
        );

        let assignee_name = pick_assignee(
            get(&rec, col_index.get("由谁完成")),
            get(&rec, col_index.get("指派给")),
            get(&rec, col_index.get("由谁创建")),
        );

        let module_name = extract_module_leaf(get(&rec, col_index.get("所属模块")));

        fn strip_h_parse(s: &str) -> Option<f64> {
            let t = s.trim();
            let stripped = t.strip_suffix('h').unwrap_or(t).trim();
            if stripped.is_empty() { None } else { stripped.parse::<f64>().ok() }
        }

        let estimated_hours = strip_h_parse(get(&rec, col_index.get("最初预计")));
        let consumed_hours = strip_h_parse(get(&rec, col_index.get("总计消耗"))).unwrap_or(0.0);

        let started_at = take_date_prefix(get(&rec, col_index.get("实际开始")));
        let completed_at = take_date_prefix(get(&rec, col_index.get("实际完成")));
        let created_at = take_date_prefix(get(&rec, col_index.get("创建日期")));

        let work_date = pick_work_date(
            get(&rec, col_index.get("实际开始")),
            get(&rec, col_index.get("实际完成")),
            get(&rec, col_index.get("创建日期")),
        );

        let due_date = {
            let d = get(&rec, col_index.get("截止日期")).trim();
            if d.is_empty() { None } else { Some(d.to_string()) }
        };

        let description = {
            let d = get(&rec, col_index.get("任务描述")).trim();
            if d.is_empty() { None } else { Some(d.to_string()) }
        };

        out.push(ParsedRow {
            row_no,
            zentao_id: format!("zentao:{zentao_num}"),
            title,
            description,
            status,
            assignee_name,
            module_name,
            estimated_hours,
            consumed_hours,
            work_date,
            due_date,
            created_at,
            started_at,
            completed_at,
        });
    }

    // Import in ascending zentao 编号 order so task ids follow the original numbering
    // (the CSV export is newest-first). Rows are keyed by "zentao:<num>".
    out.sort_by_key(|r| r.zentao_id.trim_start_matches("zentao:").parse::<i64>().unwrap_or(i64::MAX));

    Ok(out)
}

// ─── IPC handlers ────────────────────────────────────────────────────────

use crate::state::AppState;
use crate::commands::modules::{self, ModuleInput};
use crate::commands::tasks::{self, TaskInput};
use crate::commands::timelogs::{self, TimeLogInput};
use rusqlite::Connection;

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

fn read_file(file_path: &str) -> AppResult<Vec<u8>> {
    std::fs::read(file_path).map_err(|e| AppError::Validation(format!("无法读取文件: {e}")))
}

fn dedupe_ordered(items: impl IntoIterator<Item = String>) -> Vec<String> {
    // Preserve first-seen order, dedupe by string
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for s in items {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

pub(crate) fn preview_impl(
    conn: &Connection,
    project_id: i64,
    file_path: &str,
) -> AppResult<ImportPreview> {
    let bytes = read_file(file_path)?;
    let rows = parse_all(&bytes)?;
    let total_rows = rows.len() as u32;
    let cancelled = rows.iter().filter(|r| r.status.is_none()).count() as u32;

    // Count already-imported: rows whose external_ref already lives in the project
    let mut already_imported: u32 = 0;
    for r in &rows {
        let hit: Option<i64> = conn.query_row(
            "SELECT 1 FROM tasks WHERE project_id = ?1 AND external_ref = ?2 AND deleted_at IS NULL",
            rusqlite::params![project_id, r.zentao_id],
            |row| row.get(0),
        ).ok();
        if hit.is_some() {
            already_imported += 1;
        }
    }

    let member_names = dedupe_ordered(
        rows.iter().filter_map(|r| r.assignee_name.clone()),
    );
    let module_names = dedupe_ordered(
        rows.iter().filter_map(|r| r.module_name.clone()),
    );

    Ok(ImportPreview {
        total_rows,
        member_names,
        module_names,
        pre_skip: PreSkipStats { cancelled, already_imported },
    })
}

pub(crate) fn execute_impl(
    conn: &Connection,
    project_id: i64,
    file_path: &str,
    member_mapping: &HashMap<String, MemberChoice>,
    module_mapping: &HashMap<String, ModuleChoice>,
) -> AppResult<ImportReport> {
    let bytes = read_file(file_path)?;
    let rows = parse_all(&bytes)?;

    let mut imported_tasks: u32 = 0;
    let mut imported_timelogs: u32 = 0;
    let mut skipped = SkipCounts::default();
    let mut failed: Vec<FailedRow> = Vec::new();
    let mut created_module_cache: HashMap<String, i64> = HashMap::new();

    for row in rows {
        // 1) cancelled?
        if row.status.is_none() {
            skipped.cancelled += 1;
            continue;
        }

        // 2) already imported?
        let hit: Option<i64> = conn.query_row(
            "SELECT 1 FROM tasks WHERE project_id = ?1 AND external_ref = ?2 AND deleted_at IS NULL",
            rusqlite::params![project_id, row.zentao_id],
            |r| r.get(0),
        ).ok();
        if hit.is_some() {
            skipped.already_imported += 1;
            continue;
        }

        // 3) member mapping
        let assignee_key = row.assignee_name.clone().unwrap_or_default();
        let assignee_id: Option<i64> = match member_mapping.get(&assignee_key) {
            Some(MemberChoice::SkipRow) => {
                skipped.member_skipped += 1;
                continue;
            }
            Some(MemberChoice::UseMember { member_id }) => Some(*member_id),
            Some(MemberChoice::Unassigned) | None => None,
        };

        // 4) module mapping (may create on the fly, cached across rows)
        let module_key = row.module_name.clone().unwrap_or_default();
        let module_id: Option<i64> = match module_mapping.get(&module_key) {
            Some(ModuleChoice::UseModule { module_id }) => Some(*module_id),
            Some(ModuleChoice::CreateWithName { name }) => {
                if let Some(&id) = created_module_cache.get(name) {
                    Some(id)
                } else {
                    match modules::create_impl(
                        conn,
                        project_id,
                        &ModuleInput { name: name.clone(), sort_order: None },
                    ) {
                        Ok(m) => {
                            created_module_cache.insert(name.clone(), m.id);
                            Some(m.id)
                        }
                        Err(e) => {
                            failed.push(FailedRow {
                                row_no: row.row_no,
                                zentao_id: row.zentao_id.clone(),
                                error: format!("module: {e}"),
                            });
                            continue;
                        }
                    }
                }
            }
            Some(ModuleChoice::Unassigned) | None => None,
        };

        // 5) per-row transaction: task + optional timelog
        let tx = match conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => {
                failed.push(FailedRow {
                    row_no: row.row_no,
                    zentao_id: row.zentao_id.clone(),
                    error: format!("tx: {e}"),
                });
                continue;
            }
        };

        let task_input = TaskInput {
            title: row.title.clone(),
            description: row.description.clone(),
            assignee_id,
            status: row.status.clone(),
            estimated_hours: row.estimated_hours,
            due_date: row.due_date.clone(),
            started_at: row.started_at.clone(),
            completed_at: row.completed_at.clone(),
            module_id,
            external_ref: Some(row.zentao_id.clone()),
            created_at: row.created_at.clone(),
        };
        let task = match tasks::create_impl(&tx, project_id, &task_input) {
            Ok(t) => t,
            Err(e) => {
                failed.push(FailedRow {
                    row_no: row.row_no,
                    zentao_id: row.zentao_id.clone(),
                    error: format!("task: {e}"),
                });
                let _ = tx.rollback();
                continue;
            }
        };

        // 6) optional timelog
        if row.consumed_hours > 0.0 {
            if let (Some(mid), Some(wd)) = (assignee_id, row.work_date.clone()) {
                let tl_input = TimeLogInput {
                    task_id: task.id,
                    member_id: mid,
                    work_date: wd,
                    hours: row.consumed_hours,
                    notes: None,
                };
                match timelogs::create_impl(&tx, &tl_input) {
                    Ok(_) => imported_timelogs += 1,
                    Err(e) => {
                        failed.push(FailedRow {
                            row_no: row.row_no,
                            zentao_id: row.zentao_id.clone(),
                            error: format!("timelog: {e}"),
                        });
                        let _ = tx.rollback();
                        continue;
                    }
                }
            }
        }

        if let Err(e) = tx.commit() {
            failed.push(FailedRow {
                row_no: row.row_no,
                zentao_id: row.zentao_id.clone(),
                error: format!("commit: {e}"),
            });
            continue;
        }
        imported_tasks += 1;
    }

    // Cap failed list at 100 to avoid gigantic reports
    if failed.len() > 100 {
        failed.truncate(100);
    }

    Ok(ImportReport {
        imported_tasks,
        imported_timelogs,
        skipped,
        failed,
    })
}

#[tauri::command]
pub fn preview_zentao_csv(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
) -> AppResult<ImportPreview> {
    with_conn(&state, |c| preview_impl(c, project_id, &file_path))
}

#[tauri::command]
pub fn execute_zentao_import(
    state: tauri::State<AppState>,
    project_id: i64,
    file_path: String,
    member_mapping: HashMap<String, MemberChoice>,
    module_mapping: HashMap<String, ModuleChoice>,
) -> AppResult<ImportReport> {
    with_conn(&state, |c| {
        execute_impl(c, project_id, &file_path, &member_mapping, &module_mapping)
    })
}
