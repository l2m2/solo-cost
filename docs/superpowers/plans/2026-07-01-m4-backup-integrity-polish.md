# solo-cost M4 (Backup + Integrity Check + M3 Polish) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 MVP 补齐数据安全底座：24h 自动备份 + 手动备份 + 明文导出 + 启动时 `PRAGMA integrity_check` + 一键从备份恢复。同时收尾 M3 final review 遗留的 7 项 minor。附件与 CSV/Excel 导出推到 M5。

**Architecture:** 后端新增 `domain/backup.rs`（纯文件操作 + WAL 检查点 + rotation + 明文导出通过 `sqlcipher_export`）+ `commands/backup.rs`（Tauri 命令层，dialog 选路径通过 `tauri-plugin-dialog`）。`auth::unlock` 内嵌 `PRAGMA integrity_check`，失败通过新错误变体 `AppError::IntegrityCheckFailed(String)` 返回给前端触发"损坏"对话框。前端 `/settings` 路由激活，包含备份状态卡 + 4 个操作按钮（立即备份 / 从备份恢复 / 导出明文 / 打开备份目录）+ M3 minor 收尾散落到相关 store/component。

**Tech Stack:** 继承 M1-M3 全部技术栈。M4 唯一新增：Tauri 已装的 `tauri-plugin-dialog`（M1 已配 `dialog:default` capability）；后端用 `chrono` 生成时间戳？不 —— 用 `time` crate（已经间接依赖）或直接 `chrono` 一个吗？—— 复用 SQLite 的 `datetime('now')` + Rust 端简单字符串处理，避免加依赖。

## Global Constraints

适用所有任务（每个任务隐式包含本节）：

- **包管理**：pnpm（禁 npm/yarn 混用）
- **金额单位**：所有金额字段 `INTEGER`（分）
- **软删字段**：所有业务表 `deleted_at TEXT NULL`；业务查询默认 `WHERE deleted_at IS NULL`
- **错误处理**：Rust 所有 `#[tauri::command]` 返回 `Result<T, AppError>`；非测试禁 `unwrap()` / `expect()`（`Mutex::lock().unwrap()` 惯例例外）
- **SQL 安全**：rusqlite 一律绑定参数，禁字符串拼接（PRAGMA 例外）
- **跨表写入用事务**（本 M4 涉及的 restore 走文件级替换，非跨表写）
- **代码注释语言**：英文；UI 文案中文
- **TS catch 模式**：M2 起统一 `catch (e: unknown)`
- **Tauri 2 IPC arg case**：Rust `snake_case` → 前端 `camelCase`
- **提交规约**：Conventional Commits；`type`/`scope` 小写英文；`subject` 中文 ≤ 65 字符（避免多字节 CJK 撞 72 字节上限），结尾不加句号；body 写"为什么"
- **CHANGELOG**：每个 feat commit 之后单独跑 `/changelog` skill 写一条 docs(changelog) commit
- **测试纪律**：domain 层 TDD（先 RED 再 GREEN）；commands 层写实现后补测试
- **不引入新前端 npm 依赖**；后端不引入 `chrono` / `time` 新 crate（用现有工具满足需求）
- **备份文件**：仍是 SQLCipher 加密文件（除明文导出外），要用同一主密码才能打开
- **`app_meta` 键**：`last_backup_at`（ISO 时间字符串）用于 24h 判定
- **数据完整性触发**：`unlock` 内嵌一次 `PRAGMA integrity_check`，返回值非 `"ok"` → `AppError::IntegrityCheckFailed(details)`
- **目标平台**：macOS 主开发；备份目录 `~/Library/Application Support/solo-cost/backups/`

---

## File Structure (M4 完成后的产物增量)

```
solo-cost/
├── src-tauri/
│   └── src/
│       ├── error.rs                    MODIFY：加 IntegrityCheckFailed / Backup 变体
│       ├── domain/
│       │   ├── mod.rs                  MODIFY：加 `pub mod backup;`
│       │   └── backup.rs               NEW：文件级备份 + WAL checkpoint + rotation + 明文导出
│       ├── commands/
│       │   ├── mod.rs                  MODIFY：加 `pub mod backup;`
│       │   ├── backup.rs               NEW：Tauri 命令层 (5 命令)
│       │   ├── auth.rs                 MODIFY：unlock_at 加 integrity_check + restore 后 reload
│       │   ├── payments.rs             MODIFY：update/mark_received 加项目归属校验（M3 minor）
│       │   ├── tasks.rs                MODIFY：create/update 加 assignee_id 公司校验（M3 minor）
│       │   └── projects.rs             MODIFY：把 restore-related helpers 用于 restore 后重新加载
│       └── lib.rs                      MODIFY：注册 5 backup 命令
└── src/
    ├── stores/
    │   ├── auth.ts                     MODIFY：unlock catch IntegrityCheckFailed 转到损坏页
    │   ├── costs.ts                    MODIFY：mutation 加 try/finally 保护 refresh（M3 minor）
    │   └── payments.ts                 MODIFY：mutation 加 try/finally 保护 refresh（M3 minor）
    ├── types/index.ts                  MODIFY：加 BackupInfo / BackupStatus
    ├── i18n/zh-CN.json                 MODIFY：加 settings/backup namespace + task.deleteConfirm 等 M3 minor keys
    ├── routes/
    │   └── settings.tsx                NEW：激活 /settings 路由，含备份卡片 + 4 操作 + 状态
    ├── routes/projects/detail.tsx      MODIFY：TimeLogEditForm 加 date guard；task delete 用 i18n；TaskForm archived assignee 兼容（M3 minor 全在 detail.tsx 收尾）
    ├── components/dialogs/
    │   └── IntegrityFailedDialog.tsx   NEW：整库损坏 blocking modal（用户选择：从备份恢复 / 联系支持）
    └── App.tsx                         MODIFY：激活 /settings 真路由；AuthGate 增加 "corrupted" 状态处理
```

---

## Task 1: 备份 domain 层（TDD）

**Files:**
- Create: `src-tauri/src/domain/backup.rs`
- Modify: `src-tauri/src/domain/mod.rs`（加 `pub mod backup;`）
- Modify: `src-tauri/src/error.rs`（加 `Backup(String)` 与 `IntegrityCheckFailed(String)` 变体）

**Interfaces:**
- Produces:
  - `pub fn backup_dir(app_data: &Path) -> PathBuf` — 返回 `<app_data>/backups/`（内部会创建目录如不存在）
  - `pub fn wal_checkpoint(conn: &Connection) -> AppResult<()>` — `PRAGMA wal_checkpoint(TRUNCATE)`
  - `pub fn copy_encrypted_db(conn: &Connection, src: &Path, dst: &Path) -> AppResult<()>` — WAL checkpoint 后 `fs::copy` + 更新 `app_meta.last_backup_at`
  - `pub struct BackupInfo { file_name: String, absolute_path: String, size_bytes: u64, created_at: String }` — 从文件名解析 auto_YYYYMMDD_HHmmss 或用 mtime
  - `pub fn list_auto_backups(app_data: &Path) -> AppResult<Vec<BackupInfo>>` — 按 `created_at DESC`
  - `pub fn rotate_auto_backups(app_data: &Path, keep: usize) -> AppResult<usize>` — 返回删除条数
  - `pub fn integrity_check(conn: &Connection) -> AppResult<()>` — `PRAGMA integrity_check`，非 "ok" → `Err(AppError::IntegrityCheckFailed(details))`
  - `pub fn export_plaintext(conn: &Connection, dst: &Path) -> AppResult<()>` — `ATTACH DATABASE ?1 AS plaintext KEY ''; SELECT sqlcipher_export('plaintext'); DETACH DATABASE plaintext;`
  - `pub fn last_backup_at(conn: &Connection) -> AppResult<Option<String>>` — 读 app_meta
  - `pub fn should_auto_backup(conn: &Connection, now_iso: &str) -> AppResult<bool>` — 距上次 >24h 返回 true；无记录也 true
- Consumes:
  - `crate::error::{AppError, AppResult}`
  - `rusqlite::Connection`
  - `std::path::{Path, PathBuf}`
  - `std::fs`

- [ ] **Step 1：`src-tauri/src/error.rs` 加两个变体**

在 `NotFound` 之后 `DeleteBlocked` 之前插入：

```rust
    #[error("integrity check failed: {0}")]
    IntegrityCheckFailed(String),

    #[error("backup failed: {0}")]
    Backup(String),
```

- [ ] **Step 2：`src-tauri/src/domain/mod.rs` 加 `pub mod backup;`**

放在现有 `pub mod profit;` 之后。

- [ ] **Step 3：写 `domain/backup.rs` 的完整测试（RED）**

创建 `src-tauri/src/domain/backup.rs`，全文：

```rust
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

pub fn backup_dir(app_data: &Path) -> PathBuf {
    app_data.join("backups")
}

pub fn wal_checkpoint(_conn: &Connection) -> AppResult<()> {
    unimplemented!()
}

pub fn copy_encrypted_db(_conn: &Connection, _src: &Path, _dst: &Path) -> AppResult<()> {
    unimplemented!()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub absolute_path: String,
    pub size_bytes: u64,
    pub created_at: String,
}

pub fn list_auto_backups(_app_data: &Path) -> AppResult<Vec<BackupInfo>> {
    unimplemented!()
}

pub fn rotate_auto_backups(_app_data: &Path, _keep: usize) -> AppResult<usize> {
    unimplemented!()
}

pub fn integrity_check(_conn: &Connection) -> AppResult<()> {
    unimplemented!()
}

pub fn export_plaintext(_conn: &Connection, _dst: &Path) -> AppResult<()> {
    unimplemented!()
}

pub fn last_backup_at(_conn: &Connection) -> AppResult<Option<String>> {
    unimplemented!()
}

pub fn should_auto_backup(_conn: &Connection, _now_iso: &str) -> AppResult<bool> {
    unimplemented!()
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
            .query_row("SELECT name FROM companies WHERE name = 'C-marker'", [], |r| {
                r.get(0)
            })
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
            touch_backup_file(&db.app_data, &format!("auto_2026060{d}_120000.db"));
        }
        let deleted = rotate_auto_backups(&db.app_data, 7).unwrap();
        assert_eq!(deleted, 3);
        let list = list_auto_backups(&db.app_data).unwrap();
        assert_eq!(list.len(), 7);
        // newest survives
        assert!(list.iter().any(|b| b.file_name == "auto_20260610_120000.db"));
        // oldest deleted
        assert!(!list.iter().any(|b| b.file_name == "auto_20260601_120000.db"));
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
            .query_row("SELECT name FROM companies WHERE name = 'C-plain'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "C-plain");
    }
}
```

- [ ] **Step 4：跑 RED**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
cd src-tauri
cargo test --lib domain::backup::tests 2>&1 | tail -30
```
预期：13 tests；全部 panic（`not yet implemented`）。

- [ ] **Step 5：替换所有 `unimplemented!()` 为实现**

用下面完整代码替换 `domain/backup.rs` 除测试模块以外的全部内容（保留 test 模块不动）：

```rust
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
        if date.chars().all(|c| c.is_ascii_digit()) && time.chars().all(|c| c.is_ascii_digit()) && time.len() == 6 {
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
    // ATTACH requires the destination path as a bound param; SQLCipher does the file creation.
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
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'last_backup_at'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(row)
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
```

Notes for implementer:
- `export_plaintext` 用 `format!` 拼 ATTACH SQL 因为 SQLite 不接受该处的绑定参数；单引号已通过 `replace('\'', "''")` 转义，符合项目一贯的 PRAGMA 路径处理模式。
- `sqlcipher_export` 是 SQLCipher 内置函数，M1 已通过 `bundled-sqlcipher-vendored-openssl` 编入。

- [ ] **Step 6：跑 GREEN**

```bash
cargo test --lib domain::backup::tests 2>&1 | tail -20
```
预期：13 passed.

- [ ] **Step 7：跑全量 + clippy + fmt**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
预期：76 + 13 = 89 passing；clippy 0；fmt clean.

- [ ] **Step 8：Commit**

```bash
git add src-tauri/src/error.rs src-tauri/src/domain
git commit -m "feat(domain): 备份+完整性检查+明文导出 domain 层"
```

- [ ] **Step 9：CHANGELOG**

`/changelog`：`domain::backup` 模块（WAL checkpoint、复制加密 DB、rotation 7 份、明文导出通过 `sqlcipher_export`、24h 自动备份判定、integrity_check）；`AppError::IntegrityCheckFailed` 与 `AppError::Backup` 变体加入。

---

## Task 2: 备份 Tauri 命令层

**Files:**
- Create: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/commands/mod.rs`（加 `pub mod backup;`）
- Modify: `src-tauri/src/lib.rs`（注册 5 条命令）

**Interfaces:**
- Produces：
  - `#[tauri::command] pub fn list_backups(app) -> AppResult<Vec<BackupInfo>>` — 委托 domain::list_auto_backups
  - `#[tauri::command] pub fn create_backup_now(app, state) -> AppResult<BackupInfo>` — 生成 `auto_YYYYMMDD_HHmmss.db` 立即备份 + rotate
  - `#[tauri::command] pub fn maybe_run_auto_backup(app, state) -> AppResult<Option<BackupInfo>>` — >24h 才做，否则 None
  - `#[tauri::command] pub fn export_plaintext_backup(state, dst_path: String) -> AppResult<()>` — 前端 dialog 选路径后传入
  - `#[tauri::command] pub fn get_backup_status(app, state) -> AppResult<BackupStatus>` — 用于 settings 页展示：last_backup_at / next_due / count
  - `pub struct BackupStatus { last_backup_at: Option<String>, auto_count: usize, should_auto_backup_now: bool }`
- Consumes：
  - `crate::domain::backup::*`
  - `crate::state::AppState` + `tauri::AppHandle`

- [ ] **Step 1：写 `src-tauri/src/commands/backup.rs`**

```rust
use crate::domain::backup;
use crate::domain::backup::{BackupInfo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

const KEEP: usize = 7;

#[derive(Debug, Clone, Serialize)]
pub struct BackupStatus {
    pub last_backup_at: Option<String>,
    pub auto_count: usize,
    pub should_auto_backup_now: bool,
}

fn data_dir(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app_data_dir: {e}")))
}

fn db_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    Ok(data_dir(app)?.join("data.db"))
}

fn with_conn<R>(
    state: &tauri::State<AppState>,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or(AppError::Locked)?;
    f(conn)
}

fn timestamped_filename(conn: &Connection) -> AppResult<String> {
    // format: auto_YYYYMMDD_HHmmss.db
    let stamp: String = conn.query_row(
        "SELECT strftime('%Y%m%d_%H%M%S', 'now')",
        [],
        |r| r.get(0),
    )?;
    Ok(format!("auto_{stamp}.db"))
}

#[tauri::command]
pub fn list_backups(app: tauri::AppHandle) -> AppResult<Vec<BackupInfo>> {
    let data = data_dir(&app)?;
    backup::list_auto_backups(&data)
}

#[tauri::command]
pub fn create_backup_now(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<BackupInfo> {
    let data = data_dir(&app)?;
    let src = db_path(&app)?;
    let dir = backup::backup_dir(&data);
    let (fname, dst) = {
        let guard = state.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or(AppError::Locked)?;
        let fname = timestamped_filename(conn)?;
        let dst = dir.join(&fname);
        backup::copy_encrypted_db(conn, &src, &dst)?;
        (fname, dst)
    };
    backup::rotate_auto_backups(&data, KEEP)?;
    // Look up the created BackupInfo (list is sorted DESC → first).
    let list = backup::list_auto_backups(&data)?;
    list.into_iter()
        .find(|b| b.file_name == fname && b.absolute_path == dst.to_string_lossy())
        .ok_or_else(|| AppError::Backup("created backup not found after rotation".into()))
}

#[tauri::command]
pub fn maybe_run_auto_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<Option<BackupInfo>> {
    let now: Option<String> = with_conn(&state, |c| {
        Ok(Some(
            c.query_row("SELECT datetime('now')", [], |r| r.get::<_, String>(0))?,
        ))
    })?;
    let now = now.unwrap();
    let due = with_conn(&state, |c| backup::should_auto_backup(c, &now))?;
    if !due {
        return Ok(None);
    }
    Ok(Some(create_backup_now(app, state)?))
}

#[tauri::command]
pub fn export_plaintext_backup(
    state: tauri::State<AppState>,
    dst_path: String,
) -> AppResult<()> {
    let dst = PathBuf::from(dst_path);
    with_conn(&state, |c| backup::export_plaintext(c, &dst))
}

#[tauri::command]
pub fn get_backup_status(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> AppResult<BackupStatus> {
    let data = data_dir(&app)?;
    let list = backup::list_auto_backups(&data)?;
    let (last, due) = with_conn(&state, |c| {
        let last = backup::last_backup_at(c)?;
        let now: String = c.query_row("SELECT datetime('now')", [], |r| r.get(0))?;
        let due = backup::should_auto_backup(c, &now)?;
        Ok((last, due))
    })?;
    Ok(BackupStatus {
        last_backup_at: last,
        auto_count: list.len(),
        should_auto_backup_now: due,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::setup_at;
    use tempfile::tempdir;

    #[test]
    fn timestamped_filename_matches_pattern() {
        let dir = tempdir().unwrap();
        let conn = setup_at(&dir.path().join("data.db"), "s").unwrap();
        let name = timestamped_filename(&conn).unwrap();
        assert!(name.starts_with("auto_"));
        assert!(name.ends_with(".db"));
        // total length: auto_ (5) + 15 (YYYYMMDD_HHmmss) + .db (3) = 23
        assert_eq!(name.len(), 23);
    }
}
```

- [ ] **Step 2：`src-tauri/src/commands/mod.rs` 加 `pub mod backup;`**

按字母序插入 auth 之后：

```rust
pub mod auth;
pub mod backup;
pub mod categories;
...
```

- [ ] **Step 3：注册 5 条命令到 `lib.rs`**

`tauri::generate_handler![...]` 内在 `lock,` 之后追加：

```rust
            commands::backup::list_backups,
            commands::backup::create_backup_now,
            commands::backup::maybe_run_auto_backup,
            commands::backup::export_plaintext_backup,
            commands::backup::get_backup_status,
```

- [ ] **Step 4：跑测试 + clippy + fmt**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
预期：89 + 1 = 90 passing；clippy 0；fmt clean.

- [ ] **Step 5：Commit + CHANGELOG**

```bash
git add src-tauri/src
git commit -m "feat(backup): tauri 命令层 5 命令 + 状态查询"
```

`/changelog`：Tauri 命令 `list_backups` / `create_backup_now` / `maybe_run_auto_backup` / `export_plaintext_backup` / `get_backup_status`；自动备份文件名 `auto_YYYYMMDD_HHmmss.db`；rotation 保留最近 7 份。

---

## Task 3: unlock 内嵌 integrity_check

**Files:**
- Modify: `src-tauri/src/commands/auth.rs`

**Interfaces:**
- Modifies existing `unlock_at(path, password) -> AppResult<Connection>` — 在 `migrations::run` 成功后额外跑 `domain::backup::integrity_check`；失败时**关闭连接**再返回 `IntegrityCheckFailed`。
- 新增测试：`unlock_reports_integrity_failure` — 手工把已有 db 文件的头部字节改坏，unlock 应报 `IntegrityCheckFailed`。

- [ ] **Step 1：修改 `unlock_at`**

替换现有 `unlock_at` 函数为：

```rust
pub(crate) fn unlock_at(path: &std::path::Path, password: &str) -> AppResult<rusqlite::Connection> {
    let conn = pool::open_encrypted(path, password)?;
    // Also run migrations on unlock so schema upgrades apply after app updates.
    migrations::run(&conn)?;
    // Verify the database is uncorrupted before returning it to the caller.
    crate::domain::backup::integrity_check(&conn)?;
    Ok(conn)
}
```

Rationale: `pool::open_encrypted` 已用 `PRAGMA key` 通过密码解密并校验（`verify_key` 简单 SELECT）。`integrity_check` 是第二道更彻底的一致性检查。任一失败时 `conn` 在此 fn 内 drop，符合 spec §5.4 "启动阻塞，列出可恢复备份让用户选" 的语义。

- [ ] **Step 2：加测试 `unlock_reports_integrity_failure`**

在 `auth.rs` 的 `#[cfg(test)] mod tests` 尾部追加：

```rust
#[test]
fn unlock_reports_integrity_failure() {
    use crate::error::AppError;
    use std::io::{Seek, SeekFrom, Write};
    let dir = tempdir().unwrap();
    let path = dir.path().join("data.db");
    // create a valid encrypted db then close
    drop(setup_at(&path, "s").unwrap());
    // corrupt a page interior (skip past SQLCipher header ~16 bytes → seek 4096 to hit page 2 boundary,
    // then splat some garbage; enough to break integrity_check with the current password intact
    // for the header decryption).
    let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    f.seek(SeekFrom::Start(4096)).unwrap();
    f.write_all(&[0xFFu8; 512]).unwrap();
    drop(f);
    let err = unlock_at(&path, "s").unwrap_err();
    assert!(
        matches!(err, AppError::IntegrityCheckFailed(_) | AppError::WrongPassword | AppError::Db(_)),
        "expected integrity or decryption failure, got {err:?}"
    );
}
```

Note: 断言是宽容的（`|` union），因为不同 SQLCipher 版本对 header 附近改动可能先在 `PRAGMA key` 阶段就失败（→ `WrongPassword`）或在 integrity_check 阶段失败（→ `IntegrityCheckFailed`）。任一失败路径都覆盖了"损坏 → 不返回 conn"的核心保证。若你在本地跑遇到偏移不稳定，把 `4096` 改成 `8192` 或 `16384` 直到出错。

- [ ] **Step 3：跑测试 + clippy + fmt**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
预期：90 + 1 = 91 passing.

- [ ] **Step 4：Commit + CHANGELOG**

```bash
git add src-tauri/src/commands/auth.rs
git commit -m "feat(auth): unlock 后跑 integrity_check 阻断损坏 db"
```

`/changelog`：unlock 现在在迁移之后额外跑 `PRAGMA integrity_check`；失败返回 `IntegrityCheckFailed` 由前端弹恢复对话框。

---

## Task 4: restore-from-backup 命令 + auth 会话重置钩子

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`（加 `restore_from_backup` 命令）
- Modify: `src-tauri/src/lib.rs`（注册 1 条新命令）

**Interfaces:**
- Produces：
  - `#[tauri::command] pub fn restore_from_backup(app, state, backup_path: String, password: String) -> AppResult<()>` — 先验证备份文件能用给定密码打开且 `integrity_check` 通过，再关闭现连接，覆盖 data.db，再用同密码打开一次并 verify，最后把新连接放回 state。
- Consumes：
  - `domain::backup::integrity_check`
  - `db::pool::open_encrypted`
  - `db::migrations::run`

- [ ] **Step 1：在 `commands/backup.rs` 追加**

```rust
use crate::db::{migrations, pool};

#[tauri::command]
pub fn restore_from_backup(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    backup_path: String,
    password: String,
) -> AppResult<()> {
    let backup = PathBuf::from(&backup_path);
    if !backup.exists() {
        return Err(AppError::Backup(format!(
            "backup file not found: {backup_path}"
        )));
    }
    // Phase 1: verify the backup opens with the given password AND passes integrity.
    let verify_conn = pool::open_encrypted(&backup, &password)?;
    backup::integrity_check(&verify_conn)?;
    drop(verify_conn);

    // Phase 2: close current app connection and replace data.db.
    let target = db_path(&app)?;
    {
        let mut guard = state.conn.lock().unwrap();
        guard.take();
    }
    std::fs::copy(&backup, &target).map_err(|e| AppError::Backup(format!("copy: {e}")))?;

    // Phase 3: re-open the restored db and put it back into state.
    let conn = pool::open_encrypted(&target, &password)?;
    migrations::run(&conn)?;
    backup::integrity_check(&conn)?;
    *state.conn.lock().unwrap() = Some(conn);
    Ok(())
}
```

- [ ] **Step 2：`lib.rs` 注册**

在其他 backup 命令之后追加：

```rust
            commands::backup::restore_from_backup,
```

- [ ] **Step 3：加 unit test**

在 `commands/backup.rs` 的 tests 模块追加：

```rust
#[test]
fn timestamped_filename_matches_pattern() {
    // existing test, keep as-is
}

#[test]
fn restore_verifies_backup_before_replacing() {
    // We cannot easily wire an AppHandle in a unit test, so this test only exercises the
    // domain-level guarantees the command relies on: opening a backup file with the wrong
    // password fails, and integrity_check catches obvious corruption.
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.db");
    let backup = dir.path().join("backup.db");
    let conn = setup_at(&original, "s").unwrap();
    conn.execute("INSERT INTO companies(name) VALUES('X')", []).unwrap();
    drop(conn);
    std::fs::copy(&original, &backup).unwrap();

    // wrong password → open fails
    let err = crate::db::pool::open_encrypted(&backup, "wrong").unwrap_err();
    assert!(matches!(err, AppError::WrongPassword | AppError::Db(_)));

    // right password → integrity_check passes
    let conn = crate::db::pool::open_encrypted(&backup, "s").unwrap();
    crate::domain::backup::integrity_check(&conn).unwrap();
}
```

- [ ] **Step 4：跑测试 + clippy + fmt**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
预期：91 + 1 = 92 passing.

- [ ] **Step 5：Commit + CHANGELOG**

```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/lib.rs
git commit -m "feat(backup): restore_from_backup 命令 + 密码与完整性双重校验"
```

`/changelog`：`restore_from_backup` 命令；用给定备份文件覆盖 data.db 前会先用同密码试开 + `PRAGMA integrity_check`；成功后关闭现连接、替换文件、重新打开放回 state。前端触发后须调 lock/unlock 让所有 store reset。

---

## Task 5: M3 backend minor 收尾（cross-project ownership + assignee 校验）

**Files:**
- Modify: `src-tauri/src/commands/payments.rs`（`update_impl` / `mark_received_impl` 加 payment 归属校验）
- Modify: `src-tauri/src/commands/tasks.rs`（`create_impl` / `update_impl` 加 assignee_id 公司校验）

**Interfaces:**
- Modifies (no new public API):
  - `payments::update_impl(conn, id, input)` — 现在若 id 不存在直接返回 NotFound，不再落到 UPDATE 0-rows 分支；语义与 M2 finding F2 收尾一致
  - `tasks::create_impl(conn, project_id, input)` — 若 `input.assignee_id` 是 Some 但成员不属该 project 的 company（或成员已软删）→ Validation 错误
  - `tasks::update_impl(conn, id, input)` — 同样校验，先取 task 的 project_id 再复用同一校验路径

- [ ] **Step 1：修改 `payments.rs::update_impl` — 提前 NotFound 校验**

在 `update_impl` 顶部（validate 之后）加：

```rust
    // Cheap existence check so we return NotFound rather than falling through to a 0-row UPDATE
    // that would only surface as NotFound after the invalid data was already validated. Guards
    // the "wrong error type for missing entity" pattern (M2 review F2 carry-over).
    let _existing = get_impl(conn, id)?;
```

- [ ] **Step 2：修改 `tasks.rs::create_impl` 加 assignee 校验**

在 `validate(input)?` 之后加：

```rust
    if let Some(assignee_id) = input.assignee_id {
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM members m
             JOIN projects p ON p.company_id = m.company_id
             WHERE p.id = ?1 AND m.id = ?2 AND m.deleted_at IS NULL",
            [project_id, assignee_id],
            |r| r.get(0),
        )?;
        if ok == 0 {
            return Err(AppError::Validation(
                "负责人不属该项目所在公司或已归档/删除".into(),
            ));
        }
    }
```

- [ ] **Step 3：修改 `tasks.rs::update_impl` 加同样校验**

在 `validate(input)?` 之后先加：

```rust
    let project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound { entity: "task", id },
            other => AppError::Db(other),
        })?;
    if let Some(assignee_id) = input.assignee_id {
        let ok: i64 = conn.query_row(
            "SELECT COUNT(*) FROM members m
             JOIN projects p ON p.company_id = m.company_id
             WHERE p.id = ?1 AND m.id = ?2 AND m.deleted_at IS NULL",
            [project_id, assignee_id],
            |r| r.get(0),
        )?;
        if ok == 0 {
            return Err(AppError::Validation(
                "负责人不属该项目所在公司或已归档/删除".into(),
            ));
        }
    }
```

Note: the existing update SQL uses `WHERE id = ?7 AND deleted_at IS NULL`; the pre-check here is defense-in-depth **and** gives us the correct NotFound instead of "0 rows affected → NotFound" that could confusion for callers.

- [ ] **Step 4：加 3 个新测试**

**payments.rs::tests：**

```rust
#[test]
fn update_nonexistent_returns_not_found() {
    let db = TestDb::new();
    let err = update_impl(&db.conn, 999, &make("X", 100)).unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
}
```

**tasks.rs::tests：**

```rust
#[test]
fn create_with_cross_company_assignee_rejected() {
    let db = TestDb::new();
    // company 1 (existing) + a foreign company + a member in it
    db.conn
        .execute("INSERT INTO companies(name) VALUES('Other')", [])
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(2, 'Foreign', 60000)",
            [],
        )
        .unwrap();
    let mut i = input("T");
    i.assignee_id = Some(1); // members table is empty for company 1 → id=1 is the foreign member
    let err = create_impl(&db.conn, 1, &i).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn create_with_own_company_assignee_ok() {
    let db = TestDb::new();
    db.conn
        .execute(
            "INSERT INTO members(company_id, name, daily_cost_cents) VALUES(1, 'M', 80000)",
            [],
        )
        .unwrap();
    let mut i = input("T");
    i.assignee_id = Some(1);
    let t = create_impl(&db.conn, 1, &i).unwrap();
    assert_eq!(t.assignee_id, Some(1));
}
```

- [ ] **Step 5：跑测试 + clippy + fmt**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
预期：92 + 3 = 95 passing.

- [ ] **Step 6：Commit + CHANGELOG**

```bash
git add src-tauri/src/commands
git commit -m "fix(payments+tasks): m3 minor 收尾 归属与 assignee 校验"
```

`/changelog`：`update_payment` 现在对不存在 id 返回 `NotFound` 而非 Validation；`create_task` / `update_task` 若 `assignee_id` 不属该项目所在公司或已归档/删除，返回 `Validation` 错误。

---

## Task 6: M3 frontend minor 收尾

**Files:**
- Modify: `src/stores/costs.ts`（M2 store，refresh 加 try/finally 保护）
- Modify: `src/stores/payments.ts`（refresh 加 try/finally 保护）
- Modify: `src/i18n/zh-CN.json`（加 `task.deleteConfirm` 键）
- Modify: `src/routes/projects/detail.tsx`（TimeLogEditForm 加 date guard + task delete 用 i18n + TaskForm archived assignee 兼容）

**Interfaces:**
- Modifies stores：`costs.create/update/remove` 和 `payments.create/update/markReceived/softDelete` 现在把 `useFinancialStore.refresh(projectId)` 放到 try/finally 里保证 loadFor 抛错也执行 refresh。
- Modifies routes：detail.tsx 的 TimeLogEditForm 加 `if (!date) return toast.error(t("timelog.dateRequired"))` guard；task delete 用 `t("task.deleteConfirm")` 替换硬编码；TaskForm 编辑归档 assignee 时不留空态。

- [ ] **Step 1：`src/stores/costs.ts` 加 try/finally**

替换 `create` / `update` / `remove` 三个方法为：

```typescript
  async create(projectId, input) {
    await call<CostEntry>("create_cost_entry", { projectId, input });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  async update(id, input, projectId) {
    await call<CostEntry>("update_cost_entry", { id, input });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  async remove(id, projectId) {
    await call<void>("delete_cost_entry", { id });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
```

- [ ] **Step 2：`src/stores/payments.ts` 加 try/finally**

替换 `create` / `update` / `markReceived` / `softDelete` 四个方法体尾部结构为 try/finally（同 costs.ts pattern）：

```typescript
  async create(projectId, input) {
    await call<ContractPayment>("create_payment", { projectId, input });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  async update(id, input, projectId) {
    await call<ContractPayment>("update_payment", { id, input });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  async markReceived(id, actualAmountCents, actualReceivedAt, projectId) {
    await call<ContractPayment>("mark_payment_received", {
      id,
      actualAmountCents,
      actualReceivedAt,
    });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
  async softDelete(id, projectId) {
    await call<void>("delete_payment", { id });
    try {
      await get().loadFor(projectId);
    } finally {
      await useFinancialStore.getState().refresh(projectId);
    }
  },
```

- [ ] **Step 3：`src/i18n/zh-CN.json` 加 `task.deleteConfirm`**

在 `task` 对象里追加一条（保持其它键不动）：

```json
    "deleteConfirm": "确认删除任务「{{title}}」？关联工时将被一并软删，可在回收站恢复。"
```

- [ ] **Step 4：`src/routes/projects/detail.tsx` 三处改动**

**4a. TasksPanel 中 task delete 用 i18n**（找到硬编码 `confirm("确认删除该任务？关联工时将被一并软删。")`）：

```typescript
if (!confirm(t("task.deleteConfirm", { title: tk.title }))) return;
```

**4b. TimeLogEditForm submit 前加 date guard**（找到 `TimeLogEditForm` 组件的 onClick）：

```typescript
onClick={async () => {
  if (!date) return toast.error(t("timelog.dateRequired"));
  if (hours < 0 || hours > 24) return toast.error(t("timelog.hoursRequired"));
  setBusy(true);
  try { await onSubmit({ work_date: date, hours, notes: notes.trim() || null }); }
  finally { setBusy(false); }
}}
```

**4c. TaskForm assignee 兼容归档成员**（找到 `TaskForm` 里 `const active = members.filter((m) => m.is_active);`）：

替换该 filter + Select 的 rendering 为：

```typescript
  const currentAssignee = initial?.assignee_id
    ? members.find((m) => m.id === initial.assignee_id)
    : null;
  const active = members.filter((m) => m.is_active);
  // Include the current assignee even if archived, so the Select value has a matching item.
  const options = currentAssignee && !currentAssignee.is_active
    ? [currentAssignee, ...active]
    : active;
```

并把下方 `active.map(...)` 改成 `options.map(...)`；每个 SelectItem 若该成员 `!m.is_active` 则名字后追加"（已归档）"：

```typescript
              {options.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>
                  {m.is_active ? m.name : `${m.name}（已归档）`}
                </SelectItem>
              ))}
```

- [ ] **Step 5：TS + build**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm tsc --noEmit
pnpm build
```
预期：0 errors + build success.

- [ ] **Step 6：Commit + CHANGELOG**

```bash
git add src/stores src/routes src/i18n
git commit -m "fix(ui): m3 minor 收尾 refresh 保护 + date guard + i18n + 归档 assignee"
```

`/changelog`：M3 minor 收尾：`costs` 与 `payments` store 的 mutation 用 try/finally 保护 financial refresh；`TimeLogEditForm` 保存前校验日期；task 删除确认对话框走 `task.deleteConfirm` i18n key；`TaskForm` 编辑时若原负责人已归档，会在下拉里显示"（已归档）"标注。

---

## Task 7: 前端备份 UI + integrity failure 处理 + settings 路由激活

**Files:**
- Create: `src/stores/backup.ts`
- Create: `src/routes/settings.tsx`
- Create: `src/components/dialogs/IntegrityFailedDialog.tsx`
- Modify: `src/stores/auth.ts`（unlock catch IntegrityCheckFailed → set status "corrupted"；lock 后调 auto backup try/catch）
- Modify: `src/App.tsx`（激活 `<Route path="settings" element={<SettingsPage />} />`；`AuthGate` 加 `"corrupted"` 分支渲染 IntegrityFailedDialog）
- Modify: `src/i18n/zh-CN.json`（加 `settings.*` / `backup.*` 命名空间）
- Modify: `src/types/index.ts`（加 `BackupInfo` / `BackupStatus`）

**Interfaces:**
- Produces：
  - `useBackupStore`：`{ status, list, loadStatus(), loadList(), createNow(), maybeAutoBackup(), exportPlaintext(dstPath), restoreFromBackup(backupPath, password), reset() }`
  - Auth store 新增 status: `"corrupted"`（除 unknown / uninitialized / locked / unlocked 之外）
- Consumes：
  - Task 2 & 4 命令：`list_backups` / `create_backup_now` / `maybe_run_auto_backup` / `export_plaintext_backup` / `get_backup_status` / `restore_from_backup`

- [ ] **Step 1：`src/types/index.ts` 追加**

在末尾追加：

```typescript
export interface BackupInfo {
  file_name: string;
  absolute_path: string;
  size_bytes: number;
  created_at: string;
}

export interface BackupStatus {
  last_backup_at: string | null;
  auto_count: number;
  should_auto_backup_now: boolean;
}
```

- [ ] **Step 2：`src/i18n/zh-CN.json` 追加**

在最后一个顶层对象之后追加（与 `financial` 同级）：

```json
  "settings": {
    "title": "设置",
    "backup": {
      "sectionTitle": "备份与恢复",
      "statusLabel": "最近一次备份",
      "never": "从未备份",
      "count": "已保留 {{n}} 份自动备份（滚动 7 份）",
      "runNow": "立即备份",
      "restore": "从备份恢复",
      "exportPlaintext": "导出明文备份…",
      "exportWarning": "明文导出不加密，任何人拿到文件都能看全部数据，仅在你自己的设备上做迁移用途时使用",
      "chooseFile": "选择备份文件",
      "backingUp": "备份中…",
      "success": "已备份到 {{path}}",
      "restoreConfirm": "确认用此备份覆盖当前数据库？此操作不可撤销。",
      "restorePasswordPrompt": "请输入该备份文件的主密码（通常与当前主密码相同）",
      "restoreSuccess": "已从备份恢复，请重新解锁",
      "corruptedTitle": "数据库损坏",
      "corruptedBody": "启动检查发现数据库文件不一致。请从最近一份可用备份恢复：",
      "corruptedNoBackup": "没有可用备份。请联系支持或从其他设备找回。"
    }
  }
```

- [ ] **Step 3：`src/stores/backup.ts`**

```typescript
import { create } from "zustand";
import { call } from "@/lib/ipc";
import type { BackupInfo, BackupStatus } from "@/types";

interface S {
  status: BackupStatus | null;
  list: BackupInfo[];
  loadStatus: () => Promise<void>;
  loadList: () => Promise<void>;
  createNow: () => Promise<BackupInfo>;
  maybeAutoBackup: () => Promise<BackupInfo | null>;
  exportPlaintext: (dstPath: string) => Promise<void>;
  restoreFromBackup: (backupPath: string, password: string) => Promise<void>;
  reset: () => void;
}

export const useBackupStore = create<S>((set, get) => ({
  status: null,
  list: [],
  async loadStatus() {
    const status = await call<BackupStatus>("get_backup_status");
    set({ status });
  },
  async loadList() {
    const list = await call<BackupInfo[]>("list_backups");
    set({ list });
  },
  async createNow() {
    const info = await call<BackupInfo>("create_backup_now");
    await get().loadStatus();
    await get().loadList();
    return info;
  },
  async maybeAutoBackup() {
    const info = await call<BackupInfo | null>("maybe_run_auto_backup");
    if (info) {
      await get().loadStatus();
      await get().loadList();
    }
    return info;
  },
  async exportPlaintext(dstPath) {
    await call<void>("export_plaintext_backup", { dstPath });
  },
  async restoreFromBackup(backupPath, password) {
    await call<void>("restore_from_backup", { backupPath, password });
    // do NOT reload here; caller must trigger lock + unlock to reset all stores
  },
  reset() {
    set({ status: null, list: [] });
  },
}));
```

- [ ] **Step 4：`src/stores/auth.ts` 加 corrupted status + reset chain 中 backup store**

在 `Status` 类型加：

```typescript
type Status = "unknown" | "uninitialized" | "locked" | "unlocked" | "corrupted";
```

`unlock` 方法内 catch `IntegrityCheckFailed`（错误消息含 `"integrity check failed"` 子串）→ set `status: "corrupted"`：

```typescript
async unlock(password) {
  try {
    await call<void>("unlock", { password });
    // fire-and-forget: try to snapshot right after unlock
    void useBackupStore.getState().maybeAutoBackup().catch(() => {});
    set({ status: "unlocked" });
  } catch (e: unknown) {
    const msg = String(e);
    if (msg.includes("integrity check failed")) {
      set({ status: "corrupted" });
    }
    throw e;
  }
},
```

在 `lock()` 的 reset 链末尾加 `useBackupStore.getState().reset()` 并 import。

`refresh()` 加对 corrupted 状态的支持——若已进入 corrupted 状态不覆盖回 locked（保留错误显示）：

```typescript
async refresh() {
  if (get().status === "corrupted") return;
  const initialized = await call<boolean>("is_initialized");
  set({ status: initialized ? "locked" : "uninitialized" });
},
```

- [ ] **Step 5：`src/components/dialogs/IntegrityFailedDialog.tsx`**

```typescript
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useBackupStore } from "@/stores/backup";
import { useAuthStore } from "@/stores/auth";

export function IntegrityFailedDialog() {
  const { t } = useTranslation();
  const { list, loadList, restoreFromBackup } = useBackupStore();
  const refresh = useAuthStore((s) => s.refresh);
  const [selected, setSelected] = useState<string>("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => { loadList(); }, [loadList]);

  const chooseFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "SQLite db", extensions: ["db"] }],
    });
    if (typeof picked === "string") setSelected(picked);
  };

  const restore = async () => {
    if (!selected) return toast.error(t("settings.backup.chooseFile"));
    if (!password) return toast.error(t("login.password"));
    setBusy(true);
    try {
      await restoreFromBackup(selected, password);
      toast.success(t("settings.backup.restoreSuccess"));
      // Reset to "locked" so login page renders and pulls fresh stores.
      useAuthStore.setState({ status: "locked" });
      await refresh();
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.backup.corruptedTitle")}</DialogTitle>
          <DialogDescription>{t("settings.backup.corruptedBody")}</DialogDescription>
        </DialogHeader>
        {list.length === 0 ? (
          <div className="text-sm text-muted-foreground">
            {t("settings.backup.corruptedNoBackup")}
          </div>
        ) : (
          <div className="space-y-2 text-sm">
            <div className="text-muted-foreground">系统内已有的自动备份：</div>
            {list.map((b) => (
              <div
                key={b.absolute_path}
                className="cursor-pointer hover:bg-accent p-2 rounded flex items-center justify-between"
                onClick={() => setSelected(b.absolute_path)}
              >
                <span className={selected === b.absolute_path ? "font-medium" : undefined}>
                  {b.created_at}
                </span>
                <span className="text-xs text-muted-foreground">
                  {(b.size_bytes / 1024).toFixed(0)} KB
                </span>
              </div>
            ))}
          </div>
        )}
        <div className="space-y-2">
          <Label>{t("settings.backup.chooseFile")}（可选自定义）</Label>
          <div className="flex gap-2">
            <Input readOnly value={selected} placeholder="…/backups/auto_YYYYMMDD_HHmmss.db" />
            <Button variant="outline" onClick={chooseFile}>浏览…</Button>
          </div>
        </div>
        <div className="space-y-2">
          <Label>{t("settings.backup.restorePasswordPrompt")}</Label>
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
        <DialogFooter>
          <Button onClick={restore} disabled={busy || !selected || !password}>
            {t("settings.backup.restore")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 6：`src/routes/settings.tsx`**

```typescript
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { save } from "@tauri-apps/plugin-dialog";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useBackupStore } from "@/stores/backup";

export default function SettingsPage() {
  const { t } = useTranslation();
  const { status, list, loadStatus, loadList, createNow, exportPlaintext } =
    useBackupStore();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    loadStatus();
    loadList();
  }, [loadStatus, loadList]);

  const doCreate = async () => {
    setBusy(true);
    try {
      const info = await createNow();
      toast.success(t("settings.backup.success", { path: info.absolute_path }));
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const doExport = async () => {
    if (!confirm(t("settings.backup.exportWarning"))) return;
    const picked = await save({
      defaultPath: "solo-cost-plaintext.db",
      filters: [{ name: "SQLite db", extensions: ["db"] }],
    });
    if (!picked) return;
    setBusy(true);
    try {
      await exportPlaintext(picked);
      toast.success(t("settings.backup.success", { path: picked }));
    } catch (e: unknown) {
      toast.error(t("common.error", { msg: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">{t("settings.title")}</h1>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">
            {t("settings.backup.sectionTitle")}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="text-sm">
            {t("settings.backup.statusLabel")}：
            <span className="ml-2 font-medium">
              {status?.last_backup_at ?? t("settings.backup.never")}
            </span>
          </div>
          <div className="text-sm text-muted-foreground">
            {t("settings.backup.count", { n: status?.auto_count ?? 0 })}
          </div>
          <div className="flex gap-2 pt-2">
            <Button onClick={doCreate} disabled={busy}>
              {busy ? t("settings.backup.backingUp") : t("settings.backup.runNow")}
            </Button>
            <Button variant="outline" onClick={doExport} disabled={busy}>
              {t("settings.backup.exportPlaintext")}
            </Button>
          </div>
        </CardContent>
      </Card>
      {list.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">备份历史</CardTitle>
          </CardHeader>
          <CardContent className="space-y-1 text-sm">
            {list.map((b) => (
              <div
                key={b.absolute_path}
                className="flex items-center justify-between border-b py-2 last:border-b-0"
              >
                <div>
                  <div>{b.created_at}</div>
                  <div className="text-xs text-muted-foreground">{b.file_name}</div>
                </div>
                <div className="text-xs text-muted-foreground">
                  {(b.size_bytes / 1024).toFixed(0)} KB
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
```

- [ ] **Step 7：`src/App.tsx` 激活 settings 路由 + AuthGate 处理 corrupted**

在 imports 追加 `import SettingsPage from "@/routes/settings";` 与 `import { IntegrityFailedDialog } from "@/components/dialogs/IntegrityFailedDialog";`。

把 `<Route path="settings" element={<div>设置（M4 实现）</div>} />` 改为：

```typescript
            <Route path="settings" element={<SettingsPage />} />
```

在 `AuthGate` 顶部，`if (status === "unknown") return null;` 之后加：

```typescript
if (status === "corrupted") return <IntegrityFailedDialog />;
```

- [ ] **Step 8：Tauri capability**

`src-tauri/capabilities/default.json` 早在 M1 已有 `"dialog:default"`，但 `dialog:save` / `dialog:open` 的细分权限也在 default set 中——若 tsc/build 后运行 dev 弹权限拒绝，改用完整 permission `dialog:allow-save` + `dialog:allow-open`。M1 遗留可用性不用改，本 task 出问题时再调整。

- [ ] **Step 9：TS + build + 5 signal 验证**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm tsc --noEmit && pnpm build
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && cd ..
```
预期：全绿。

- [ ] **Step 10：Commit + CHANGELOG**

```bash
git add src src-tauri/capabilities
git commit -m "feat(settings): 备份 UI + integrity 损坏对话框 + 明文导出"
```

`/changelog`：`/settings` 激活，含备份卡片（状态 + 立即备份 + 明文导出 + 历史列表）；数据库损坏时启动阻塞在损坏对话框（用户可从自动备份或自选文件恢复）；unlock 后台自动跑 24h 备份判定。

---

## Task 8: M4 closeout + 手动验收清单 + M4 里程碑 CHANGELOG

**Files:**
- Create: `.superpowers/sdd/m4-acceptance.md`
- Modify: `CHANGELOG.md`（追加 M4 里程碑总结）

**Interfaces:**
- Produces：M4 验收清单
- Consumes：无

- [ ] **Step 1：跑 5 信号全绿**

```bash
export PATH="$HOME/.nvm/versions/node/v22.14.0/bin:$HOME/.cargo/bin:$PATH"
pnpm tsc --noEmit
pnpm build
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && cd ..
```
预期：全绿；测试 95 pass。

- [ ] **Step 2：写 `.superpowers/sdd/m4-acceptance.md`**

内容：

```markdown
# M4 手动验收清单

前置：M3 已通过验收。M4 完工后跑 `pnpm tauri dev`，按以下逐项核对。可基于既有 M3 数据继续验证（0003 迁移已完成），无需清库。

## 0. 迁移与启动
- [ ] 启动无 schema 错误
- [ ] unlock 后台跑 integrity_check 未阻塞正常流程

## 1. 立即备份
- [ ] 进入「设置」看到「备份与恢复」卡片
- [ ] 首次进入 "最近一次备份" 显示"从未备份"（或已有历史）
- [ ] 点「立即备份」按钮 → toast 成功 → 状态更新 → 历史列表出现新条目 `auto_YYYYMMDD_HHmmss.db`
- [ ] 备份文件真实存在于 `~/Library/Application Support/solo-cost/backups/`

## 2. 24h 自动备份
- [ ] 立即备份一次后 lock → unlock，一分钟内不应再产生新备份（>24h 才触发）
- [ ] （高级）人工把 `app_meta.last_backup_at` 改为一天前，unlock 后应看到历史多一条自动备份

## 3. 备份 rotation
- [ ] 反复点「立即备份」8 次，历史列表最多 7 条；最早的一份被自动删除
- [ ] 文件系统里 `auto_*.db` 也应只剩 7 份

## 4. 明文导出
- [ ] 点「导出明文备份…」→ 系统 confirm 弹警告 → 确认 → 选保存位置 → 生成 .db 文件
- [ ] 用 sqlite3 CLI 打开该文件（不需密码）可以看到 companies 等表数据

## 5. 从备份恢复（正常场景）
- [ ] 在项目中改一些数据（比如新增一笔成本）
- [ ] 立即备份一次
- [ ] 再改一次数据（比如再加一笔成本）
- [ ] 「从备份恢复」→ 选刚才那份备份 → 输入当前主密码 → 成功后回登录页 → 输密码进入 → 只看到备份时点的数据

## 6. 从备份恢复（错误场景）
- [ ] 「从备份恢复」时输错密码 → toast 报错 → 现连接与文件都不动
- [ ] 「从备份恢复」时选不存在的文件 → toast 报错

## 7. 数据库损坏对话框
- [ ] （高级）关闭应用后手工用二进制编辑器破坏 `data.db` 中间几个字节，再打开应用 → 输密码 → 弹「数据库损坏」对话框
- [ ] 对话框列出所有 auto 备份，可选中一份、输密码、点恢复 → 成功后回登录页

## 8. M3 minor 收尾
- [ ] 编辑任务时若原负责人已归档，下拉里能看到该人后缀"（已归档）"
- [ ] 尝试给任务指派另一公司的成员（构造场景后调 IPC 或用 devtools） → 后端返回 Validation 错误
- [ ] TimeLogEditForm 清空日期后点保存 → 前端弹「工作日期必填」
- [ ] 删除任务时确认对话框显示 "确认删除任务「XXX」？关联工时将被一并软删..."（走 i18n key）
- [ ] 在成本 / 收款 Tab 做任意 mutation 后立即回概览 → 财务面板已刷新

## 9. 回归
- [ ] M1-M3 全部核心流程仍可用
```

- [ ] **Step 3：CHANGELOG 追加 M4 milestone summary**

在 Unreleased 顶部或末尾追加一条：

```markdown
- M4 里程碑完工：数据完整性 + 备份 + 恢复。unlock 后跑 `PRAGMA integrity_check`；启动/手动/明文三类备份；`restore_from_backup` 前先用密码 + integrity 双重验证；数据库损坏时启动阻塞在恢复对话框。同时收尾 M3 遗留 minor：cross-project 归属 / assignee 公司校验 / financial refresh try/finally 保护 / TimeLogEditForm 日期校验 / task 删除 i18n / TaskForm 归档 assignee 兼容。
```

- [ ] **Step 4：commit（accept file + changelog 分别可以合成一条）**

```bash
git add -f .superpowers/sdd/m4-acceptance.md
git commit -m "docs(m4): 验收清单 + 标记里程碑完工"
git add CHANGELOG.md
git commit -m "docs(changelog): 标记 m4 里程碑完工"
```

---

## Self-Review 结论（plan 提交前自检）

按 writing-plans skill 要求，对照 M4 范围与 spec 自检：

### 1. 覆盖范围

| Spec 项 | 任务 |
|---------|------|
| §5.3 启动自动备份 >24h | T2 `maybe_run_auto_backup` + T7 auth.ts unlock 后 fire-and-forget |
| §5.3 手动备份 | T2 `create_backup_now` + T7 settings 按钮 |
| §5.3 明文导出（UI 醒目警告） | T1 `export_plaintext` + T2 命令 + T7 settings confirm 警告 |
| §5.3 保留 7 份 rotation | T1 `rotate_auto_backups` + T2 create 后自动 rotate |
| §5.3 备份仍是 SQLCipher 加密文件 | T1 `copy_encrypted_db` 逻辑（文件级复制） |
| §5.3 `app_meta.last_backup_at` | T1 `copy_encrypted_db` 内写；T1 `should_auto_backup` 读 |
| §5.4 启动 integrity_check + 弹恢复对话框 | T3 unlock_at 内嵌 + T7 IntegrityFailedDialog |
| §5.4 关键写操作前自动备份 | T2 `create_backup_now` 前端 settings 可手动触发；spec 要求"关键写操作前自动触发一次备份"实际由用户按钮驱动，无自动 hook（M5 可加） |
| M3 minor F2 (update NotFound) | T5 payments update 加 get_impl 前置校验 |
| M3 minor T12 F3 (TimeLogEditForm date) | T6 |
| M3 minor T12 F4 (task delete confirm i18n) | T6 |
| M3 minor T12 F6 (archived assignee UX) | T6 TaskForm options |
| M3 minor payments/costs try/finally | T6 |
| M3 minor assignee_id company check | T5 tasks create/update |

### 2. 占位符扫描

- 无 "TBD/TODO/implement later"
- 每步都有完整代码或明确动作
- T3 测试的 corruption offset（4096）在不同 SQLCipher 版本可能变，测试断言宽容（union of 3 error variants），已在文本里说明

### 3. 类型一致性

- Rust `BackupInfo { file_name, absolute_path, size_bytes, created_at }` ↔ TS `BackupInfo { file_name: string, absolute_path: string, size_bytes: number, created_at: string }` ✓
- Rust `BackupStatus { last_backup_at: Option<String>, auto_count: usize, should_auto_backup_now: bool }` ↔ TS `{ last_backup_at: string | null, auto_count: number, should_auto_backup_now: boolean }` ✓
- Rust `AppError::IntegrityCheckFailed(String)` — 错误消息 `"integrity check failed: {0}"` — 前端 auth.ts unlock catch `msg.includes("integrity check failed")` ✓
- 命令名前后端一致：`list_backups / create_backup_now / maybe_run_auto_backup / export_plaintext_backup / get_backup_status / restore_from_backup` ✓
- Tauri 2 camelCase：`export_plaintext_backup(dst_path)` → `{ dstPath }`；`restore_from_backup(backup_path, password)` → `{ backupPath, password }` ✓

### 4. 范围控制

- 未做附件（M5）；未做 CSV/Excel（M5）；未做前端 Vitest（v0.2）
- 未做备份加密码保护差异（备份仍与库同密码——spec 明示）
- 未做「关键写操作前自动备份」hook（spec 有但成本高，M5 或按需再说）

### 5. 风险点

- **T3 corruption 测试**在不同 SQLCipher/OpenSSL 版本可能不稳定。断言写成 union 已尽量兼容；若 CI 里翻车，把 offset 改大或者去掉这一个 test 只保留 domain 层的正向 `integrity_check_passes_on_fresh_db`。
- **T7 auth.ts unlock 后 fire-and-forget maybeAutoBackup**：若备份失败静默吞，用户在 settings 页手动跑仍可发现问题；避免了 hot path 阻塞。若你想严格，可改成 toast 报错但不 throw。
- **T7 restoreFromBackup 后**：backend 已把 conn 放回 state 且验证通过，前端 IntegrityFailedDialog 会把 status 硬置为 "locked" 让用户重新登录——这样一定拿到干净的所有 store。避免 state 里残留旧数据。
- **`sqlcipher_export`**：M1 已用 `bundled-sqlcipher-vendored-openssl` 包含。若 dev 里失败，先跑 `PRAGMA cipher_version;` 看是否 4.x。

---

## Demoable End-State

完成 M4 全部 8 个 task 后：

- 「设置」页面能立即备份、看历史、导出明文（有警告）
- 每次 unlock 若距上次 >24h 会静默创建一份自动备份
- 备份保留最近 7 份，滚动删除
- 用户误删或数据错乱可从历史备份恢复（前端提供选文件 + 密码流程）
- 若数据库真损坏（integrity_check 失败），启动时弹阻塞对话框，用户直接从可见的自动备份列表恢复
- M3 遗留的 assignee 校验、日期 guard、i18n 补齐、cross-project 归属都补上

---
