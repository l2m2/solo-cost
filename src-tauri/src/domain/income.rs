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
    Company {
        company_id: i64,
    },
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
    #[serde(flatten, default)]
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
    pub company_id: i64,
    pub company_name: String,
    pub name: String,
    pub metrics: IncomeMetrics,
    pub collection_rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendGranularity {
    Month,
    Year,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncomeTrendInput {
    #[serde(flatten, default)]
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
    #[serde(flatten, default)]
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

#[derive(Debug, Clone)]
struct ProjectRow {
    id: i64,
    company_id: i64,
    company_name: String,
    name: String,
    client_id: Option<i64>,
    client_name: Option<String>,
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

fn fixed_commission_realized_in_range(
    conn: &Connection,
    project_id: i64,
    fixed_amount_cents: i64,
    range: &DateRange,
) -> AppResult<i64> {
    let mut statement = conn.prepare(
        "SELECT actual_amount_cents, actual_received_at
         FROM contract_payments
         WHERE project_id = ?1 AND deleted_at IS NULL AND actual_received_at IS NOT NULL
         ORDER BY actual_received_at, id",
    )?;
    let rows = statement.query_map([project_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let receipts: Vec<(i64, String)> = rows.collect::<Result<_, _>>()?;
    if receipts.is_empty() {
        return Ok(0);
    }

    let total_received: i64 = receipts.iter().map(|(amount, _)| amount).sum();
    let mut allocated = 0;
    let mut realized_in_range = 0;
    for (index, (amount, received_at)) in receipts.iter().enumerate() {
        let is_last = index + 1 == receipts.len();
        let share = if is_last || total_received == 0 {
            fixed_amount_cents - allocated
        } else {
            (fixed_amount_cents as f64 * *amount as f64 / total_received as f64).round() as i64
        };
        allocated += share;
        let after_start = range
            .start_date
            .as_deref()
            .map_or(true, |start| received_at.as_str() >= start);
        let before_end = range
            .end_date
            .as_deref()
            .map_or(true, |end| received_at.as_str() <= end);
        if after_start && before_end {
            realized_in_range += share;
        }
    }
    Ok(realized_in_range)
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
    _range: &DateRange,
) -> AppResult<Vec<ProjectRow>> {
    let mut statement = conn.prepare(
        "SELECT p.id, p.company_id, co.name, p.name, p.client_id, c.name,
                p.contract_amount_cents, p.contract_amount_is_tax_inclusive, p.tax_rate,
                p.commission_mode, p.commission_rate, p.commission_amount_cents,
                p.commission_settled, p.start_date, p.end_date
         FROM projects p
         JOIN companies co ON co.id = p.company_id
         LEFT JOIN clients c ON c.id = p.client_id AND c.deleted_at IS NULL
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL
         ORDER BY p.id",
    )?;
    let mut output = Vec::new();
    for company in companies {
        let rows = statement.query_map([company.id], |row| {
            Ok(ProjectRow {
                id: row.get(0)?,
                company_id: row.get(1)?,
                company_name: row.get(2)?,
                name: row.get(3)?,
                client_id: row.get(4)?,
                client_name: row.get(5)?,
                contract_cents: row.get(6)?,
                tax_inclusive: row.get::<_, i64>(7)? != 0,
                tax_rate: row.get(8)?,
                commission_mode: row.get(9)?,
                commission_rate: row.get(10)?,
                commission_amount_cents: row.get(11)?,
                commission_settled: row.get::<_, i64>(12)? != 0,
            })
        })?;
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

fn project_metrics(
    conn: &Connection,
    project: &ProjectRow,
    range: &DateRange,
) -> AppResult<IncomeMetrics> {
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
    let received_exclusive = (received_inclusive as f64 / (1.0 + project.tax_rate)).round() as i64;
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
        "fixed" if project.commission_settled => fixed_commission_realized_in_range(
            conn,
            project.id,
            project.commission_amount_cents.unwrap_or(0),
            range,
        )?,
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
    if input.limit < 1 {
        return Err(AppError::Validation("limit 必须大于 0".into()));
    }
    let companies = resolve_companies(conn, &input.scope)?;
    let projects = load_projects(conn, &companies, &input.range)?;
    let mut groups: BTreeMap<(i64, i64, String, String), (IncomeMetrics, i64, i64)> =
        BTreeMap::new();
    for project in &projects {
        let metrics = project_metrics(conn, project, &input.range)?;
        let key = match input.dimension {
            RankDimension::Project => (
                project.id,
                project.company_id,
                project.company_name.clone(),
                project.name.clone(),
            ),
            RankDimension::Client => (
                project.client_id.unwrap_or(0),
                project.company_id,
                project.company_name.clone(),
                project
                    .client_name
                    .clone()
                    .unwrap_or_else(|| "未分配客户".into()),
            ),
        };
        let entry = groups.entry(key).or_default();
        add_metrics(&mut entry.0, &metrics);
        entry.1 += metrics.received_exclusive_cents;
        entry.2 += metrics.contract_exclusive_cents;
    }
    let mut rows: Vec<IncomeRankRow> = groups
        .into_iter()
        .map(
            |((id, company_id, company_name, name), (metrics, received, contract))| IncomeRankRow {
                id,
                company_id,
                company_name,
                name,
                collection_rate: if contract == 0 {
                    0.0
                } else {
                    received as f64 / contract as f64
                },
                metrics,
            },
        )
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
    let limit = usize::try_from(input.limit).unwrap_or(usize::MAX);
    rows.truncate(limit);
    Ok(rows)
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
                let month_start =
                    NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").unwrap();
                let (year, month) = if month_start.month() == 12 {
                    (month_start.year() + 1, 1)
                } else {
                    (month_start.year(), month_start.month() + 1)
                };
                let next = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                DateRange {
                    start_date: Some(std::cmp::max(month_start, start).to_string()),
                    end_date: Some(
                        std::cmp::min(next - chrono::Duration::days(1), end).to_string(),
                    ),
                }
            }
            TrendGranularity::Year => DateRange {
                start_date: Some(
                    std::cmp::max(
                        NaiveDate::parse_from_str(&format!("{period}-01-01"), "%Y-%m-%d").unwrap(),
                        start,
                    )
                    .to_string(),
                ),
                end_date: Some(
                    std::cmp::min(
                        NaiveDate::parse_from_str(&format!("{period}-12-31"), "%Y-%m-%d").unwrap(),
                        end,
                    )
                    .to_string(),
                ),
            },
        };
        for project in &projects {
            let mut period_metrics = project_metrics(conn, project, &range)?;
            // Contract and potential commission describe the whole project. They are
            // not repeated in every cash-flow period; the trend is an earned/received
            // view, while the overview remains the source for the potential total.
            period_metrics.contract_exclusive_cents = 0;
            period_metrics.commission_potential_cents = 0;
            period_metrics.take_home_potential_cents = 0;
            period_metrics.residual_profit_potential_cents = 0;
            add_metrics(metrics, &period_metrics);
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
        let rows =
            statement.query_map(params![company_id, input.project_id, start, end], |row| {
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

pub fn list_receivables_as_of(
    conn: &Connection,
    scope: &IncomeScope,
    end_date: &str,
) -> AppResult<Vec<PaymentRow>> {
    parse_date(end_date)?;
    let companies = resolve_companies(conn, scope)?;
    let mut receivables = Vec::new();
    let mut statement = conn.prepare(
        "SELECT cp.id, p.company_id, co.name, p.id, p.name, cp.name,
                cp.expected_amount_cents, cp.expected_date,
                CASE WHEN cp.actual_received_at <= ?2 THEN cp.actual_amount_cents ELSE NULL END,
                CASE WHEN cp.actual_received_at <= ?2 THEN cp.actual_received_at ELSE NULL END
         FROM contract_payments cp
         JOIN projects p ON p.id = cp.project_id
         JOIN companies co ON co.id = p.company_id
         WHERE p.company_id = ?1 AND p.deleted_at IS NULL AND cp.deleted_at IS NULL
           AND (cp.expected_date IS NULL OR cp.expected_date <= ?2)
           AND cp.expected_amount_cents
               - COALESCE(CASE WHEN cp.actual_received_at <= ?2 THEN cp.actual_amount_cents END, 0) > 0
         ORDER BY cp.expected_date IS NULL, cp.expected_date, cp.id",
    )?;
    for company in companies {
        let rows = statement.query_map(params![company.id, end_date], |row| {
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
            receivables.push(row?);
        }
    }
    Ok(receivables)
}
