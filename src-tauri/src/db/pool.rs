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

// Used by unit tests in this crate; not part of the production API surface.
#[cfg(test)]
pub fn open_in_memory_for_test(password: &str) -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    apply_key(&conn, password)?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

// Reserved for the M4 `change_password` command; tests already exercise it.
#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_with_correct_password() {
        let conn = open_in_memory_for_test("secret").unwrap();
        conn.execute("CREATE TABLE t (x INTEGER)", []).unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn rekey_changes_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = open_encrypted(&path, "old-pass").unwrap();
        conn.execute("CREATE TABLE marker (v TEXT)", []).unwrap();
        conn.execute("INSERT INTO marker VALUES ('hi')", [])
            .unwrap();
        rekey(&conn, "new-pass").unwrap();
        drop(conn);

        // old password should fail
        assert!(open_encrypted(&path, "old-pass").is_err());
        // new password should succeed and data should be intact
        let conn = open_encrypted(&path, "new-pass").unwrap();
        let v: String = conn
            .query_row("SELECT v FROM marker", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hi");
    }

    #[test]
    fn wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        let conn = open_encrypted(&path, "right").unwrap();
        conn.execute("CREATE TABLE t (x INTEGER)", []).unwrap();
        drop(conn);

        assert!(open_encrypted(&path, "wrong").is_err());
    }
}
