//! The errors of the project list. `hub.rs` re-exports `HubError`, so `hub::HubError` keeps working.

use axum::http::StatusCode;

use crate::routes::ApiFailure;

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("That is not a folder.")]
    NotAFolder,
    #[error("That folder is not a git repository. Run git init there first.")]
    NotGit,
    #[error("That project is already in the list.")]
    AlreadyThere,
    #[error("This app cannot add projects.")]
    CannotAdd,
    #[error("Could not open the project: {0}")]
    Open(String),
    #[error("No such project.")]
    NotFound,
    #[error("The first project stays. It also holds the paired phones and the keys.")]
    IsPrimary,
    #[error("A run is live in that project. Stop it first.")]
    Live,
}

impl From<HubError> for ApiFailure {
    fn from(error: HubError) -> Self {
        let status = match error {
            HubError::NotAFolder | HubError::NotGit | HubError::Open(_) | HubError::IsPrimary => {
                StatusCode::BAD_REQUEST
            }
            HubError::AlreadyThere | HubError::Live => StatusCode::CONFLICT,
            HubError::CannotAdd => StatusCode::NOT_IMPLEMENTED,
            HubError::NotFound => StatusCode::NOT_FOUND,
        };
        ApiFailure::new(status, error.to_string())
    }
}
