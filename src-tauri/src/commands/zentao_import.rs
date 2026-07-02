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
                Some("done".into())
            } else {
                None // cancelled / duplicate / etc → skip whole row
            }
        }
        "已完成" => Some("done".into()),
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

pub(crate) fn pick_work_date(actual_start: &str, actual_end: &str, created_at: &str) -> Option<String> {
    fn take_date_prefix(s: &str) -> Option<String> {
        let t = s.trim();
        if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
            Some(t[0..10].into())
        } else if t.is_empty() {
            None
        } else {
            None
        }
    }
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
        });
    }

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
            module_id,
            external_ref: Some(row.zentao_id.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Encoding tests ──────────────────────────────────────────

    #[test]
    fn detects_utf8() {
        let s = "编号,任务名称,任务状态\n1,做事,已完成";
        let out = detect_and_decode(s.as_bytes()).unwrap();
        assert!(out.contains("任务名称"));
    }

    #[test]
    fn detects_utf8_with_bom() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice("编号,任务名称,任务状态\n1,做事,已完成".as_bytes());
        let out = detect_and_decode(&bytes).unwrap();
        assert!(out.starts_with("编号"));
    }

    #[test]
    fn detects_gbk() {
        // Encode a small header line as GBK using encoding_rs (the same crate used at runtime)
        let (bytes, _, _) = encoding_rs::GBK.encode("编号,任务名称,任务状态\n1,做事,已完成");
        let out = detect_and_decode(&bytes).unwrap();
        assert!(out.contains("任务名称"));
    }

    #[test]
    fn rejects_missing_required_columns() {
        let s = "任务名称,任务状态\nA,已完成";
        let err = parse_all(s.as_bytes()).unwrap_err();
        assert!(matches!(err, AppError::Validation(msg) if msg.contains("编号")));
    }

    // ─── Status mapping tests ────────────────────────────────────

    #[test]
    fn parse_status_closed_done_maps_to_done() {
        assert_eq!(map_status("已关闭", "已完成"), Some("done".into()));
    }

    #[test]
    fn parse_status_done_maps_to_done() {
        assert_eq!(map_status("已完成", ""), Some("done".into()));
    }

    #[test]
    fn parse_status_in_progress_maps_to_in_progress() {
        assert_eq!(map_status("进行中", ""), Some("in_progress".into()));
        assert_eq!(map_status("已激活", ""), Some("in_progress".into()));
    }

    #[test]
    fn parse_status_paused_maps_to_todo() {
        assert_eq!(map_status("已暂停", ""), Some("todo".into()));
    }

    #[test]
    fn parse_status_wait_maps_to_todo() {
        assert_eq!(map_status("未开始", ""), Some("todo".into()));
    }

    #[test]
    fn parse_status_cancelled_yields_none() {
        assert_eq!(map_status("已取消", ""), None);
    }

    #[test]
    fn parse_status_closed_non_done_yields_none() {
        assert_eq!(map_status("已关闭", "已取消"), None);
    }

    #[test]
    fn parse_status_unknown_falls_back_to_todo() {
        assert_eq!(map_status("foo", ""), Some("todo".into()));
    }

    // ─── Assignee fallback tests ─────────────────────────────────

    #[test]
    fn parse_assignee_completer_first() {
        assert_eq!(pick_assignee("李黎明", "Closed", "他人"), Some("李黎明".into()));
    }

    #[test]
    fn parse_assignee_falls_back_to_assigned_when_completer_empty() {
        assert_eq!(pick_assignee("", "小王", "创建人"), Some("小王".into()));
    }

    #[test]
    fn parse_assignee_treats_closed_sentinel_as_empty() {
        assert_eq!(pick_assignee("", "Closed", "创建人"), Some("创建人".into()));
    }

    #[test]
    fn parse_assignee_returns_none_when_all_empty() {
        assert_eq!(pick_assignee("", "Closed", ""), None);
    }

    // ─── Module leaf extraction tests ────────────────────────────

    #[test]
    fn parse_module_leaf_from_nested_path() {
        assert_eq!(extract_module_leaf("/前端/表单(#8)"), Some("表单".into()));
    }

    #[test]
    fn parse_module_leaf_from_single_level() {
        assert_eq!(extract_module_leaf("/前端(#5)"), Some("前端".into()));
    }

    #[test]
    fn parse_module_root_yields_none() {
        assert_eq!(extract_module_leaf("/(#0)"), None);
        assert_eq!(extract_module_leaf("/"), None);
        assert_eq!(extract_module_leaf(""), None);
    }

    // ─── Work date fallback ──────────────────────────────────────

    #[test]
    fn parse_workdate_falls_back_from_start_to_end_to_created() {
        assert_eq!(pick_work_date("2026-06-28 08:44:00", "", ""), Some("2026-06-28".into()));
        assert_eq!(pick_work_date("", "2026-06-27", ""), Some("2026-06-27".into()));
        assert_eq!(pick_work_date("", "", "2026-06-01 10:00:00"), Some("2026-06-01".into()));
        assert_eq!(pick_work_date("", "", ""), None);
    }

    // ─── End-to-end parse_all smoke ──────────────────────────────

    #[test]
    fn parse_all_smoke_from_sample_csv_shape() {
        let s = "编号,所属项目,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                 368,a005-2(#25),现场实施 20260628,已关闭,已完成,8h,8h,2026-06-28 08:44:00,李黎明,Closed,李黎明,/(#0)\n\
                 367,a005-2(#25),重写串口通信,已关闭,已完成,4h,4h,2026-06-26 22:00:00,李黎明,Closed,李黎明,/(#0)\n";
        let rows = parse_all(s.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].zentao_id, "zentao:368");
        assert_eq!(rows[0].title, "现场实施 20260628");
        assert_eq!(rows[0].status.as_deref(), Some("done"));
        assert_eq!(rows[0].assignee_name.as_deref(), Some("李黎明"));
        assert_eq!(rows[0].module_name, None);
        assert!((rows[0].estimated_hours.unwrap() - 8.0).abs() < 1e-9);
        assert!((rows[0].consumed_hours - 8.0).abs() < 1e-9);
        assert_eq!(rows[0].work_date.as_deref(), Some("2026-06-28"));
    }

    // ─── Execution / preview tests ───────────────────────────────

    use crate::commands::auth::setup_at;
    use tempfile::{tempdir, TempDir};
    use std::path::PathBuf;

    struct TestDb {
        conn: Connection,
        _dir: TempDir,
    }
    impl TestDb {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let conn = setup_at(&dir.path().join("test.db"), "p").unwrap();
            conn.execute("INSERT INTO companies(name) VALUES('Co')", []).unwrap();
            conn.execute("INSERT INTO projects(company_id, name) VALUES(1, 'P')", []).unwrap();
            conn.execute(
                "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, '李黎明', 80000)",
                [],
            ).unwrap();
            Self { conn, _dir: dir }
        }
    }

    fn write_csv(dir: &TempDir, name: &str, body: &str) -> PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    fn one_row_csv() -> &'static str {
        "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
         368,现场实施,已关闭,已完成,8h,8h,2026-06-28 08:44:00,李黎明,Closed,李黎明,/(#0)\n"
    }

    fn mapping_use_member(name: &str, member_id: i64) -> HashMap<String, MemberChoice> {
        let mut m = HashMap::new();
        m.insert(name.into(), MemberChoice::UseMember { member_id });
        m
    }

    #[test]
    fn preview_counts_total_and_already_imported() {
        let db = TestDb::new();
        // Seed one existing task with external_ref=zentao:368
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, external_ref) VALUES(1, 'seed', 'zentao:368')",
            [],
        ).unwrap();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let out = preview_impl(&db.conn, 1, path.to_str().unwrap()).unwrap();
        assert_eq!(out.total_rows, 1);
        assert_eq!(out.pre_skip.already_imported, 1);
        assert_eq!(out.pre_skip.cancelled, 0);
    }

    #[test]
    fn preview_collects_member_names() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let out = preview_impl(&db.conn, 1, path.to_str().unwrap()).unwrap();
        assert_eq!(out.member_names, vec!["李黎明"]);
        assert!(out.module_names.is_empty());
    }

    #[test]
    fn execute_creates_task_with_external_ref() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 1);
        // verify DB
        let ref_val: String = db.conn.query_row(
            "SELECT external_ref FROM tasks WHERE project_id = 1",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(ref_val, "zentao:368");
    }

    #[test]
    fn execute_skips_timelog_when_hours_zero() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   369,零工时,已关闭,已完成,0h,0h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 0);
    }

    #[test]
    fn execute_skips_timelog_when_member_unassigned() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mut mapping = HashMap::new();
        mapping.insert("李黎明".into(), MemberChoice::Unassigned);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 1);
        assert_eq!(out.imported_timelogs, 0);
    }

    #[test]
    fn execute_skips_row_when_member_skip_row() {
        let db = TestDb::new();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mut mapping = HashMap::new();
        mapping.insert("李黎明".into(), MemberChoice::SkipRow);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.member_skipped, 1);
    }

    #[test]
    fn execute_skips_row_when_status_cancelled() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   370,取消的任务,已取消,,0h,0h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.cancelled, 1);
    }

    #[test]
    fn execute_skips_row_when_external_ref_already_imported() {
        let db = TestDb::new();
        // Seed
        db.conn.execute(
            "INSERT INTO tasks(project_id, title, external_ref) VALUES(1, 'seed', 'zentao:368')",
            [],
        ).unwrap();
        let path = write_csv(&db._dir, "in.csv", one_row_csv());
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 0);
        assert_eq!(out.skipped.already_imported, 1);
    }

    #[test]
    fn execute_creates_module_on_the_fly() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   371,前端任务,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/前端(#5)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let mut mmap = HashMap::new();
        mmap.insert("前端".into(), ModuleChoice::CreateWithName { name: "前端".into() });
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &mmap).unwrap();
        assert_eq!(out.imported_tasks, 1);
        // Verify a module named "前端" was created under project 1
        let module_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM modules WHERE project_id = 1 AND name = '前端' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(module_count, 1);
    }

    #[test]
    fn execute_reuses_created_module_across_rows() {
        let db = TestDb::new();
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   372,前端任务1,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/前端(#5)\n\
                   373,前端任务2,已关闭,已完成,2h,2h,2026-06-29,李黎明,Closed,李黎明,/前端(#5)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let mut mmap = HashMap::new();
        mmap.insert("前端".into(), ModuleChoice::CreateWithName { name: "前端".into() });
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &mmap).unwrap();
        assert_eq!(out.imported_tasks, 2);
        let module_count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM modules WHERE project_id = 1 AND name = '前端' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(module_count, 1);
    }

    #[test]
    fn execute_records_failure_and_continues() {
        let db = TestDb::new();
        // Row 1 fine; row 2 has invalid hours (>24); row 3 fine
        let csv = "编号,任务名称,任务状态,关闭原因,最初预计,总计消耗,实际开始,由谁完成,指派给,由谁创建,所属模块\n\
                   374,ok1,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n\
                   375,bad,已关闭,已完成,4h,99h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n\
                   376,ok2,已关闭,已完成,4h,4h,2026-06-28,李黎明,Closed,李黎明,/(#0)\n";
        let path = write_csv(&db._dir, "in.csv", csv);
        let mapping = mapping_use_member("李黎明", 1);
        let out = execute_impl(&db.conn, 1, path.to_str().unwrap(), &mapping, &HashMap::new()).unwrap();
        assert_eq!(out.imported_tasks, 2);
        assert_eq!(out.failed.len(), 1);
        assert_eq!(out.failed[0].zentao_id, "zentao:375");
    }
}
