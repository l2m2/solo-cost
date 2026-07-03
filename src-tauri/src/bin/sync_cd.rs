// One-off: sync project/client/payment data from ~/OneDrive/gongwo/C and D contracts.
// Adds C (c001 / 大同市盛讯) and D (d001 / 无锡星禾) on top of the existing A data.
// Projects are skipped if one with the same name already exists, so re-runs are safe.
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
            name: "大同市盛讯科技有限公司",
            contact_name: Some("王嘉乐"),
            contact_info: Some("18135266306"),
            tax_id: Some("91140200MA0MURUA7D"),
            legal_name: Some("大同市盛讯科技有限公司"),
            notes: Some("山西省大同市平城区迎宾东路14号凯旋城1栋2016室"),
        },
        ClientRow {
            name: "无锡星禾数字科技有限公司",
            contact_name: Some("喻成明"),
            contact_info: Some("13301518765"),
            tax_id: Some("91320214MAD09PCD0L"),
            legal_name: Some("无锡星禾数字科技有限公司"),
            notes: Some("无锡市新吴区龙山路4号C幢120-3；另签有长期采购框架合同（一事一议，按开票结算）"),
        },
    ]
}

fn projects() -> Vec<ProjectRow> {
    let r = |name: &'static str, amt: i64, date: &'static str| Milestone { name, amount_cents: amt, received_at: Some(date) };

    vec![
        // ---- 大同市盛讯 (c001) ----
        // GW2404001-01 离线OCR服务，2000 元含 1%，交付 2024/4/12，全额发票 2024-06-12。
        ProjectRow {
            name: "c001", client_key: "大同市盛讯科技有限公司",
            amount_cents: 200_000, tax_rate: 0.01, is_tax_inclusive: true, status: "settled",
            milestones: vec![r("合同款", 200_000, "2024-06-12")],
        },
        // ---- 无锡星禾 (d001) ----
        // GW250806-01 数采平台 GWCAT，8000 元含 3%，全额发票 2025-10-17。
        ProjectRow {
            name: "d001", client_key: "无锡星禾数字科技有限公司",
            amount_cents: 800_000, tax_rate: 0.03, is_tax_inclusive: true, status: "settled",
            milestones: vec![r("合同款", 800_000, "2025-10-17")],
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

    // 2. Insert projects + payments (skip if project name already exists)
    println!("\n--- Projects ---");
    let receipt_total = |ms: &[Milestone]| -> i64 { ms.iter().filter(|m| m.received_at.is_some()).map(|m| m.amount_cents).sum() };
    let mut project_count = 0u32;
    let mut payment_count = 0u32;
    let mut total_contract = 0i64;
    let mut total_received = 0i64;

    for p in &projects() {
        let client_id = *client_ids.get(p.client_key)
            .unwrap_or_else(|| panic!("client not found: {}", p.client_key));

        let existing: Option<i64> = tx.query_row(
            "SELECT id FROM projects WHERE company_id=?1 AND name=?2 AND deleted_at IS NULL",
            params![company_id, p.name], |r| r.get(0),
        ).ok();
        if let Some(pid) = existing {
            println!("  skip {:<8} (already exists id={})", p.name, pid);
            continue;
        }

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
