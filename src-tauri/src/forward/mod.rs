//! Forward module
//!
//! Handles request forwarding to upstream API providers (OpenAI, Anthropic, Gemini).
//!
//! ## Architecture
//!
//! ```text
//! Request -> Middleware -> Handler -> Upstream
//!              |              |
//!              v              v
//!         ForwardContext   Provider-specific
//!         (auth, model,    request handling
//!          upstream info)
//! ```
//!
//! ## API Endpoints
//!
//! ### Unified Endpoints (auto-route based on model)
//! - `POST /v1/chat/completions` - OpenAI-compatible, routes to appropriate provider
//! - `GET /v1/models` - List available models
//!
//! ### Provider-Specific Endpoints
//! - `POST /openai/v1/chat/completions` - OpenAI API
//! - `POST /anthropic/v1/messages` - Anthropic Messages API
//! - `POST /gemini/v1beta/*` - Gemini API
//!
//! ## Components
//!
//! - `middleware`: Request parsing, authentication, and context building
//! - `handlers`: Provider-specific request/response handling
//! - `client`: HTTP client utilities with retry logic
//! - `context`: Shared data structures
//! - `error`: Error types

pub mod client;
pub mod context;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod routing;

use axum::{extract::Path, http::HeaderMap, response::{IntoResponse, Response}, Json};
use serde_json::Value;

use crate::{config, routing::latency};

// Re-export commonly used types (allow unused for public API)
#[allow(unused_imports)]
pub use context::{ForwardContext, ForwardPlan, Provider, RetryConfig, TokenUsage, UpstreamResponse};
#[allow(unused_imports)]
pub use error::{ForwardError, ForwardResult};
#[allow(unused_imports)]
pub use handlers::{get_handler, ProviderHandler};

// ============================================================================
// Unified API Endpoints (Auto-routing based on model)
// ============================================================================

/// Unified chat completions endpoint (OpenAI-compatible)
///
/// This is the main entry point for all editors. It automatically routes
/// requests to the appropriate provider based on the model configuration.
///
/// Route: POST /v1/chat/completions
///
/// Supports:
/// - Streaming (stream: true)
/// - Multimodal (images in messages)
/// - Tool use
/// - All OpenAI-compatible parameters
pub async fn unified_chat_completions(
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Build plan using middleware
    let plan = match middleware::build_forward_plan(&headers, &payload, None) {
        Ok(plan) => plan,
        Err(e) => return e.into_response(),
    };

    // Get the appropriate handler based on provider
    let handler = handlers::get_handler(plan.primary.model.provider);

    // Handle streaming vs non-streaming
    if plan.primary.is_streaming {
        match handler.handle_stream(plan.primary, payload).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    } else {
        handle_request_with_fallback(handler, plan, payload).await
    }
}

/// List available models (OpenAI-compatible)
///
/// Route: GET /v1/models
///
/// Returns a list of all configured models in OpenAI format.
pub async fn list_models(headers: HeaderMap) -> impl IntoResponse {
    // Check authentication if forward_token is configured
    if let Err(e) = middleware::determine_auth_mode(&headers) {
        return e.into_response();
    }

    let cfg = config::load();
    let models: Vec<Value> = cfg
        .models
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "object": "model",
                "created": 1700000000,
                "owned_by": m.provider,
                "permission": [],
                "root": m.id,
                "parent": null
            })
        })
        .collect();

    Json(serde_json::json!({
        "object": "list",
        "data": models
    }))
    .into_response()
}

/// Get model details (OpenAI-compatible)
///
/// Route: GET /v1/models/:model_id
pub async fn get_model(Path(model_id): Path<String>, headers: HeaderMap) -> impl IntoResponse {
    // Check authentication
    if let Err(e) = middleware::determine_auth_mode(&headers) {
        return e.into_response();
    }

    let cfg = config::load();
    if let Some(m) = cfg.models.iter().find(|m| m.id == model_id) {
        Json(serde_json::json!({
            "id": m.id,
            "object": "model",
            "created": 1700000000,
            "owned_by": m.provider,
            "permission": [],
            "root": m.id,
            "parent": null
        }))
        .into_response()
    } else {
        error::ForwardError::ModelNotFound(format!("Model '{}' not found", model_id))
            .into_response()
    }
}

// ============================================================================
// Provider-Specific Endpoints
// ============================================================================

/// OpenAI compatible chat completions endpoint
///
/// Route: POST /openai/v1/chat/completions
pub async fn openai_chat(headers: HeaderMap, Json(payload): Json<Value>) -> impl IntoResponse {
    // Build plan using middleware
    let plan = match middleware::build_forward_plan(&headers, &payload, Some(Provider::OpenAI)) {
        Ok(plan) => plan,
        Err(e) => return e.into_response(),
    };

    // Use the handler matching the selected provider.
    let handler = handlers::get_handler(plan.primary.model.provider);

    // Handle streaming vs non-streaming
    if plan.primary.is_streaming {
        match handler.handle_stream(plan.primary, payload).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    } else {
        handle_request_with_fallback(handler, plan, payload).await
    }
}

/// Anthropic messages endpoint
///
/// Route: POST /anthropic/v1/messages
pub async fn anthropic_messages(
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Build plan using middleware
    let plan = match middleware::build_forward_plan(&headers, &payload, Some(Provider::Anthropic)) {
        Ok(plan) => plan,
        Err(e) => return e.into_response(),
    };

    // Get the appropriate handler
    let handler = handlers::get_handler(plan.primary.model.provider);

    // Handle streaming vs non-streaming
    if plan.primary.is_streaming {
        match handler.handle_stream(plan.primary, payload).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    } else {
        handle_request_with_fallback(handler, plan, payload).await
    }
}

/// Gemini generate endpoint
///
/// Route: POST /gemini/v1beta/*endpoint
pub async fn gemini_generate(
    Path(endpoint): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    gemini_generate_with_version(endpoint, headers, payload, "v1beta").await
}

/// Gemini generate endpoint (v1)
///
/// Route: POST /gemini/v1/*endpoint
pub async fn gemini_generate_v1(
    Path(endpoint): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    gemini_generate_with_version(endpoint, headers, payload, "v1").await
}

async fn gemini_generate_with_version(
    endpoint: String,
    headers: HeaderMap,
    payload: Value,
    api_version: &str,
) -> Response {
    // Build plan using Gemini-specific middleware
    let plan = match middleware::build_gemini_plan(&headers, &payload, &endpoint, api_version) {
        Ok(plan) => plan,
        Err(e) => return e.into_response(),
    };

    // Get the appropriate handler
    let handler = handlers::get_handler(plan.primary.model.provider);

    // Handle streaming vs non-streaming
    if plan.primary.is_streaming {
        match handler.handle_stream(plan.primary, payload).await {
            Ok(response) => response,
            Err(e) => e.into_response(),
        }
    } else {
        handle_request_with_fallback(handler, plan, payload).await
    }
}

// ============================================================================
// Utility Endpoints
// ============================================================================

/// List supported API styles/providers
pub fn api_styles() -> Vec<&'static str> {
    vec!["openai", "anthropic", "gemini"]
}

/// List API styles endpoint
pub async fn list_api_styles() -> Json<Value> {
    Json(serde_json::json!({ "providers": api_styles() }))
}

/// Get latency for an upstream's endpoints
pub async fn upstream_latency(Path(upstream_id): Path<String>) -> Json<Value> {
    let cfg = config::load();
    let Some(up) = cfg.upstreams.iter().find(|u| u.id == upstream_id) else {
        return Json(serde_json::json!({
            "error": "upstream_not_found",
            "message": format!("Upstream '{}' not found", upstream_id)
        }));
    };
    let stats = latency::measure_all(up.endpoints.clone()).await;
    Json(serde_json::json!({
        "upstream": upstream_id,
        "latency": stats
    }))
}

/// Test latency for a list of URLs directly
pub async fn test_latency_urls(Json(urls): Json<Vec<String>>) -> Json<Value> {
    if urls.is_empty() {
        return Json(serde_json::json!({
            "error": "no_urls",
            "message": "No URLs provided"
        }));
    }
    let stats = latency::measure_all(urls).await;
    Json(serde_json::json!({
        "latency": stats
    }))
}

/// Get current forward token
pub async fn get_forward_token() -> Json<Value> {
    let cfg = config::load();
    Json(serde_json::json!({
        "token": cfg.forward_token.unwrap_or_default()
    }))
}

/// Refresh forward token
pub async fn refresh_forward_token() -> Json<Value> {
    let token = config::refresh_forward_token();
    Json(serde_json::json!({
        "token": token
    }))
}

/// Health check for API endpoints
pub async fn api_health() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "providers": api_styles()
    }))
}

fn parse_status_code(message: &str) -> Option<u16> {
    for token in message.split(|c: char| !c.is_ascii_digit()) {
        if token.len() == 3 {
            if let Ok(code) = token.parse::<u16>() {
                if (100..=599).contains(&code) {
                    return Some(code);
                }
            }
        }
    }
    None
}

fn should_retry_error(err: &ForwardError) -> bool {
    match err {
        ForwardError::Timeout(_) => true,
        ForwardError::RequestFailed(message) => {
            if let Some(code) = parse_status_code(message) {
                client::should_retry(code)
            } else {
                true
            }
        }
        _ => false,
    }
}

async fn handle_request_with_fallback(
    handler: ProviderHandler,
    plan: ForwardPlan,
    payload: Value,
) -> Response {
    let retry_config = RetryConfig::from_config();
    let mut contexts = Vec::new();
    contexts.push(plan.primary);
    contexts.extend(plan.fallbacks);

    if contexts.is_empty() {
        return ForwardError::ModelNotFound("No routes configured".to_string()).into_response();
    }

    let max_attempts = retry_config.max_attempts as usize;
    if contexts.len() > max_attempts {
        contexts.truncate(max_attempts);
    }

    let total_attempts = contexts.len();
    for (attempt_idx, ctx) in contexts.into_iter().enumerate() {
        match handler.handle_request(ctx, payload.clone()).await {
            Ok(response) => return Json(response.body).into_response(),
            Err(err) => {
                let should_retry = should_retry_error(&err);
                let is_last = attempt_idx + 1 >= total_attempts;
                if !should_retry || is_last {
                    return err.into_response();
                }
                let delay = client::calculate_retry_delay((attempt_idx + 1) as u32, &retry_config);
                tokio::time::sleep(delay).await;
            }
        }
    }

    ForwardError::RequestFailed("No upstreams available".to_string()).into_response()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_styles() {
        let styles = api_styles();
        assert!(styles.contains(&"openai"));
        assert!(styles.contains(&"anthropic"));
        assert!(styles.contains(&"gemini"));
    }
}
