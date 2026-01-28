//! Gemini API handler
//!
//! Handles forwarding requests to Google's Gemini API (AI Studio).
//! Supports streaming, multimodal (images, video, audio), and tool use.

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

use super::{anthropic, ProviderHandlerImpl};

/// Allowed fields for Gemini API
const ALLOWED_FIELDS: &[&str] = &[
    "contents",
    "generationConfig",
    "safetySettings",
    "tools",
    "toolConfig",
    "systemInstruction",
    "cachedContent",
];

/// Allowed fields for generationConfig
const ALLOWED_GENERATION_CONFIG_FIELDS: &[&str] = &[
    "temperature",
    "topK",
    "topP",
    "candidateCount",
    "maxOutputTokens",
    "stopSequences",
    "presencePenalty",
    "frequencyPenalty",
    "seed",
    "responseMimeType",
    "responseSchema",
    "responseModalities",
    "thinkingConfig",
    "audioConfig",
    "mediaResolution",
    "speechConfig",
    "routingConfig",
    // OpenAI compatibility fields
    "max_tokens",
    "stop",
];

/// Default Gemini API endpoint
#[allow(dead_code)]
const DEFAULT_ENDPOINT: &str = "https://generativelanguage.googleapis.com";

/// Gemini API handler
pub struct GeminiHandler;

impl ProviderHandlerImpl for GeminiHandler {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn build_url(&self, ctx: &ForwardContext, path: &str) -> String {
        let endpoint = ctx.primary_endpoint().unwrap_or(DEFAULT_ENDPOINT);
        let base_url = format!("{}{}", endpoint.trim_end_matches('/'), path);

        // Gemini uses API key in query parameter, unless upstream expects Authorization.
        if let Some(api_key) = ctx.get_api_key() {
            if should_use_query_key(ctx, endpoint) {
                let separator = if base_url.contains('?') { "&" } else { "?" };
                format!("{}{}key={}", base_url, separator, api_key)
            } else {
                base_url
            }
        } else {
            base_url
        }
    }

    fn build_headers(&self, ctx: &ForwardContext) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Content-Type is required
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        // Also support x-goog-api-key header as alternative
        if let Some(api_key) = ctx.get_api_key() {
            if should_use_query_key(ctx, ctx.primary_endpoint().unwrap_or(DEFAULT_ENDPOINT)) {
                if let Ok(value) = HeaderValue::from_str(&api_key) {
                    headers.insert("x-goog-api-key", value);
                }
            }
            if should_send_authorization(ctx) {
                if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", api_key)) {
                    headers.insert("authorization", value);
                }
            }
        }

        headers
    }

    fn transform_request(&self, ctx: &ForwardContext, payload: &Value) -> Value {
        let filtered = filter_payload(payload, ALLOWED_FIELDS, ctx);

        // Log the transformed request
        crate::logger::debug(
            "gemini",
            &format!(
                "Transform request: upstream={}, model={} -> {}",
                ctx.upstream.id,
                ctx.model.id,
                ctx.model.upstream_model()
            ),
        );

        filtered
    }

    fn parse_response(&self, response: &Value) -> TokenUsage {
        extract_usage(response)
    }

    fn estimate_request_tokens(&self, payload: &Value) -> i64 {
        let contents = payload
            .get("contents")
            .map(|c| c.to_string())
            .unwrap_or_default();
        estimate_tokens(&contents)
    }

    async fn handle_request(
        &self,
        ctx: ForwardContext,
        payload: Value,
    ) -> ForwardResult<UpstreamResponse> {
        let upstream_style = upstream_style(&ctx);
        if upstream_style != Provider::Gemini {
            return match upstream_style {
                Provider::OpenAI => handle_gemini_to_openai_request(ctx, payload).await,
                Provider::Anthropic => handle_gemini_to_anthropic_request(ctx, payload).await,
                Provider::Gemini => unreachable!(),
            };
        }

        let start = Instant::now();

        crate::logger::info(
            "gemini",
            &format!(
                "Request started: model={}, upstream={}, streaming=false",
                ctx.model.id,
                ctx.upstream.id
            ),
        );

        // Build request
        let _headers = self.build_headers(&ctx);
        let body = self.transform_request(&ctx, &payload);
        let config = ctx.retry_config();
        let client = client::default_client()?;

        // Build URL with model
        let model = ctx.model.upstream_model();
        let path = format!(
            "/{}/models/{}:generateContent",
            ctx.gemini_version(),
            model
        );

        // Log the request URL
        let endpoint = ctx.primary_endpoint().unwrap_or("unknown");
        crate::logger::debug(
            "gemini",
            &format!("Request URL: {}{}", endpoint, path),
        );

        // For Gemini, we need to handle URL building specially (API key in query)
        let use_query_key = should_use_query_key(&ctx, endpoint);
        let endpoints: Vec<String> = ctx
            .all_endpoints()
            .iter()
            .map(|ep| {
                let base = format!("{}{}", ep.trim_end_matches('/'), path);
                if use_query_key {
                    if let Some(api_key) = ctx.get_api_key() {
                        format!("{}?key={}", base, api_key)
                    } else {
                        base
                    }
                } else {
                    base
                }
            })
            .collect();

        // Send request with retry (use empty path since URL is complete)
        let result = client::send_with_retry(
            &client,
            &endpoints,
            "",
            self.build_headers(&ctx),
            &body,
            &config,
        )
        .await?;

        // Parse response
        let status = result.response.status();
        let status_code = status.as_u16();
        let response_body: Value =
            result.response.json().await.map_err(|e| {
                ForwardError::RequestFailed(format!("Failed to parse response: {}", e))
            })?;

        // Check if response indicates an error
        if !status.is_success() {
            // Don't log usage for failed requests
            return Err(ForwardError::RequestFailed(format!(
                "Upstream returned {}: {}",
                status_code,
                response_body.to_string()
            )));
        }

        // Check for blocked content
        if let Some(block_reason) = response_body
            .get("promptFeedback")
            .and_then(|pf| pf.get("blockReason"))
        {
            return Err(ForwardError::RequestFailed(format!(
                "Content blocked: {:?}",
                block_reason
            )));
        }

        // Extract usage
        let mut usage = extract_usage(&response_body);
        if usage.prompt_tokens == 0 {
            usage.prompt_tokens = self.estimate_request_tokens(&payload);
        }

        let latency_ms = start.elapsed().as_millis() as u64;

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
        if upstream_style != Provider::Gemini {
            return match upstream_style {
                Provider::OpenAI => handle_gemini_to_openai_stream(ctx, payload).await,
                Provider::Anthropic => handle_gemini_to_anthropic_stream(ctx, payload).await,
                Provider::Gemini => unreachable!(),
            };
        }

        // Build request
        let body = self.transform_request(&ctx, &payload);

        let client = client::streaming_client()?;
        let endpoint = ctx
            .primary_endpoint()
            .ok_or_else(|| ForwardError::UpstreamNotFound("No endpoints configured".to_string()))?;

        // Build streaming URL with ?alt=sse
        let model = ctx.model.upstream_model();
        let mut url = format!(
            "{}/{}/models/{}:streamGenerateContent?alt=sse",
            endpoint.trim_end_matches('/'),
            ctx.gemini_version(),
            model
        );

        // Add API key if using Gemini-style query auth
        if should_use_query_key(&ctx, endpoint) {
            if let Some(api_key) = ctx.get_api_key() {
                url = format!("{}&key={}", url, api_key);
            }
        }

        // Make request
        let response = client
            .post(&url)
            .headers(self.build_headers(&ctx))
            .json(&body)
            .send()
            .await
            .map_err(|e| ForwardError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
        }

        // Clone context for use in stream processing
        let ctx_clone = ctx.clone();
        let estimated_prompt_tokens = self.estimate_request_tokens(&payload);

        // Create usage tracker
        let usage_tracker = Arc::new(Mutex::new(TokenUsage::new(estimated_prompt_tokens, 0)));
        let usage_tracker_clone = Arc::clone(&usage_tracker);

        // Stream the response and parse SSE events
        let stream = response.bytes_stream().map(move |result| {
            match result {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for line in text.lines() {
                            if let Some(data) = parse_sse_data(line) {
                                if let Ok(json) = serde_json::from_str::<Value>(data) {
                                    // Extract usage from usageMetadata
                                    if let Some(metadata) = json.get("usageMetadata") {
                                        let prompt_tokens = metadata
                                            .get("promptTokenCount")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);
                                        let candidates_tokens = metadata
                                            .get("candidatesTokenCount")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);

                                        if let Ok(mut tracker) = usage_tracker_clone.lock() {
                                            if prompt_tokens > 0 {
                                                tracker.prompt_tokens = prompt_tokens;
                                            }
                                            if candidates_tokens > tracker.completion_tokens {
                                                tracker.completion_tokens = candidates_tokens;
                                            }
                                        }
                                    }

                                    // Estimate from content if no usage
                                    if let Some(candidates) =
                                        json.get("candidates").and_then(|c| c.as_array())
                                    {
                                        for candidate in candidates {
                                            if let Some(content) = candidate.get("content") {
                                                if let Some(parts) =
                                                    content.get("parts").and_then(|p| p.as_array())
                                                {
                                                    for part in parts {
                                                        if let Some(text) = part
                                                            .get("text")
                                                            .and_then(|t| t.as_str())
                                                        {
                                                            let tokens = estimate_tokens(text);
                                                            if let Ok(mut tracker) =
                                                                usage_tracker_clone.lock()
                                                            {
                                                                if tracker.completion_tokens == 0 {
                                                                    tracker.completion_tokens +=
                                                                        tokens;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(bytes)
                }
                Err(e) => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )),
            }
        });

        // Log usage when stream completes
        let ctx_for_log = ctx_clone;
        let usage_for_log = Arc::clone(&usage_tracker);

        let logged_stream = stream
            .chain(futures_util::stream::once(async move {
                if let Ok(usage) = usage_for_log.lock() {
                    ctx_for_log.log_usage(&usage);
                }
                Err(std::io::Error::new(std::io::ErrorKind::Other, "stream_end"))
            }))
            .filter_map(|result| async move {
                match result {
                    Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                    Err(e) if e.to_string() == "stream_end" => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .body(Body::from_stream(logged_stream))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
    }
}

/// Filter payload to only include allowed fields
fn filter_payload(payload: &Value, allowed: &[&str], _ctx: &ForwardContext) -> Value {
    if let Some(obj) = payload.as_object() {
        let filtered: serde_json::Map<String, Value> = obj
            .iter()
            .filter(|(key, _)| allowed.contains(&key.as_str()))
            .filter_map(|(k, v)| {
                if k == "generationConfig" {
                    filter_generation_config(v).map(|value| (k.clone(), value))
                } else {
                    sanitize_value(v).map(|value| (k.clone(), value))
                }
            })
            .collect();
        Value::Object(filtered)
    } else {
        payload.clone()
    }
}

/// Filter generationConfig to only include allowed fields
fn filter_generation_config(config: &Value) -> Option<Value> {
    if let Some(obj) = config.as_object() {
        let has_max_output = obj
            .get("maxOutputTokens")
            .and_then(sanitize_value)
            .is_some();
        let has_stop_sequences = obj
            .get("stopSequences")
            .and_then(sanitize_value)
            .is_some();

        let mut filtered = serde_json::Map::new();
        for (key, value) in obj {
            if !ALLOWED_GENERATION_CONFIG_FIELDS.contains(&key.as_str()) {
                continue;
            }

            match key.as_str() {
                "max_tokens" => {
                    if has_max_output {
                        continue;
                    }
                    if let Some(cleaned) = sanitize_value(value) {
                        filtered.insert("maxOutputTokens".to_string(), cleaned);
                    }
                }
                "stop" => {
                    if has_stop_sequences {
                        continue;
                    }
                    if let Some(cleaned) = sanitize_value(value) {
                        let mapped = match cleaned {
                            Value::String(text) => Value::Array(vec![Value::String(text)]),
                            other => other,
                        };
                        filtered.insert("stopSequences".to_string(), mapped);
                    }
                }
                _ => {
                    if let Some(cleaned) = sanitize_value(value) {
                        filtered.insert(key.clone(), cleaned);
                    }
                }
            }
        }

        if filtered.is_empty() {
            None
        } else {
            Some(Value::Object(filtered))
        }
    } else {
        sanitize_value(config)
    }
}

pub(crate) fn should_use_query_key(ctx: &ForwardContext, endpoint: &str) -> bool {
    if let Some(style) = ctx.upstream.api_style.as_deref() {
        if style.eq_ignore_ascii_case("gemini") {
            return true;
        }
    }
    is_google_gemini_endpoint(endpoint)
}

fn should_send_authorization(ctx: &ForwardContext) -> bool {
    !is_google_gemini_endpoint(ctx.primary_endpoint().unwrap_or(DEFAULT_ENDPOINT))
}

fn is_google_gemini_endpoint(endpoint: &str) -> bool {
    let normalized = endpoint.trim().to_ascii_lowercase();
    normalized.contains("googleapis.com")
}

fn upstream_style(ctx: &ForwardContext) -> Provider {
    ctx.upstream
        .api_style
        .as_deref()
        .and_then(Provider::from_str)
        .unwrap_or(Provider::Gemini)
}

fn with_provider(ctx: &ForwardContext, provider: Provider) -> ForwardContext {
    let mut next = ctx.clone();
    next.model.provider = provider;
    next
}

fn sanitize_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::String(text) if is_undefined_string(text) => None,
        Value::Array(items) => {
            let cleaned: Vec<Value> = items.iter().filter_map(sanitize_value).collect();
            if cleaned.is_empty() {
                None
            } else {
                Some(Value::Array(cleaned))
            }
        }
        Value::Object(obj) => {
            let mut cleaned = serde_json::Map::new();
            for (key, value) in obj {
                if let Some(entry) = sanitize_value(value) {
                    cleaned.insert(key.clone(), entry);
                }
            }
            if cleaned.is_empty() {
                None
            } else {
                Some(Value::Object(cleaned))
            }
        }
        _ => Some(value.clone()),
    }
}

fn is_undefined_string(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.eq_ignore_ascii_case("undefined") || trimmed == "[undefined]"
}

/// Extract usage from Gemini response
fn extract_usage(response: &Value) -> TokenUsage {
    if let Some(metadata) = response.get("usageMetadata") {
        let prompt = metadata
            .get("promptTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let candidates = metadata
            .get("candidatesTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let cached = metadata
            .get("cachedContentTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        TokenUsage::new(prompt + cached, candidates)
    } else {
        TokenUsage::default()
    }
}

fn epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn build_gemini_endpoints(ctx: &ForwardContext, path: &str) -> Vec<String> {
    let use_query_key = ctx
        .primary_endpoint()
        .map(|ep| should_use_query_key(ctx, ep))
        .unwrap_or(true);
    ctx.all_endpoints()
        .iter()
        .map(|ep| {
            let base = format!("{}{}", ep.trim_end_matches('/'), path);
            if use_query_key {
                if let Some(api_key) = ctx.get_api_key() {
                    format!("{}?key={}", base, api_key)
                } else {
                    base
                }
            } else {
                base
            }
        })
        .collect()
}

pub(crate) fn build_gemini_stream_url(ctx: &ForwardContext, model: &str) -> Option<String> {
    let endpoint = ctx.primary_endpoint()?;
    let mut url = format!(
        "{}/{}/models/{}:streamGenerateContent?alt=sse",
        endpoint.trim_end_matches('/'),
        ctx.gemini_version(),
        model
    );
    if should_use_query_key(ctx, endpoint) {
        if let Some(api_key) = ctx.get_api_key() {
            url = format!("{}&key={}", url, api_key);
        }
    }
    Some(url)
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

fn openai_content_to_gemini_parts(content: &Value) -> Vec<Value> {
    let mut parts_out = Vec::new();
    match content {
        Value::String(text) => {
            if !text.is_empty() {
                parts_out.push(serde_json::json!({ "text": text }));
            }
        }
        Value::Array(parts) => {
            for part in parts {
                let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match part_type {
                    "text" => {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            parts_out.push(serde_json::json!({ "text": text }));
                        }
                    }
                    "image_url" => {
                        let url = part
                            .get("image_url")
                            .and_then(|v| v.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some((mime, data)) = parse_data_url(url) {
                            parts_out.push(serde_json::json!({
                                "inline_data": { "mime_type": mime, "data": data }
                            }));
                        } else if !url.is_empty() {
                            parts_out.push(serde_json::json!({ "text": format!("[Image] {}", url) }));
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                parts_out.push(serde_json::json!({ "text": text }));
            }
        }
        _ => {}
    }
    parts_out
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }
    let rest = trimmed.trim_start_matches("data:");
    let mut parts = rest.splitn(2, ";base64,");
    let mime = parts.next()?.to_string();
    let data = parts.next()?.to_string();
    if mime.is_empty() || data.is_empty() {
        None
    } else {
        Some((mime, data))
    }
}

fn is_gemini_thought_part(part: &Value) -> bool {
    part.get("thought")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn map_openai_tools_to_gemini(tools: &Value) -> Option<Value> {
    let tools_array = tools.as_array()?;
    let mut declarations = Vec::new();

    for tool in tools_array {
        let tool_type = tool.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !tool_type.eq_ignore_ascii_case("function") {
            continue;
        }
        let function = tool.get("function")?;
        let name = function.get("name")?.as_str()?;

        let mut decl = serde_json::Map::new();
        decl.insert("name".to_string(), Value::String(name.to_string()));
        if let Some(desc) = function.get("description") {
            decl.insert("description".to_string(), desc.clone());
        }
        if let Some(params) = function.get("parameters") {
            decl.insert("parameters".to_string(), params.clone());
        }
        declarations.push(Value::Object(decl));
    }

    if declarations.is_empty() {
        None
    } else {
        Some(Value::Array(vec![serde_json::json!({
            "functionDeclarations": declarations
        })]))
    }
}

fn map_openai_tool_choice_to_gemini(choice: &Value) -> Option<Value> {
    let mode = match choice {
        Value::String(raw) => match raw.to_ascii_uppercase().as_str() {
            "AUTO" | "auto" => Some(("AUTO".to_string(), None)),
            "NONE" | "none" => Some(("NONE".to_string(), None)),
            _ => None,
        },
        Value::Object(obj) => {
            if obj.get("type").and_then(|v| v.as_str()) == Some("function") {
                let name = obj
                    .get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())?;
                Some(("ANY".to_string(), Some(vec![name.to_string()])))
            } else {
                None
            }
        }
        _ => None,
    }?;

    let mut config = serde_json::Map::new();
    config.insert("mode".to_string(), Value::String(mode.0));
    if let Some(names) = mode.1 {
        config.insert(
            "allowedFunctionNames".to_string(),
            Value::Array(names.into_iter().map(Value::String).collect()),
        );
    }

    Some(serde_json::json!({
        "functionCallingConfig": Value::Object(config)
    }))
}

fn map_gemini_tools_to_openai(tools: &Value) -> Option<Value> {
    let tools_array = tools.as_array()?;
    let mut mapped = Vec::new();

    for tool in tools_array {
        let declarations = tool
            .get("functionDeclarations")
            .or_else(|| tool.get("function_declarations"))
            .and_then(|v| v.as_array())?;
        for decl in declarations {
            let name = decl.get("name").and_then(|v| v.as_str())?;
            let mut func = serde_json::Map::new();
            func.insert("name".to_string(), Value::String(name.to_string()));
            if let Some(desc) = decl.get("description") {
                func.insert("description".to_string(), desc.clone());
            }
            if let Some(params) = decl.get("parameters") {
                func.insert("parameters".to_string(), params.clone());
            }
            let mut entry = serde_json::Map::new();
            entry.insert("type".to_string(), Value::String("function".to_string()));
            entry.insert("function".to_string(), Value::Object(func));
            mapped.push(Value::Object(entry));
        }
    }

    if mapped.is_empty() {
        None
    } else {
        Some(Value::Array(mapped))
    }
}

fn map_gemini_tool_config_to_openai(tool_config: &Value) -> Option<Value> {
    let config = tool_config
        .get("functionCallingConfig")
        .or_else(|| tool_config.get("function_calling_config"))?;
    let mode = config.get("mode").and_then(|v| v.as_str()).unwrap_or("");
    match mode.to_ascii_uppercase().as_str() {
        "AUTO" | "ANY" => Some(Value::String("auto".to_string())),
        "NONE" => Some(Value::String("none".to_string())),
        _ => {
            let names = config
                .get("allowedFunctionNames")
                .or_else(|| config.get("allowed_function_names"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let name = names
                .iter()
                .filter_map(|v| v.as_str())
                .next()?;
            Some(serde_json::json!({
                "type": "function",
                "function": { "name": name }
            }))
        }
    }
}

fn map_openai_finish_reason_to_gemini(reason: &str) -> String {
    match reason {
        "stop" => "STOP",
        "length" => "MAX_TOKENS",
        "content_filter" => "SAFETY",
        "tool_calls" | "function_call" => "STOP",
        _ => "STOP",
    }
    .to_string()
}

fn map_gemini_finish_reason_to_openai(reason: &str) -> Value {
    let mapped = match reason.to_ascii_uppercase().as_str() {
        "STOP" => "stop",
        "MAX_TOKENS" => "length",
        "SAFETY" | "RECITATION" => "content_filter",
        _ => "stop",
    };
    Value::String(mapped.to_string())
}

fn openai_tool_calls_to_gemini_parts(tool_calls: &Value) -> Vec<Value> {
    let mut parts = Vec::new();
    if let Some(calls) = tool_calls.as_array() {
        for call in calls {
            let name = call
                .get("function")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            let args_raw = call
                .get("function")
                .and_then(|v| v.get("arguments"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = serde_json::from_str::<Value>(args_raw)
                .unwrap_or(Value::String(args_raw.to_string()));
            parts.push(serde_json::json!({
                "functionCall": { "name": name, "args": args }
            }));
        }
    }
    parts
}

fn build_generation_config_from_openai(payload: &Value) -> Option<Value> {
    let mut config = serde_json::Map::new();

    if let Some(max_tokens) = payload.get("max_tokens").or_else(|| payload.get("max_completion_tokens")) {
        config.insert("maxOutputTokens".to_string(), max_tokens.clone());
    }
    if let Some(temperature) = payload.get("temperature") {
        config.insert("temperature".to_string(), temperature.clone());
    }
    if let Some(top_p) = payload.get("top_p") {
        config.insert("topP".to_string(), top_p.clone());
    }
    if let Some(penalty) = payload.get("presence_penalty") {
        config.insert("presencePenalty".to_string(), penalty.clone());
    }
    if let Some(penalty) = payload.get("frequency_penalty") {
        config.insert("frequencyPenalty".to_string(), penalty.clone());
    }
    if let Some(seed) = payload.get("seed") {
        config.insert("seed".to_string(), seed.clone());
    }
    if let Some(stop) = payload.get("stop") {
        let mapped = match stop {
            Value::String(text) => Value::Array(vec![Value::String(text.clone())]),
            _ => stop.clone(),
        };
        config.insert("stopSequences".to_string(), mapped);
    }

    if config.is_empty() {
        None
    } else {
        Some(Value::Object(config))
    }
}

fn map_generation_config_to_openai(config: &Value) -> serde_json::Map<String, Value> {
    let mut mapped = serde_json::Map::new();
    if let Some(max) = config.get("maxOutputTokens") {
        mapped.insert("max_tokens".to_string(), max.clone());
    }
    if let Some(temp) = config.get("temperature") {
        mapped.insert("temperature".to_string(), temp.clone());
    }
    if let Some(top_p) = config.get("topP") {
        mapped.insert("top_p".to_string(), top_p.clone());
    }
    if let Some(stop) = config.get("stopSequences") {
        mapped.insert("stop".to_string(), stop.clone());
    }
    if let Some(penalty) = config.get("presencePenalty") {
        mapped.insert("presence_penalty".to_string(), penalty.clone());
    }
    if let Some(penalty) = config.get("frequencyPenalty") {
        mapped.insert("frequency_penalty".to_string(), penalty.clone());
    }
    if let Some(seed) = config.get("seed") {
        mapped.insert("seed".to_string(), seed.clone());
    }
    mapped
}

/// Convert OpenAI request format to Gemini format
pub(crate) fn convert_openai_to_gemini_request(payload: &Value, _model: &str) -> Value {
    let mut gemini_request = serde_json::Map::new();
    let mut contents = Vec::new();
    let mut system_parts = Vec::new();

    if let Some(messages) = payload.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            if role.eq_ignore_ascii_case("system") {
                if let Some(content) = msg.get("content") {
                    let text = openai_content_to_gemini_parts(content)
                        .iter()
                        .filter_map(|p| p.get("text").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        system_parts.push(text);
                    }
                }
                continue;
            }
            let gem_role = if role.eq_ignore_ascii_case("assistant") {
                "model"
            } else {
                "user"
            };
            let mut parts = Vec::new();
            if let Some(content) = msg.get("content") {
                parts.extend(openai_content_to_gemini_parts(content));
            }
            if let Some(tool_calls) = msg.get("tool_calls") {
                parts.extend(openai_tool_calls_to_gemini_parts(tool_calls));
            }
            if let Some(function_call) = msg.get("function_call") {
                let name = function_call.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                let args_raw = function_call
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = serde_json::from_str::<Value>(args_raw)
                    .unwrap_or(Value::String(args_raw.to_string()));
                parts.push(serde_json::json!({ "functionCall": { "name": name, "args": args } }));
            }

            if parts.is_empty() {
                parts.push(serde_json::json!({ "text": "" }));
            }

            contents.push(serde_json::json!({
                "role": gem_role,
                "parts": parts
            }));
        }
    }

    if !contents.is_empty() {
        gemini_request.insert("contents".to_string(), Value::Array(contents));
    }

    if !system_parts.is_empty() {
        gemini_request.insert(
            "systemInstruction".to_string(),
            serde_json::json!({ "parts": [ { "text": system_parts.join("\n\n") } ] }),
        );
    }

    let mut generation_config = build_generation_config_from_openai(payload);
    if let Some(extra_config) = payload.get("generationConfig").and_then(filter_generation_config) {
        match (&mut generation_config, extra_config) {
            (Some(Value::Object(base)), Value::Object(extra)) => {
                for (key, value) in extra {
                    base.insert(key, value);
                }
            }
            (_, extra) => {
                generation_config = Some(extra);
            }
        }
    }
    if let Some(gen) = generation_config {
        gemini_request.insert("generationConfig".to_string(), gen);
    }

    if let Some(tools) = payload.get("tools") {
        if let Some(mapped) = map_openai_tools_to_gemini(tools) {
            gemini_request.insert("tools".to_string(), mapped);
        }
    }

    if let Some(tool_choice) = payload.get("tool_choice") {
        if let Some(mapped) = map_openai_tool_choice_to_gemini(tool_choice) {
            gemini_request.insert("toolConfig".to_string(), mapped);
        }
    }

    Value::Object(gemini_request)
}

/// Convert Gemini request format to OpenAI format
pub(crate) fn convert_gemini_to_openai_request(payload: &Value, model: &str) -> Value {
    let mut openai_request = serde_json::Map::new();
    openai_request.insert("model".to_string(), Value::String(model.to_string()));

    let mut messages = Vec::new();

    if let Some(system_instruction) = payload.get("systemInstruction") {
        if let Some(text) = system_instruction
            .get("parts")
            .and_then(|v| v.as_array())
            .and_then(|parts| parts.first())
            .and_then(|p| p.get("text"))
            .and_then(|v| v.as_str())
        {
            messages.push(serde_json::json!({
                "role": "system",
                "content": text
            }));
        }
    }

    if let Some(contents) = payload.get("contents").and_then(|v| v.as_array()) {
        let mut tool_index = 0usize;
        for content in contents {
            let role_raw = content.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let role = if role_raw.eq_ignore_ascii_case("model") {
                "assistant"
            } else {
                "user"
            };

            let mut parts_out = Vec::new();
            let mut tool_calls = Vec::new();
            let mut tool_messages = Vec::new();

            if let Some(parts) = content.get("parts").and_then(|v| v.as_array()) {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        parts_out.push(serde_json::json!({ "type": "text", "text": text }));
                    } else if let Some(inline) = part.get("inline_data") {
                        let mime = inline.get("mime_type").and_then(|v| v.as_str()).unwrap_or("application/octet-stream");
                        let data = inline.get("data").and_then(|v| v.as_str()).unwrap_or("");
                        if !data.is_empty() {
                            let url = format!("data:{};base64,{}", mime, data);
                            parts_out.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": { "url": url }
                            }));
                        }
                    } else if let Some(file) = part.get("file_data") {
                        let uri = file.get("file_uri").and_then(|v| v.as_str()).unwrap_or("");
                        if !uri.is_empty() {
                            parts_out.push(serde_json::json!({ "type": "text", "text": format!("[File] {}", uri) }));
                        }
                    } else if let Some(call) = part.get("functionCall") {
                        let name = call.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                        let args = call.get("args").cloned().unwrap_or(Value::Null);
                        let args_raw = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                        let id = format!("gemini_call_{}", tool_index);
                        tool_index += 1;
                        tool_calls.push(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": args_raw }
                        }));
                    } else if let Some(response) = part.get("functionResponse") {
                        let name = response.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                        let value = response.get("response").cloned().unwrap_or(Value::Null);
                        let content_text = value.to_string();
                        tool_messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": name,
                            "content": content_text
                        }));
                    }
                }
            }

            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String(role.to_string()));
            if parts_out.is_empty() {
                msg.insert("content".to_string(), Value::String(String::new()));
            } else {
                msg.insert("content".to_string(), openai_content_from_parts(parts_out));
            }
            if !tool_calls.is_empty() {
                msg.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }
            messages.push(Value::Object(msg));
            messages.extend(tool_messages);
        }
    }

    if !messages.is_empty() {
        openai_request.insert("messages".to_string(), Value::Array(messages));
    }

    if let Some(gen_config) = payload.get("generationConfig") {
        for (key, value) in map_generation_config_to_openai(gen_config) {
            openai_request.insert(key, value);
        }
    }

    if let Some(tools) = payload.get("tools") {
        if let Some(mapped) = map_gemini_tools_to_openai(tools) {
            openai_request.insert("tools".to_string(), mapped);
        }
    }
    if let Some(tool_config) = payload.get("toolConfig") {
        if let Some(mapped) = map_gemini_tool_config_to_openai(tool_config) {
            openai_request.insert("tool_choice".to_string(), mapped);
        }
    }

    Value::Object(openai_request)
}

/// Convert Gemini response format to OpenAI format
pub(crate) fn convert_gemini_response_to_openai(response: &Value, model: &str) -> Value {
    let id = format!("chatcmpl_gemini_{}", epoch_seconds());
    let created = epoch_seconds();

    let mut choices = Vec::new();
    if let Some(candidates) = response.get("candidates").and_then(|v| v.as_array()) {
        for (idx, candidate) in candidates.iter().enumerate() {
            let finish_reason = candidate
                .get("finishReason")
                .and_then(|v| v.as_str())
                .map(map_gemini_finish_reason_to_openai);

            let (content, tool_calls, reasoning) = candidate
                .get("content")
                .map(|c| {
                    let mut tool_calls = Vec::new();
                    let mut parts_out = Vec::new();
                    let mut thoughts = Vec::new();
                    if let Some(parts) = c.get("parts").and_then(|v| v.as_array()) {
                        for (tool_idx, part) in parts.iter().enumerate() {
                            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                if is_gemini_thought_part(part) {
                                    thoughts.push(text.to_string());
                                } else {
                                    parts_out.push(serde_json::json!({ "type": "text", "text": text }));
                                }
                            } else if let Some(call) = part.get("functionCall") {
                                let name = call.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                                let args = call.get("args").cloned().unwrap_or(Value::Null);
                                let args_raw = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                                tool_calls.push(serde_json::json!({
                                    "id": format!("gemini_call_{}", tool_idx),
                                    "type": "function",
                                    "function": { "name": name, "arguments": args_raw }
                                }));
                            }
                        }
                    }
                    let content_value = if parts_out.is_empty() {
                        Value::String(String::new())
                    } else {
                        openai_content_from_parts(parts_out)
                    };
                    let reasoning_value = if thoughts.is_empty() {
                        None
                    } else {
                        Some(Value::String(thoughts.join("\n")))
                    };
                    (content_value, tool_calls, reasoning_value)
                })
                .unwrap_or_else(|| (Value::String(String::new()), Vec::new(), None));

            let mut message = serde_json::Map::new();
            message.insert("role".to_string(), Value::String("assistant".to_string()));
            message.insert("content".to_string(), content);
            if let Some(reasoning_content) = reasoning {
                message.insert("reasoning_content".to_string(), reasoning_content);
            }
            if !tool_calls.is_empty() {
                message.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }

            choices.push(serde_json::json!({
                "index": idx as i64,
                "message": Value::Object(message),
                "finish_reason": finish_reason
            }));
        }
    }

    let usage = response.get("usageMetadata").map(|usage| {
        let prompt = usage
            .get("promptTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let cached = usage
            .get("cachedContentTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let completion = usage
            .get("candidatesTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        serde_json::json!({
            "prompt_tokens": prompt + cached,
            "completion_tokens": completion,
            "total_tokens": prompt + cached + completion
        })
    }).unwrap_or_else(|| serde_json::json!({
        "prompt_tokens": 0,
        "completion_tokens": 0,
        "total_tokens": 0
    }));

    serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": choices,
        "usage": usage
    })
}

/// Convert OpenAI response format to Gemini format
pub(crate) fn convert_openai_response_to_gemini(response: &Value, model: &str) -> Value {
    let mut candidates = Vec::new();
    if let Some(choices) = response.get("choices").and_then(|v| v.as_array()) {
        for (idx, choice) in choices.iter().enumerate() {
            let finish_reason = choice
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .map(map_openai_finish_reason_to_gemini);

            let message = choice.get("message").or_else(|| choice.get("delta"));
            let mut parts = Vec::new();
            if let Some(msg) = message {
                if let Some(reasoning) = msg.get("reasoning_content") {
                    if let Some(text) = reasoning.as_str() {
                        parts.push(serde_json::json!({ "text": text, "thought": true }));
                    } else if !reasoning.is_null() {
                        parts.push(serde_json::json!({
                            "text": reasoning.to_string(),
                            "thought": true
                        }));
                    }
                }
                if let Some(content) = msg.get("content") {
                    parts.extend(openai_content_to_gemini_parts(content));
                }
                if let Some(tool_calls) = msg.get("tool_calls") {
                    parts.extend(openai_tool_calls_to_gemini_parts(tool_calls));
                }
            }
            if parts.is_empty() {
                parts.push(serde_json::json!({ "text": "" }));
            }

            candidates.push(serde_json::json!({
                "index": idx as i64,
                "content": { "role": "model", "parts": parts },
                "finishReason": finish_reason
            }));
        }
    }

    let usage = response.get("usage").map(|usage| {
        let prompt = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let completion = usage
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        serde_json::json!({
            "promptTokenCount": prompt,
            "candidatesTokenCount": completion,
            "totalTokenCount": prompt + completion
        })
    });

    let mut response_obj = serde_json::Map::new();
    response_obj.insert("candidates".to_string(), Value::Array(candidates));
    response_obj.insert("modelVersion".to_string(), Value::String(model.to_string()));
    if let Some(usage_metadata) = usage {
        response_obj.insert("usageMetadata".to_string(), usage_metadata);
    }

    Value::Object(response_obj)
}

pub(crate) struct GeminiToOpenAIStreamState {
    pub id: String,
    pub created: i64,
    pub model: String,
    pub sent_role: bool,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub finished: bool,
}

impl GeminiToOpenAIStreamState {
    pub fn new(model: &str) -> Self {
        let now = epoch_seconds();
        Self {
            id: format!("chatcmpl_gemini_{}", now),
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
    state: &GeminiToOpenAIStreamState,
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

pub(crate) fn convert_gemini_event_to_openai_chunks(
    event: &Value,
    state: &mut GeminiToOpenAIStreamState,
) -> Vec<Value> {
    let mut out = Vec::new();

    if let Some(metadata) = event.get("usageMetadata") {
        let prompt = metadata
            .get("promptTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let completion = metadata
            .get("candidatesTokenCount")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if prompt > 0 {
            state.prompt_tokens = prompt;
        }
        if completion > 0 {
            state.completion_tokens = completion;
        }
    }

    if let Some(candidates) = event.get("candidates").and_then(|v| v.as_array()) {
        for candidate in candidates {
            if !state.sent_role {
                out.push(build_openai_stream_chunk(
                    state,
                    serde_json::json!({ "role": "assistant" }),
                    None,
                    false,
                ));
                state.sent_role = true;
            }

            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content.get("parts").and_then(|v| v.as_array()) {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            let delta = if is_gemini_thought_part(part) {
                                serde_json::json!({ "reasoning_content": text })
                            } else {
                                serde_json::json!({ "content": text })
                            };
                            out.push(build_openai_stream_chunk(state, delta, None, false));
                        } else if let Some(call) = part.get("functionCall") {
                            let name = call.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                            let args = call.get("args").cloned().unwrap_or(Value::Null);
                            let args_raw =
                                serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                            let delta = serde_json::json!({
                                "tool_calls": [{
                                    "index": 0,
                                    "id": "gemini_call",
                                    "type": "function",
                                    "function": { "name": name, "arguments": args_raw }
                                }]
                            });
                            out.push(build_openai_stream_chunk(state, delta, None, false));
                        }
                    }
                }
            }

            let finish_reason = candidate
                .get("finishReason")
                .and_then(|v| v.as_str())
                .map(map_gemini_finish_reason_to_openai);
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
    }

    out
}

pub(crate) struct OpenAIToGeminiStreamState {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

impl OpenAIToGeminiStreamState {
    pub fn new() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
        }
    }
}

pub(crate) fn convert_openai_chunk_to_gemini(
    chunk: &Value,
    state: &mut OpenAIToGeminiStreamState,
) -> Vec<Value> {
    let mut out = Vec::new();

    if let Some(usage) = chunk.get("usage") {
        if let Some(prompt) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
            state.prompt_tokens = prompt;
        }
        if let Some(completion) = usage.get("completion_tokens").and_then(|v| v.as_i64()) {
            state.completion_tokens = completion;
        }
    }

    if let Some(choices) = chunk.get("choices").and_then(|v| v.as_array()) {
        for choice in choices {
            let delta = choice.get("delta").unwrap_or(&Value::Null);
            let mut parts = Vec::new();

            if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                parts.push(serde_json::json!({ "text": text }));
            }
            if let Some(reasoning) = delta.get("reasoning_content") {
                if let Some(text) = reasoning.as_str() {
                    parts.push(serde_json::json!({ "text": text, "thought": true }));
                } else if !reasoning.is_null() {
                    parts.push(serde_json::json!({
                        "text": reasoning.to_string(),
                        "thought": true
                    }));
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls") {
                parts.extend(openai_tool_calls_to_gemini_parts(tool_calls));
            }

            let finish_reason = choice
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .map(map_openai_finish_reason_to_gemini);

            if parts.is_empty() && finish_reason.is_none() && chunk.get("usage").is_none() {
                continue;
            }

            let mut candidate = serde_json::Map::new();
            candidate.insert(
                "content".to_string(),
                serde_json::json!({ "role": "model", "parts": parts }),
            );
            if let Some(reason) = finish_reason {
                candidate.insert("finishReason".to_string(), Value::String(reason));
            }

            let mut event = serde_json::Map::new();
            event.insert("candidates".to_string(), Value::Array(vec![Value::Object(candidate)]));

            if state.prompt_tokens > 0 || state.completion_tokens > 0 {
                event.insert(
                    "usageMetadata".to_string(),
                    serde_json::json!({
                        "promptTokenCount": state.prompt_tokens,
                        "candidatesTokenCount": state.completion_tokens,
                        "totalTokenCount": state.prompt_tokens + state.completion_tokens
                    }),
                );
            }

            out.push(Value::Object(event));
        }
    }

    out
}

fn estimate_gemini_prompt_tokens(payload: &Value) -> i64 {
    let contents = payload
        .get("contents")
        .map(|c| c.to_string())
        .unwrap_or_default();
    estimate_tokens(&contents)
}

fn build_openai_headers(ctx: &ForwardContext) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(api_key) = ctx.get_api_key() {
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", api_key)) {
            headers.insert("authorization", value);
        }
    }
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers
}

async fn handle_gemini_to_openai_request(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<UpstreamResponse> {
    let start = Instant::now();
    let upstream_ctx = with_provider(&ctx, Provider::OpenAI);

    let mut body = convert_gemini_to_openai_request(&payload, ctx.model.upstream_model());
    client::normalize_stream_flag(&mut body);

    let config = ctx.retry_config();
    let client = client::default_client()?;
    let headers = build_openai_headers(&upstream_ctx);

    let result = client::send_with_retry(
        &client,
        upstream_ctx.all_endpoints(),
        "/chat/completions",
        headers,
        &body,
        &config,
    )
    .await?;

    let status = result.response.status();
    let status_code = status.as_u16();
    let response_text = result.response.text().await.map_err(|e| {
        ForwardError::RequestFailed(format!("Failed to read response: {}", e))
    })?;
    let response_body: Value = client::parse_json_response(&response_text)
        .map_err(|e| ForwardError::RequestFailed(format!("Failed to parse response: {}", e)))?;

    if !status.is_success() {
        return Err(ForwardError::RequestFailed(format!(
            "Upstream returned {}: {}",
            status_code,
            response_body.to_string()
        )));
    }

    let gemini_body = convert_openai_response_to_gemini(&response_body, ctx.model.upstream_model());
    let mut usage = extract_usage(&gemini_body);
    if usage.prompt_tokens == 0 {
        usage.prompt_tokens = estimate_gemini_prompt_tokens(&payload);
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    ctx.log_usage(&usage);

    Ok(UpstreamResponse {
        body: gemini_body,
        latency_ms,
        status: status_code,
        usage,
    })
}

async fn handle_gemini_to_anthropic_request(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<UpstreamResponse> {
    let start = Instant::now();
    let upstream_ctx = with_provider(&ctx, Provider::Anthropic);

    let openai_payload = convert_gemini_to_openai_request(&payload, ctx.model.upstream_model());
    let mut anthropic_payload =
        anthropic::convert_openai_to_anthropic_request(&openai_payload, ctx.model.upstream_model());
    client::normalize_stream_flag(&mut anthropic_payload);

    let handler = anthropic::AnthropicHandler;
    let mut headers = handler.build_headers(&upstream_ctx);
    headers.insert("accept", HeaderValue::from_static("application/json"));

    let config = ctx.retry_config();
    let client = client::default_client()?;
    let result = client::send_with_retry(
        &client,
        upstream_ctx.all_endpoints(),
        "/v1/messages",
        headers,
        &anthropic_payload,
        &config,
    )
    .await?;

    let status = result.response.status();
    let status_code = status.as_u16();
    let response_text = result.response.text().await.map_err(|e| {
        ForwardError::RequestFailed(format!("Failed to read response: {}", e))
    })?;
    let response_body: Value = client::parse_json_response(&response_text)
        .map_err(|e| ForwardError::RequestFailed(format!("Failed to parse response: {}", e)))?;

    if !status.is_success() {
        return Err(ForwardError::RequestFailed(format!(
            "Upstream returned {}: {}",
            status_code,
            response_body.to_string()
        )));
    }

    let openai_response =
        anthropic::convert_anthropic_response_to_openai(&response_body, ctx.model.upstream_model());
    let gemini_body = convert_openai_response_to_gemini(&openai_response, ctx.model.upstream_model());
    let mut usage = extract_usage(&gemini_body);
    if usage.prompt_tokens == 0 {
        usage.prompt_tokens = estimate_gemini_prompt_tokens(&payload);
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    ctx.log_usage(&usage);

    Ok(UpstreamResponse {
        body: gemini_body,
        latency_ms,
        status: status_code,
        usage,
    })
}

async fn handle_gemini_to_openai_stream(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<Response> {
    let upstream_ctx = with_provider(&ctx, Provider::OpenAI);
    let mut body = convert_gemini_to_openai_request(&payload, ctx.model.upstream_model());
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), Value::Bool(true));
        obj.insert(
            "stream_options".to_string(),
            serde_json::json!({ "include_usage": true }),
        );
    }

    let headers = build_openai_headers(&upstream_ctx);
    let client = client::streaming_client()?;
    let endpoint = upstream_ctx.primary_endpoint().ok_or_else(|| {
        ForwardError::UpstreamNotFound("No endpoints configured".to_string())
    })?;
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| ForwardError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
    }

    let estimated_prompt_tokens = estimate_gemini_prompt_tokens(&payload);
    let state = Arc::new(Mutex::new({
        let mut s = OpenAIToGeminiStreamState::new();
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
                                let events = convert_openai_chunk_to_gemini(&json, &mut state);
                                for event in events {
                                    output.extend_from_slice(
                                        format!("data: {}\n\n", event).as_bytes(),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            logger::error("gemini", &format!("Failed to parse OpenAI SSE JSON: {}", e));
                        }
                    }
                }
            }
            Ok(Bytes::from(output))
        }
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
    });

    let ctx_for_log = ctx.clone();
    let state_for_log = Arc::clone(&state);

    let logged_stream = stream
        .chain(futures_util::stream::once(async move {
            if let Ok(state) = state_for_log.lock() {
                let usage = TokenUsage::new(state.prompt_tokens, state.completion_tokens);
                ctx_for_log.log_usage(&usage);
            }
            Ok(Bytes::new())
        }))
        .filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                Err(e) => {
                    logger::error("gemini", &format!("Stream filter error: {}", e));
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
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

async fn handle_gemini_to_anthropic_stream(
    ctx: ForwardContext,
    payload: Value,
) -> ForwardResult<Response> {
    let upstream_ctx = with_provider(&ctx, Provider::Anthropic);
    let openai_payload = convert_gemini_to_openai_request(&payload, ctx.model.upstream_model());
    let mut anthropic_payload =
        anthropic::convert_openai_to_anthropic_request(&openai_payload, ctx.model.upstream_model());
    if let Some(obj) = anthropic_payload.as_object_mut() {
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
        .json(&anthropic_payload)
        .send()
        .await
        .map_err(|e| ForwardError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ForwardError::RequestFailed(format!("{}: {}", status, text)));
    }

    let estimated_prompt_tokens = estimate_gemini_prompt_tokens(&payload);
    let anthropic_state = Arc::new(Mutex::new({
        let mut s = anthropic::AnthropicToOpenAIStreamState::new(ctx.model.upstream_model());
        s.prompt_tokens = estimated_prompt_tokens;
        s
    }));
    let anthropic_state_clone = Arc::clone(&anthropic_state);
    let gemini_state = Arc::new(Mutex::new({
        let mut s = OpenAIToGeminiStreamState::new();
        s.prompt_tokens = estimated_prompt_tokens;
        s
    }));
    let gemini_state_clone = Arc::clone(&gemini_state);

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
                            if let Ok(mut state) = anthropic_state_clone.lock() {
                                let openai_chunks =
                                    anthropic::convert_anthropic_event_to_openai_chunks(
                                        &json, &mut state,
                                    );
                                for chunk in openai_chunks {
                                    let mut gemini_state = gemini_state_clone.lock().unwrap();
                                    let events =
                                        convert_openai_chunk_to_gemini(&chunk, &mut gemini_state);
                                    for event in events {
                                        output.extend_from_slice(
                                            format!("data: {}\n\n", event).as_bytes(),
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            logger::error("gemini", &format!("Failed to parse Anthropic SSE JSON: {}", e));
                        }
                    }
                }
            }
            Ok(Bytes::from(output))
        }
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
    });

    let ctx_for_log = ctx.clone();
    let gemini_state_for_log = Arc::clone(&gemini_state);

    let logged_stream = stream
        .chain(futures_util::stream::once(async move {
            if let Ok(state) = gemini_state_for_log.lock() {
                let usage = TokenUsage::new(state.prompt_tokens, state.completion_tokens);
                ctx_for_log.log_usage(&usage);
            }
            Ok(Bytes::new())
        }))
        .filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(Ok::<Bytes, std::io::Error>(bytes)),
                Err(e) => {
                    logger::error("gemini", &format!("Stream filter error: {}", e));
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
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_payload() {
        let payload = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "generationConfig": {"temperature": 0.7},
            "custom_field": "should_be_removed"
        });

        // Create a minimal context for testing
        let ctx = create_test_context();
        let filtered = filter_payload(&payload, ALLOWED_FIELDS, &ctx);
        let obj = filtered.as_object().unwrap();

        assert!(obj.contains_key("contents"));
        assert!(obj.contains_key("generationConfig"));
        assert!(!obj.contains_key("custom_field"));
    }

    #[test]
    fn test_filter_generation_config_nested_fields() {
        // Test that extra fields inside generationConfig are removed
        // This is the fix for: "custom.input_examples: Extra inputs are not permitted"
        let payload = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 1000,
                "custom": {
                    "input_examples": ["example1", "example2"]
                },
                "extra_field": "should_be_removed"
            }
        });

        let ctx = create_test_context();
        let filtered = filter_payload(&payload, ALLOWED_FIELDS, &ctx);
        let gen_config = filtered
            .get("generationConfig")
            .unwrap()
            .as_object()
            .unwrap();

        // Allowed fields should be preserved
        assert!(gen_config.contains_key("temperature"));
        assert!(gen_config.contains_key("maxOutputTokens"));

        // Extra fields should be removed
        assert!(!gen_config.contains_key("custom"));
        assert!(!gen_config.contains_key("extra_field"));
    }

    #[test]
    fn test_filter_payload_removes_undefined_and_maps_openai_fields() {
        let payload = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello"}]}],
            "systemInstruction": "[undefined]",
            "safetySettings": "[undefined]",
            "generationConfig": {
                "temperature": "[undefined]",
                "max_tokens": 256,
                "stop": "END",
                "topP": 0.9
            }
        });

        let ctx = create_test_context();
        let filtered = filter_payload(&payload, ALLOWED_FIELDS, &ctx);
        let obj = filtered.as_object().unwrap();

        assert!(obj.get("systemInstruction").is_none());
        assert!(obj.get("safetySettings").is_none());

        let gen_config = obj.get("generationConfig").unwrap().as_object().unwrap();
        assert!(gen_config.get("temperature").is_none());
        assert_eq!(
            gen_config.get("maxOutputTokens").and_then(|v| v.as_i64()),
            Some(256)
        );
        assert!(gen_config.get("max_tokens").is_none());
        assert_eq!(
            gen_config
                .get("stopSequences")
                .and_then(|v| v.as_array())
                .map(|v| v.len()),
            Some(1)
        );
        assert!(gen_config.get("topP").is_some());
    }

    #[test]
    fn test_extract_usage() {
        let response = serde_json::json!({
            "usageMetadata": {
                "promptTokenCount": 100,
                "candidatesTokenCount": 50,
                "totalTokenCount": 150
            }
        });

        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn test_extract_usage_with_cache() {
        let response = serde_json::json!({
            "usageMetadata": {
                "promptTokenCount": 100,
                "candidatesTokenCount": 50,
                "cachedContentTokenCount": 20,
                "totalTokenCount": 170
            }
        });

        let usage = extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 120); // 100 + 20 cached
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn test_multimodal_message_format() {
        // Test that Gemini multimodal messages are preserved correctly
        let payload = serde_json::json!({
            "contents": [{
                "parts": [
                    {
                        "text": "What's in this image?"
                    },
                    {
                        "inline_data": {
                            "mime_type": "image/jpeg",
                            "data": "base64encodeddata..."
                        }
                    }
                ]
            }],
            "generationConfig": {
                "temperature": 0.4,
                "maxOutputTokens": 1000
            }
        });

        let ctx = create_test_context();
        let filtered = filter_payload(&payload, ALLOWED_FIELDS, &ctx);
        let contents = filtered.get("contents").unwrap().as_array().unwrap();
        let parts = contents[0].get("parts").unwrap().as_array().unwrap();

        assert_eq!(parts.len(), 2);
        assert!(parts[0].get("text").is_some());
        assert!(parts[1].get("inline_data").is_some());

        let inline_data = parts[1].get("inline_data").unwrap();
        assert_eq!(inline_data.get("mime_type").unwrap(), "image/jpeg");
    }

    #[test]
    fn test_multimodal_file_uri() {
        // Test file URI format (for files uploaded to Gemini)
        let payload = serde_json::json!({
            "contents": [{
                "parts": [
                    {"text": "Describe this video"},
                    {
                        "file_data": {
                            "mime_type": "video/mp4",
                            "file_uri": "https://generativelanguage.googleapis.com/v1beta/files/abc123"
                        }
                    }
                ]
            }]
        });

        let ctx = create_test_context();
        let filtered = filter_payload(&payload, ALLOWED_FIELDS, &ctx);
        let contents = filtered.get("contents").unwrap().as_array().unwrap();
        let parts = contents[0].get("parts").unwrap().as_array().unwrap();

        assert!(parts[1].get("file_data").is_some());
    }

    fn create_test_context() -> ForwardContext {
        use crate::forward::context::*;

        ForwardContext {
            auth_mode: AuthMode::UseConfiguredKey,
            model: ModelInfo {
                id: "gemini-pro".to_string(),
                display_name: "Gemini Pro".to_string(),
                provider: Provider::Gemini,
                upstream_id: "gemini".to_string(),
                upstream_model_id: None,
                price_prompt_per_1k: 0.0,
                price_completion_per_1k: 0.0,
            },
            upstream: UpstreamInfo {
                id: "gemini".to_string(),
                endpoints: vec!["https://generativelanguage.googleapis.com".to_string()],
                api_style: Some("gemini".to_string()),
                api_key: Some("test-key".to_string()),
            },
            gemini_api_version: None,
            meta: RequestMeta::default(),
            is_streaming: false,
            retry_max_attempts_override: None,
        }
    }
}
