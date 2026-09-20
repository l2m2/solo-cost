use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModuleLaborStat {
    pub module_id: Option<i64>,
    pub module_name: Option<String>,
    pub hours: f64,
    pub cost_cents: i64,
}

pub fn labor_by_module(
    conn: &Connection,
    project_id: i64,
) -> AppResult<Vec<ModuleLaborStat>> {
    // Coalesce tasks pointing at soft-deleted modules into the "unassigned"
    // bucket: the LEFT JOIN yields m.id = NULL for both truly-unassigned tasks
    // and orphans, so grouping by t.module_id alone would split them into two
    // rows both rendered as "未分类". The CASE folds orphans into NULL.
    let mut stmt = conn.prepare(
        "SELECT CASE WHEN m.id IS NOT NULL THEN t.module_id ELSE NULL END AS module_id,
                m.name AS module_name,
                COALESCE(SUM(tl.hours), 0.0) AS hours,
                COALESCE(CAST(SUM(ROUND(tl.hours / 8.0 * tl.daily_cost_snapshot_cents)) AS INTEGER), 0) AS cost
         FROM tasks t
         LEFT JOIN modules m
                ON m.id = t.module_id AND m.deleted_at IS NULL
         LEFT JOIN time_logs tl
                ON tl.task_id = t.id AND tl.deleted_at IS NULL
         WHERE t.project_id = ?1 AND t.deleted_at IS NULL
         GROUP BY CASE WHEN m.id IS NOT NULL THEN t.module_id ELSE NULL END, m.name
         HAVING hours > 0
         ORDER BY m.sort_order ASC NULLS LAST, m.id ASC",
    )?;
    let rows = stmt.query_map([project_id], |r| {
        Ok(ModuleLaborStat {
            module_id: r.get(0)?,
            module_name: r.get(1)?,
            hours: r.get(2)?,
            cost_cents: r.get::<_, i64>(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
