//! Error types for sentinel-brain.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrainError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Format error in {path}: {reason}")]
    Format { path: String, reason: String },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, BrainError>;
