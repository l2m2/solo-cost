use serde::{Serialize, Serializer};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("wrong master password")]
    WrongPassword,

    #[error("not initialized")]
    NotInitialized,

    #[error("already initialized")]
    AlreadyInitialized,

    #[error("locked: please unlock first")]
    Locked,

    #[error("validation: {0}")]
    Validation(String),

    #[error("not found: {entity} #{id}")]
    NotFound { entity: &'static str, id: i64 },

    // Consumed by T3 (unlock integrity gate); suppress until then.
    #[allow(dead_code)]
    #[error("integrity check failed: {0}")]
    IntegrityCheckFailed(String),

    #[error("backup failed: {0}")]
    Backup(String),

    #[error("cannot delete: {0}")]
    DeleteBlocked(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
