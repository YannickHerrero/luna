use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use luna_protocol::{ApiError, ErrorCode};
use luna_storage::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Auth(#[from] crate::auth::AuthError),
    #[error(transparent)]
    Pi(#[from] luna_pi::PiError),
    #[error(transparent)]
    PiSession(#[from] luna_pi::SessionError),
    #[error(transparent)]
    Time(#[from] time::error::Format),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("authentication required")]
    AuthenticationRequired,
    #[error("forbidden")]
    Forbidden,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, retryable) = match &self {
            Self::NotFound | Self::Storage(StorageError::NotFound) => (
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound,
                "The requested Luna resource was not found.",
                false,
            ),
            Self::InvalidRequest(message) => (
                StatusCode::BAD_REQUEST,
                ErrorCode::InvalidRequest,
                message.as_str(),
                false,
            ),
            Self::AuthenticationRequired => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::AuthenticationRequired,
                "Pair this device with Luna before continuing.",
                false,
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden,
                "This request is not allowed.",
                false,
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::InternalError,
                "Luna could not complete the request.",
                true,
            ),
        };
        (status, Json(ApiError::new(code, message, retryable))).into_response()
    }
}
