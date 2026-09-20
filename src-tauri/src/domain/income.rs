use crate::error::{AppError, AppResult};
use chrono::{Datelike, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "scope")]
pub enum IncomeScope {
    #[default]
    CurrentCompany,
    Company { company_id: i64 },
    AllCompanies,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DateRange {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompanyRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct IncomeMetrics {
    pub contract_exclusive_cents: i64,
    pub received_exclusive_cents: i64,
    pub commission_potential_cents: i64,
    pub commission_realized_cents: i64,
    pub general_cost_cents: i64,
    pub take_home_potential_cents: i64,
    pub take_home_realized_cents: i64,
    pub labor_income_cents: i64,
    pub residual_profit_potential_cents: i64,
    pub residual_profit_realized_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeOverview {
    pub generated_at: String,
    pub companies: Vec<CompanyRef>,
    pub basis: String,
    pub metrics: IncomeMetrics,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankDimension {
    Client,
    Project,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankMetric {
    Income,
    TakeHome,
    LaborIncome,
    ResidualProfit,
    CollectionRate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RankIncomeInput {
    #[serde(flatten)]
    pub scope: IncomeScope,
    #[serde(flatten)]
    pub range: DateRange,
    pub dimension: RankDimension,
    pub metric: RankMetric,
    #[serde(default = "default_rank_limit")]
    pub limit: i64,
}

fn default_rank_limit() -> i64 {
    10
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeRankRow {
    pub id: i64,
    pub name: String,
    pub metrics: IncomeMetrics,
    pub collection_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDecisionSummary {
    pub generated_at: String,
    pub company: CompanyRef,
    pub project_id: i64,
    pub project_name: String,
    pub client_name: Option<String>,
    pub status: String,
    pub metrics: IncomeMetrics,
    pub expected_payment_cents: i64,
    pub received_inclusive_cents: i64,
    pub outstanding_cents: i64,
    pub collection_rate: f64,
    pub task_total: i64,
    pub task_completed: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendGranularity {
    Month,
    Year,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncomeTrendInput {
    #[serde(flatten)]
    pub scope: IncomeScope,
    #[serde(flatten)]
    pub range: DateRange,
    pub granularity: TrendGranularity,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeTrendRow {
    pub period: String,
    pub metrics: IncomeMetrics,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaymentListInput {
    #[serde(flatten)]
    pub scope: IncomeScope,
    #[serde(flatten)]
    pub range: DateRange,
    pub project_id: Option<i64>,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_page_limit")]
    pub limit: i64,
}

fn default_page_limit() -> i64 {
    50
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentRow {
    pub id: i64,
    pub company_id: i64,
    pub company_name: String,
    pub project_id: i64,
    pub project_name: String,
    pub name: String,
    pub expected_amount_cents: i64,
    pub expected_date: Option<String>,
    pub actual_amount_cents: Option<i64>,
    pub actual_received_at: Option<String>,
    pub outstanding_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentPage {
    pub generated_at: String,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<PaymentRow>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncomeDetailKind {
    GeneralCost,
    Commission,
    Labor,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncomeDetailInput {
    #[serde(flatten)]
    pub scope: IncomeScope,
    #[serde(flatten)]
    pub range: DateRange,
    pub kind: IncomeDetailKind,
    pub project_id: Option<i64>,
    pub member_id: Option<i64>,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_page_limit")]
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeDetailRow {
    pub id: i64,
    pub kind: String,
    pub company_id: i64,
    pub company_name: String,
    pub project_id: i64,
    pub project_name: String,
    pub occurred_at: Option<String>,
    pub label: String,
    pub amount_cents: i64,
    pub hours: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeDetailPage {
    pub generated_at: String,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub items: Vec<IncomeDetailRow>,
}

#[derive(Debug, Clone)]
struct ProjectRow {
    id: i64,
    company_id: i64,
    company_name: String,
    name: String,
    client_id: Option<i64>,
    client_name: Option<String>,
    status: String,
    contract_cents: i64,
    tax_inclusive: bool,
    tax_rate: f64,
    commission_mode: String,
    commission_rate: Option<f64>,
    commission_amount_cents: Option<i64>,
    commission_settled: bool,
}

fn generated_at() -> String {
    Utc::now().to_rfc3339()
}

fn parse_date(value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("日期格式无效: {value}")))
}

fn validate_date_range(range: &DateRange) -> AppResult<()> {
    let start = range.start_date.as_deref().map(parse_date).transpose()?;
    let end = range.end_date.as_deref().map(parse_date).transpose()?;
    if matches!((start, end), (Some(start), Some(end)) if end < start) {
        return Err(AppError::Validation("结束日期不能早于开始日期".into()));
    }
    Ok(())
}

fn validate_page(offset: i64, limit: i64) -> AppResult<()> {
    if offset < 0 {
        return Err(AppError::Validation("offset 不能小于 0".into()));
    }
    if !(1..=200).contains(&limit) {
        return Err(AppError::Validation("limit 必须在 1 到 200 之间".into()));
    }
    Ok(())
}

fn resolve_companies(conn: &Connection, scope: &IncomeScope) -> AppResult<Vec<CompanyRef>> {
    let requested = match scope {
        IncomeScope::CurrentCompany => {
            let value: Option<String> = conn
                .query_row(
                    "SELECT value FROM app_meta WHERE key = 'current_company_id'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            Some(
                value
                    .and_then(|value| value.parse::<i64>().ok())
                    .ok_or_else(|| AppError::Validation("尚未选择当前公司".into()))?,
            )
        }
        IncomeScope::Company { company_id } => Some(*company_id),
        IncomeScope::AllCompanies => None,
    };

    let mut output = Vec::new();
    if let Some(company_id) = requested {
        let company = conn
            .query_row(
                "SELECT id, name FROM companies WHERE id = ?1 AND deleted_at IS NULL",
                [company_id],
                |row| {
                    Ok(CompanyRef {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or(AppError::NotFound {
                entity: "company",
                id: company_id,
            })?;
        output.push(company);
    } else {
        let mut statement =
            conn.prepare("SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok(CompanyRef {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        for row in rows {
            output.push(row?);
        }
    }
    Ok(output)
}

fn load_projects(
    conn: &Connection,
    companies: &[CompanyRef],
    range: &DateRange,
) -> AppResult<Vec<ProjectRow>> {
    let mut statement = conn.prepare(
        "SELECT p.id, p.company_id, co.name, p.name, p.client_id, c.name, p.status,
                p.contract_amount_cents, p.contract_amount_is_tax_inclusive, p.tax_rate,
                p.commission_mode, p.commission_rate, p.commission_amount_cents,
                p.commission_settled, p.start_date, p.end_date
         FROM projects p
         JOIN companies co ON co.id = p.company_id
         LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL
           AND (?2 IS NULL OR p.end_date IS NULL OR p.end_date >= ?2)
           AND (?3 IS NULL OR p.start_date IS NULL OR p.start_date <= ?3)
         ORDER BY p.id",
    )?;
    let mut output = Vec::new();
    for company in companies {
        let rows = statement.query_map(
            params![company.id, range.start_date, range.end_date],
            |row| {
                Ok(ProjectRow {
                    id: row.get(0)?,
                    company_id: row.get(1)?,
                    company_name: row.get(2)?,
                    name: row.get(3)?,
                    client_id: row.get(4)?,
                    client_name: row.get(5)?,
                    status: row.get(6)?,
                    contract_cents: row.get(7)?,
                    tax_inclusive: row.get::<_, i64>(8)? != 0,
                    tax_rate: row.get(9)?,
                    commission_mode: row.get(10)?,
                    commission_rate: row.get(11)?,
                    commission_amount_cents: row.get(12)?,
                    commission_settled: row.get::<_, i64>(13)? != 0,
                })
            },
        )?;
        for row in rows {
            output.push(row?);
        }
    }
    Ok(output)
}

fn contract_values(project: &ProjectRow) -> (i64, i64) {
    let multiplier = 1.0 + project.tax_rate;
    if project.tax_inclusive {
        (
            project.contract_cents,
            (project.contract_cents as f64 / multiplier).round() as i64,
        )
    } else {
        (
            (project.contract_cents as f64 * multiplier).round() as i64,
            project.contract_cents,
        )
    }
}

fn date_bounds(range: &DateRange) -> (Option<&str>, Option<&str>) {
    (range.start_date.as_deref(), range.end_date.as_deref())
}

fn project_metrics(conn: &Connection, project: &ProjectRow, range: &DateRange) -> AppResult<IncomeMetrics> {
    let (start, end) = date_bounds(range);
    let (contract_inclusive, contract_exclusive) = contract_values(project);
    let received_inclusive: i64 = conn.query_row(
        "SELECT COALESCE(SUM(actual_amount_cents), 0)
         FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL AND actual_received_at IS NOT NULL
           AND (?2 IS NULL OR actual_received_at >= ?2)
           AND (?3 IS NULL OR actual_received_at <= ?3)",
        params![project.id, start, end],
        |row| row.get(0),
    )?;
    let received_exclusive =
        (received_inclusive as f64 / (1.0 + project.tax_rate)).round() as i64;
    let general_cost: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM cost_entries
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND (?2 IS NULL OR incurred_at >= ?2)
           AND (?3 IS NULL OR incurred_at <= ?3)",
        params![project.id, start, end],
        |row| row.get(0),
    )?;
    let labor_income = project_labor_income_in_range(conn, project.id, range)?;
    let commission_potential = match project.commission_mode.as_str() {
        "rate" => {
            (contract_inclusive as f64 * project.commission_rate.unwrap_or(0.0)).round() as i64
        }
        "fixed" => project.commission_amount_cents.unwrap_or(0),
        _ => 0,
    };
    let commission_realized = match project.commission_mode.as_str() {
        "rate" => {
            (received_inclusive as f64 * project.commission_rate.unwrap_or(0.0)).round() as i64
        }
        "fixed" if project.commission_settled => project.commission_amount_cents.unwrap_or(0),
        _ => 0,
    };
    let take_home_potential = contract_exclusive - commission_potential - general_cost;
    let take_home_realized = received_exclusive - commission_realized - general_cost;
    Ok(IncomeMetrics {
        contract_exclusive_cents: contract_exclusive,
        received_exclusive_cents: received_exclusive,
        commission_potential_cents: commission_potential,
        commission_realized_cents: commission_realized,
        general_cost_cents: general_cost,
        take_home_potential_cents: take_home_potential,
        take_home_realized_cents: take_home_realized,
        labor_income_cents: labor_income,
        residual_profit_potential_cents: take_home_potential - labor_income,
        residual_profit_realized_cents: take_home_realized - labor_income,
    })
}

fn add_metrics(target: &mut IncomeMetrics, value: &IncomeMetrics) {
    target.contract_exclusive_cents += value.contract_exclusive_cents;
    target.received_exclusive_cents += value.received_exclusive_cents;
    target.commission_potential_cents += value.commission_potential_cents;
    target.commission_realized_cents += value.commission_realized_cents;
    target.general_cost_cents += value.general_cost_cents;
    target.take_home_potential_cents += value.take_home_potential_cents;
    target.take_home_realized_cents += value.take_home_realized_cents;
    target.labor_income_cents += value.labor_income_cents;
    target.residual_profit_potential_cents += value.residual_profit_potential_cents;
    target.residual_profit_realized_cents += value.residual_profit_realized_cents;
}

pub fn project_labor_income(conn: &Connection, project_id: i64) -> AppResult<i64> {
    project_labor_income_in_range(conn, project_id, &DateRange::default())
}

fn project_labor_income_in_range(
    conn: &Connection,
    project_id: i64,
    range: &DateRange,
) -> AppResult<i64> {
    let (start, end) = date_bounds(range);
    let mut statement = conn.prepare(
        "SELECT tl.hours, tl.daily_cost_snapshot_cents
         FROM time_logs tl
         JOIN tasks t ON t.id = tl.task_id
         WHERE t.project_id = ?1 AND tl.deleted_at IS NULL AND t.deleted_at IS NULL
           AND (?2 IS NULL OR tl.work_date >= ?2)
           AND (?3 IS NULL OR tl.work_date <= ?3)",
    )?;
    let rows = statement.query_map(params![project_id, start, end], |row| {
        Ok((row.get::<_, f64>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut total = 0;
    for row in rows {
        let (hours, daily_cost) = row?;
        total += (hours / 8.0 * daily_cost as f64).round() as i64;
    }
    Ok(total)
}

pub fn get_income_overview(
    conn: &Connection,
    scope: &IncomeScope,
    range: &DateRange,
) -> AppResult<IncomeOverview> {
    validate_date_range(range)?;
    let companies = resolve_companies(conn, scope)?;
    let projects = load_projects(conn, &companies, range)?;
    let mut metrics = IncomeMetrics::default();
    for project in &projects {
        add_metrics(&mut metrics, &project_metrics(conn, project, range)?);
    }
    Ok(IncomeOverview {
        generated_at: generated_at(),
        companies,
        basis: "potential=合同不含税收入；realized=实收不含税收入；到手=收入-销售分成-一般成本；剩余利润=到手-人工收入".into(),
        metrics,
    })
}

pub fn rank_income_sources(
    conn: &Connection,
    input: &RankIncomeInput,
) -> AppResult<Vec<IncomeRankRow>> {
    validate_date_range(&input.range)?;
    if !(1..=200).contains(&input.limit) {
        return Err(AppError::Validation("limit 必须在 1 到 200 之间".into()));
    }
    let companies = resolve_companies(conn, &input.scope)?;
    let projects = load_projects(conn, &companies, &input.range)?;
    let mut groups: BTreeMap<(i64, String), (IncomeMetrics, i64, i64)> = BTreeMap::new();
    for project in &projects {
        let metrics = project_metrics(conn, project, &input.range)?;
        let key = match input.dimension {
            RankDimension::Project => (project.id, project.name.clone()),
            RankDimension::Client => (
                project.client_id.unwrap_or(0),
                project.client_name.clone().unwrap_or_else(|| "未分配客户".into()),
            ),
        };
        let entry = groups.entry(key).or_default();
        add_metrics(&mut entry.0, &metrics);
        entry.1 += metrics.received_exclusive_cents;
        entry.2 += metrics.contract_exclusive_cents;
    }
    let mut rows: Vec<IncomeRankRow> = groups
        .into_iter()
        .map(|((id, name), (metrics, received, contract))| IncomeRankRow {
            id,
            name,
            collection_rate: if contract == 0 {
                0.0
            } else {
                received as f64 / contract as f64
            },
            metrics,
        })
        .collect();
    rows.sort_by(|left, right| {
        let score = |row: &IncomeRankRow| match input.metric {
            RankMetric::Income => row.metrics.received_exclusive_cents as f64,
            RankMetric::TakeHome => row.metrics.take_home_realized_cents as f64,
            RankMetric::LaborIncome => row.metrics.labor_income_cents as f64,
            RankMetric::ResidualProfit => row.metrics.residual_profit_realized_cents as f64,
            RankMetric::CollectionRate => row.collection_rate,
        };
        score(right)
            .partial_cmp(&score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(input.limit as usize);
    Ok(rows)
}

pub fn get_project_decision_summary(
    conn: &Connection,
    project_id: i64,
) -> AppResult<ProjectDecisionSummary> {
    let project = load_project(conn, project_id)?;
    let metrics = project_metrics(conn, &project, &DateRange::default())?;
    let (expected, received): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(expected_amount_cents),0),
                COALESCE(SUM(CASE WHEN actual_received_at IS NOT NULL THEN actual_amount_cents ELSE 0 END),0)
         FROM contract_payments WHERE project_id = ?1 AND deleted_at IS NULL",
        [project_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (task_total, task_completed): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status IN ('done','closed') THEN 1 ELSE 0 END),0)
         FROM tasks WHERE project_id = ?1 AND deleted_at IS NULL",
        [project_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(ProjectDecisionSummary {
        generated_at: generated_at(),
        company: CompanyRef {
            id: project.company_id,
            name: project.company_name,
        },
        project_id,
        project_name: project.name,
        client_name: project.client_name,
        status: project.status,
        metrics,
        expected_payment_cents: expected,
        received_inclusive_cents: received,
        outstanding_cents: expected - received,
        collection_rate: if expected == 0 {
            0.0
        } else {
            received as f64 / expected as f64
        },
        task_total,
        task_completed,
    })
}

fn load_project(conn: &Connection, project_id: i64) -> AppResult<ProjectRow> {
    conn.query_row(
        "SELECT p.id, p.company_id, co.name, p.name, p.client_id, c.name, p.status,
                p.contract_amount_cents, p.contract_amount_is_tax_inclusive, p.tax_rate,
                p.commission_mode, p.commission_rate, p.commission_amount_cents,
                p.commission_settled, p.start_date, p.end_date
         FROM projects p JOIN companies co ON co.id = p.company_id
         LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
         WHERE p.id = ?1 AND p.deleted_at IS NULL",
        [project_id],
        |row| {
            Ok(ProjectRow {
                id: row.get(0)?,
                company_id: row.get(1)?,
                company_name: row.get(2)?,
                name: row.get(3)?,
                client_id: row.get(4)?,
                client_name: row.get(5)?,
                status: row.get(6)?,
                contract_cents: row.get(7)?,
                tax_inclusive: row.get::<_, i64>(8)? != 0,
                tax_rate: row.get(9)?,
                commission_mode: row.get(10)?,
                commission_rate: row.get(11)?,
                commission_amount_cents: row.get(12)?,
                commission_settled: row.get::<_, i64>(13)? != 0,
            })
        },
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound {
            entity: "project",
            id: project_id,
        },
        other => AppError::Db(other),
    })
}

pub fn get_income_trend(
    conn: &Connection,
    input: &IncomeTrendInput,
) -> AppResult<Vec<IncomeTrendRow>> {
    validate_date_range(&input.range)?;
    let start = input
        .range
        .start_date
        .as_deref()
        .ok_or_else(|| AppError::Validation("趋势查询必须提供开始日期".into()))
        .and_then(parse_date)?;
    let end = input
        .range
        .end_date
        .as_deref()
        .ok_or_else(|| AppError::Validation("趋势查询必须提供结束日期".into()))
        .and_then(parse_date)?;
    let mut periods = BTreeMap::new();
    let mut cursor = match input.granularity {
        TrendGranularity::Month => NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap(),
        TrendGranularity::Year => NaiveDate::from_ymd_opt(start.year(), 1, 1).unwrap(),
    };
    while cursor <= end {
        let label = match input.granularity {
            TrendGranularity::Month => cursor.format("%Y-%m").to_string(),
            TrendGranularity::Year => cursor.format("%Y").to_string(),
        };
        periods.insert(label, IncomeMetrics::default());
        cursor = match input.granularity {
            TrendGranularity::Month => {
                let (year, month) = if cursor.month() == 12 {
                    (cursor.year() + 1, 1)
                } else {
                    (cursor.year(), cursor.month() + 1)
                };
                NaiveDate::from_ymd_opt(year, month, 1).unwrap()
            }
            TrendGranularity::Year => NaiveDate::from_ymd_opt(cursor.year() + 1, 1, 1).unwrap(),
        };
    }

    let companies = resolve_companies(conn, &input.scope)?;
    let projects = load_projects(conn, &companies, &input.range)?;
    for (period, metrics) in &mut periods {
        let range = match input.granularity {
            TrendGranularity::Month => {
                let month_start = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").unwrap();
                let (year, month) = if month_start.month() == 12 {
                    (month_start.year() + 1, 1)
                } else {
                    (month_start.year(), month_start.month() + 1)
                };
                let next = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                DateRange {
                    start_date: Some(month_start.to_string()),
                    end_date: Some((next - chrono::Duration::days(1)).to_string()),
                }
            }
            TrendGranularity::Year => DateRange {
                start_date: Some(format!("{period}-01-01")),
                end_date: Some(format!("{period}-12-31")),
            },
        };
        for project in &projects {
            add_metrics(metrics, &project_metrics(conn, project, &range)?);
        }
    }
    Ok(periods
        .into_iter()
        .map(|(period, metrics)| IncomeTrendRow { period, metrics })
        .collect())
}

pub fn list_payments_page(conn: &Connection, input: &PaymentListInput) -> AppResult<PaymentPage> {
    validate_date_range(&input.range)?;
    validate_page(input.offset, input.limit)?;
    let companies = resolve_companies(conn, &input.scope)?;
    let company_ids: Vec<i64> = companies.iter().map(|company| company.id).collect();
    let mut all = Vec::new();
    let (start, end) = date_bounds(&input.range);
    let mut statement = conn.prepare(
        "SELECT cp.id, p.company_id, co.name, p.id, p.name, cp.name,
                cp.expected_amount_cents, cp.expected_date, cp.actual_amount_cents,
                cp.actual_received_at
         FROM contract_payments cp
         JOIN projects p ON p.id = cp.project_id
         JOIN companies co ON co.id = p.company_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND cp.deleted_at IS NULL
           AND (?2 IS NULL OR p.id = ?2)
           AND (?3 IS NULL OR COALESCE(cp.actual_received_at, cp.expected_date) >= ?3)
           AND (?4 IS NULL OR COALESCE(cp.actual_received_at, cp.expected_date) <= ?4)
         ORDER BY COALESCE(cp.actual_received_at, cp.expected_date) DESC, cp.id DESC",
    )?;
    for company_id in company_ids {
        let rows = statement.query_map(params![company_id, input.project_id, start, end], |row| {
            let expected: i64 = row.get(6)?;
            let actual: Option<i64> = row.get(8)?;
            Ok(PaymentRow {
                id: row.get(0)?,
                company_id: row.get(1)?,
                company_name: row.get(2)?,
                project_id: row.get(3)?,
                project_name: row.get(4)?,
                name: row.get(5)?,
                expected_amount_cents: expected,
                expected_date: row.get(7)?,
                actual_amount_cents: actual,
                actual_received_at: row.get(9)?,
                outstanding_cents: expected - actual.unwrap_or(0),
            })
        })?;
        for row in rows {
            all.push(row?);
        }
    }
    let total = all.len() as i64;
    let items = all
        .into_iter()
        .skip(input.offset as usize)
        .take(input.limit as usize)
        .collect();
    Ok(PaymentPage {
        generated_at: generated_at(),
        offset: input.offset,
        limit: input.limit,
        total,
        items,
    })
}

pub fn list_income_details(
    conn: &Connection,
    input: &IncomeDetailInput,
) -> AppResult<IncomeDetailPage> {
    validate_date_range(&input.range)?;
    validate_page(input.offset, input.limit)?;
    let companies = resolve_companies(conn, &input.scope)?;
    let mut items = Vec::new();
    for company in companies {
        match input.kind {
            IncomeDetailKind::GeneralCost => {
                let mut statement = conn.prepare(
                    "SELECT ce.id, p.id, p.name, ce.incurred_at,
                            COALESCE(ce.description, cc.name), ce.amount_cents
                     FROM cost_entries ce
                     JOIN projects p ON p.id = ce.project_id
                     JOIN cost_categories cc ON cc.id = ce.category_id
                     WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND ce.deleted_at IS NULL
                       AND (?2 IS NULL OR p.id = ?2) AND (?3 IS NULL OR cc.id = ?3)
                       AND (?4 IS NULL OR ce.incurred_at >= ?4)
                       AND (?5 IS NULL OR ce.incurred_at <= ?5)
                     ORDER BY ce.incurred_at DESC, ce.id DESC",
                )?;
                let rows = statement.query_map(
                    params![
                        company.id,
                        input.project_id,
                        input.category_id,
                        input.range.start_date,
                        input.range.end_date
                    ],
                    |row| {
                        Ok(IncomeDetailRow {
                            id: row.get(0)?,
                            kind: "general_cost".into(),
                            company_id: company.id,
                            company_name: company.name.clone(),
                            project_id: row.get(1)?,
                            project_name: row.get(2)?,
                            occurred_at: row.get(3)?,
                            label: row.get(4)?,
                            amount_cents: row.get(5)?,
                            hours: None,
                        })
                    },
                )?;
                for row in rows {
                    items.push(row?);
                }
            }
            IncomeDetailKind::Labor => {
                let mut statement = conn.prepare(
                    "SELECT tl.id, p.id, p.name, tl.work_date, m.name, tl.hours,
                            CAST(ROUND(tl.hours / 8.0 * tl.daily_cost_snapshot_cents) AS INTEGER)
                     FROM time_logs tl
                     JOIN tasks t ON t.id = tl.task_id
                     JOIN projects p ON p.id = t.project_id
                     JOIN members m ON m.id = tl.member_id
                     WHERE p.company_id = ?1 AND p.deleted_at IS NULL
                       AND t.deleted_at IS NULL AND tl.deleted_at IS NULL
                       AND (?2 IS NULL OR p.id = ?2) AND (?3 IS NULL OR m.id = ?3)
                       AND (?4 IS NULL OR tl.work_date >= ?4)
                       AND (?5 IS NULL OR tl.work_date <= ?5)
                     ORDER BY tl.work_date DESC, tl.id DESC",
                )?;
                let rows = statement.query_map(
                    params![
                        company.id,
                        input.project_id,
                        input.member_id,
                        input.range.start_date,
                        input.range.end_date
                    ],
                    |row| {
                        Ok(IncomeDetailRow {
                            id: row.get(0)?,
                            kind: "labor".into(),
                            company_id: company.id,
                            company_name: company.name.clone(),
                            project_id: row.get(1)?,
                            project_name: row.get(2)?,
                            occurred_at: row.get(3)?,
                            label: row.get(4)?,
                            hours: Some(row.get(5)?),
                            amount_cents: row.get(6)?,
                        })
                    },
                )?;
                for row in rows {
                    items.push(row?);
                }
            }
            IncomeDetailKind::Commission => {
                let projects = load_projects(
                    conn,
                    &[company.clone()],
                    &input.range,
                )?;
                for project in projects {
                    if input.project_id.is_some_and(|id| id != project.id) {
                        continue;
                    }
                    let metrics = project_metrics(conn, &project, &input.range)?;
                    items.push(IncomeDetailRow {
                        id: project.id,
                        kind: "commission".into(),
                        company_id: company.id,
                        company_name: company.name.clone(),
                        project_id: project.id,
                        project_name: project.name,
                        occurred_at: None,
                        label: if project.commission_settled {
                            "已实现销售分成".into()
                        } else {
                            "潜在销售分成".into()
                        },
                        amount_cents: if project.commission_settled {
                            metrics.commission_realized_cents
                        } else {
                            metrics.commission_potential_cents
                        },
                        hours: None,
                    });
                }
            }
        }
    }
    items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at).then(right.id.cmp(&left.id)));
    let total = items.len() as i64;
    let items = items
        .into_iter()
        .skip(input.offset as usize)
        .take(input.limit as usize)
        .collect();
    Ok(IncomeDetailPage {
        generated_at: generated_at(),
        offset: input.offset,
        limit: input.limit,
        total,
        items,
    })
}
