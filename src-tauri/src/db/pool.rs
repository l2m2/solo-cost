use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::Path;

pub fn open_encrypted(path: &Path, password: &str) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    apply_key(&conn, password)?;
    verify_key(&conn)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

// Reserved for the M4 `change_password` command.
#[allow(dead_code)]
pub fn rekey(conn: &Connection, new_password: &str) -> AppResult<()> {
    let escaped = escape_sqlite_string(new_password);
    conn.execute_batch(&format!("PRAGMA rekey = '{}';", escaped))?;
    Ok(())
}

fn apply_key(conn: &Connection, password: &str) -> AppResult<()> {
    let escaped = escape_sqlite_string(password);
    conn.execute_batch(&format!("PRAGMA key = '{}';", escaped))?;
    Ok(())
}

fn verify_key(conn: &Connection) -> AppResult<()> {
    // Attempt a read from sqlite_master to confirm the password is correct.
    // If the password is wrong, SQLCipher will fail to decrypt and return an error.
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(_) => Err(AppError::WrongPassword),
    }
}

fn apply_pragmas(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

// Escape a string for use in a SQLite single-quoted literal by doubling any single quotes.
// Used only for PRAGMA key/rekey which do not support bound parameters.
fn escape_sqlite_string(s: &str) -> String {
    s.replace('\'', "''")
}
