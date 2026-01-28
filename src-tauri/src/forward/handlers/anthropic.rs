//! Anthropic (Claude) API handler
//!
//! Handles forwarding requests to Anthropic's Messages API.
//! Supports streaming, multimodal (images), tool use, and extended thinking.

use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::forward::client::{self, drain_sse_lines, is_sse_done, parse_sse_data};
use crate::forward::context::{estimate_tokens, ForwardContext, Provider, TokenUsage, UpstreamResponse};
use crate::forward::error::{ForwardError, ForwardResult};
use crate::logger;

use super::{gemini, ProviderHandlerImpl};

/// Allowed fields for Anthropic Messages API
const ALLOWED_FIELDS: &[&str] = &[
    "model",
    "messages",
    "max_tokens",
    "stream",
    "system",
    "temperature",
    "top_p",
    "top_k",
    "stop_sequences",
    "metadata",
    "tools",
    "tool_choice",
    "thinking",
    "betas",
];

/// Anthropic API version header value
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API handler
pub struct AnthropicHandler;

fn upstream_style(ctx: &ForwardContext) -> Provider {
    ctx.upstream
        .api_style
        .as_deref()
        .and_then(Provider::from_str)
        .unwrap_or(Provider::Anthropic)
}

fn with_provider(ctx: &ForwardContext, provider: Provider) -> ForwardContext {
    let mut next = ctx.clone();
    next.model.provider = provider;
    next
}

/// Detect if upstream uses OpenAI-style API (explicit config only)
fn is_openai_style(ctx: &ForwardContext) -> bool {
    matches!(upstream_style(ctx), Provider::OpenAI)
}

fn parse_boolish(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(num) => num.as_i64().map(|v| v != 0),
        Value::String(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            if matches!(normalized.as_str(), "true" | "1" | "yes" | "on" | "enabled") {
                Some(true)
            } else if matches!(
                normalized.as_str(),
                "false" | "0" | "no" | "off" | "disabled" | "none"
            ) {
                Some(false)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_thinking_enabled(payload: &Value) -> bool {
    let Some(thinking) = payload.get("thinking") else {
        return true;
    };

    if let Some(flag) = parse_boolish(thinking) {
        return flag;
    }

    if let Some(obj) = thinking.as_object() {
        if let Some(flag) = obj.get("enabled").and_then(parse_boolish) {
            return flag;
        }
        if let Some(flag) = obj.get("enable").and_then(parse_boolish) {
            return flag;
        }
        if let Some(kind) = obj.get("type").and_then(|v| v.as_str()) {
            let normalized = kind.trim().to_ascii_lowercase();
            if matches!(normalized.as_str(), "disabled" | "off" | "false" | "none") {
                return false;
            }
            if matches!(normalized.as_str(), "enabled" | "on" | "true") {
                return true;
            }
        }
        if obj.get("budget_tokens").is_some() {
            return true;
        }
    }

    true
}

impl ProviderHandlerImpl for AnthropicHandler {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn build_url(&self, ctx: &ForwardContext, path: &str) -> String {
        let endpoint = ctx
            .primary_endpoint()
            .unwrap_or("https://api.anthropic.com");
        format!("{}{}", endpoint.trim_end_matches('/'), path)
    }

    fn build_headers(&self, ctx: &ForwardContext) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Get API key
        if let Some(api_key) = ctx.get_api_key() {
            // Check if upstream uses OpenAI-compatible API style
            let use_bearer = is_openai_style(ctx);

            if use_bearer {
                // OpenAI-compatible: use Authorization Bearer
                if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", api_key)) {
                    headers.insert("authorization", value);
                }
            } else {
                // Native Anthropic: use x-api-key
                if let Ok(value) = HeaderValue::from_str(&api_key) {
                    headers.insert("x-api-key", value);
                }
            }
        }

        // Anthropic-specific headers
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        headers
    }

    fn transform_request(&self, ctx: &ForwardContext, payload: &Value) -> Value {
        // Check if upstream uses OpenAI-compatible API
        let is_openai_style = is_openai_style(ctx);

        if is_openai_style {
            // Convert Anthropic format to OpenAI format
            logger::info(
                "anthropic",
                &format!(
                    "Converting Anthropic request to OpenAI format for upstream={}",
                    ctx.upstream.id
                ),
            );
            let mut converted = convert_anthropic_to_openai(payload, &ctx.model.upstream_model());
            client::normalize_stream_flag(&mut converted);
            converted
        } else {
            // Native Anthropic format
            let mut filtered = filter_payload(payload, ALLOWED_FIELDS);

            // Replace model with upstream model name
            if let Some(obj) = filtered.as_object_mut() {
                obj.insert(
                    "model".to_string(),
                    Value::String(ctx.model.upstream_model().to_string()),
                );
            }

            // Log the transformed request
            logger::debug(
                "anthropic",
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
    }

    fn parse_response(&self, response: &Value) -> TokenUsage {
        extract_usage(response)
    }

    fn estimate_request_tokens(&self, payload: &Value) -> i64 {
        let messages = payload
            .get("messages")
            .map(|m| m.to_string())
            .unwrap_or_default();
        let system = payload
            .get("system")
            .map(|s| s.to_string())
            .unwrap_or_default();
        estimate_tokens(&format!("{}{}", system, messages))
    }

    async fn handle_request(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> ForwardResult<UpstreamResponse> {
        let start = Instant::now();
        let upstream_style = upstream_style(&ctx);
        let thinking_enabled = is_thinking_enabled(&payload);

        if matches!(upstream_style, Provider::Gemini) {
            return handle_anthropic_to_gemini_request(ctx, payload, thinking_enabled).await;
        }

        let is_openai_style = matches!(upstream_style, Provider::OpenAI);

        logger::info(
            "anthropic",
            &format!(
                "[DEBUG] handle_request called: model={}, upstream={}, provider={:?}",
                ctx.model.id,
                ctx.upstream.id,
                ctx.model.provider
            ),
        );

        // Build request
        let mut headers = self.build_headers(&ctx);
        headers.insert("accept", HeaderValue::from_static("text/event-stream"));
        headers.insert("accept-encoding", HeaderValue::from_static("identity"));
        let body = self.transform_request(&ctx, &payload);
        let config = ctx.retry_config();
        let client = client::default_client()?;

        // Log the request URL
        let endpoint = ctx.primary_endpoint().unwrap_or("unknown");
        let path = if is_openai_style {
            "/chat/completions"
        } else {
            "/v1/messages"
        };
        let full_url = format!("{}{}", endpoint.trim_end_matches('/'), path);
        logger::debug(
            "anthropic",
            &format!("Request URL: {}", full_url),
        );

        // Send request with retry
        let result = client::send_with_retry(
            &client,
            ctx.all_endpoints(),
            path,
            headers,
            &body,
            &config,
        )
        .await?;

        // Parse response
        let status = result.response.status();
        let status_code = status.as_u16();
        let response_text = result.response.text().await.map_err(|e| {
            logger::error("anthropic", &format!("Failed to read response body: {}", e));
            ForwardError::RequestFailed(format!("Failed to read response: {}", e))
        })?;

        // Log response for debugging empty responses
        if response_text.is_empty() {
            logger::warn("anthropic", "Received empty response body from upstream");
        }

        let response_body: Value = client::parse_json_response(&response_text).map_err(|e| {
            logger::error("anthropic", &format!("Failed to parse response JSON: {}, body: {}", e, &response_text[..response_text.len().min(500)]));
            ForwardError::RequestFailed(format!("Failed to parse response: {}", e))
        })?;

        // Check if response indicates an error
        if !status.is_success() {
            logger::warn(
                "anthropic",
                &format!("Request failed: status={}, response={}", status_code, response_body),
            );
            // Don't log usage for failed requests
            return Err(ForwardError::RequestFailed(format!(
                "Upstream returned {}: {}",
                status_code,
                response_body.to_string()
            )));
        }

        // Runtime format detection: Check if response is OpenAI format
        // This handles cases where upstream is misconfigured
        let response_object = response_body.get("object")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let is_openai_response = response_object == "chat.completion" || response_object == "chat.completion.chunk";

        // Log if we detect format mismatch
        if is_openai_response && !is_openai_style {
            logger::warn(
                "anthropic",
                &format!(
                    "RUNTIME: Detected OpenAI format from upstream '{}', converting. Consider setting 'api_style = \"openai\"'.",
                    ctx.upstream.id
                ),
            );
        }

        // Extract usage based on actual response format
        let mut usage = if is_openai_response {
            extract_openai_usage(&response_body)
        } else {
            extract_usage(&response_body)
        };
        if usage.prompt_tokens == 0 {
            // Estimate if not provided
            usage.prompt_tokens = self.estimate_request_tokens(&payload);
        }

        let latency_ms = start.elapsed().as_millis() as u64;

        logger::info(
            "anthropic",
            &format!(
                "Request completed: model={}, latency={}ms, tokens={}/{}",
                ctx.model.id,
                latency_ms,
                usage.prompt_tokens,
                usage.completion_tokens
            ),
        );

        // Convert response based on actual format, not just configuration
        // DEBUG: Force conversion for all responses to test
        let response_body = if is_openai_response {
            convert_openai_response_to_anthropic(
                &response_body,
                ctx.model.upstream_model(),
                thinking_enabled,
            )
        } else if response_body.get("choices").is_some() {
            // Force conversion if response has 'choices' (likely OpenAI format)
            logger::warn("anthropic", "Force converting response with 'choices' key");
            convert_openai_response_to_anthropic(
                &response_body,
                ctx.model.upstream_model(),
                thinking_enabled,
            )
        } else {
            response_body
        };

        // Log usage to database only for successful requests
        ctx.log_usage(&usage);

        // DEBUG: Add marker to confirm this code is executed
        let mut debug_body = response_body;
        if let Some(obj) = debug_body.as_object_mut() {
            obj.insert("_debug_marker".to_string(), Value::String("handle_request_executed".to_string()));
        }

        Ok(UpstreamResponse {
            body: debug_body,
            latency_ms,
            status: status_code,
            usage,
        })
    }

    async fn handle_stream(&self, ctx: ForwardContext, payload: Value) -> ForwardResult<Response> {
        let upstream_style = upstream_style(&ctx);
        let thinking_enabled = is_thinking_enabled(&payload);

        if matches!(upstream_style, Provider::Gemini) {
            return handle_anthropic_to_gemini_stream(ctx, payload, thinking_enabled).await;
        }

        let openai_style = matches!(upstream_style, Provider::OpenAI);
        if openai_style {
            // Use OpenAI-style streaming with format conversion
            logger::info(
                "anthropic",
                &format!(
                    "Detected OpenAI-style upstream, using conversion handler: upstream={}",
                    ctx.upstream.id
                ),
            );
            return self
                .handle_openai_style_stream(ctx, payload, thinking_enabled)
                .await;
        }

        // Native Anthropic streaming
        // Build request
        let mut headers = self.build_headers(&ctx);
        headers.insert("accept", HeaderValue::from_static("text/event-stream"));
        headers.insert("accept-encoding", HeaderValue::from_static("identity"));
        let mut body = self.transform_request(&ctx, &payload);

        // Ensure stream is enabled
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(true));
        }

        let client = client::streaming_client()?;
        let endpoint = ctx
            .primary_endpoint()
            .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;
        let url = format!("{}/v1/messages", endpoint.trim_end_matches('/'));

        logger::info(
            "anthropic",
            &format!(
                "Starting native Anthropic stream: model={}, upstream={}",
                ctx.model.id, ctx.upstream.id
            ),
        );

        // Make request
        let response = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                logger::error(
                    "anthropic",
                    &format!("Stream request failed: url={}, error={}", url, e),
                );
                ForwardError::RequestFailed(e.to_string())
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            logger::error(
                "anthropic",
                &format!(
                    "Stream request returned error: status={}, body={}",
                    status,
                    &text[..text.len().min(500)]
                ),
            );
            return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
        }

        // Clone context for use in stream processing
        let ctx_clone = ctx.clone();
        let estimated_prompt_tokens = self.estimate_request_tokens(&payload);

        // Create usage tracker for accumulating streaming usage
        // Anthropic returns input_tokens in message_start and output_tokens in message_delta
        let usage_tracker = Arc::new(Mutex::new(TokenUsage::new(estimated_prompt_tokens, 0)));
        let usage_tracker_clone = Arc::clone(&usage_tracker);
        let line_buffer = Arc::new(Mutex::new(Vec::new()));
        let line_buffer_clone = Arc::clone(&line_buffer);

        // Track if we detected OpenAI format and need to do runtime conversion
        let openai_format_detected = Arc::new(Mutex::new(false));
        let openai_format_clone = Arc::clone(&openai_format_detected);
        let model_id_for_log = Arc::new(ctx.model.id.clone());
        let upstream_id_for_log = Arc::new(ctx.upstream.id.clone());

        // Track conversion state for OpenAI format
        let openai_first_chunk = Arc::new(Mutex::new(true));
        let openai_first_clone = Arc::clone(&openai_first_chunk);
        let openai_stop_sent = Arc::new(Mutex::new(false));
        let openai_stop_clone = Arc::clone(&openai_stop_sent);
        let openai_block_state = Arc::new(Mutex::new(OpenAIStreamBlockState::default()));
        let openai_block_clone = Arc::clone(&openai_block_state);
        let thinking_enabled = Arc::new(thinking_enabled);
        let thinking_enabled_clone = Arc::clone(&thinking_enabled);

        // Stream the response and parse SSE events
        // We support both native Anthropic format and runtime OpenAI format conversion
        let stream = response
            .bytes_stream()
            .then(move |result| {
                let line_buffer = Arc::clone(&line_buffer_clone);
                let usage_tracker = Arc::clone(&usage_tracker_clone);
                let openai_detected = Arc::clone(&openai_format_clone);
                let openai_first = Arc::clone(&openai_first_clone);
                let openai_stop = Arc::clone(&openai_stop_clone);
                let openai_blocks = Arc::clone(&openai_block_clone);
                let thinking_enabled = Arc::clone(&thinking_enabled_clone);
                let model_id = Arc::clone(&model_id_for_log);
                let upstream_id = Arc::clone(&upstream_id_for_log);
                async move {
                    let mut output_chunks: Vec<Result<Bytes, std::io::Error>> = Vec::new();

                    match result {
                        Ok(bytes) => {
                            let lines = {
                                let mut buffer = line_buffer.lock().unwrap();
                                drain_sse_lines(&mut buffer, bytes.as_ref())
                            };

                            let mut event_chunks = Vec::new();

                            for line in lines {
                                if let Some(data) = parse_sse_data(&line) {
                                    // Check for [DONE] marker
                                    if data.trim() == "[DONE]" {
                                        let mut detected = openai_detected.lock().unwrap();
                                        if !*detected {
                                            *detected = true;
                                            logger::warn(
                                                "anthropic",
                                                &format!(
                                                    "RUNTIME: Detected OpenAI format from upstream '{}' via [DONE] sentinel, enabling runtime conversion. Model: '{}'. Consider setting 'api_style = \"openai\"' in upstream configuration for better performance.",
                                                    upstream_id, model_id
                                                ),
                                            );
                                        }
                                        drop(detected);

                                        let should_send_stop = {
                                            let mut stop_sent = openai_stop.lock().unwrap();
                                            if *stop_sent {
                                                false
                                            } else {
                                                *stop_sent = true;
                                                true
                                            }
                                        };
                                        if should_send_stop {
                                            let indices = openai_blocks
                                                .lock()
                                                .map(|mut state| state.stop_started())
                                                .unwrap_or_default();
                                            for index in indices {
                                                push_content_block_stop(&mut event_chunks, index);
                                            }
                                            let stop_event = serde_json::json!({
                                                "type": "message_stop"
                                            });
                                            event_chunks.push(Bytes::from(format!(
                                                "event: message_stop\ndata: {}\n\n",
                                                stop_event
                                            )));
                                        }
                                        continue;
                                    }

                                    // Try to parse as JSON
                                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                                        // Check if this is OpenAI format
                                        let is_openai = json
                                            .get("object")
                                            .and_then(|v| v.as_str())
                                            == Some("chat.completion.chunk")
                                            || json.get("choices").and_then(|v| v.as_array()).is_some();

                                        if is_openai {
                                            // Runtime OpenAI format conversion
                                            let mut detected = openai_detected.lock().unwrap();
                                            if !*detected {
                                                *detected = true;
                                                logger::warn(
                                                    "anthropic",
                                                    &format!(
                                                        "RUNTIME: Detected OpenAI format from upstream '{}', enabling runtime conversion. Model: '{}'. Consider setting 'api_style = \"openai\"' in upstream configuration for better performance.",
                                                        upstream_id, model_id
                                                    ),
                                                );
                                            }
                                            drop(detected);

                                            // Handle OpenAI format conversion
                                            let mut first = openai_first.lock().unwrap();
                                            let is_first = *first;
                                            if is_first {
                                                *first = false;
                                                drop(first);

                                                // Send message_start event
                                                let id_raw = json
                                                    .get("id")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("msg_unknown");
                                                let id = if id_raw.starts_with("msg_") {
                                                    id_raw.to_string()
                                                } else {
                                                    format!("msg_{}", id_raw)
                                                };
                                                let input_tokens = json
                                                    .get("usage")
                                                    .and_then(|v| v.get("prompt_tokens"))
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(estimated_prompt_tokens);
                                                let start_event = serde_json::json!({
                                                    "type": "message_start",
                                                    "message": {
                                                        "id": id,
                                                        "type": "message",
                                                        "role": "assistant",
                                                        "content": [],
                                                        "model": json.get("model").unwrap_or(&Value::String("unknown".to_string())),
                                                        "stop_reason": null,
                                                        "stop_sequence": null,
                                                        "usage": {
                                                            "input_tokens": input_tokens,
                                                            "output_tokens": 0
                                                        }
                                                    }
                                                });
                                                event_chunks.push(Bytes::from(format!(
                                                    "event: message_start\ndata: {}\n\n",
                                                    start_event
                                                )));
                                            } else {
                                                drop(first);
                                            }

                                            // Process OpenAI choices
                                            if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                                                if let Some(choice) = choices.first() {
                                                    if let Some(delta) = choice.get("delta") {
                                                        let mut reasoning = delta
                                                            .get("reasoning_content")
                                                            .and_then(|r| r.as_str())
                                                            .unwrap_or("");
                                                        let content = delta
                                                            .get("content")
                                                            .and_then(|c| c.as_str())
                                                            .unwrap_or("");
                                                        if !*thinking_enabled {
                                                            reasoning = "";
                                                        }

                                                        if !reasoning.is_empty() || !content.is_empty() {
                                                            let mut thinking_info = None;
                                                            let mut stop_thinking = None;
                                                            let mut text_info = None;
                                                            if let Ok(mut state) = openai_blocks.lock() {
                                                                let mut allow_reasoning = !reasoning.is_empty();
                                                                if allow_reasoning
                                                                    && state.text_index.is_some()
                                                                    && state.thinking_index.is_none()
                                                                {
                                                                    allow_reasoning = false;
                                                                }
                                                                if allow_reasoning {
                                                                    let (index, started) =
                                                                        state.ensure_thinking();
                                                                    thinking_info = Some((index, started));
                                                                }
                                                                if !content.is_empty() {
                                                                    if state.thinking_index.is_some()
                                                                        && state.text_index.is_none()
                                                                    {
                                                                        stop_thinking = state.close_thinking();
                                                                    }
                                                                    let (index, started) =
                                                                        state.ensure_text();
                                                                    text_info = Some((index, started));
                                                                }
                                                            }

                                                            if let Some((index, started)) = thinking_info {
                                                                if started {
                                                                    push_content_block_start(
                                                                        &mut event_chunks,
                                                                        index,
                                                                        "thinking",
                                                                    );
                                                                }
                                                                push_content_block_delta(
                                                                    &mut event_chunks,
                                                                    index,
                                                                    "thinking",
                                                                    reasoning,
                                                                );
                                                                let tokens = estimate_tokens(reasoning);
                                                                if let Ok(mut tracker) = usage_tracker.lock() {
                                                                    tracker.completion_tokens += tokens;
                                                                }
                                                            }

                                                            if let Some(index) = stop_thinking {
                                                                push_content_block_stop(
                                                                    &mut event_chunks,
                                                                    index,
                                                                );
                                                            }

                                                            if let Some((index, started)) = text_info {
                                                                if started {
                                                                    push_content_block_start(
                                                                        &mut event_chunks,
                                                                        index,
                                                                        "text",
                                                                    );
                                                                }
                                                                push_content_block_delta(
                                                                    &mut event_chunks,
                                                                    index,
                                                                    "text",
                                                                    content,
                                                                );
                                                                let tokens = estimate_tokens(content);
                                                                if let Ok(mut tracker) = usage_tracker.lock() {
                                                                    tracker.completion_tokens += tokens;
                                                                }
                                                            }
                                                        }

                                                        // Check for finish_reason
                                                        if let Some(finish_reason) = choice.get("finish_reason") {
                                                            if !finish_reason.is_null() {
                                                                // Send message_delta event
                                                                let output_tokens = usage_tracker
                                                                    .lock()
                                                                    .map(|tracker| tracker.completion_tokens)
                                                                    .unwrap_or(0);
                                                                let stop_reason = map_openai_finish_reason(
                                                                    finish_reason.as_str(),
                                                                );
                                                                let indices = openai_blocks
                                                                    .lock()
                                                                    .map(|mut state| state.stop_started())
                                                                    .unwrap_or_default();
                                                                for index in indices {
                                                                    push_content_block_stop(
                                                                        &mut event_chunks,
                                                                        index,
                                                                    );
                                                                }
                                                                let delta_event = serde_json::json!({
                                                                    "type": "message_delta",
                                                                    "delta": {
                                                                        "stop_reason": stop_reason,
                                                                        "stop_sequence": null
                                                                    },
                                                                    "usage": {
                                                                        "output_tokens": output_tokens
                                                                    }
                                                                });
                                                                event_chunks.push(Bytes::from(format!(
                                                                    "event: message_delta\ndata: {}\n\n",
                                                                    delta_event
                                                                )));
                                                            }
                                                        }
                                                    }

                                                    // Track usage from chunk
                                                    if let Some(usage) = json.get("usage") {
                                                        if let Ok(mut tracker) = usage_tracker.lock() {
                                                            if let Some(prompt_tokens) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
                                                                tracker.prompt_tokens = prompt_tokens;
                                                            }
                                                            if let Some(completion_tokens) = usage.get("completion_tokens").and_then(|v| v.as_i64()) {
                                                                tracker.completion_tokens = completion_tokens;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // Native Anthropic format processing
                                            let event_type = json.get("type").and_then(|t| t.as_str());

                                            match event_type {
                                                Some("message_start") => {
                                                    if let Some(message) = json.get("message") {
                                                        if let Some(usage) = message.get("usage") {
                                                            let input_tokens = usage
                                                                .get("input_tokens")
                                                                .and_then(|v| v.as_i64())
                                                                .unwrap_or(0);
                                                            if let Ok(mut tracker) = usage_tracker.lock() {
                                                                tracker.prompt_tokens = input_tokens;
                                                            }
                                                        }
                                                    }
                                                }
                                                Some("message_delta") => {
                                                    if let Some(usage) = json.get("usage") {
                                                        let output_tokens = usage
                                                            .get("output_tokens")
                                                            .and_then(|v| v.as_i64())
                                                            .unwrap_or(0);
                                                        if let Ok(mut tracker) = usage_tracker.lock() {
                                                            tracker.completion_tokens = output_tokens;
                                                        }
                                                    }
                                                }
                                                Some("content_block_delta") => {
                                                    if let Some(delta) = json.get("delta") {
                                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                            let tokens = estimate_tokens(text);
                                                            if let Ok(mut tracker) = usage_tracker.lock() {
                                                                if tracker.completion_tokens == 0 {
                                                                    tracker.completion_tokens += tokens;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }

                            if !event_chunks.is_empty() {
                                output_chunks.extend(event_chunks.into_iter().map(Ok));
                            } else {
                                // Pass through original bytes for native Anthropic format only
                                let openai_active =
                                    openai_detected.lock().map(|v| *v).unwrap_or(false);
                                if !openai_active {
                                    output_chunks.push(Ok(bytes));
                                }
                            }
                        }
                        Err(e) => {
                            logger::error("anthropic", &format!("Stream bytes error: {}", e));
                            output_chunks.push(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            )));
                        }
                    }

                    output_chunks
                }
            })
            .flat_map(futures_util::stream::iter);
        // Create a wrapper stream that logs usage when done
        let ctx_for_log = ctx_clone;
        let usage_for_log = Arc::clone(&usage_tracker);
        let model_id = ctx.model.id.clone();

        let logged_stream = stream
            .chain(futures_util::stream::once(async move {
                // Log final usage when stream completes
                if let Ok(usage) = usage_for_log.lock() {
                    logger::info(
                        "anthropic",
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
                        "anthropic",
                        &format!("Failed to acquire usage tracker lock for model={}", model_id),
                    );
                }
                Err(std::io::Error::new(std::io::ErrorKind::Other, "stream_end"))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                    Err(e) if e.to_string() == "stream_end" => None,
                    Err(e) => {
                        logger::error(
                            "anthropic",
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
                    "anthropic",
                    &format!("Failed to build stream response: {}", e),
                );
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }))
    }
}

impl AnthropicHandler {
    /// Handle streaming for OpenAI-style upstreams (like GLM)
    async fn handle_openai_style_stream(
        &self,
        ctx: ForwardContext,
        payload: Value,
        thinking_enabled: bool,
    ) -> ForwardResult<Response> {
        // Build request
        let headers = self.build_headers(&ctx);
        let mut body = self.transform_request(&ctx, &payload);

        // Ensure stream is enabled
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(true));
        }

        let client = client::streaming_client()?;
        let endpoint = ctx
            .primary_endpoint()
            .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;

        // Use OpenAI-style endpoint
        let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

        logger::info(
            "anthropic",
            &format!(
                "Starting OpenAI-style stream (will convert to Anthropic format): model={}, upstream={}, url={}",
                ctx.model.id, ctx.upstream.id, url
            ),
        );

        // Make request
        let response = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                logger::error(
                    "anthropic",
                    &format!("OpenAI-style stream request failed: url={}, error={}", url, e),
                );
                ForwardError::RequestFailed(e.to_string())
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            logger::error(
                "anthropic",
                &format!(
                    "OpenAI-style stream returned error: status={}, body={}",
                    status,
                    &text[..text.len().min(500)]
                ),
            );
            return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
        }

        // Clone context for use in stream processing
        let ctx_clone = ctx.clone();
        let estimated_prompt_tokens = self.estimate_request_tokens(&payload);

        // Create usage tracker
        let usage_tracker = Arc::new(Mutex::new(TokenUsage::new(estimated_prompt_tokens, 0)));
        let usage_tracker_clone = Arc::clone(&usage_tracker);
        let line_buffer = Arc::new(Mutex::new(Vec::new()));
        let line_buffer_clone = Arc::clone(&line_buffer);

        // Track if this is the first chunk
        let is_first_chunk = Arc::new(Mutex::new(true));
        let is_first_clone = Arc::clone(&is_first_chunk);
        let block_state = Arc::new(Mutex::new(OpenAIStreamBlockState::default()));
        let block_state_clone = Arc::clone(&block_state);
        let openai_stop_sent = Arc::new(Mutex::new(false));
        let openai_stop_clone = Arc::clone(&openai_stop_sent);
        let thinking_enabled = Arc::new(thinking_enabled);
        let thinking_enabled_clone = Arc::clone(&thinking_enabled);

        // Track conversion errors for final logging
        let conversion_errors = Arc::new(Mutex::new(0usize));
        let conversion_errors_clone = Arc::clone(&conversion_errors);

        // Stream the response and convert OpenAI format to Anthropic format
        let stream = response
            .bytes_stream()
            .then(move |result| {
                let usage_tracker = Arc::clone(&usage_tracker_clone);
                let is_first = Arc::clone(&is_first_clone);
                let line_buffer = Arc::clone(&line_buffer_clone);
                let errors = Arc::clone(&conversion_errors_clone);
                let block_state = Arc::clone(&block_state_clone);
                let openai_stop = Arc::clone(&openai_stop_clone);
                let thinking_enabled = Arc::clone(&thinking_enabled_clone);

                async move {
                    let mut output_chunks: Vec<Result<Bytes, std::io::Error>> = Vec::new();

                    match result {
                        Ok(bytes) => {
                            let lines = {
                                let mut buffer = line_buffer.lock().unwrap();
                                drain_sse_lines(&mut buffer, bytes.as_ref())
                            };

                            if lines.is_empty() {
                                return output_chunks;
                            }

                            let mut event_chunks = Vec::new();

                            for line in lines {
                                if let Some(data) = parse_sse_data(&line) {
                                    if data.trim() == "[DONE]" {
                                        let should_send_stop = {
                                            let mut stop_sent = openai_stop.lock().unwrap();
                                            if *stop_sent {
                                                false
                                            } else {
                                                *stop_sent = true;
                                                true
                                            }
                                        };
                                        if should_send_stop {
                                            let indices = block_state
                                                .lock()
                                                .map(|mut state| state.stop_started())
                                                .unwrap_or_default();
                                            for index in indices {
                                                push_content_block_stop(&mut event_chunks, index);
                                            }
                                            let stop_event = serde_json::json!({
                                                "type": "message_stop"
                                            });
                                            let sse_line = format!(
                                                "event: message_stop\ndata: {}\n\n",
                                                stop_event
                                            );
                                            event_chunks.push(Bytes::from(sse_line));
                                        }
                                        continue;
                                    }

                                    match serde_json::from_str::<Value>(data) {
                                        Ok(json) => {
                                            // Check if this is the first chunk
                                            let mut first = is_first.lock().unwrap();
                                            let is_first_chunk = *first;
                                            if is_first_chunk {
                                                *first = false;
                                                drop(first);

                                                // Send message_start event
                                                if let Some(anthropic_event) =
                                                    convert_openai_chunk_to_anthropic(
                                                        &json,
                                                        true,
                                                        estimated_prompt_tokens,
                                                    )
                                                {
                                                    let sse_line = format!(
                                                        "event: message_start\ndata: {}\n\n",
                                                        anthropic_event
                                                    );
                                                    event_chunks.push(Bytes::from(sse_line));
                                                }
                                            }

                                            if let Some(choices) =
                                                json.get("choices").and_then(|c| c.as_array())
                                            {
                                                if let Some(choice) = choices.first() {
                                                    if let Some(delta) = choice.get("delta") {
                                                        let mut reasoning = delta
                                                            .get("reasoning_content")
                                                            .and_then(|r| r.as_str())
                                                            .unwrap_or("");
                                                        let content = delta
                                                            .get("content")
                                                            .and_then(|c| c.as_str())
                                                            .unwrap_or("");

                                                        if !*thinking_enabled {
                                                            reasoning = "";
                                                        }

                                                        if !reasoning.is_empty() || !content.is_empty() {
                                                            let mut thinking_info = None;
                                                            let mut stop_thinking = None;
                                                            let mut text_info = None;
                                                            if let Ok(mut state) = block_state.lock() {
                                                                let mut allow_reasoning = !reasoning.is_empty();
                                                                if allow_reasoning
                                                                    && state.text_index.is_some()
                                                                    && state.thinking_index.is_none()
                                                                {
                                                                    allow_reasoning = false;
                                                                }
                                                                if allow_reasoning {
                                                                    let (index, started) =
                                                                        state.ensure_thinking();
                                                                    thinking_info = Some((index, started));
                                                                }
                                                                if !content.is_empty() {
                                                                    if state.thinking_index.is_some()
                                                                        && state.text_index.is_none()
                                                                    {
                                                                        stop_thinking = state.close_thinking();
                                                                    }
                                                                    let (index, started) =
                                                                        state.ensure_text();
                                                                    text_info = Some((index, started));
                                                                }
                                                            }

                                                            if let Some((index, started)) = thinking_info {
                                                                if started {
                                                                    push_content_block_start(
                                                                        &mut event_chunks,
                                                                        index,
                                                                        "thinking",
                                                                    );
                                                                }
                                                                push_content_block_delta(
                                                                    &mut event_chunks,
                                                                    index,
                                                                    "thinking",
                                                                    reasoning,
                                                                );
                                                                let tokens = estimate_tokens(reasoning);
                                                                if let Ok(mut tracker) = usage_tracker.lock() {
                                                                    tracker.completion_tokens += tokens;
                                                                }
                                                            }

                                                            if let Some(index) = stop_thinking {
                                                                push_content_block_stop(
                                                                    &mut event_chunks,
                                                                    index,
                                                                );
                                                            }

                                                            if let Some((index, started)) = text_info {
                                                                if started {
                                                                    push_content_block_start(
                                                                        &mut event_chunks,
                                                                        index,
                                                                        "text",
                                                                    );
                                                                }
                                                                push_content_block_delta(
                                                                    &mut event_chunks,
                                                                    index,
                                                                    "text",
                                                                    content,
                                                                );
                                                                let tokens = estimate_tokens(content);
                                                                if let Ok(mut tracker) = usage_tracker.lock() {
                                                                    tracker.completion_tokens += tokens;
                                                                }
                                                            }
                                                        }
                                                    }

                                                    if let Some(finish_reason) = choice.get("finish_reason") {
                                                        if !finish_reason.is_null() {
                                                            let indices = block_state
                                                                .lock()
                                                                .map(|mut state| state.stop_started())
                                                                .unwrap_or_default();
                                                            for index in indices {
                                                                push_content_block_stop(
                                                                    &mut event_chunks,
                                                                    index,
                                                                );
                                                            }
                                                            let output_tokens = usage_tracker
                                                                .lock()
                                                                .map(|tracker| tracker.completion_tokens)
                                                                .unwrap_or(0);
                                                            let stop_reason = map_openai_finish_reason(
                                                                finish_reason.as_str(),
                                                            );
                                                            let delta_event = serde_json::json!({
                                                                "type": "message_delta",
                                                                "delta": {
                                                                    "stop_reason": stop_reason,
                                                                    "stop_sequence": null
                                                                },
                                                                "usage": {
                                                                    "output_tokens": output_tokens
                                                                }
                                                            });
                                                            let sse_line = format!(
                                                                "event: message_delta\ndata: {}\n\n",
                                                                delta_event
                                                            );
                                                            event_chunks.push(Bytes::from(sse_line));

                                                            let should_send_stop = {
                                                                let mut stop_sent = openai_stop.lock().unwrap();
                                                                if *stop_sent {
                                                                    false
                                                                } else {
                                                                    *stop_sent = true;
                                                                    true
                                                                }
                                                            };
                                                            if should_send_stop {
                                                                let stop_event = serde_json::json!({
                                                                    "type": "message_stop"
                                                                });
                                                                let sse_line = format!(
                                                                    "event: message_stop\ndata: {}\n\n",
                                                                    stop_event
                                                                );
                                                                event_chunks.push(Bytes::from(sse_line));
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Check for usage in the chunk
                                            if let Some(usage) = json.get("usage") {
                                                if let Ok(mut tracker) = usage_tracker.lock() {
                                                    if let Some(prompt_tokens) =
                                                        usage.get("prompt_tokens").and_then(|v| v.as_i64())
                                                    {
                                                        tracker.prompt_tokens = prompt_tokens;
                                                    }
                                                    if let Some(completion_tokens) = usage
                                                        .get("completion_tokens")
                                                        .and_then(|v| v.as_i64())
                                                    {
                                                        tracker.completion_tokens = completion_tokens;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            // Increment error counter
                                            if let Ok(mut err_count) = errors.lock() {
                                                *err_count += 1;
                                            }
                                            logger::error(
                                                "anthropic",
                                                &format!(
                                                    "Failed to parse OpenAI SSE JSON chunk: error={}, data={}",
                                                    e,
                                                    &data[..data.len().min(200)]
                                                ),
                                            );
                                        }
                                    }
                                }
                            }

                            if !event_chunks.is_empty() {
                                output_chunks.extend(event_chunks.into_iter().map(Ok));
                            }
                        }
                        Err(e) => {
                            logger::error(
                                "anthropic",
                                &format!("OpenAI-style stream bytes error: {}", e),
                            );
                            output_chunks.push(Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            )));
                        }
                    }

                    output_chunks
                }
            })
            .flat_map(futures_util::stream::iter);
        // Create a wrapper stream that logs usage when done
        let ctx_for_log = ctx_clone;
        let usage_for_log = Arc::clone(&usage_tracker);
        let model_id = ctx.model.id.clone();
        let errors_for_log = Arc::clone(&conversion_errors);

        let logged_stream = stream
            .chain(futures_util::stream::once(async move {
                // Log final usage when stream completes
                let error_count = if let Ok(err_count) = errors_for_log.lock() {
                    *err_count
                } else {
                    0
                };

                if let Ok(usage) = usage_for_log.lock() {
                    let log_msg = format!(
                        "OpenAI-style stream completed: model={}, tokens={}/{}, conversion_errors={}",
                        model_id,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        error_count
                    );

                    if error_count > 0 {
                        logger::warn("anthropic", &log_msg);
                    } else {
                        logger::info("anthropic", &log_msg);
                    }

                    ctx_for_log.log_usage(&usage);
                } else {
                    logger::error(
                        "anthropic",
                        &format!("Failed to acquire usage tracker lock for model={}", model_id),
                    );
                }
                Err(std::io::Error::new(std::io::ErrorKind::Other, "stream_end"))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                    Err(e) if e.to_string() == "stream_end" => None,
                    Err(e) => {
                        logger::error(
                            "anthropic",
                            &format!("OpenAI-style stream filter error: {}", e),
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
                    "anthropic",
                    &format!("Failed to build OpenAI-style stream response: {}", e),
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

/// Extract usage from Anthropic response
/// Anthropic uses input_tokens/output_tokens instead of prompt_tokens/completion_tokens
fn extract_usage(response: &Value) -> TokenUsage {
    if let Some(usage) = response.get("usage") {
        let input = usage
            .get("input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        // Also consider cache tokens
        let cache_creation = usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let cache_read = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        TokenUsage::new(input + cache_creation + cache_read, output)
    } else {
        TokenUsage::default()
    }
}

/// Extract usage from OpenAI-compatible responses.
fn extract_openai_usage(response: &Value) -> TokenUsage {
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

/// Convert OpenAI-compatible response payload into Anthropic message format.
fn convert_openai_response_to_anthropic(
    response: &Value,
    default_model: &str,
    thinking_enabled: bool,
) -> Value {
    let mut message = serde_json::Map::new();

    // DEBUG: Add a marker to confirm conversion is called
    let id = response
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("msg_unknown");
    message.insert("id".to_string(), Value::String(format!("[CONVERTED]{}", id)));
    message.insert("type".to_string(), Value::String("message".to_string()));
    message.insert("role".to_string(), Value::String("assistant".to_string()));

    let model_value = response
        .get("model")
        .cloned()
        .unwrap_or_else(|| Value::String(default_model.to_string()));
    message.insert("model".to_string(), model_value);

    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    if let Some(choice) = response
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
    {
        finish_reason = choice.get("finish_reason").and_then(|v| v.as_str());
        content_blocks = extract_openai_choice_content(choice, thinking_enabled);
    }

    if content_blocks.is_empty() {
        content_blocks.push(serde_json::json!({
            "type": "text",
            "text": ""
        }));
    }

    message.insert("content".to_string(), Value::Array(content_blocks));
    message.insert(
        "stop_reason".to_string(),
        Value::String(map_openai_finish_reason(finish_reason)),
    );
    message.insert("stop_sequence".to_string(), Value::Null);

    if let Some(usage) = convert_openai_usage_to_anthropic(response) {
        message.insert("usage".to_string(), usage);
    }

    Value::Object(message)
}

fn extract_openai_choice_content(choice: &Value, thinking_enabled: bool) -> Vec<Value> {
    if let Some(message) = choice.get("message") {
        let blocks = openai_message_to_anthropic_content(message, thinking_enabled);
        if !blocks.is_empty() {
            return blocks;
        }
    }

    if let Some(delta) = choice.get("delta") {
        let blocks = openai_message_to_anthropic_content(delta, thinking_enabled);
        if !blocks.is_empty() {
            return blocks;
        }
    }

    if let Some(text) = choice.get("text").and_then(|v| v.as_str()) {
        return vec![serde_json::json!({
            "type": "text",
            "text": text
        })];
    }

    if let Some(content) = choice.get("content") {
        let blocks = openai_message_to_anthropic_content(content, thinking_enabled);
        if !blocks.is_empty() {
            return blocks;
        }
    }

    Vec::new()
}

fn openai_message_to_anthropic_content(message: &Value, thinking_enabled: bool) -> Vec<Value> {
    let mut blocks = Vec::new();

    if thinking_enabled {
        if let Some(reasoning) = message.get("reasoning_content").and_then(|v| v.as_str()) {
            if !reasoning.is_empty() {
                push_thinking_block(&mut blocks, reasoning);
            }
        }
    }

    if let Some(text) = message.as_str() {
        push_text_block(&mut blocks, text);
        return blocks;
    }

    if let Some(parts) = message.as_array() {
        append_openai_content_parts(&mut blocks, parts);
        return blocks;
    }

    if let Some(content) = message.get("content") {
        match content {
            Value::String(text) => push_text_block(&mut blocks, text),
            Value::Array(parts) => {
                append_openai_content_parts(&mut blocks, parts);
            }
            Value::Object(obj) => {
                if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                    push_text_block(&mut blocks, text);
                } else if let Some(value) = obj.get("value").and_then(|t| t.as_str()) {
                    push_text_block(&mut blocks, value);
                }
            }
            _ => {}
        }
    } else if let Some(obj) = message.as_object() {
        if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
            push_text_block(&mut blocks, text);
        } else if let Some(value) = obj.get("value").and_then(|t| t.as_str()) {
            push_text_block(&mut blocks, value);
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for tool_call in tool_calls {
            if let Some(block) = convert_openai_tool_call(tool_call) {
                blocks.push(block);
            }
        }
    } else if let Some(function_call) = message.get("function_call") {
        if let Some(block) = convert_openai_function_call(function_call) {
            blocks.push(block);
        }
    }

    blocks
}

fn push_text_block(blocks: &mut Vec<Value>, text: &str) {
    if text.is_empty() {
        return;
    }
    blocks.push(serde_json::json!({
        "type": "text",
        "text": text
    }));
}

fn push_thinking_block(blocks: &mut Vec<Value>, thinking: &str) {
    if thinking.is_empty() {
        return;
    }
    blocks.push(serde_json::json!({
        "type": "thinking",
        "thinking": thinking
    }));
}

fn append_openai_content_parts(blocks: &mut Vec<Value>, parts: &[Value]) {
    for part in parts {
        let Some(part_type) = part.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        match part_type {
            "text" => {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    push_text_block(blocks, text);
                }
            }
            "image_url" => {
                push_text_block(blocks, "[Image]");
            }
            _ => {}
        }
    }
}

fn convert_openai_tool_call(tool_call: &Value) -> Option<Value> {
    let function = tool_call.get("function")?;
    let name = function.get("name")?.as_str()?;
    let args_raw = function
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let input = serde_json::from_str(args_raw)
        .unwrap_or_else(|_| Value::String(args_raw.to_string()));
    let id = tool_call.get("id").and_then(|v| v.as_str()).unwrap_or("tool_call");

    Some(serde_json::json!({
        "type": "tool_use",
        "id": id,
        "name": name,
        "input": input
    }))
}

fn convert_openai_function_call(function_call: &Value) -> Option<Value> {
    let name = function_call.get("name")?.as_str()?;
    let args_raw = function_call
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let input = serde_json::from_str(args_raw)
        .unwrap_or_else(|_| Value::String(args_raw.to_string()));

    Some(serde_json::json!({
        "type": "tool_use",
        "id": "function_call",
        "name": name,
        "input": input
    }))
}

fn convert_openai_usage_to_anthropic(response: &Value) -> Option<Value> {
    let usage = response.get("usage")?;
    let input = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    Some(serde_json::json!({
        "input_tokens": input,
        "output_tokens": output
    }))
}

fn map_openai_finish_reason(reason: Option<&str>) -> String {
    match reason {
        Some("stop") => "end_turn".to_string(),
        Some("length") => "max_tokens".to_string(),
        Some("tool_calls") | Some("function_call") => "tool_use".to_string(),
        Some("content_filter") => "content_filter".to_string(),
        Some(other) => other.to_string(),
        None => "end_turn".to_string(),
    }
}

#[derive(Default)]
struct OpenAIStreamBlockState {
    thinking_index: Option<usize>,
    text_index: Option<usize>,
    next_index: usize,
}

impl OpenAIStreamBlockState {
    fn ensure_thinking(&mut self) -> (usize, bool) {
        if let Some(index) = self.thinking_index {
            return (index, false);
        }
        let index = self.next_index;
        self.next_index += 1;
        self.thinking_index = Some(index);
        (index, true)
    }

    fn ensure_text(&mut self) -> (usize, bool) {
        if let Some(index) = self.text_index {
            return (index, false);
        }
        let index = self.next_index;
        self.next_index += 1;
        self.text_index = Some(index);
        (index, true)
    }

    fn close_thinking(&mut self) -> Option<usize> {
        self.thinking_index.take()
    }

    fn stop_started(&mut self) -> Vec<usize> {
        let mut indices = Vec::new();
        if let Some(index) = self.thinking_index.take() {
            indices.push(index);
        }
        if let Some(index) = self.text_index.take() {
            indices.push(index);
        }
        indices.sort_unstable();
        indices
    }
}

fn push_content_block_start(output: &mut Vec<Bytes>, index: usize, block_type: &str) {
    let content_block = match block_type {
        "thinking" => serde_json::json!({
            "type": "thinking",
            "thinking": ""
        }),
        _ => serde_json::json!({
            "type": "text",
            "text": ""
        }),
    };
    let event = serde_json::json!({
        "type": "content_block_start",
        "index": index,
        "content_block": content_block
    });
    output.push(Bytes::from(format!(
        "event: content_block_start\ndata: {}\n\n",
        event
    )));
}

fn push_content_block_delta(output: &mut Vec<Bytes>, index: usize, block_type: &str, text: &str) {
    let delta = match block_type {
        "thinking" => serde_json::json!({
            "type": "thinking_delta",
            "thinking": text
        }),
        _ => serde_json::json!({
            "type": "text_delta",
            "text": text
        }),
    };
    let event = serde_json::json!({
        "type": "content_block_delta",
        "index": index,
        "delta": delta
    });
    output.push(Bytes::from(format!(
        "event: content_block_delta\ndata: {}\n\n",
        event
    )));
}

fn push_content_block_stop(output: &mut Vec<Bytes>, index: usize) {
    let event = serde_json::json!({
        "type": "content_block_stop",
        "index": index
    });
    output.push(Bytes::from(format!(
        "event: content_block_stop\ndata: {}\n\n",
        event
    )));
}

fn map_openai_tools_to_anthropic(tools: &Value) -> Option<Value> {
    let tools_array = tools.as_array()?;
    let mut mapped = Vec::new();

    for tool in tools_array {
        let tool_type = tool.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !tool_type.eq_ignore_ascii_case("function") {
            continue;
        }
        let function = tool.get("function")?;
        let name = function.get("name")?.as_str()?;

        let mut entry = serde_json::Map::new();
        entry.insert("name".to_string(), Value::String(name.to_string()));

        if let Some(desc) = function.get("description") {
            entry.insert("description".to_string(), desc.clone());
        }
        if let Some(params) = function.get("parameters") {
            entry.insert("input_schema".to_string(), params.clone());
        }

        mapped.push(Value::Object(entry));
    }

    if mapped.is_empty() {
        None
    } else {
        Some(Value::Array(mapped))
    }
}

fn map_openai_tool_choice_to_anthropic(choice: &Value) -> Option<Value> {
    match choice {
        Value::String(raw) => match raw.to_ascii_lowercase().as_str() {
            "auto" => Some(serde_json::json!({ "type": "auto" })),
            "none" => Some(serde_json::json!({ "type": "none" })),
            _ => None,
        },
        Value::Object(obj) => {
            let kind = obj.get("type").and_then(|v| v.as_str())?;
            if kind.eq_ignore_ascii_case("function") {
                let name = obj
                    .get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())?;
                Some(serde_json::json!({ "type": "tool", "name": name }))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn map_anthropic_tools_to_openai(tools: &Value) -> Option<Value> {
    let tools_array = tools.as_array()?;
    let mut mapped = Vec::new();

    for tool in tools_array {
        let name = tool.get("name").and_then(|v| v.as_str())?;
        let mut function = serde_json::Map::new();
        function.insert("name".to_string(), Value::String(name.to_string()));

        if let Some(desc) = tool.get("description") {
            function.insert("description".to_string(), desc.clone());
        }
        if let Some(schema) = tool.get("input_schema") {
            function.insert("parameters".to_string(), schema.clone());
        }

        let mut entry = serde_json::Map::new();
        entry.insert("type".to_string(), Value::String("function".to_string()));
        entry.insert("function".to_string(), Value::Object(function));
        mapped.push(Value::Object(entry));
    }

    if mapped.is_empty() {
        None
    } else {
        Some(Value::Array(mapped))
    }
}

fn map_anthropic_tool_choice_to_openai(choice: &Value) -> Option<Value> {
    match choice {
        Value::String(raw) => match raw.to_ascii_lowercase().as_str() {
            "auto" | "any" => Some(Value::String("auto".to_string())),
            "none" => Some(Value::String("none".to_string())),
            _ => None,
        },
        Value::Object(obj) => {
            let kind = obj.get("type").and_then(|v| v.as_str())?;
            match kind.to_ascii_lowercase().as_str() {
                "auto" | "any" => Some(Value::String("auto".to_string())),
                "none" => Some(Value::String("none".to_string())),
                "tool" => {
                    let name = obj.get("name").and_then(|v| v.as_str())?;
                    Some(serde_json::json!({
                        "type": "function",
                        "function": { "name": name }
                    }))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn openai_content_to_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let mut texts = Vec::new();
            for part in parts {
                if let Some(part_type) = part.get("type").and_then(|v| v.as_str()) {
                    if part_type == "text" {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            texts.push(text.to_string());
                        }
                    }
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        Value::Object(obj) => obj
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn append_openai_content_blocks(blocks: &mut Vec<Value>, content: &Value) {
    match content {
        Value::String(text) => {
            if !text.is_empty() {
                blocks.push(serde_json::json!({ "type": "text", "text": text }));
            }
        }
        Value::Array(parts) => {
            for part in parts {
                let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match part_type {
                    "text" => {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            blocks.push(serde_json::json!({ "type": "text", "text": text }));
                        }
                    }
                    "image_url" => {
                        let url = part
                            .get("image_url")
                            .and_then(|v| v.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("[Image]");
                        blocks.push(serde_json::json!({
                            "type": "text",
                            "text": format!("[Image] {}", url)
                        }));
                    }
                    _ => {}
                }
            }
        }
        Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                blocks.push(serde_json::json!({ "type": "text", "text": text }));
            }
        }
        _ => {}
    }
}

/// Convert OpenAI request format to Anthropic format
pub(crate) fn convert_openai_to_anthropic_request(payload: &Value, model: &str) -> Value {
    let mut anthropic_request = serde_json::Map::new();
    anthropic_request.insert("model".to_string(), Value::String(model.to_string()));

    let mut system_parts = Vec::new();
    let mut messages_out = Vec::new();

    if let Some(messages) = payload.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user");

            if role.eq_ignore_ascii_case("system") {
                if let Some(text) = msg.get("content").and_then(openai_content_to_text) {
                    if !text.is_empty() {
                        system_parts.push(text);
                    }
                }
                continue;
            }

            let mut blocks = Vec::new();
            if let Some(content) = msg.get("content") {
                append_openai_content_blocks(&mut blocks, content);
            }

            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                for tool_call in tool_calls {
                    if let Some(block) = convert_openai_tool_call(tool_call) {
                        blocks.push(block);
                    }
                }
            }

            if let Some(function_call) = msg.get("function_call") {
                if let Some(block) = convert_openai_function_call(function_call) {
                    blocks.push(block);
                }
            }

            if blocks.is_empty() {
                blocks.push(serde_json::json!({ "type": "text", "text": "" }));
            }

            let mut msg_obj = serde_json::Map::new();
            msg_obj.insert("role".to_string(), Value::String(role.to_string()));
            msg_obj.insert("content".to_string(), Value::Array(blocks));
            messages_out.push(Value::Object(msg_obj));
        }
    }

    if !messages_out.is_empty() {
        anthropic_request.insert("messages".to_string(), Value::Array(messages_out));
    }

    if !system_parts.is_empty() {
        anthropic_request.insert(
            "system".to_string(),
            Value::String(system_parts.join("\n\n")),
        );
    }

    if let Some(max_tokens) = payload.get("max_tokens").or_else(|| payload.get("max_completion_tokens")) {
        anthropic_request.insert("max_tokens".to_string(), max_tokens.clone());
    }
    if let Some(temperature) = payload.get("temperature") {
        anthropic_request.insert("temperature".to_string(), temperature.clone());
    }
    if let Some(top_p) = payload.get("top_p") {
        anthropic_request.insert("top_p".to_string(), top_p.clone());
    }
    if let Some(stream) = payload.get("stream") {
        anthropic_request.insert("stream".to_string(), stream.clone());
    }
    if let Some(stop) = payload.get("stop") {
        let mapped = match stop {
            Value::String(text) => Value::Array(vec![Value::String(text.clone())]),
            _ => stop.clone(),
        };
        anthropic_request.insert("stop_sequences".to_string(), mapped);
    }
    if let Some(metadata) = payload.get("metadata") {
        anthropic_request.insert("metadata".to_string(), metadata.clone());
    }
    if let Some(tools) = payload.get("tools") {
        if let Some(mapped) = map_openai_tools_to_anthropic(tools) {
            anthropic_request.insert("tools".to_string(), mapped);
        }
    }
    if let Some(tool_choice) = payload.get("tool_choice") {
        if let Some(mapped) = map_openai_tool_choice_to_anthropic(tool_choice) {
            anthropic_request.insert("tool_choice".to_string(), mapped);
        }
    }

    Value::Object(anthropic_request)
}

fn openai_content_from_parts(parts: Vec<Value>) -> Value {
    if parts.len() == 1 {
        if let Some(obj) = parts[0].as_object() {
            if obj.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    return Value::String(text.to_string());
                }
            }
        }
    }
    Value::Array(parts)
}

fn text_from_anthropic_block(block: &Value) -> Option<String> {
    match block.get("type").and_then(|v| v.as_str()) {
        Some("text") => block.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        Some("thinking") => block.get("thinking").and_then(|v| v.as_str()).map(|s| format!("[Thinking] {}", s)),
        _ => None,
    }
}

fn push_anthropic_image_as_openai_part(parts: &mut Vec<Value>, block: &Value) {
    let source = block.get("source");
    let source_type = source.and_then(|v| v.get("type")).and_then(|v| v.as_str()).unwrap_or("");
    if source_type.eq_ignore_ascii_case("base64") {
        let media_type = source
            .and_then(|v| v.get("media_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("application/octet-stream");
        let data = source
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !data.is_empty() {
            let url = format!("data:{};base64,{}", media_type, data);
            parts.push(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": url }
            }));
            return;
        }
    }
    parts.push(serde_json::json!({
        "type": "text",
        "text": "[Image]"
    }));
}

/// Convert Anthropic request format to OpenAI format
pub(crate) fn convert_anthropic_to_openai(payload: &Value, model: &str) -> Value {
    let mut openai_request = serde_json::Map::new();
    openai_request.insert("model".to_string(), Value::String(model.to_string()));

    let mut openai_messages = Vec::new();

    if let Some(messages) = payload.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user");

            let mut content_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut tool_messages = Vec::new();

            match msg.get("content") {
                Some(Value::String(text)) => {
                    if !text.is_empty() {
                        content_parts.push(serde_json::json!({ "type": "text", "text": text }));
                    }
                }
                Some(Value::Array(blocks)) => {
                    for block in blocks {
                        match block.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                            "text" | "thinking" => {
                                if let Some(text) = text_from_anthropic_block(block) {
                                    content_parts.push(serde_json::json!({ "type": "text", "text": text }));
                                }
                            }
                            "image" => {
                                push_anthropic_image_as_openai_part(&mut content_parts, block);
                            }
                            "tool_use" => {
                                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                                let id = block
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("tool_call");
                                let input = block.get("input").cloned().unwrap_or(Value::Null);
                                let args = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
                                tool_calls.push(serde_json::json!({
                                    "id": id,
                                    "type": "function",
                                    "function": { "name": name, "arguments": args }
                                }));
                            }
                            "tool_result" => {
                                let tool_id = block
                                    .get("tool_use_id")
                                    .or_else(|| block.get("id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("tool_call");
                                let content = match block.get("content") {
                                    Some(Value::String(text)) => text.clone(),
                                    Some(Value::Array(parts)) => parts
                                        .iter()
                                        .filter_map(|p| p.get("text").and_then(|v| v.as_str()))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    Some(other) => other.to_string(),
                                    None => String::new(),
                                };
                                tool_messages.push(serde_json::json!({
                                    "role": "tool",
                                    "tool_call_id": tool_id,
                                    "content": content
                                }));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            let mut openai_msg = serde_json::Map::new();
            openai_msg.insert("role".to_string(), Value::String(role.to_string()));
            if content_parts.is_empty() {
                openai_msg.insert("content".to_string(), Value::String(String::new()));
            } else {
                openai_msg.insert("content".to_string(), openai_content_from_parts(content_parts));
            }
            if !tool_calls.is_empty() {
                openai_msg.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }

            openai_messages.push(Value::Object(openai_msg));
            openai_messages.extend(tool_messages);
        }
    }

    if let Some(system) = payload.get("system") {
        if let Some(system_text) = system.as_str() {
            let mut system_msg = serde_json::Map::new();
            system_msg.insert("role".to_string(), Value::String("system".to_string()));
            system_msg.insert("content".to_string(), Value::String(system_text.to_string()));
            openai_messages.insert(0, Value::Object(system_msg));
        }
    }

    if !openai_messages.is_empty() {
        openai_request.insert("messages".to_string(), Value::Array(openai_messages));
    }

    if let Some(max_tokens) = payload.get("max_tokens") {
        openai_request.insert("max_tokens".to_string(), max_tokens.clone());
    }
    if let Some(temperature) = payload.get("temperature") {
        openai_request.insert("temperature".to_string(), temperature.clone());
    }
    if let Some(top_p) = payload.get("top_p") {
        openai_request.insert("top_p".to_string(), top_p.clone());
    }
    if let Some(stream) = payload.get("stream") {
        openai_request.insert("stream".to_string(), stream.clone());
    }
    if let Some(stop_sequences) = payload.get("stop_sequences") {
        openai_request.insert("stop".to_string(), stop_sequences.clone());
    }
    if let Some(metadata) = payload.get("metadata") {
        openai_request.insert("metadata".to_string(), metadata.clone());
    }
    if let Some(tools) = payload.get("tools") {
        if let Some(mapped) = map_anthropic_tools_to_openai(tools) {
            openai_request.insert("tools".to_string(), mapped);
        }
    }
    if let Some(tool_choice) = payload.get("tool_choice") {
        if let Some(mapped) = map_anthropic_tool_choice_to_openai(tool_choice) {
            openai_request.insert("tool_choice".to_string(), mapped);
        }
    }

    Value::Object(openai_request)
}

/// Convert OpenAI streaming chunk to Anthropic format
pub(crate) fn convert_openai_chunk_to_anthropic(
    chunk: &Value,
    is_first: bool,
    prompt_tokens: i64,
) -> Option<Value> {
    let mut anthropic_event = serde_json::Map::new();

    if is_first {
        // First chunk - send message_start event
        anthropic_event.insert("type".to_string(), Value::String("message_start".to_string()));

        let id_raw = chunk
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("msg_unknown");
        let id = if id_raw.starts_with("msg_") {
            id_raw.to_string()
        } else {
            format!("msg_{}", id_raw)
        };
        let input_tokens = chunk
            .get("usage")
            .and_then(|v| v.get("prompt_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(prompt_tokens);
        let output_tokens = chunk
            .get("usage")
            .and_then(|v| v.get("completion_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let mut message = serde_json::Map::new();
        message.insert("id".to_string(), Value::String(id));
        message.insert("type".to_string(), Value::String("message".to_string()));
        message.insert("role".to_string(), Value::String("assistant".to_string()));
        message.insert("content".to_string(), Value::Array(vec![]));
        message.insert("model".to_string(), chunk.get("model").cloned().unwrap_or(Value::String("unknown".to_string())));
        message.insert("stop_reason".to_string(), Value::Null);
        message.insert("stop_sequence".to_string(), Value::Null);
        message.insert(
            "usage".to_string(),
            serde_json::json!({
                "input_tokens": input_tokens,
                "output_tokens": output_tokens
            }),
        );

        anthropic_event.insert("message".to_string(), Value::Object(message));

        return Some(Value::Object(anthropic_event));
    }

    // Check for choices
    if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(delta) = choice.get("delta") {
                let content = delta.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let reasoning = delta
                    .get("reasoning_content")
                    .and_then(|r| r.as_str())
                    .unwrap_or("");

                if !content.is_empty() {
                    // Send content_block_delta event
                    anthropic_event.insert(
                        "type".to_string(),
                        Value::String("content_block_delta".to_string()),
                    );
                    anthropic_event.insert("index".to_string(), Value::Number(0.into()));

                    let mut delta_obj = serde_json::Map::new();
                    delta_obj.insert("type".to_string(), Value::String("text_delta".to_string()));
                    delta_obj.insert("text".to_string(), Value::String(content.to_string()));
                    anthropic_event.insert("delta".to_string(), Value::Object(delta_obj));

                    return Some(Value::Object(anthropic_event));
                }

                if !reasoning.is_empty() {
                    // Send thinking delta event
                    anthropic_event.insert(
                        "type".to_string(),
                        Value::String("content_block_delta".to_string()),
                    );
                    anthropic_event.insert("index".to_string(), Value::Number(0.into()));

                    let mut delta_obj = serde_json::Map::new();
                    delta_obj.insert(
                        "type".to_string(),
                        Value::String("thinking_delta".to_string()),
                    );
                    delta_obj.insert("thinking".to_string(), Value::String(reasoning.to_string()));
                    anthropic_event.insert("delta".to_string(), Value::Object(delta_obj));

                    return Some(Value::Object(anthropic_event));
                }

                // Check for finish_reason
                if let Some(finish_reason) = choice.get("finish_reason") {
                    if !finish_reason.is_null() {
                        // Send message_delta event with stop_reason
                        anthropic_event.insert("type".to_string(), Value::String("message_delta".to_string()));

                        let mut delta_obj = serde_json::Map::new();
                        delta_obj.insert("stop_reason".to_string(), Value::String("end_turn".to_string()));
                        anthropic_event.insert("delta".to_string(), Value::Object(delta_obj));

                        // Add usage if available
                        if let Some(usage) = chunk.get("usage") {
                            let mut anthropic_usage = serde_json::Map::new();
                            if let Some(output_tokens) = usage.get("completion_tokens") {
                                anthropic_usage.insert("output_tokens".to_string(), output_tokens.clone());
                            }
                            anthropic_event.insert("usage".to_string(), Value::Object(anthropic_usage));
                        }

                        return Some(Value::Object(anthropic_event));
                    }
                }
            }
        }
    }

    None
}

fn epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn extract_anthropic_usage_counts(usage: &Value) -> (i64, i64) {
    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    (input + cache_create + cache_read, output)
}

fn map_anthropic_stop_reason(reason: &str) -> Value {
    let mapped = match reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        "content_filter" => "content_filter",
        other => other,
    };
    Value::String(mapped.to_string())
}

fn convert_anthropic_usage_to_openai(usage: &Value) -> Value {
    let (prompt_tokens, completion_tokens) = extract_anthropic_usage_counts(usage);
    serde_json::json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": prompt_tokens + completion_tokens
    })
}

fn anthropic_content_to_openai_message(content: &Value) -> (Value, Vec<Value>) {
    let mut parts = Vec::new();
    let mut tool_calls = Vec::new();

    match content {
        Value::String(text) => {
            if !text.is_empty() {
                parts.push(serde_json::json!({ "type": "text", "text": text }));
            }
        }
        Value::Array(blocks) => {
            for block in blocks {
                match block.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                    "text" | "thinking" => {
                        if let Some(text) = text_from_anthropic_block(block) {
                            parts.push(serde_json::json!({ "type": "text", "text": text }));
                        }
                    }
                    "image" => {
                        push_anthropic_image_as_openai_part(&mut parts, block);
                    }
                    "tool_use" => {
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool_call");
                        let input = block.get("input").cloned().unwrap_or(Value::Null);
                        let args =
                            serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
                        tool_calls.push(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": args }
                        }));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let content_value = if parts.is_empty() {
        Value::String(String::new())
    } else {
        openai_content_from_parts(parts)
    };

    (content_value, tool_calls)
}

/// Convert Anthropic response format to OpenAI format
pub(crate) fn convert_anthropic_response_to_openai(response: &Value, model: &str) -> Value {
    let id = response
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| {
            if s.starts_with("chatcmpl_") {
                s.to_string()
            } else {
                format!("chatcmpl_{}", s)
            }
        })
        .unwrap_or_else(|| format!("chatcmpl_{}", epoch_seconds()));

    let created = epoch_seconds();

    let (content, tool_calls) = response
        .get("content")
        .map(anthropic_content_to_openai_message)
        .unwrap_or_else(|| (Value::String(String::new()), Vec::new()));

    let mut message = serde_json::Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    message.insert("content".to_string(), content);
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }

    let finish_reason = response
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(map_anthropic_stop_reason);

    let choice = serde_json::json!({
        "index": 0,
        "message": Value::Object(message),
        "finish_reason": finish_reason
    });

    let usage = response
        .get("usage")
        .map(convert_anthropic_usage_to_openai)
        .unwrap_or_else(|| serde_json::json!({
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0
        }));

    serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": response.get("model").cloned().unwrap_or(Value::String(model.to_string())),
        "choices": [choice],
        "usage": usage
    })
}

pub(crate) struct AnthropicToOpenAIStreamState {
    pub id: String,
    pub created: i64,
    pub model: String,
    pub sent_role: bool,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub finished: bool,
}

impl AnthropicToOpenAIStreamState {
    pub fn new(model: &str) -> Self {
        let now = epoch_seconds();
        Self {
            id: format!("chatcmpl_{}", now),
            created: now,
            model: model.to_string(),
            sent_role: false,
            prompt_tokens: 0,
            completion_tokens: 0,
            finished: false,
        }
    }
}

fn build_openai_stream_chunk(
    state: &AnthropicToOpenAIStreamState,
    delta: Value,
    finish_reason: Option<Value>,
    include_usage: bool,
) -> Value {
    let mut chunk = serde_json::json!({
        "id": state.id,
        "object": "chat.completion.chunk",
        "created": state.created,
        "model": state.model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason
        }]
    });

    if include_usage {
        if let Some(obj) = chunk.as_object_mut() {
            obj.insert(
                "usage".to_string(),
                serde_json::json!({
                    "prompt_tokens": state.prompt_tokens,
                    "completion_tokens": state.completion_tokens,
                    "total_tokens": state.prompt_tokens + state.completion_tokens
                }),
            );
        }
    }

    chunk
}

pub(crate) fn convert_anthropic_event_to_openai_chunks(
    event: &Value,
    state: &mut AnthropicToOpenAIStreamState,
) -> Vec<Value> {
    let mut out = Vec::new();
    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "message_start" => {
            if let Some(message) = event.get("message") {
                if let Some(id) = message.get("id").and_then(|v| v.as_str()) {
                    state.id = if id.starts_with("chatcmpl_") {
                        id.to_string()
                    } else {
                        format!("chatcmpl_{}", id)
                    };
                }
                if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                    state.model = model.to_string();
                }
                if let Some(usage) = message.get("usage") {
                    let (prompt, completion) = extract_anthropic_usage_counts(usage);
                    if prompt > 0 {
                        state.prompt_tokens = prompt;
                    }
                    if completion > 0 {
                        state.completion_tokens = completion;
                    }
                }
            }

            if !state.sent_role {
                out.push(build_openai_stream_chunk(
                    state,
                    serde_json::json!({ "role": "assistant" }),
                    None,
                    false,
                ));
                state.sent_role = true;
            }
        }
        "content_block_start" => {
            if let Some(block) = event.get("content_block") {
                if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                    let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("tool_call");
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    let args =
                        serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
                    let delta = serde_json::json!({
                        "tool_calls": [{
                            "index": 0,
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": args }
                        }]
                    });
                    out.push(build_openai_stream_chunk(state, delta, None, false));
                }
            }
        }
        "content_block_delta" => {
            if let Some(delta) = event.get("delta") {
                let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            out.push(build_openai_stream_chunk(
                                state,
                                serde_json::json!({ "content": text }),
                                None,
                                false,
                            ));
                        }
                    }
                    "thinking_delta" => {
                        if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                            out.push(build_openai_stream_chunk(
                                state,
                                serde_json::json!({ "reasoning_content": text }),
                                None,
                                false,
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        "message_delta" => {
            if let Some(usage) = event.get("usage") {
                let (prompt, completion) = extract_anthropic_usage_counts(usage);
                if prompt > 0 {
                    state.prompt_tokens = prompt;
                }
                if completion > 0 {
                    state.completion_tokens = completion;
                }
            }

            let finish_reason = event
                .get("delta")
                .and_then(|v| v.get("stop_reason"))
                .and_then(|v| v.as_str())
                .map(map_anthropic_stop_reason);

            if finish_reason.is_some() {
                state.finished = true;
                out.push(build_openai_stream_chunk(
                    state,
                    serde_json::json!({}),
                    finish_reason,
                    true,
                ));
            }
        }
        "message_stop" => {
            if !state.finished {
                out.push(build_openai_stream_chunk(
                    state,
                    serde_json::json!({}),
                    Some(Value::String("stop".to_string())),
                    true,
                ));
                state.finished = true;
            }
        }
        _ => {}
    }

    out
}

fn estimate_anthropic_prompt_tokens(payload: &Value) -> i64 {
    let messages = payload
        .get("messages")
        .map(|m| m.to_string())
        .unwrap_or_default();
    let system = payload
        .get("system")
        .map(|s| s.to_string())
        .unwrap_or_default();
    estimate_tokens(&format!("{}{}", system, messages))
}

async fn handle_anthropic_to_gemini_request(
    ctx: ForwardContext,
    payload: Value,
    thinking_enabled: bool,
) -> ForwardResult<UpstreamResponse> {
    let start = Instant::now();
    let upstream_ctx = with_provider(&ctx, Provider::Gemini);

    logger::info(
        "anthropic",
        &format!(
            "Request started: model={}, upstream={}, streaming=false (gemini)",
            ctx.model.id, ctx.upstream.id
        ),
    );

    let openai_payload =
        convert_anthropic_to_openai(&payload, ctx.model.upstream_model());
    let gemini_payload =
        gemini::convert_openai_to_gemini_request(&openai_payload, ctx.model.upstream_model());

    let handler = gemini::GeminiHandler;
    let headers = handler.build_headers(&upstream_ctx);
    let config = ctx.retry_config();
    let client = client::default_client()?;

    let path = format!(
        "/{}/models/{}:generateContent",
        upstream_ctx.gemini_version(),
        ctx.model.upstream_model()
    );
    let endpoints = gemini::build_gemini_endpoints(&upstream_ctx, &path);

    let result = client::send_with_retry(&client, &endpoints, "", headers, &gemini_payload, &config)
        .await?;

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

    let openai_response =
        gemini::convert_gemini_response_to_openai(&response_body, ctx.model.upstream_model());
    let anthropic_response = convert_openai_response_to_anthropic(
        &openai_response,
        ctx.model.upstream_model(),
        thinking_enabled,
    );

    let mut usage = extract_usage(&anthropic_response);
    if usage.prompt_tokens == 0 {
        usage.prompt_tokens = estimate_anthropic_prompt_tokens(&payload);
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    ctx.log_usage(&usage);

    Ok(UpstreamResponse {
        body: anthropic_response,
        latency_ms,
        status: status_code,
        usage,
    })
}

async fn handle_anthropic_to_gemini_stream(
    ctx: ForwardContext,
    payload: Value,
    _thinking_enabled: bool,
) -> ForwardResult<Response> {
    let upstream_ctx = with_provider(&ctx, Provider::Gemini);
    let openai_payload =
        convert_anthropic_to_openai(&payload, ctx.model.upstream_model());
    let gemini_payload =
        gemini::convert_openai_to_gemini_request(&openai_payload, ctx.model.upstream_model());

    let handler = gemini::GeminiHandler;
    let headers = handler.build_headers(&upstream_ctx);
    let client = client::streaming_client()?;
    let url = gemini::build_gemini_stream_url(&upstream_ctx, ctx.model.upstream_model())
        .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;

    let response = client
        .post(&url)
        .headers(headers)
        .json(&gemini_payload)
        .send()
        .await
        .map_err(|e| {
            logger::error("anthropic", &format!("Gemini stream request failed: {}", e));
            ForwardError::RequestFailed(e.to_string())
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        logger::error(
            "anthropic",
            &format!("Gemini stream error: status={}, body={}", status, text),
        );
        return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
    }

    let estimated_prompt_tokens = estimate_anthropic_prompt_tokens(&payload);
    let gemini_state = Arc::new(Mutex::new({
        let mut s = gemini::GeminiToOpenAIStreamState::new(ctx.model.upstream_model());
        s.prompt_tokens = estimated_prompt_tokens;
        s
    }));
    let gemini_state_clone = Arc::clone(&gemini_state);

    let openai_state = Arc::new(Mutex::new((true, estimated_prompt_tokens)));
    let openai_state_clone = Arc::clone(&openai_state);

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
                            if let Ok(mut state) = gemini_state_clone.lock() {
                                let openai_chunks =
                                    gemini::convert_gemini_event_to_openai_chunks(
                                        &json, &mut state,
                                    );
                                for chunk in openai_chunks {
                                    let mut openai_state = openai_state_clone.lock().unwrap();
                                    let (ref mut is_first, prompt_tokens) = *openai_state;
                                    if let Some(event) =
                                        convert_openai_chunk_to_anthropic(
                                            &chunk,
                                            *is_first,
                                            prompt_tokens,
                                        )
                                    {
                                        *is_first = false;
                                        let event_type = event
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("message_delta");
                                        output.extend_from_slice(
                                            format!(
                                                "event: {}\ndata: {}\n\n",
                                                event_type, event
                                            )
                                            .as_bytes(),
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            logger::error(
                                "anthropic",
                                &format!("Failed to parse Gemini SSE JSON: {}", e),
                            );
                        }
                    }
                }
            }
            Ok(Bytes::from(output))
        }
        Err(e) => {
            logger::error("anthropic", &format!("Stream bytes error: {}", e));
            Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    });

    let ctx_for_log = ctx.clone();
    let gemini_state_for_log = Arc::clone(&gemini_state);

    let logged_stream = stream
        .chain(futures_util::stream::once(async move {
            if let Ok(state) = gemini_state_for_log.lock() {
                let usage = TokenUsage::new(state.prompt_tokens, state.completion_tokens);
                ctx_for_log.log_usage(&usage);
            }
            let stop_event = serde_json::json!({ "type": "message_stop" });
            Ok(Bytes::from(format!(
                "event: message_stop\ndata: {}\n\n",
                stop_event
            )))
        }))
        .filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                Err(e) => {
                    logger::error("anthropic", &format!("Stream filter error: {}", e));
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
            logger::error("anthropic", &format!("Failed to build stream response: {}", e));
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_payload() {
        let payload = serde_json::json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 100,
            "thinking": {"enabled": true},
            "custom_field": "should_be_removed"
        });

        let filtered = filter_payload(&payload, ALLOWED_FIELDS);
        let obj = filtered.as_object().unwrap();

        assert!(obj.contains_key("model"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("max_tokens"));
        assert!(obj.contains_key("thinking"));
        assert!(!obj.contains_key("custom_field"));
    }

    #[test]
    fn test_extract_usage() {
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_creation_input_tokens": 10,
                "cache_read_input_tokens": 5
            }
        });

        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 115); // 100 + 10 + 5
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn test_extract_usage_simple() {
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn test_multimodal_message_format() {
        // Test that multimodal messages are preserved correctly
        let payload = serde_json::json!({
            "model": "claude-3-opus",
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/jpeg",
                            "data": "base64data..."
                        }
                    },
                    {
                        "type": "text",
                        "text": "What's in this image?"
                    }
                ]
            }],
            "max_tokens": 1000
        });

        let filtered = filter_payload(&payload, ALLOWED_FIELDS);
        let messages = filtered.get("messages").unwrap().as_array().unwrap();
        let content = messages[0].get("content").unwrap().as_array().unwrap();

        assert_eq!(content.len(), 2);
        assert_eq!(content[0].get("type").unwrap(), "image");
        assert_eq!(content[1].get("type").unwrap(), "text");
    }

    #[test]
    fn test_convert_openai_response_content_object() {
        let response = serde_json::json!({
            "id": "chatcmpl_test",
            "model": "glm-4.7",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": { "type": "text", "text": "hello" }
                },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });

        let converted = convert_openai_response_to_anthropic(&response, "glm-4.7", true);
        let content = converted.get("content").unwrap().as_array().unwrap();
        assert_eq!(content[0].get("type").unwrap(), "text");
        assert_eq!(content[0].get("text").unwrap(), "hello");
    }

    #[test]
    fn test_convert_openai_response_choice_text() {
        let response = serde_json::json!({
            "id": "cmpl_test",
            "model": "glm-4.7",
            "choices": [{
                "text": "fallback text",
                "finish_reason": "stop"
            }]
        });

        let converted = convert_openai_response_to_anthropic(&response, "glm-4.7", true);
        let content = converted.get("content").unwrap().as_array().unwrap();
        assert_eq!(content[0].get("type").unwrap(), "text");
        assert_eq!(content[0].get("text").unwrap(), "fallback text");
    }
}
