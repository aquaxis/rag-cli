use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(String),

    #[error("qdrant error: {0}")]
    Qdrant(String),

    #[error("ollama error: {0}")]
    Ollama(String),

    #[error("docling error: {0}")]
    Docling(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("dim mismatch: expected {expected}, got {got}")]
    DimMismatch { expected: usize, got: usize },

    #[error("config error: {0}")]
    Config(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn http<E: std::fmt::Display>(e: E) -> Self {
        Self::Http(e.to_string())
    }
    pub fn parse<E: std::fmt::Display>(e: E) -> Self {
        Self::Parse(e.to_string())
    }
    pub fn validation<S: Into<String>>(s: S) -> Self {
        Self::Validation(s.into())
    }
    pub fn internal<S: Into<String>>(s: S) -> Self {
        Self::Internal(anyhow::anyhow!(s.into()))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(json!({ "error": self.to_string() }));
        (status, body).into_response()
    }
}
