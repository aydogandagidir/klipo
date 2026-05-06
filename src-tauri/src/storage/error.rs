//! Storage layer error type.
//!
//! Variants are stable across the public API surface; commands.rs maps these
//! into IPC error strings for the frontend.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database query failed: {0}")]
    Sql(#[from] sqlx::Error),

    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("clip not found: {0}")]
    NotFound(String),

    #[error("invalid clip kind: {0}")]
    InvalidKind(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;
