// One-off: sync project/client/payment data from ~/OneDrive/gongwo/A contracts.
// Delete this file after use.

use rusqlite::{params, Connection};
use std::io::{self, BufRead, Write};

struct ClientRow {
    name: &'static str,
    contact_name: Option<&'static str>,
    contact_info: Option<&'static str>,
    tax_id: Option<&'static str>,
    legal_name: Option<&'static str>,
    notes: Option<&'static str>,
}

struct Milestone {
    name: &'static str,
    amount_cents: i64,
    received_at: Option<&'static str>,
}

struct ProjectRow {
    name: &'static str,
    client_key: &'static str,
    amount_cents: i64,
    tax_rate: f64,
    is_tax_inclusive: bool,
    status: &'static str,
    milestones: Vec<Milestone>,
}

fn clients() -> Vec<ClientRow> {
    vec![
        ClientRow {
            name: "南通市登越智能科技有限公司",
            contact_name: Some("王晓斌"),
            contact_info: Some("15358009606"),
            tax_id: Some("91320682MA26G5TNXT"),
            legal_name: Some("南通市登越智能科技有限公司"),
            notes: Some("邮寄地址：江苏省无锡市梁溪区圆融发展中心1707，收件人：单志昶 13921284126"),
        },
        ClientRow {
            name: "四川梯鸥智能科技有限公司",
            contact_name: Some("王晓斌"),
            contact_info: Some("028-85895775"),
            tax_id: Some("91510100MA63DKQ445"),
            legal_name: Some("四川梯鸥智能科技有限公司"),
            notes: Some("成都市高新区世纪城南路599号5栋7层702号"),
        },
        ClientRow {
            name: "无锡灏爵智能科技有限公司",
            contact_name: Some("王晓斌"),
            contact_info: None,
            tax_id: Some("91320214MAD190FW8A"),
            legal_name: Some("无锡灏爵智能科技有限公司"),
            notes: Some("无锡市新吴区旺庄街道龙山路4-C-703"),
        },
        ClientRow {
            name: "重庆鑫方盛电子商务有限公司",
            contact_name: None,
            contact_info: Some("023-68060593"),
            tax_id: Some("91500107663561511M"),
            legal_name: Some("重庆鑫方盛电子商务有限公司"),
            notes: Some("重庆市九龙坡区西彭镇森迪大道141号（供应商，非客户）"),
        },
    ]
}

fn projects() -> Vec<ProjectRow> {
    let r = |name: &'static str, amt: i64, date: &'static str| Milestone { name, amount_cents: amt, received_at: Some(date) };
    let u = |name: &'static str, amt: i64| Milestone { name, amount_cents: amt, received_at: None };

    vec![
        // ---- 登越 (9 projects) ----
        ProjectRow {
            name: "a000", client_key: "南通市登越智能科技有限公司",
            amount_cents: 500_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![r("合同款", 500_000, "2025-03-20")],
        },
        ProjectRow {
            name: "a000-2", client_key: "南通市登越智能科技有限公司",
            amount_cents: 1_500_000, tax_rate: 0.03, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 30%", 450_000, "2025-07-23"),
                r("验收款 60%", 900_000, "2025-11-25"),
                u("质保款 10%", 150_000),
            ],
        },
        ProjectRow {
            name: "a001", client_key: "南通市登越智能科技有限公司",
            amount_cents: 5_000_000, tax_rate: 0.03, is_tax_inclusive: true, status: "settled",
            milestones: vec![r("合同款", 5_000_000, "2023-01-06")],
        },
        ProjectRow {
            name: "a003", client_key: "南通市登越智能科技有限公司",
            amount_cents: 24_500_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 40%", 9_800_000, "2023-01-16"),
                r("交付款 30%", 7_350_000, "2023-11-30"),
                r("验收款 30%", 7_350_000, "2024-07-31"),
            ],
        },
        ProjectRow {
            name: "a005", client_key: "南通市登越智能科技有限公司",
            amount_cents: 1_800_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 50%", 900_000, "2023-08-07"),
                u("验收款 50%", 900_000),
            ],
        },
        ProjectRow {
            name: "a008", client_key: "南通市登越智能科技有限公司",
            amount_cents: 3_800_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 30%", 1_140_000, "2024-07-22"),
                u("验收款 70%", 2_660_000),
            ],
        },
        ProjectRow {
            name: "a010", client_key: "南通市登越智能科技有限公司",
            amount_cents: 12_100_000, tax_rate: 0.03, is_tax_inclusive: true, status: "delivered",
            milestones: vec![
                r("预付款 30%", 3_630_000, "2025-01-06"),
                r("验收款 70%", 8_470_000, "2025-11-25"),
            ],
        },
        ProjectRow {
            name: "a010-2", client_key: "南通市登越智能科技有限公司",
            amount_cents: 500_000, tax_rate: 0.03, is_tax_inclusive: true, status: "delivered",
            milestones: vec![r("验收款 100%", 500_000, "2026-07-01")],
        },
        ProjectRow {
            name: "a011", client_key: "南通市登越智能科技有限公司",
            amount_cents: 3_700_000, tax_rate: 0.03, is_tax_inclusive: true, status: "delivered",
            milestones: vec![
                r("预付款 30%", 1_110_000, "2025-07-22"),
                u("验收款 60%", 2_220_000),
                u("质保款 10%", 370_000),
            ],
        },
        // ---- 梯鸥 (4 projects) ----
        ProjectRow {
            name: "a007", client_key: "四川梯鸥智能科技有限公司",
            amount_cents: 3_600_000, tax_rate: 0.0, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 30%", 1_080_000, "2024-05-10"),
                r("年末款 70%", 2_520_000, "2024-06-25"),
            ],
        },
        ProjectRow {
            name: "a009", client_key: "四川梯鸥智能科技有限公司",
            amount_cents: 10_000_000, tax_rate: 0.03, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 30%", 3_000_000, "2024-10-08"),
                r("验收款 60%", 6_000_000, "2024-12-26"),
                r("尾款 10%", 1_000_000, "2025-07-22"),
            ],
        },
        ProjectRow {
            name: "a010-3", client_key: "四川梯鸥智能科技有限公司",
            amount_cents: 9_200_000, tax_rate: 0.03, is_tax_inclusive: true, status: "delivered",
            milestones: vec![
                r("预付款 30%", 2_760_000, "2026-03-17"),
                r("验收款 60%", 5_520_000, "2026-07-01"),
                u("质保款 10%", 920_000),
            ],
        },
        ProjectRow {
            name: "a012", client_key: "四川梯鸥智能科技有限公司",
            amount_cents: 25_000_000, tax_rate: 0.03, is_tax_inclusive: true, status: "in_progress",
            milestones: vec![
                r("预付款 30%", 7_500_000, "2026-03-02"),
                u("验收款 60%", 15_000_000),
                u("质保款 10%", 2_500_000),
            ],
        },
        // ---- 灏爵 (1 project) ----
        ProjectRow {
            name: "a006", client_key: "无锡灏爵智能科技有限公司",
            amount_cents: 6_000_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![
                r("预付款 50%", 3_000_000, "2023-11-30"),
                r("验收款 50%", 3_000_000, "2024-05-10"),
            ],
        },
        // ---- 鑫方盛 (1 project) ----
        ProjectRow {
            name: "a005-2", client_key: "重庆鑫方盛电子商务有限公司",
            amount_cents: 1_500_000, tax_rate: 0.03, is_tax_inclusive: true, status: "settled",
            milestones: vec![r("货款 100%", 1_500_000, "2026-06-26")],
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    let db_path = std::path::PathBuf::from(home)
        .join("Library/Application Support/com.tauri.dev/data.db");
    println!("DB: {}", db_path.display());

    print!("Password: ");
    io::stdout().flush()?;
    let mut pw = String::new();
    io::stdin().lock().read_line(&mut pw)?;
    let pw = pw.trim_end_matches(['\r', '\n']);

    let conn = Connection::open(&db_path)?;
    conn.execute_batch(&format!("PRAGMA key = '{}';", pw.replace('\'', "''")))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|_| "wrong password or DB not decryptable")?;

    let company_id: i64 = conn.query_row(
        "SELECT id FROM companies WHERE deleted_at IS NULL ORDER BY id LIMIT 1",
        [], |r| r.get(0),
    )?;
    println!("Company ID: {company_id}");

    let tx = conn.unchecked_transaction()?;

    // 1. Upsert clients
    println!("\n--- Clients ---");
    let mut client_ids: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for c in &clients() {
        tx.execute(
            "INSERT OR IGNORE INTO clients(company_id, name, contact_name, contact_info,
                                           tax_id, legal_name, notes)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![company_id, c.name, c.contact_name, c.contact_info, c.tax_id, c.legal_name, c.notes],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM clients WHERE company_id=?1 AND name=?2 AND deleted_at IS NULL",
            params![company_id, c.name], |r| r.get(0),
        )?;
        client_ids.insert(c.name, id);
        println!("  {:<30} id={}", c.name, id);
    }

    // 2. Insert projects + payments
    println!("\n--- Projects ---");
    let receipt_total = |ms: &[Milestone]| -> i64 { ms.iter().filter(|m| m.received_at.is_some()).map(|m| m.amount_cents).sum() };
    let mut project_count = 0u32;
    let mut payment_count = 0u32;
    let mut total_contract = 0i64;
    let mut total_received = 0i64;

    for p in &projects() {
        let client_id = *client_ids.get(p.client_key)
            .unwrap_or_else(|| panic!("client not found: {}", p.client_key));

        tx.execute(
            "INSERT INTO projects(company_id, name, client_id, status,
                                   contract_amount_cents,
                                   contract_amount_is_tax_inclusive, tax_rate)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![company_id, p.name, client_id, p.status, p.amount_cents,
                    p.is_tax_inclusive as i64, p.tax_rate],
        )?;
        let project_id = tx.last_insert_rowid();
        let rcvd = receipt_total(&p.milestones);
        total_contract += p.amount_cents;
        total_received += rcvd;
        println!("  id={:<4} {:<8} contract={:>10} received={:>10}  {}", project_id, p.name, p.amount_cents, rcvd, p.status);
        project_count += 1;

        for (i, m) in p.milestones.iter().enumerate() {
            tx.execute(
                "INSERT INTO contract_payments(project_id, name, expected_amount_cents,
                                               actual_amount_cents, actual_received_at, sort_order)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![project_id, m.name, m.amount_cents,
                        m.received_at.map(|_| m.amount_cents), m.received_at, i as i64],
            )?;
            payment_count += 1;
        }
    }

    tx.commit()?;

    println!("\n--- Summary ---");
    println!("  Clients:      {}", client_ids.len());
    println!("  Projects:     {project_count}");
    println!("  Payments:     {payment_count}");
    println!("  Total contract: {:>12}", total_contract);
    println!("  Total received: {:>12}", total_received);
    println!("  Outstanding:    {:>12}", total_contract - total_received);
    Ok(())
}
