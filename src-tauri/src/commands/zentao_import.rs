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
}
