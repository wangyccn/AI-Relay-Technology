//! Forward handlers module
//!
//! Contains handlers for each supported API provider.

pub mod anthropic;
pub mod gemini;
pub mod openai;

use axum::response::Response;
use reqwest::header::HeaderMap;
use serde_json::Value;

use super::context::{ForwardContext, Provider, TokenUsage, UpstreamResponse};
use super::error::ForwardResult;

/// Provider handler enum for dispatching to the correct handler
pub enum ProviderHandler {
    OpenAI(openai::OpenAIHandler),
    Anthropic(anthropic::AnthropicHandler),
    Gemini(gemini::GeminiHandler),
}

#[allow(dead_code)]
impl ProviderHandler {
    /// Get the provider name
    pub fn name(&self) -> &'static str {
        match self {
            ProviderHandler::OpenAI(h) => h.name(),
            ProviderHandler::Anthropic(h) => h.name(),
            ProviderHandler::Gemini(h) => h.name(),
        }
    }

    /// Build the request URL
    pub fn build_url(&self, ctx: &ForwardContext, path: &str) -> String {
        match self {
            ProviderHandler::OpenAI(h) => h.build_url(ctx, path),
            ProviderHandler::Anthropic(h) => h.build_url(ctx, path),
            ProviderHandler::Gemini(h) => h.build_url(ctx, path),
        }
    }

    /// Build request headers
    pub fn build_headers(&self, ctx: &ForwardContext) -> HeaderMap {
        match self {
            ProviderHandler::OpenAI(h) => h.build_headers(ctx),
            ProviderHandler::Anthropic(h) => h.build_headers(ctx),
            ProviderHandler::Gemini(h) => h.build_headers(ctx),
        }
    }

    /// Transform request payload
    pub fn transform_request(&self, ctx: &ForwardContext, payload: &Value) -> Value {
        match self {
            ProviderHandler::OpenAI(h) => h.transform_request(ctx, payload),
            ProviderHandler::Anthropic(h) => h.transform_request(ctx, payload),
            ProviderHandler::Gemini(h) => h.transform_request(ctx, payload),
        }
    }

    /// Parse response and extract token usage
    pub fn parse_response(&self, response: &Value) -> TokenUsage {
        match self {
            ProviderHandler::OpenAI(h) => h.parse_response(response),
            ProviderHandler::Anthropic(h) => h.parse_response(response),
            ProviderHandler::Gemini(h) => h.parse_response(response),
        }
    }

    /// Estimate tokens from request
    pub fn estimate_request_tokens(&self, payload: &Value) -> i64 {
        match self {
            ProviderHandler::OpenAI(h) => h.estimate_request_tokens(payload),
            ProviderHandler::Anthropic(h) => h.estimate_request_tokens(payload),
            ProviderHandler::Gemini(h) => h.estimate_request_tokens(payload),
        }
    }
}

impl ProviderHandler {
    /// Handle non-streaming request
    pub async fn handle_request(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> ForwardResult<UpstreamResponse> {
        match self {
            ProviderHandler::OpenAI(h) => h.handle_request(ctx, payload).await,
            ProviderHandler::Anthropic(h) => h.handle_request(ctx, payload).await,
            ProviderHandler::Gemini(h) => h.handle_request(ctx, payload).await,
        }
    }

    /// Handle streaming request
    pub async fn handle_stream(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> ForwardResult<Response> {
        match self {
            ProviderHandler::OpenAI(h) => h.handle_stream(ctx, payload).await,
            ProviderHandler::Anthropic(h) => h.handle_stream(ctx, payload).await,
            ProviderHandler::Gemini(h) => h.handle_stream(ctx, payload).await,
        }
    }
}

/// Get the appropriate handler for a provider
pub fn get_handler(provider: Provider) -> ProviderHandler {
    match provider {
        Provider::OpenAI => ProviderHandler::OpenAI(openai::OpenAIHandler),
        Provider::Anthropic => ProviderHandler::Anthropic(anthropic::AnthropicHandler),
        Provider::Gemini => ProviderHandler::Gemini(gemini::GeminiHandler),
    }
}

/// Trait interface for implementing provider-specific behavior
///
/// Each provider implements this trait to handle:
/// - Request transformation (adapting payload to provider format)
/// - Header building (authentication, content-type, etc.)
/// - Response parsing (extracting usage, handling errors)
/// - Streaming support
#[allow(dead_code)]
pub trait ProviderHandlerImpl: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &'static str;

    /// Build the request URL for the given endpoint
    fn build_url(&self, ctx: &ForwardContext, path: &str) -> String;

    /// Build request headers
    fn build_headers(&self, ctx: &ForwardContext) -> HeaderMap;

    /// Transform request payload to provider format
    fn transform_request(&self, ctx: &ForwardContext, payload: &Value) -> Value;

    /// Parse response and extract token usage
    fn parse_response(&self, response: &Value) -> TokenUsage;

    /// Estimate tokens from request (fallback when response doesn't include usage)
    fn estimate_request_tokens(&self, payload: &Value) -> i64;

    /// Handle non-streaming request
    fn handle_request(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> impl std::future::Future<Output = ForwardResult<UpstreamResponse>> + Send;

    /// Handle streaming request
    fn handle_stream(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> impl std::future::Future<Output = ForwardResult<Response>> + Send;
}
