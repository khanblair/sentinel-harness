//! Errors for the shared types.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("invalid path `{path}`: {reason}")]
    InvalidPath { path: String, reason: &'static str },
    #[error("invalid money amount `{0}`")]
    InvalidMoney(String),
    #[error("task cannot move from {from} to {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
}
