// One-off maintenance tool: refresh stale daily_cost_snapshot_cents for
// zentao-imported time_logs to match current members.daily_cost_cents.
// Delete this file after use.

use rusqlite::{params, Connection};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    let db_path = PathBuf::from(home).join("Library/Application Support/com.tauri.dev/data.db");
    println!("DB: {}", db_path.display());

    print!("Password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().lock().read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);

    let conn = Connection::open(&db_path)?;
    conn.execute_batch(&format!(
        "PRAGMA key = '{}';",
        password.replace('\'', "''")
    ))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Verify decryption
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| "wrong password or DB not decryptable")?;

    let candidates: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM time_logs tl
         WHERE tl.deleted_at IS NULL
           AND EXISTS (
             SELECT 1 FROM tasks t
             WHERE t.id = tl.task_id
               AND t.deleted_at IS NULL
               AND t.external_ref LIKE 'zentao:%'
           )",
        [],
        |r| r.get(0),
    )?;

    let stale: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM time_logs tl
         JOIN members m ON m.id = tl.member_id
         WHERE tl.deleted_at IS NULL
           AND tl.daily_cost_snapshot_cents <> m.daily_cost_cents
           AND EXISTS (
             SELECT 1 FROM tasks t
             WHERE t.id = tl.task_id
               AND t.deleted_at IS NULL
               AND t.external_ref LIKE 'zentao:%'
           )",
        [],
        |r| r.get(0),
    )?;

    println!("Zentao-imported time_logs total:            {}", candidates);
    println!("Rows where snapshot differs from current:   {}", stale);

    if stale == 0 {
        println!("Nothing to update.");
        return Ok(());
    }

    // Preview: per-member sample of what will change
    let mut stmt = conn.prepare(
        "SELECT m.name,
                COUNT(*) AS n,
                tl.daily_cost_snapshot_cents AS old_cents,
                m.daily_cost_cents AS new_cents
         FROM time_logs tl
         JOIN members m ON m.id = tl.member_id
         WHERE tl.deleted_at IS NULL
           AND tl.daily_cost_snapshot_cents <> m.daily_cost_cents
           AND EXISTS (
             SELECT 1 FROM tasks t
             WHERE t.id = tl.task_id
               AND t.deleted_at IS NULL
               AND t.external_ref LIKE 'zentao:%'
           )
         GROUP BY m.id, tl.daily_cost_snapshot_cents, m.daily_cost_cents
         ORDER BY m.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    println!("\nPreview (member | rows | old→new, in cents):");
    for row in rows {
        let (name, n, old_c, new_c) = row?;
        println!("  {:<20} {:>4}  {} -> {}", name, n, old_c, new_c);
    }

    print!("\nProceed with UPDATE? (y/N): ");
    io::stdout().flush()?;
    let mut confirm = String::new();
    io::stdin().lock().read_line(&mut confirm)?;
    if confirm.trim() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE time_logs
         SET daily_cost_snapshot_cents = (
           SELECT m.daily_cost_cents FROM members m WHERE m.id = time_logs.member_id
         )
         WHERE deleted_at IS NULL
           AND task_id IN (
             SELECT id FROM tasks
             WHERE deleted_at IS NULL AND external_ref LIKE 'zentao:%'
           )",
        params![],
    )?;
    tx.commit()?;

    println!("Updated {} rows.", n);
    Ok(())
}
