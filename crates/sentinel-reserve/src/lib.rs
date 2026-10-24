//! Reservation table and wave builder. One path has one writer at a time.

pub mod table;
pub mod waves;

mod claims;
mod holdings;

pub use table::{Grant, ReservationTable};
pub use waves::build_wave;

use sentinel_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReserveError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ReserveError>;
