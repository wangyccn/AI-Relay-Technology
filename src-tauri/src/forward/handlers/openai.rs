//! OpenAI API handler
//!
//! Handles forwarding requests to OpenAI-compatible APIs.

use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::forward::client::{self, drain_sse_lines, is_sse_done, parse_sse_data};
use crate::forward::context::{estimate_tokens, ForwardContext, Provider, TokenUsage, UpstreamResponse};
use crate::forward::error::{ForwardError, ForwardResult};
use crate::logger;

use super::{anthropic, gemini, ProviderHandlerImpl};

/// Allowed fields for OpenAI Chat Completions API
const ALLOWED_FIELDS: &[&str] = &[
    "model",
    "messages",
    "max_tokens",
    "max_completion_tokens",
    "temperature",
    "top_p",
    "n",
    "stream",
    "stream_options",
    "stop",
    "presence_penalty",
    "frequency_penalty",
    "logit_bias",
    "user",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "response_format",
    "seed",
    "logprobs",
    "top_logprobs",
    "function_call",
    "functions",
    "service_tier",
    "store",
    "reasoning_effort",
    "metadata",
];

/// OpenAI API handler
pub struct OpenAIHandler;

impl ProviderHandlerImpl for OpenAIHandler {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn build_url(&self, ctx: &ForwardContext, path: &str) -> String {
        let endpoint = ctx.primary_endpoint().unwrap_or("https://api.openai.com");
        format!("{}{}", endpoint.trim_end_matches('/'), path)
    }

    fn build_headers(&self, ctx: &ForwardContext) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Add authorization header
        if let Some(api_key) = ctx.get_api_key() {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", api_key)) {
                headers.insert("authorization", value);
            }
        }

        // Content-Type
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        headers
    }

    fn transform_request(&self, ctx: &ForwardContext, payload: &Value) -> Value {
        // Determine allowed fields based on upstream capabilities
        let allowed_fields = get_allowed_fields_for_upstream(&ctx.upstream.id);
        let mut filtered = filter_payload(payload, allowed_fields);

        // Replace model with upstream model name
        if let Some(obj) = filtered.as_object_mut() {
            obj.insert(
                "model".to_string(),
                Value::String(ctx.model.upstream_model().to_string()),
            );
        }

        // Transform messages for GLM compatibility (convert multimodal content array to string)
        if is_glm_upstream(&ctx.upstream.id) {
            transform_messages_for_glm(&mut filtered);
        }

        // Log the transformed request
        logger::debug(
            "openai",
            &format!(
                "Transform request: upstream={}, model={} -> {}",
                ctx.upstream.id,
                ctx.model.id,
                ctx.model.upstream_model()
            ),
        );

        client::normalize_stream_flag(&mut filtered);
        filtered
    }

    fn parse_response(&self, response: &Value) -> TokenUsage {
        extract_usage(response)
    }

    fn estimate_request_tokens(&self, payload: &Value) -> i64 {
        let messages = payload
            .get("messages")
            .map(|m| m.to_string())
            .unwrap_or_default();
        estimate_tokens(&messages)
    }

    async fn handle_request(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> ForwardResult<UpstreamResponse> {
        let upstream_style = upstream_style(&ctx);
        if upstream_style != Provider::OpenAI {
            return match upstream_style {
                Provider::Anthropic => handle_openai_to_anthropic_request(ctx, payload).await,
                Provider::Gemini => handle_openai_to_gemini_request(ctx, payload).await,
                Provider::OpenAI => unreachable!(),
            };
        }

        let start = Instant::now();

        logger::info(
            "openai",
            &format!(
                "Request started: model={}, upstream={}, streaming=false",
                ctx.model.id,
                ctx.upstream.id
            ),
        );

        // Build request
        let headers = self.build_headers(&ctx);
        let body = self.transform_request(&ctx, &payload);
        let config = ctx.retry_config();
        let client = client::default_client()?;

        // Log the request URL
        let endpoint = ctx.primary_endpoint().unwrap_or("unknown");
        let full_url = format!("{}{}", endpoint.trim_end_matches('/'), "/chat/completions");
        logger::debug(
            "openai",
            &format!("Request URL: {}", full_url),
        );

        // Send request with retry
        // Use /chat/completions instead of /v1/chat/completions to support custom API paths like /v4/chat/completions
        let result = client::send_with_retry(
            &client,
            ctx.all_endpoints(),
            "/chat/completions",
            headers,
            &body,
            &config,
        )
        .await?;

        // Parse response
        let status = result.response.status();
        let status_code = status.as_u16();
        let response_text = result.response.text().await.map_err(|e| {
            logger::error("openai", &format!("Failed to read response body: {}", e));
            ForwardError::RequestFailed(format!("Failed to read response: {}", e))
        })?;

        // Log response for debugging empty responses
        if response_text.is_empty() {
            logger::warn("openai", "Received empty response body from upstream");
        }

        let response_body: Value = client::parse_json_response(&response_text).map_err(|e| {
            logger::error("openai", &format!("Failed to parse response JSON: {}, body: {}", e, &response_text[..response_text.len().min(500)]));
            ForwardError::RequestFailed(format!("Failed to parse response: {}", e))
        })?;

        // Check if response indicates an error
        if !status.is_success() {
            logger::warn(
                "openai",
                &format!("Request failed: status={}, response={}", status_code, response_body),
            );
            // Don't log usage for failed requests
            return Err(ForwardError::RequestFailed(format!(
                "Upstream returned {}: {}",
                status_code,
                response_body.to_string()
            )));
        }

        // Extract usage
        let mut usage = extract_usage(&response_body);
        if usage.prompt_tokens == 0 {
            // Estimate if not provided
            usage.prompt_tokens = self.estimate_request_tokens(&payload);
        }

        let latency_ms = start.elapsed().as_millis() as u64;

        logger::info(
            "openai",
            &format!(
                "Request completed: model={}, latency={}ms, tokens={}/{}",
                ctx.model.id,
                latency_ms,
                usage.prompt_tokens,
                usage.completion_tokens
            ),
        );

        // Log usage to database only for successful requests
        ctx.log_usage(&usage);

        Ok(UpstreamResponse {
            body: response_body,
            latency_ms,
            status: status_code,
            usage,
        })
    }

    async fn handle_stream(&self, ctx: ForwardContext, payload: Value) -> ForwardResult<Response> {
        let upstream_style = upstream_style(&ctx);
        if upstream_style != Provider::OpenAI {
            return match upstream_style {
                Provider::Anthropic => handle_openai_to_anthropic_stream(ctx, payload).await,
                Provider::Gemini => handle_openai_to_gemini_stream(ctx, payload).await,
                Provider::OpenAI => unreachable!(),
            };
        }

        // Build request
        let headers = self.build_headers(&ctx);
        let mut body = self.transform_request(&ctx, &payload);

        // Ensure stream is enabled
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(true));
            // Only add stream_options for non-GLM upstreams (GLM doesn't support it)
            if !is_glm_upstream(&ctx.upstream.id) {
                obj.insert(
                    "stream_options".to_string(),
                    serde_json::json!({"include_usage": true}),
                );
            }
        }

        let client = client::streaming_client()?;
        let endpoint = ctx
            .primary_endpoint()
            .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;
        // Use /chat/completions instead of /v1/chat/completions to support custom API paths like /v4/chat/completions
        let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

        logger::info(
            "openai",
            &format!(
                "Starting stream request: model={}, upstream={}, url={}",
                ctx.model.id, ctx.upstream.id, url
            ),
        );

        // Make request
        let response = client
            .post(&url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                logger::error(
                    "openai",
                    &format!("Stream request failed: url={}, error={}", url, e),
                );
                ForwardError::RequestFailed(e.to_string())
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            logger::error(
                "openai",
                &format!(
                    "Stream request returned error: status={}, body={}",
                    status,
                    &text[..text.len().min(500)]
                ),
            );
            return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
        }

        logger::debug(
            "openai",
            &format!("Stream response status: {}", status),
        );

        // Clone context for use in stream processing
        let ctx_clone = ctx.clone();
        let estimated_prompt_tokens = self.estimate_request_tokens(&payload);

        // Create usage tracker for accumulating streaming usage
        let usage_tracker = Arc::new(Mutex::new(TokenUsage::new(estimated_prompt_tokens, 0)));
        let usage_tracker_clone = Arc::clone(&usage_tracker);
        let line_buffer = Arc::new(Mutex::new(Vec::new()));
        let line_buffer_clone = Arc::clone(&line_buffer);

        // Stream the response
        let stream = response.bytes_stream().map(move |result| {
            match result {
                Ok(bytes) => {
                    let lines = {
                        let mut buffer = line_buffer_clone.lock().unwrap();
                        drain_sse_lines(&mut buffer, bytes.as_ref())
                    };

                    for line in lines {
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(data) = parse_sse_data(&line) {
                            if !is_sse_done(data) {
                                // Try to parse as JSON and extract usage
                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
                                        // Check for final usage in streaming response
                                        if let Some(usage) = json.get("usage") {
                                            let chunk_usage = TokenUsage::new(
                                                usage
                                                    .get("prompt_tokens")
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(0),
                                                usage
                                                    .get("completion_tokens")
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(0),
                                            );
                                            if let Ok(mut tracker) = usage_tracker_clone.lock() {
                                                *tracker = chunk_usage;
                                            }
                                        }
                                        // Also count completion tokens from delta content
                                        if let Some(choices) =
                                            json.get("choices").and_then(|c| c.as_array())
                                        {
                                            for choice in choices {
                                                if let Some(delta) = choice.get("delta") {
                                                    // Handle regular content field
                                                    if let Some(content) = delta
                                                        .get("content")
                                                        .and_then(|c| c.as_str())
                                                    {
                                                        let tokens = estimate_tokens(content);
                                                        if let Ok(mut tracker) =
                                                            usage_tracker_clone.lock()
                                                        {
                                                            tracker.completion_tokens += tokens;
                                                        }
                                                    }
                                                    // Handle GLM reasoning_content field
                                                    if let Some(reasoning_content) = delta
                                                        .get("reasoning_content")
                                                        .and_then(|c| c.as_str())
                                                    {
                                                        let tokens = estimate_tokens(reasoning_content);
                                                        if let Ok(mut tracker) =
                                                            usage_tracker_clone.lock()
                                                        {
                                                            tracker.completion_tokens += tokens;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // Log JSON parse errors with the problematic data
                                        logger::error(
                                            "openai",
                                            &format!(
                                                "Failed to parse SSE JSON chunk: error={}, data={}",
                                                e,
                                                &data[..data.len().min(200)]
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Ok(bytes)
                }
                Err(e) => {
                    logger::error(
                        "openai",
                        &format!("Stream bytes error: {}", e),
                    );
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ))
                }
            }
        });

        // Create a wrapper stream that logs usage when done
        let ctx_for_log = ctx_clone;
        let usage_for_log = Arc::clone(&usage_tracker);
        let model_id = ctx.model.id.clone();

        let logged_stream = stream
            .chain(futures_util::stream::once(async move {
                // Log final usage when stream completes
                if let Ok(usage) = usage_for_log.lock() {
                    logger::info(
                        "openai",
                        &format!(
                            "Stream completed: model={}, tokens={}/{}",
                            model_id,
                            usage.prompt_tokens,
                            usage.completion_tokens
                        ),
                    );
                    ctx_for_log.log_usage(&usage);
                } else {
                    logger::error(
                        "openai",
                        &format!("Failed to acquire usage tracker lock for model={}", model_id),
                    );
                }
                // Return empty bytes to signal completion without adding data
                Err(std::io::Error::new(std::io::ErrorKind::Other, "stream_end"))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                    Err(e) if e.to_string() == "stream_end" => None,
                    Err(e) => {
                        logger::error(
                            "openai",
                            &format!("Stream filter error: {}", e),
                        );
                        Some(Err(e))
                    }
                }
            });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .body(Body::from_stream(logged_stream))
            .unwrap_or_else(|e| {
                logger::error(
                    "openai",
                    &format!("Failed to build stream response: {}", e),
                );
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }))
    }
}

/// Filter payload to only include allowed fields
fn filter_payload(payload: &Value, allowed: &[&str]) -> Value {
    if let Some(obj) = payload.as_object() {
        let filtered: serde_json::Map<String, Value> = obj
            .iter()
            .filter(|(key, _)| allowed.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Value::Object(filtered)
    } else {
        payload.clone()
    }
}

/// Extract usage from OpenAI response
fn extract_usage(response: &Value) -> TokenUsage {
    if let Some(usage) = response.get("usage") {
        TokenUsage::new(
            usage
                .get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            usage
                .get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        )
    } else {
        TokenUsage::default()
    }
}

/// Get allowed fields based on upstream capabilities
/// Some upstreams (like Z.ai/GLM) don't support tools/function calling
fn get_allowed_fields_for_upstream(upstream_id: &str) -> &'static [&'static str] {
    // Check if this is Z.ai (GLM) upstream which doesn't support tools
    // Support both "zai" (new normalized ID) and "Z.ai" (legacy ID) for backward compatibility
    if upstream_id.eq_ignore_ascii_case("zai") || upstream_id.eq_ignore_ascii_case("Z.ai") {
        // Fields supported by GLM API - removed unsupported fields:
        // - metadata: not supported by GLM
        // - stream_options: not supported by GLM
        // - logit_bias: not supported by GLM
        // - logprobs/top_logprobs: not supported by GLM
        // - service_tier/store/reasoning_effort: not supported by GLM
        &[
            "model",
            "messages",
            "max_tokens",
            "temperature",
            "top_p",
            "n",
            "stream",
            "stop",
            "presence_penalty",
            "frequency_penalty",
            "user",
            "response_format",
            "seed",
        ]
    } else {
        // Full OpenAI compatibility with tools support
        ALLOWED_FIELDS
    }
}

/// Check if upstream is GLM/Z.ai
fn is_glm_upstream(upstream_id: &str) -> bool {
    upstream_id.eq_ignore_ascii_case("zai") || upstream_id.eq_ignore_ascii_case("Z.ai")
}

/// Transform messages for GLM compatibility
/// GLM doesn't support multimodal content array format, convert to string
fn transform_messages_for_glm(payload: &mut Value) {
    if let Some(messages) = payload.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for message in messages.iter_mut() {
            if let Some(content) = message.get_mut("content") {
                // If content is an array (multimodal format), convert to string
                if let Some(content_array) = content.as_array() {
                    let text_content: String = content_array
                        .iter()
                        .filter_map(|item| {
                            // Only extract text content, ignore images for GLM
                            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    *content = Value::String(text_content);
                }
            }
        }
    }
}

fn upstream_style(ctx: &ForwardContext) -> Provider {
    ctx.upstream
        .api_style
        .as_deref()
        .and_then(Provider::from_str)
        .unwrap_or(Provider::OpenAI)
}

fn with_provider(ctx: &ForwardContext, provider: Provider) -> ForwardContext {
    let mut next = ctx.clone();
    next.model.provider = provider;
    next
}

fn estimate_openai_prompt_tokens(payload: &Value) -> i64 {
    let messages = payload
        .get("messages")
        .map(|m| m.to_string())
        .unwrap_or_default();
    estimate_tokens(&messages)
}

async fn handle_openai_to_anthropic_request(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<UpstreamResponse> {
    let start = Instant::now();
    let upstream_ctx = with_provider(&ctx, Provider::Anthropic);

    logger::info(
        "openai",
        &format!(
            "Request started: model={}, upstream={}, streaming=false (anthropic)",
            ctx.model.id, ctx.upstream.id
        ),
    );

    let handler = anthropic::AnthropicHandler;
    let mut headers = handler.build_headers(&upstream_ctx);
    headers.insert("accept", HeaderValue::from_static("application/json"));

    let mut body =
        anthropic::convert_openai_to_anthropic_request(&payload, ctx.model.upstream_model());
    client::normalize_stream_flag(&mut body);

    let config = ctx.retry_config();
    let client = client::default_client()?;
    let result = client::send_with_retry(
        &client,
        upstream_ctx.all_endpoints(),
        "/v1/messages",
        headers,
        &body,
        &config,
    )
    .await?;

    let status = result.response.status();
    let status_code = status.as_u16();
    let response_text = result.response.text().await.map_err(|e| {
        logger::error("openai", &format!("Failed to read response body: {}", e));
        ForwardError::RequestFailed(format!("Failed to read response: {}", e))
    })?;
    let response_body: Value = client::parse_json_response(&response_text).map_err(|e| {
        logger::error(
            "openai",
            &format!(
                "Failed to parse response JSON: {}, body: {}",
                e,
                &response_text[..response_text.len().min(500)]
            ),
        );
        ForwardError::RequestFailed(format!("Failed to parse response: {}", e))
    })?;

    if !status.is_success() {
        logger::warn(
            "openai",
            &format!("Request failed: status={}, response={}", status_code, response_body),
        );
        return Err(ForwardError::RequestFailed(format!(
            "Upstream returned {}: {}",
            status_code,
            response_body.to_string()
        )));
    }

    let openai_body =
        anthropic::convert_anthropic_response_to_openai(&response_body, ctx.model.upstream_model());
    let mut usage = extract_usage(&openai_body);
    if usage.prompt_tokens == 0 {
        usage.prompt_tokens = estimate_openai_prompt_tokens(&payload);
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    ctx.log_usage(&usage);

    Ok(UpstreamResponse {
        body: openai_body,
        latency_ms,
        status: status_code,
        usage,
    })
}

async fn handle_openai_to_gemini_request(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<UpstreamResponse> {
    let start = Instant::now();
    let upstream_ctx = with_provider(&ctx, Provider::Gemini);

    logger::info(
        "openai",
        &format!(
            "Request started: model={}, upstream={}, streaming=false (gemini)",
            ctx.model.id, ctx.upstream.id
        ),
    );

    let handler = gemini::GeminiHandler;
    let headers = handler.build_headers(&upstream_ctx);
    let body = gemini::convert_openai_to_gemini_request(&payload, ctx.model.upstream_model());
    let config = ctx.retry_config();
    let client = client::default_client()?;

    let path = format!(
        "/{}/models/{}:generateContent",
        upstream_ctx.gemini_version(),
        ctx.model.upstream_model()
    );
    let endpoints = gemini::build_gemini_endpoints(&upstream_ctx, &path);

    let result = client::send_with_retry(&client, &endpoints, "", headers, &body, &config).await?;

    let status = result.response.status();
    let status_code = status.as_u16();
    let response_body: Value = result
        .response
        .json()
        .await
        .map_err(|e| ForwardError::RequestFailed(format!("Failed to parse response: {}", e)))?;

    if !status.is_success() {
        return Err(ForwardError::RequestFailed(format!(
            "Upstream returned {}: {}",
            status_code,
            response_body.to_string()
        )));
    }

    if let Some(block_reason) = response_body
        .get("promptFeedback")
        .and_then(|pf| pf.get("blockReason"))
    {
        return Err(ForwardError::RequestFailed(format!(
            "Content blocked: {:?}",
            block_reason
        )));
    }

    let openai_body =
        gemini::convert_gemini_response_to_openai(&response_body, ctx.model.upstream_model());
    let mut usage = extract_usage(&openai_body);
    if usage.prompt_tokens == 0 {
        usage.prompt_tokens = estimate_openai_prompt_tokens(&payload);
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    ctx.log_usage(&usage);

    Ok(UpstreamResponse {
        body: openai_body,
        latency_ms,
        status: status_code,
        usage,
    })
}

async fn handle_openai_to_anthropic_stream(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<Response> {
    let upstream_ctx = with_provider(&ctx, Provider::Anthropic);
    let mut body =
        anthropic::convert_openai_to_anthropic_request(&payload, ctx.model.upstream_model());
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), Value::Bool(true));
    }

    let handler = anthropic::AnthropicHandler;
    let mut headers = handler.build_headers(&upstream_ctx);
    headers.insert("accept", HeaderValue::from_static("text/event-stream"));
    headers.insert("accept-encoding", HeaderValue::from_static("identity"));

    let client = client::streaming_client()?;
    let endpoint = upstream_ctx.primary_endpoint().ok_or_else(|| {
        ForwardError::UpstreamNotFound("No endpoints configured".to_string())
    })?;
    let url = format!("{}/v1/messages", endpoint.trim_end_matches('/'));

    let response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            logger::error("openai", &format!("Stream request failed: {}", e));
            ForwardError::RequestFailed(e.to_string())
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        logger::error(
            "openai",
            &format!("Anthropic stream error: status={}, body={}", status, text),
        );
        return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
    }

    let estimated_prompt_tokens = estimate_openai_prompt_tokens(&payload);
    let state = Arc::new(Mutex::new({
        let mut s = anthropic::AnthropicToOpenAIStreamState::new(ctx.model.upstream_model());
        s.prompt_tokens = estimated_prompt_tokens;
        s
    }));
    let state_clone = Arc::clone(&state);
    let line_buffer = Arc::new(Mutex::new(Vec::new()));
    let line_buffer_clone = Arc::clone(&line_buffer);

    let stream = response.bytes_stream().map(move |result| match result {
        Ok(bytes) => {
            let lines = {
                let mut buffer = line_buffer_clone.lock().unwrap();
                drain_sse_lines(&mut buffer, bytes.as_ref())
            };

            let mut output = Vec::new();
            for line in lines {
                if let Some(data) = parse_sse_data(&line) {
                    if is_sse_done(data) {
                        continue;
                    }
                    match serde_json::from_str::<Value>(data) {
                        Ok(json) => {
                            if let Ok(mut state) = state_clone.lock() {
                                let chunks =
                                    anthropic::convert_anthropic_event_to_openai_chunks(
                                        &json, &mut state,
                                    );
                                for chunk in chunks {
                                    output.extend_from_slice(
                                        format!("data: {}\n\n", chunk).as_bytes(),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            logger::error(
                                "openai",
                                &format!("Failed to parse Anthropic SSE JSON: {}", e),
                            );
                        }
                    }
                }
            }
            Ok(Bytes::from(output))
        }
        Err(e) => {
            logger::error("openai", &format!("Stream bytes error: {}", e));
            Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    });

    let ctx_for_log = ctx.clone();
    let state_for_log = Arc::clone(&state);

    let logged_stream = stream
        .chain(futures_util::stream::once(async move {
            if let Ok(state) = state_for_log.lock() {
                let usage = TokenUsage::new(state.prompt_tokens, state.completion_tokens);
                ctx_for_log.log_usage(&usage);
            }
            Ok(Bytes::from("data: [DONE]\n\n"))
        }))
        .filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                Err(e) => {
                    logger::error("openai", &format!("Stream filter error: {}", e));
                    None
                }
            }
        });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from_stream(logged_stream))
        .unwrap_or_else(|e| {
            logger::error("openai", &format!("Failed to build stream response: {}", e));
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
}

async fn handle_openai_to_gemini_stream(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<Response> {
    let upstream_ctx = with_provider(&ctx, Provider::Gemini);
    let body = gemini::convert_openai_to_gemini_request(&payload, ctx.model.upstream_model());

    let handler = gemini::GeminiHandler;
    let headers = handler.build_headers(&upstream_ctx);
    let client = client::streaming_client()?;
    let url = gemini::build_gemini_stream_url(&upstream_ctx, ctx.model.upstream_model())
        .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;

    let response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            logger::error("openai", &format!("Gemini stream request failed: {}", e));
            ForwardError::RequestFailed(e.to_string())
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        logger::error(
            "openai",
            &format!("Gemini stream error: status={}, body={}", status, text),
        );
        return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
    }

    let estimated_prompt_tokens = estimate_openai_prompt_tokens(&payload);
    let state = Arc::new(Mutex::new({
        let mut s = gemini::GeminiToOpenAIStreamState::new(ctx.model.upstream_model());
        s.prompt_tokens = estimated_prompt_tokens;
        s
    }));
    let state_clone = Arc::clone(&state);
    let line_buffer = Arc::new(Mutex::new(Vec::new()));
    let line_buffer_clone = Arc::clone(&line_buffer);

    let stream = response.bytes_stream().map(move |result| match result {
        Ok(bytes) => {
            let lines = {
                let mut buffer = line_buffer_clone.lock().unwrap();
                drain_sse_lines(&mut buffer, bytes.as_ref())
            };

            let mut output = Vec::new();
            for line in lines {
                if let Some(data) = parse_sse_data(&line) {
                    if is_sse_done(data) {
                        continue;
                    }
                    match serde_json::from_str::<Value>(data) {
                        Ok(json) => {
                            if let Ok(mut state) = state_clone.lock() {
                                let chunks =
                                    gemini::convert_gemini_event_to_openai_chunks(
                                        &json, &mut state,
                                    );
                                for chunk in chunks {
                                    output.extend_from_slice(
                                        format!("data: {}\n\n", chunk).as_bytes(),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            logger::error(
                                "openai",
                                &format!("Failed to parse Gemini SSE JSON: {}", e),
                            );
                        }
                    }
                }
            }
            Ok(Bytes::from(output))
        }
        Err(e) => {
            logger::error("openai", &format!("Stream bytes error: {}", e));
            Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    });

    let ctx_for_log = ctx.clone();
    let state_for_log = Arc::clone(&state);

    let logged_stream = stream
        .chain(futures_util::stream::once(async move {
            if let Ok(state) = state_for_log.lock() {
                let usage = TokenUsage::new(state.prompt_tokens, state.completion_tokens);
                ctx_for_log.log_usage(&usage);
            }
            Ok(Bytes::from("data: [DONE]\n\n"))
        }))
        .filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                Err(e) => {
                    logger::error("openai", &format!("Stream filter error: {}", e));
                    None
                }
            }
        });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from_stream(logged_stream))
        .unwrap_or_else(|e| {
            logger::error("openai", &format!("Failed to build stream response: {}", e));
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_payload() {
        let payload = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 100,
            "custom_field": "should_be_removed"
        });

        let filtered = filter_payload(&payload, ALLOWED_FIELDS);
        let obj = filtered.as_object().unwrap();

        assert!(obj.contains_key("model"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("max_tokens"));
        assert!(!obj.contains_key("custom_field"));
    }

    #[test]
    fn test_extract_usage() {
        let response = serde_json::json!({
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        });

        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_extract_usage_missing() {
        let response = serde_json::json!({});
        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
    }

    #[test]
    fn test_multimodal_message_format() {
        // Test that OpenAI multimodal messages (vision) are preserved correctly
        let payload = serde_json::json!({
            "model": "gpt-4-vision-preview",
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What's in this image?"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/jpeg;base64,/9j/4AAQSkZJRg...",
                            "detail": "high"
                        }
                    }
                ]
            }],
            "max_tokens": 1000
        });

        let filtered = filter_payload(&payload, ALLOWED_FIELDS);
        let messages = filtered.get("messages").unwrap().as_array().unwrap();
        let content = messages[0].get("content").unwrap().as_array().unwrap();

        assert_eq!(content.len(), 2);
        assert_eq!(content[0].get("type").unwrap(), "text");
        assert_eq!(content[1].get("type").unwrap(), "image_url");

        // Verify image_url structure is preserved
        let image_url = content[1].get("image_url").unwrap();
        assert!(image_url.get("url").is_some());
        assert_eq!(image_url.get("detail").unwrap(), "high");
    }

    #[test]
    fn test_multimodal_url_image() {
        // Test with URL-based image
        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Describe this image"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "https://example.com/image.jpg"
                        }
                    }
                ]
            }],
            "max_tokens": 500
        });

        let filtered = filter_payload(&payload, ALLOWED_FIELDS);
        let messages = filtered.get("messages").unwrap().as_array().unwrap();
        let content = messages[0].get("content").unwrap().as_array().unwrap();

        assert_eq!(content.len(), 2);
        let image_url = content[1].get("image_url").unwrap();
        assert_eq!(
            image_url.get("url").unwrap(),
            "https://example.com/image.jpg"
        );
    }
}
