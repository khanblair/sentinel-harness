//! Verification: test gate, verdict, quality notes, honest report.

pub mod gate;
pub mod quality;
pub mod report;
pub mod verdict;
pub mod verifier;

use std::io;

use thiserror::Error;

/// Error type for verification operations.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Gate error: {0}")]
    Gate(String),
}

pub type Result<T> = std::result::Result<T, VerifyError>;
