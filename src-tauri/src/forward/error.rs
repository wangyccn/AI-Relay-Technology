//! Forward module error types
//!
//! Defines all error types used in the forward module for request handling.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// Error types for the forward module
#[derive(Debug, Clone)]
pub enum ForwardError {
    /// Authentication token missing or invalid
    Unauthorized(String),
    /// Access denied (valid token but no permission)
    #[allow(dead_code)]
    Forbidden(String),
    /// Requested model not found in configuration
    ModelNotFound(String),
    /// Upstream provider not found in configuration
    UpstreamNotFound(String),
    /// Request to upstream provider failed
    RequestFailed(String),
    /// Invalid request format or parameters
    InvalidRequest(String),
    /// Request rejected by rate limiting or quotas
    RateLimited(String),
    /// Request timeout
    Timeout(String),
    /// Internal server error
    Internal(String),
}

impl std::fmt::Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ForwardError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ForwardError::ModelNotFound(msg) => write!(f, "Model not found: {}", msg),
            ForwardError::UpstreamNotFound(msg) => write!(f, "Upstream not found: {}", msg),
            ForwardError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            ForwardError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ForwardError::RateLimited(msg) => write!(f, "Rate limited: {}", msg),
            ForwardError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ForwardError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ForwardError {}

impl IntoResponse for ForwardError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            ForwardError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", msg.clone())
            }
            ForwardError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg.clone()),
            ForwardError::ModelNotFound(msg) => {
                (StatusCode::NOT_FOUND, "model_not_found", msg.clone())
            }
            ForwardError::UpstreamNotFound(msg) => {
                (StatusCode::NOT_FOUND, "upstream_not_found", msg.clone())
            }
            ForwardError::RequestFailed(msg) => {
                (StatusCode::BAD_GATEWAY, "request_failed", msg.clone())
            }
            ForwardError::InvalidRequest(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_request", msg.clone())
            }
            ForwardError::RateLimited(msg) => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                msg.clone(),
            ),
            ForwardError::Timeout(msg) => (StatusCode::GATEWAY_TIMEOUT, "timeout", msg.clone()),
            ForwardError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.clone(),
            ),
        };

        // Log all errors being returned to client
        crate::logger::error(
            "forward_error",
            &format!(
                "Returning error response: status={}, type={}, message={}",
                status.as_u16(),
                error_type,
                message
            ),
        );

        (
            status,
            Json(serde_json::json!({
                "error": {
                    "type": error_type,
                    "message": message
                }
            })),
        )
            .into_response()
    }
}

/// Result type alias for forward operations
pub type ForwardResult<T> = Result<T, ForwardError>;
