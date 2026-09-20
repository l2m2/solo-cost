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
