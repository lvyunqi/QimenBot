use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug)]
pub enum AdminError {
    BadRequest(String),
    Unauthorized,
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl AdminError {
    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication is required".to_string(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "revision_conflict", message),
            Self::Internal(message) => {
                tracing::error!(error = %message, "admin API request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "the request could not be completed".to_string(),
                )
            }
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

impl From<std::io::Error> for AdminError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error)
    }
}

impl From<toml::de::Error> for AdminError {
    fn from(error: toml::de::Error) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<toml_edit::TomlError> for AdminError {
    fn from(error: toml_edit::TomlError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<qimen_error::QimenError> for AdminError {
    fn from(error: qimen_error::QimenError) -> Self {
        Self::BadRequest(error.to_string())
    }
}
