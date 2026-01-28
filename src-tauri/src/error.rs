//! Unified Error Types for CCR
//!
//! Provides consistent error handling across the application.

#![allow(dead_code)]

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;

/// Application-wide error types
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Error response structure
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => {
                crate::logger::error("server", &format!("Database error: {}", e));
                (StatusCode::INTERNAL_SERVER_ERROR, "Database operation failed")
            }
            AppError::Io(e) => {
                crate::logger::error("server", &format!("IO error: {}", e));
                (StatusCode::INTERNAL_SERVER_ERROR, "File operation failed")
            }
            AppError::JsonSerialization(e) => {
                crate::logger::error("server", &format!("JSON error: {}", e));
                (StatusCode::INTERNAL_SERVER_ERROR, "Data serialization failed")
            }
            AppError::Config(msg) => {
                crate::logger::error("server", &format!("Config error: {}", msg));
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error")
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, msg.as_str())
            }
            AppError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, msg.as_str())
            }
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, msg.as_str())
            }
            AppError::Internal(msg) => {
                crate::logger::error("server", &format!("Internal error: {}", msg));
                (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str())
            }
        };

        let body = Json(ErrorResponse {
            error: status.as_u16().to_string(),
            message: message.to_string(),
            details: Some(self.to_string()),
        });

        (status, body).into_response()
    }
}

/// Result type alias for app operations
pub type AppResult<T> = Result<T, AppError>;
