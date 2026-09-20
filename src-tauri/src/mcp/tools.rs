use crate::domain::income::{
    self, DateRange, IncomeDetailInput, IncomeScope, IncomeTrendInput, PaymentListInput,
    RankIncomeInput,
};
use crate::error::AppError;
use crate::state::AppState;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "get_income_overview",
            "查询收入决策总览。到手已包含人工收入，人工收入不是额外加项。",
            scope_schema(Map::from_iter([
                ("start_date".into(), date_property()),
                ("end_date".into(), date_property()),
            ])),
        ),
        tool(
            "rank_income_sources",
            "按客户或项目排行收入、到手、人工收入、剩余利润或回款率。",
            scope_schema(Map::from_iter([
                ("start_date".into(), date_property()),
                ("end_date".into(), date_property()),
                (
                    "dimension".into(),
                    json!({"type":"string","enum":["client","project"]}),
                ),
                (
                    "metric".into(),
                    json!({"type":"string","enum":["income","take_home","labor_income","residual_profit","collection_rate"]}),
                ),
                (
                    "limit".into(),
                    json!({"type":"integer","minimum":1,"maximum":200,"default":10}),
                ),
            ])),
        ),
        tool(
            "get_project_decision_summary",
            "查询单个项目的合同、回款、到手、人工收入、剩余利润和任务进度。",
            json!({
                "type":"object",
                "properties":{"project_id":{"type":"integer","minimum":1}},
                "required":["project_id"],
                "additionalProperties":false
            }),
        ),
        tool(
            "get_income_trend",
            "按月或按年查询收入、到手、人工收入和剩余利润趋势。",
            scope_schema(Map::from_iter([
                ("start_date".into(), date_property()),
                ("end_date".into(), date_property()),
                (
                    "granularity".into(),
                    json!({"type":"string","enum":["month","year"]}),
                ),
            ])),
        ),
        tool(
            "list_payments",
            "分页查询应收、实收、回款日期和未回款余额。",
            scope_schema(Map::from_iter([
                ("start_date".into(), date_property()),
                ("end_date".into(), date_property()),
                ("project_id".into(), json!({"type":["integer","null"],"minimum":1})),
                ("offset".into(), json!({"type":"integer","minimum":0,"default":0})),
                ("limit".into(), json!({"type":"integer","minimum":1,"maximum":200,"default":50})),
            ])),
        ),
        tool(
            "list_income_details",
            "下钻一般成本、销售分成或人工明细，不返回备注全文。",
            scope_schema(Map::from_iter([
                ("start_date".into(), date_property()),
                ("end_date".into(), date_property()),
                ("kind".into(), json!({"type":"string","enum":["general_cost","commission","labor"]})),
                ("project_id".into(), json!({"type":["integer","null"],"minimum":1})),
                ("member_id".into(), json!({"type":["integer","null"],"minimum":1})),
                ("category_id".into(), json!({"type":["integer","null"],"minimum":1})),
                ("offset".into(), json!({"type":"integer","minimum":0,"default":0})),
                ("limit".into(), json!({"type":"integer","minimum":1,"maximum":200,"default":50})),
            ])),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn date_property() -> Value {
    json!({"type":["string","null"],"format":"date"})
}

fn scope_schema(extra: Map<String, Value>) -> Value {
    let mut properties = Map::from_iter([
        (
            "scope".into(),
            json!({"type":"string","enum":["current_company","company","all_companies"],"default":"current_company"}),
        ),
        ("company_id".into(), json!({"type":["integer","null"],"minimum":1})),
    ]);
    properties.extend(extra);
    json!({
        "type":"object",
        "properties":properties,
        "additionalProperties":false
    })
}

pub fn call(app: &AppHandle, name: &str, arguments: Value) -> Value {
    let result = match name {
        "get_income_overview" => parse::<OverviewInput>(arguments).and_then(|input| {
            with_conn(app, |conn| {
                income::get_income_overview(conn, &input.scope, &input.range)
            })
            .and_then(to_value)
        }),
        "rank_income_sources" => parse::<RankIncomeInput>(arguments).and_then(|input| {
            with_conn(app, |conn| income::rank_income_sources(conn, &input)).and_then(to_value)
        }),
        "get_project_decision_summary" => {
            parse::<ProjectInput>(arguments).and_then(|input| {
                with_conn(app, |conn| {
                    income::get_project_decision_summary(conn, input.project_id)
                })
                .and_then(to_value)
            })
        }
        "get_income_trend" => parse::<IncomeTrendInput>(arguments).and_then(|input| {
            with_conn(app, |conn| income::get_income_trend(conn, &input)).and_then(to_value)
        }),
        "list_payments" => parse::<PaymentListInput>(arguments).and_then(|input| {
            with_conn(app, |conn| income::list_payments_page(conn, &input)).and_then(to_value)
        }),
        "list_income_details" => parse::<IncomeDetailInput>(arguments).and_then(|input| {
            with_conn(app, |conn| income::list_income_details(conn, &input)).and_then(to_value)
        }),
        _ => Err(ToolError::new("INVALID_ARGUMENT", "未知工具")),
    };

    match result {
        Ok(value) => json!({
            "content":[{"type":"text","text":value.to_string()}],
            "structuredContent":value,
            "isError":false
        }),
        Err(error) => json!({
            "content":[{"type":"text","text":error.message}],
            "structuredContent":{"error":{"code":error.code,"message":error.message}},
            "isError":true
        }),
    }
}

#[derive(serde::Deserialize)]
struct OverviewInput {
    #[serde(flatten, default)]
    scope: IncomeScope,
    #[serde(flatten)]
    range: DateRange,
}

#[derive(serde::Deserialize)]
struct ProjectInput {
    project_id: i64,
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
    serde_json::from_value(arguments)
        .map_err(|_| ToolError::new("INVALID_ARGUMENT", "工具参数无效"))
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, ToolError> {
    serde_json::to_value(value)
        .map_err(|_| ToolError::new("INTERNAL_ERROR", "无法序列化查询结果"))
}

fn with_conn<T>(
    app: &AppHandle,
    query: impl FnOnce(&rusqlite::Connection) -> Result<T, AppError>,
) -> Result<T, ToolError> {
    let state = app.state::<AppState>();
    state.with_conn(query).map_err(ToolError::from)
}

struct ToolError {
    code: &'static str,
    message: String,
}

impl ToolError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<AppError> for ToolError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Locked => Self::new("APP_LOCKED", "应用尚未解锁"),
            AppError::NotFound { .. } => Self::new("NOT_FOUND", "未找到请求的数据"),
            AppError::Validation(message) => {
                let code = if message.contains("日期") {
                    "INVALID_DATE_RANGE"
                } else if message.contains("limit") {
                    "RESULT_LIMIT_EXCEEDED"
                } else {
                    "INVALID_ARGUMENT"
                };
                Self::new(code, message)
            }
            other => {
                tracing::error!(error_kind = %other, "MCP tool query failed");
                Self::new("INTERNAL_ERROR", "查询失败")
            }
        }
    }
}
