// One-off: HARD delete every task and time_log across all projects, so the zentao
// CSVs can be re-imported with the new fields (started_at / completed_at / closed).
// Reads the master password from stdin. Prints counts and asks for confirmation.
// Delete this file after use.

use rusqlite::Connection;
use std::io::{self, BufRead, Write};

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
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|_| "wrong password or DB not decryptable")?;

    let tasks: i64 = conn.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))?;
    let logs: i64 = conn.query_row("SELECT count(*) FROM time_logs", [], |r| r.get(0))?;
    println!("\nAbout to HARD DELETE: {tasks} tasks, {logs} time_logs (all projects).");
    print!("Type 'yes' to confirm: ");
    io::stdout().flush()?;
    let mut ans = String::new();
    io::stdin().lock().read_line(&mut ans)?;
    if ans.trim() != "yes" {
        println!("Aborted.");
        return Ok(());
    }

    // Disable FK enforcement so the child-first ordering isn't strictly required,
    // then delete logs before tasks anyway to keep it clean.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let tx = conn.unchecked_transaction()?;
    let del_logs = tx.execute("DELETE FROM time_logs", [])?;
    let del_tasks = tx.execute("DELETE FROM tasks", [])?;
    tx.commit()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    println!("Deleted {del_logs} time_logs, {del_tasks} tasks.");
    Ok(())
}
