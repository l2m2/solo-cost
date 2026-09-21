use crate::domain::income::{
    self, DateRange, IncomeScope, IncomeTrendInput, PaymentListInput, RankDimension,
    RankIncomeInput, RankMetric, TrendGranularity,
};
use crate::error::{AppError, AppResult};
use chrono::Utc;
use rusqlite::Connection;

pub struct ChatGptReportInput {
    pub company_id: i64,
    pub start_date: String,
    pub end_date: String,
}

fn money(cents: i64) -> String {
    format!("¥{:.2}", cents as f64 / 100.0)
}

fn percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

pub fn build(conn: &Connection, input: &ChatGptReportInput) -> AppResult<String> {
    if input.company_id <= 0 {
        return Err(AppError::Validation("请选择公司".into()));
    }

    let scope = IncomeScope::Company {
        company_id: input.company_id,
    };
    let range = DateRange {
        start_date: Some(input.start_date.clone()),
        end_date: Some(input.end_date.clone()),
    };
    let overview = income::get_income_overview(conn, &scope, &range)?;
    let projects = income::rank_income_sources(
        conn,
        &RankIncomeInput {
            scope: scope.clone(),
            range: range.clone(),
            dimension: RankDimension::Project,
            metric: RankMetric::TakeHome,
            limit: 200,
        },
    )?;
    let trend = income::get_income_trend(
        conn,
        &IncomeTrendInput {
            scope: scope.clone(),
            range: range.clone(),
            granularity: TrendGranularity::Month,
        },
    )?;
    let mut payment_rows = Vec::new();
    loop {
        let page = income::list_payments_page(
            conn,
            &PaymentListInput {
                scope: scope.clone(),
                range: range.clone(),
                project_id: None,
                offset: payment_rows.len() as i64,
                limit: 200,
            },
        )?;
        let total = page.total;
        let page_len = page.items.len();
        payment_rows.extend(page.items);
        if page_len == 0 || payment_rows.len() as i64 >= total {
            break;
        }
    }

    let company = overview
        .companies
        .first()
        .ok_or_else(|| AppError::Validation("找不到所选公司".into()))?;
    let metrics = &overview.metrics;
    let expected_total: i64 = payment_rows
        .iter()
        .map(|row| row.expected_amount_cents)
        .sum();
    let received_total: i64 = payment_rows
        .iter()
        .map(|row| row.actual_amount_cents.unwrap_or(0))
        .sum();
    let outstanding_total: i64 = payment_rows
        .iter()
        .map(|row| row.outstanding_cents.max(0))
        .sum();
    let collection_rate = if expected_total == 0 {
        0.0
    } else {
        received_total as f64 / expected_total as f64
    };

    let mut output = String::new();
    output.push_str("# Solo Cost · ChatGPT 决策报表\n\n");
    output.push_str(&format!("- 公司：{}\n", markdown_cell(&company.name)));
    output.push_str(&format!(
        "- 统计范围：{} 至 {}\n",
        input.start_date, input.end_date
    ));
    output.push_str(&format!(
        "- 生成时间：{}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    output.push_str("- 金额单位：人民币元\n\n");

    output.push_str("## 使用说明与统计口径\n\n");
    output.push_str("请基于本报表分析收入、回款、成本、人工收入、利润和应收风险；先引用数据，再给出判断与行动建议。\n\n");
    output.push_str("- 合同收入、实际回款均按不含税口径统计。\n");
    output.push_str("- 到手收入 = 收入 - 销售分成 - 一般成本。\n");
    output.push_str("- 人工收入属于本人收入，已经包含在到手收入中，不应重复相加。\n");
    output.push_str("- 剩余利润 = 到手收入 - 人工收入。\n");
    output.push_str("- 潜在值以合同收入计算；已实现值以实际回款计算。\n\n");

    output.push_str("## 核心指标\n\n");
    output.push_str("| 指标 | 潜在/合同口径 | 已实现/回款口径 |\n");
    output.push_str("| --- | ---: | ---: |\n");
    output.push_str(&format!(
        "| 收入 | {} | {} |\n",
        money(metrics.contract_exclusive_cents),
        money(metrics.received_exclusive_cents)
    ));
    output.push_str(&format!(
        "| 销售分成 | {} | {} |\n",
        money(metrics.commission_potential_cents),
        money(metrics.commission_realized_cents)
    ));
    output.push_str(&format!(
        "| 一般成本 | {} | {} |\n",
        money(metrics.general_cost_cents),
        money(metrics.general_cost_cents)
    ));
    output.push_str(&format!(
        "| 到手收入 | {} | {} |\n",
        money(metrics.take_home_potential_cents),
        money(metrics.take_home_realized_cents)
    ));
    output.push_str(&format!(
        "| 人工收入 | {} | {} |\n",
        money(metrics.labor_income_cents),
        money(metrics.labor_income_cents)
    ));
    output.push_str(&format!(
        "| 剩余利润 | {} | {} |\n\n",
        money(metrics.residual_profit_potential_cents),
        money(metrics.residual_profit_realized_cents)
    ));
    output.push_str(&format!(
        "- 计划收款（含税录入金额）：{}\n- 已收款（含税录入金额）：{}\n- 应收款（含税录入金额）：{}\n- 收款率：{}\n\n",
        money(expected_total),
        money(received_total),
        money(outstanding_total),
        percent(collection_rate)
    ));

    output.push_str("## 项目排行（按已收到手收入）\n\n");
    output.push_str("| 项目 | 合同收入 | 实际回款 | 销售分成 | 一般成本 | 人工收入 | 到手收入 | 剩余利润 | 收款率 |\n");
    output.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in &projects {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(&row.name),
            money(row.metrics.contract_exclusive_cents),
            money(row.metrics.received_exclusive_cents),
            money(row.metrics.commission_realized_cents),
            money(row.metrics.general_cost_cents),
            money(row.metrics.labor_income_cents),
            money(row.metrics.take_home_realized_cents),
            money(row.metrics.residual_profit_realized_cents),
            percent(row.collection_rate)
        ));
    }
    if projects.is_empty() {
        output.push_str("| 暂无项目数据 | - | - | - | - | - | - | - | - |\n");
    }

    output.push_str("\n## 月度趋势\n\n");
    output.push_str("| 月份 | 实际回款 | 销售分成 | 一般成本 | 人工收入 | 到手收入 | 剩余利润 |\n");
    output.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in &trend {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            row.period,
            money(row.metrics.received_exclusive_cents),
            money(row.metrics.commission_realized_cents),
            money(row.metrics.general_cost_cents),
            money(row.metrics.labor_income_cents),
            money(row.metrics.take_home_realized_cents),
            money(row.metrics.residual_profit_realized_cents)
        ));
    }

    output.push_str("\n## 应收明细\n\n");
    output.push_str("| 项目 | 款项 | 预计日期 | 计划金额 | 已收金额 | 应收金额 |\n");
    output.push_str("| --- | --- | --- | ---: | ---: | ---: |\n");
    let mut receivable_count = 0;
    for row in payment_rows.iter().filter(|row| row.outstanding_cents > 0) {
        receivable_count += 1;
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(&row.project_name),
            markdown_cell(&row.name),
            row.expected_date.as_deref().unwrap_or("未填写"),
            money(row.expected_amount_cents),
            money(row.actual_amount_cents.unwrap_or(0)),
            money(row.outstanding_cents)
        ));
    }
    if receivable_count == 0 {
        output.push_str("| 暂无应收款 | - | - | - | - | - |\n");
    }

    Ok(output)
}
