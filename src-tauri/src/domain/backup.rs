use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

pub fn backup_dir(app_data: &Path) -> PathBuf {
    app_data.join("backups")
}

pub fn wal_checkpoint(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

pub fn copy_encrypted_db(conn: &Connection, src: &Path, dst: &Path) -> AppResult<()> {
    wal_checkpoint(conn)?;
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst).map_err(|e| AppError::Backup(format!("copy: {e}")))?;
    let now: String = conn.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('last_backup_at', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&now],
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub absolute_path: String,
    pub size_bytes: u64,
    pub created_at: String,
}

fn parse_auto_backup_created_at(file_name: &str) -> String {
    // auto_YYYYMMDD_HHmmss.db → "YYYY-MM-DD HH:MM:SS"
    let stem = file_name.trim_end_matches(".db");
    let rest = stem.strip_prefix("auto_").unwrap_or(stem);
    // rest = "YYYYMMDD_HHmmss"
    if rest.len() == 15 && rest.chars().nth(8) == Some('_') {
        let (date, time) = rest.split_at(8);
        let (_, time) = time.split_at(1); // drop underscore
        if date.chars().all(|c| c.is_ascii_digit())
            && time.chars().all(|c| c.is_ascii_digit())
            && time.len() == 6
        {
            return format!(
                "{}-{}-{} {}:{}:{}",
                &date[0..4],
                &date[4..6],
                &date[6..8],
                &time[0..2],
                &time[2..4],
                &time[4..6],
            );
        }
    }
    // fallback: whole filename as-is
    file_name.to_string()
}

pub fn list_auto_backups(app_data: &Path) -> AppResult<Vec<BackupInfo>> {
    let dir = backup_dir(app_data);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with("auto_") || !name.ends_with(".db") {
            continue;
        }
        let meta = entry.metadata()?;
        out.push(BackupInfo {
            file_name: name.clone(),
            absolute_path: path.to_string_lossy().into_owned(),
            size_bytes: meta.len(),
            created_at: parse_auto_backup_created_at(&name),
        });
    }
    out.sort_by(|a, b| b.file_name.cmp(&a.file_name));
    Ok(out)
}

pub fn rotate_auto_backups(app_data: &Path, keep: usize) -> AppResult<usize> {
    let list = list_auto_backups(app_data)?;
    if list.len() <= keep {
        return Ok(0);
    }
    let mut deleted = 0;
    for old in list.into_iter().skip(keep) {
        let path = PathBuf::from(&old.absolute_path);
        fs::remove_file(&path).map_err(|e| AppError::Backup(format!("remove: {e}")))?;
        deleted += 1;
    }
    Ok(deleted)
}

// Consumed by T3 (unlock integrity gate); suppress until then.
#[allow(dead_code)]
pub fn integrity_check(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
    let mut rows = stmt.query([])?;
    let first: String = match rows.next()? {
        Some(row) => row.get(0)?,
        None => return Err(AppError::IntegrityCheckFailed("no rows returned".into())),
    };
    if first == "ok" {
        return Ok(());
    }
    // collect all details
    let mut details = vec![first];
    while let Some(row) = rows.next()? {
        details.push(row.get(0)?);
    }
    Err(AppError::IntegrityCheckFailed(details.join("; ")))
}

pub fn export_plaintext(conn: &Connection, dst: &Path) -> AppResult<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // ATTACH requires the destination path as a literal; SQLite does not accept bind params here.
    // Single quotes are escaped via replace to match the project's PRAGMA key escaping pattern.
    let dst_str = dst.to_string_lossy();
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS plaintext KEY '';
         SELECT sqlcipher_export('plaintext');
         DETACH DATABASE plaintext;",
        dst_str.replace('\'', "''"),
    ))
    .map_err(|e| AppError::Backup(format!("export: {e}")))?;
    Ok(())
}

pub fn last_backup_at(conn: &Connection) -> AppResult<Option<String>> {
    match conn.query_row(
        "SELECT value FROM app_meta WHERE key = 'last_backup_at'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Db(e)),
    }
}

pub fn should_auto_backup(conn: &Connection, now_iso: &str) -> AppResult<bool> {
    let last = last_backup_at(conn)?;
    let last = match last {
        Some(t) => t,
        None => return Ok(true),
    };
    // diff computed via SQL to avoid pulling a chrono dep
    let hours: f64 = conn.query_row(
        "SELECT (julianday(?1) - julianday(?2)) * 24.0",
        [now_iso, &last],
        |r| r.get(0),
    )?;
    Ok(hours > 24.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use crate::db::pool;
    use tempfile::{tempdir, TempDir};

    struct TestDb {
        conn: Connection,
        _dir: TempDir,
        db_path: PathBuf,
        app_data: PathBuf,
    }

    impl TestDb {
        fn new(password: &str) -> Self {
            let dir = tempdir().unwrap();
            let app_data = dir.path().to_path_buf();
            let db_path = app_data.join("data.db");
            let conn = setup_at(&db_path, password).unwrap();
            Self {
                conn,
                _dir: dir,
                db_path,
                app_data,
            }
        }
    }

    #[test]
    fn wal_checkpoint_returns_ok_on_fresh_db() {
        let db = TestDb::new("s");
        wal_checkpoint(&db.conn).unwrap();
    }

    #[test]
    fn integrity_check_passes_on_fresh_db() {
        let db = TestDb::new("s");
        integrity_check(&db.conn).unwrap();
    }

    #[test]
    fn copy_encrypted_db_produces_openable_backup() {
        let db = TestDb::new("secret");
        // seed a marker so we can verify contents after backup
        db.conn
            .execute("INSERT INTO companies(name) VALUES('C-marker')", [])
            .unwrap();
        let dst_dir = backup_dir(&db.app_data);
        fs::create_dir_all(&dst_dir).unwrap();
        let dst = dst_dir.join("test_backup.db");
        copy_encrypted_db(&db.conn, &db.db_path, &dst).unwrap();
        // backup file exists
        assert!(dst.exists());
        // reopen backup with same password and confirm marker
        let backup_conn = pool::open_encrypted(&dst, "secret").unwrap();
        let name: String = backup_conn
            .query_row(
                "SELECT name FROM companies WHERE name = 'C-marker'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "C-marker");
    }

    #[test]
    fn copy_encrypted_db_updates_last_backup_at() {
        let db = TestDb::new("s");
        let before = last_backup_at(&db.conn).unwrap();
        assert!(before.is_none());
        let dst = backup_dir(&db.app_data).join("t.db");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        copy_encrypted_db(&db.conn, &db.db_path, &dst).unwrap();
        let after = last_backup_at(&db.conn).unwrap();
        assert!(after.is_some());
    }

    #[test]
    fn wrong_password_backup_fails_to_open() {
        let db = TestDb::new("right");
        let dst = backup_dir(&db.app_data).join("t.db");
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        copy_encrypted_db(&db.conn, &db.db_path, &dst).unwrap();
        assert!(pool::open_encrypted(&dst, "wrong").is_err());
    }

    fn touch_backup_file(app_data: &Path, name: &str) {
        let dir = backup_dir(app_data);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), b"dummy").unwrap();
    }

    #[test]
    fn list_auto_backups_returns_descending_by_name() {
        let db = TestDb::new("s");
        touch_backup_file(&db.app_data, "auto_20260601_120000.db");
        touch_backup_file(&db.app_data, "auto_20260630_090000.db");
        touch_backup_file(&db.app_data, "auto_20260615_170000.db");
        touch_backup_file(&db.app_data, "manual_20260620.db"); // should be excluded
        let list = list_auto_backups(&db.app_data).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].file_name, "auto_20260630_090000.db");
        assert_eq!(list[1].file_name, "auto_20260615_170000.db");
        assert_eq!(list[2].file_name, "auto_20260601_120000.db");
    }

    #[test]
    fn list_auto_backups_empty_when_no_dir() {
        let db = TestDb::new("s");
        let list = list_auto_backups(&db.app_data).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn rotate_keeps_newest_seven() {
        let db = TestDb::new("s");
        for d in 1..=10 {
            touch_backup_file(&db.app_data, &format!("auto_202606{d:02}_120000.db"));
        }
        let deleted = rotate_auto_backups(&db.app_data, 7).unwrap();
        assert_eq!(deleted, 3);
        let list = list_auto_backups(&db.app_data).unwrap();
        assert_eq!(list.len(), 7);
        // newest survives
        assert!(list
            .iter()
            .any(|b| b.file_name == "auto_20260610_120000.db"));
        // oldest deleted
        assert!(!list
            .iter()
            .any(|b| b.file_name == "auto_20260601_120000.db"));
    }

    #[test]
    fn rotate_noop_when_under_limit() {
        let db = TestDb::new("s");
        touch_backup_file(&db.app_data, "auto_20260601_120000.db");
        let deleted = rotate_auto_backups(&db.app_data, 7).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn should_auto_backup_true_when_never_backed() {
        let db = TestDb::new("s");
        assert!(should_auto_backup(&db.conn, "2026-07-01 10:00:00").unwrap());
    }

    #[test]
    fn should_auto_backup_true_when_over_24h() {
        let db = TestDb::new("s");
        db.conn
            .execute(
                "INSERT INTO app_meta(key, value) VALUES('last_backup_at', '2026-06-30 09:00:00')",
                [],
            )
            .unwrap();
        assert!(should_auto_backup(&db.conn, "2026-07-01 10:00:00").unwrap());
    }

    #[test]
    fn should_auto_backup_false_when_under_24h() {
        let db = TestDb::new("s");
        db.conn
            .execute(
                "INSERT INTO app_meta(key, value) VALUES('last_backup_at', '2026-07-01 09:00:00')",
                [],
            )
            .unwrap();
        assert!(!should_auto_backup(&db.conn, "2026-07-01 10:00:00").unwrap());
    }

    #[test]
    fn export_plaintext_produces_unencrypted_readable_db() {
        let db = TestDb::new("secret");
        db.conn
            .execute("INSERT INTO companies(name) VALUES('C-plain')", [])
            .unwrap();
        let dst = db.app_data.join("exported.db");
        export_plaintext(&db.conn, &dst).unwrap();
        // open the exported file WITHOUT a key
        let plain = Connection::open(&dst).unwrap();
        let name: String = plain
            .query_row(
                "SELECT name FROM companies WHERE name = 'C-plain'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "C-plain");
    }
}
