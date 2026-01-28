//! Forward middleware
//!
//! Handles request parsing, authentication, and routing to appropriate handlers.

use axum::http::HeaderMap;
use rand::seq::SliceRandom;
use serde_json::Value;
use std::collections::HashMap;

use crate::config;

use super::context::{
    AuthMode, ForwardContext, ForwardPlan, ModelInfo, Provider, RequestMeta, UpstreamInfo,
};
use super::error::{ForwardError, ForwardResult};

/// Header name for CCR forward token
const FORWARD_TOKEN_HEADER: &str = "x-ccr-forward-token";

/// Extract authentication token from request headers
///
/// Priority order:
/// 1. x-ccr-forward-token (custom header)
/// 2. Authorization: Bearer <token>
/// 3. x-api-key (Anthropic style)
/// 4. x-goog-api-key (Gemini style)
pub fn extract_request_token(headers: &HeaderMap) -> Option<String> {
    // Priority 1: Custom forward token header
    if let Some(token) = extract_header_value(headers, FORWARD_TOKEN_HEADER) {
        return Some(token);
    }

    // Priority 2: Authorization Bearer
    if let Some(auth) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| raw.strip_prefix("Bearer "))
    {
        let token = auth.trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }

    // Priority 3: x-api-key (Anthropic style)
    if let Some(token) = extract_header_value(headers, "x-api-key") {
        return Some(token);
    }

    // Priority 4: x-goog-api-key (Gemini style)
    if let Some(token) = extract_header_value(headers, "x-goog-api-key") {
        return Some(token);
    }

    None
}

/// Extract and trim a header value
fn extract_header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Determine authentication mode based on request token and configured forward_token
///
/// Returns:
/// - `UseConfiguredKey`: if token matches forward_token, use upstream's configured api_key
/// - `UseRequestToken`: if no forward_token configured OR token doesn't match, treat request token as API key
pub fn determine_auth_mode(headers: &HeaderMap) -> ForwardResult<AuthMode> {
    let cfg = config::load();
    let request_token = extract_request_token(headers);

    match &cfg.forward_token {
        Some(forward_token) if !forward_token.is_empty() => {
            // System has forward_token configured
            match request_token {
                Some(token) if token == *forward_token => {
                    // Token matches forward_token -> use configured upstream API key
                    Ok(AuthMode::UseConfiguredKey)
                }
                Some(token) => {
                    // Token doesn't match forward_token -> treat as API key (passthrough)
                    Ok(AuthMode::UseRequestToken(token))
                }
                None => {
                    // No token provided but forward_token is configured -> error
                    Err(ForwardError::Unauthorized(
                        "Missing authentication token".to_string(),
                    ))
                }
            }
        }
        _ => {
            // No forward_token configured -> passthrough mode
            match request_token {
                Some(token) => Ok(AuthMode::UseRequestToken(token)),
                None => {
                    // No token at all, might still work if upstream has api_key configured
                    Ok(AuthMode::UseConfiguredKey)
                }
            }
        }
    }
}

/// Extract request metadata from headers
pub fn extract_request_meta(headers: &HeaderMap) -> RequestMeta {
    RequestMeta {
        channel: extract_header_value(headers, "x-ccr-channel")
            .unwrap_or_else(|| "web".to_string()),
        tool: extract_header_value(headers, "x-ccr-tool").unwrap_or_else(|| "unknown".to_string()),
    }
}

/// Check if request is streaming
pub fn is_streaming_request(payload: &Value) -> bool {
    match payload.get("stream") {
        Some(Value::Bool(stream)) => *stream,
        Some(Value::Number(value)) => value.as_i64().map(|v| v != 0).unwrap_or(false),
        Some(Value::String(value)) => {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "true" | "1" | "yes" | "on")
        }
        _ => false,
    }
}

fn is_gemini_streaming_request(payload: &Value, endpoint_path: &str) -> bool {
    if is_streaming_request(payload) {
        return true;
    }
    let normalized = endpoint_path.to_ascii_lowercase();
    normalized.contains("streamgeneratecontent")
}

/// Extract model ID from request payload
pub fn extract_model_id(payload: &Value) -> ForwardResult<String> {
    payload
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ForwardError::InvalidRequest("Missing or empty 'model' field".to_string()))
}

/// Find model configuration by ID or display name (case-insensitive)
/// If "auto" is specified, returns the highest priority available model
#[allow(dead_code)]
pub fn find_model_config(model_id: &str) -> ForwardResult<config::ModelCfg> {
    let cfg = config::load();

    crate::logger::debug("middleware", &format!("Looking up model: {}", model_id));

    // Handle "auto" mode - return highest priority model
    if model_id.eq_ignore_ascii_case("auto") {
        let model = cfg
            .models
            .iter()
            .filter(|m| !m.is_temporary) // Exclude temporary models from auto selection
            .max_by_key(|m| m.priority)
            .cloned()
            .ok_or_else(|| ForwardError::ModelNotFound("No models configured for auto routing".to_string()))?;

        crate::logger::info("middleware", &format!("Auto selected model: {} (priority: {}, upstream: {})", model.id, model.priority, model.upstream_id));
        return Ok(model);
    }

    // Normal model lookup
    let model_id_lower = model_id.to_lowercase();
    let mut models: Vec<_> = cfg
        .models
        .iter()
        .filter(|m| {
            m.id.eq_ignore_ascii_case(model_id)
                || m.display_name.eq_ignore_ascii_case(model_id)
                || model_id_lower.contains(&m.id.to_lowercase())
        })
        .cloned()
        .collect();

    if models.is_empty() {
        return Err(ForwardError::ModelNotFound(format!(
            "Model '{}' not configured",
            model_id
        )));
    }

    models.sort_by(|a, b| b.priority.cmp(&a.priority));
    if models.len() > 1 {
        crate::logger::warn(
            "middleware",
            &format!(
                "Multiple models matched '{}', selecting highest priority: {}",
                model_id, models[0].id
            ),
        );
    }
    let model = models[0].clone();

    crate::logger::debug("middleware", &format!("Found model: {} (priority: {}, upstream: {})", model.id, model.priority, model.upstream_id));
    Ok(model)
}

/// Find all models with the same ID, sorted by priority (highest first)
#[allow(dead_code)]
pub fn find_models_by_priority(model_id: &str) -> ForwardResult<Vec<config::ModelCfg>> {
    let cfg = config::load();

    // For "auto" mode, return all non-temporary models sorted by priority
    if model_id.eq_ignore_ascii_case("auto") {
        let mut models: Vec<_> = cfg.models
            .iter()
            .filter(|m| !m.is_temporary)
            .cloned()
            .collect();
        models.sort_by(|a, b| b.priority.cmp(&a.priority));
        if models.is_empty() {
            return Err(ForwardError::ModelNotFound("No models configured for auto routing".to_string()));
        }
        return Ok(models);
    }

    // Find all models matching the ID (supports duplicate IDs with different priorities)
    let mut models: Vec<_> = cfg.models
        .iter()
        .filter(|m| m.id.eq_ignore_ascii_case(model_id) || m.display_name.eq_ignore_ascii_case(model_id))
        .cloned()
        .collect();

    if models.is_empty() {
        return Err(ForwardError::ModelNotFound(format!("Model '{}' not configured", model_id)));
    }

    // Sort by priority (highest first)
    models.sort_by(|a, b| b.priority.cmp(&a.priority));
    Ok(models)
}

fn collect_models_for_id(
    model_id: &str,
    cfg: &config::Settings,
) -> ForwardResult<Vec<config::ModelCfg>> {
    if model_id.eq_ignore_ascii_case("auto") {
        let model = cfg
            .models
            .iter()
            .filter(|m| !m.is_temporary)
            .max_by_key(|m| m.priority)
            .cloned()
            .ok_or_else(|| {
                ForwardError::ModelNotFound("No models configured for auto routing".to_string())
            })?;
        return Ok(vec![model]);
    }

    let mut models: Vec<_> = cfg
        .models
        .iter()
        .filter(|m| m.id.eq_ignore_ascii_case(model_id) || m.display_name.eq_ignore_ascii_case(model_id))
        .cloned()
        .collect();

    if models.is_empty() {
        return Err(ForwardError::ModelNotFound(format!(
            "Model '{}' not configured",
            model_id
        )));
    }

    models.sort_by(|a, b| b.priority.cmp(&a.priority));
    Ok(models)
}

fn resolve_routes_for_models(models: &[config::ModelCfg]) -> Vec<config::ModelRoute> {
    let mut routes = Vec::new();
    for model in models {
        routes.extend(model.resolved_routes());
    }
    routes
}

fn filter_routes_by_provider(
    routes: Vec<config::ModelRoute>,
    provider_hint: Option<Provider>,
) -> ForwardResult<Vec<config::ModelRoute>> {
    if let Some(provider) = provider_hint {
        let provider_name = provider.as_str();
        let filtered: Vec<_> = routes
            .into_iter()
            .filter(|route| route.provider.eq_ignore_ascii_case(provider_name))
            .collect();
        if filtered.is_empty() {
            return Err(ForwardError::ModelNotFound(format!(
                "Model is not configured for provider '{}'",
                provider_name
            )));
        }
        return Ok(filtered);
    }
    Ok(routes)
}

fn order_routes_for_attempts(mut routes: Vec<config::ModelRoute>) -> Vec<config::ModelRoute> {
    if routes.len() <= 1 {
        return routes;
    }

    let any_priority = routes.iter().any(|route| route.priority.is_some());
    let mut rng = rand::thread_rng();

    if !any_priority {
        routes.shuffle(&mut rng);
        return routes;
    }

    let mut groups: HashMap<u32, Vec<config::ModelRoute>> = HashMap::new();
    for route in routes {
        let priority = route.priority.unwrap_or(0);
        groups.entry(priority).or_default().push(route);
    }

    let mut grouped: Vec<_> = groups.into_iter().collect();
    grouped.sort_by(|a, b| b.0.cmp(&a.0));

    let mut ordered = Vec::new();
    for (_, mut group_routes) in grouped {
        group_routes.shuffle(&mut rng);
        ordered.extend(group_routes);
    }

    ordered
}

fn build_plan_from_routes(
    auth_mode: AuthMode,
    meta: RequestMeta,
    is_streaming: bool,
    model_cfg: config::ModelCfg,
    routes: Vec<config::ModelRoute>,
    enable_retry_fallback: bool,
    gemini_api_version: Option<&str>,
) -> ForwardResult<ForwardPlan> {
    let ordered_routes = order_routes_for_attempts(routes);
    if ordered_routes.is_empty() {
        return Err(ForwardError::ModelNotFound(format!(
            "Model '{}' has no configured routes",
            model_cfg.id
        )));
    }

    let retry_override = if enable_retry_fallback && ordered_routes.len() > 1 {
        Some(1)
    } else {
        None
    };

    let mut contexts = Vec::new();
    for route in ordered_routes {
        if route.upstream_id.trim().is_empty() {
            return Err(ForwardError::InvalidRequest(format!(
                "Missing upstream_id for model '{}'",
                model_cfg.id
            )));
        }
        if route.upstream_id.eq_ignore_ascii_case("auto") {
            return Err(ForwardError::InvalidRequest(
                "Upstream auto-selection is disabled; please set an explicit upstream_id"
                    .to_string(),
            ));
        }

        let upstream_cfg = find_upstream_config(&route.upstream_id).map_err(|e| {
            crate::logger::error(
                "middleware",
                &format!(
                    "Upstream lookup failed: upstream_id='{}', error={}",
                    route.upstream_id, e
                ),
            );
            e
        })?;

        let provider = Provider::from_str(&route.provider).ok_or_else(|| {
            let err = ForwardError::InvalidRequest(format!(
                "Unknown provider: {}",
                route.provider
            ));
            crate::logger::error("middleware", &format!("Invalid provider: {}", route.provider));
            err
        })?;

        let upstream_model_id = route
            .upstream_model_id
            .clone()
            .or_else(|| model_cfg.upstream_model_id.clone())
            .filter(|s| !s.trim().is_empty());

        let gemini_version = if matches!(provider, Provider::Gemini) {
            gemini_api_version.map(|s| s.to_string())
        } else {
            None
        };

        contexts.push(ForwardContext {
            auth_mode: auth_mode.clone(),
            model: ModelInfo {
                id: model_cfg.id.clone(),
                display_name: model_cfg.display_name.clone(),
                provider,
                upstream_id: upstream_cfg.id.clone(),
                upstream_model_id,
                price_prompt_per_1k: model_cfg.price_prompt_per_1k,
                price_completion_per_1k: model_cfg.price_completion_per_1k,
            },
            upstream: UpstreamInfo {
                id: upstream_cfg.id,
                endpoints: upstream_cfg.endpoints,
                api_style: upstream_cfg.api_style,
                api_key: upstream_cfg.api_key,
            },
            gemini_api_version: gemini_version,
            meta: meta.clone(),
            is_streaming,
            retry_max_attempts_override: retry_override,
        });
    }

    let primary = contexts
        .first()
        .cloned()
        .ok_or_else(|| ForwardError::ModelNotFound("No routes configured".to_string()))?;
    let fallbacks = if enable_retry_fallback {
        contexts.into_iter().skip(1).collect()
    } else {
        Vec::new()
    };

    Ok(ForwardPlan { primary, fallbacks })
}

/// Find upstream configuration by ID (case-insensitive)
///
/// Supports:
/// 1. Direct ID match (case-insensitive)
/// 2. Index-based lookup (for legacy configs)
/// 3. Single upstream fallback
pub fn find_upstream_config(upstream_id: &str) -> ForwardResult<config::Upstream> {
    let cfg = config::load();

    // Try by ID first (case-insensitive)
    if let Some(upstream) = cfg.upstreams.iter().find(|u| u.id.eq_ignore_ascii_case(upstream_id)) {
        return Ok(upstream.clone());
    }

    // Try by index (legacy support)
    if let Ok(idx) = upstream_id.parse::<usize>() {
        if let Some(upstream) = cfg.upstreams.get(idx) {
            return Ok(upstream.clone());
        }
    }

    // Fallback to single upstream
    if cfg.upstreams.len() == 1 {
        return Ok(cfg.upstreams[0].clone());
    }

    let available: Vec<_> = cfg.upstreams.iter().map(|u| u.id.clone()).collect();
    Err(ForwardError::UpstreamNotFound(format!(
        "Upstream '{}' not found. Available: {:?}",
        upstream_id, available
    )))
}

/// Build forward plan from request
///
/// This is the main middleware function that:
/// 1. Determines authentication mode
/// 2. Extracts model ID from payload
/// 3. Looks up model and upstream configurations
/// 4. Builds the complete ForwardPlan
pub fn build_forward_plan(
    headers: &HeaderMap,
    payload: &Value,
    provider_hint: Option<Provider>,
) -> ForwardResult<ForwardPlan> {
    let cfg = config::load();

    // 1. Determine auth mode
    let auth_mode = determine_auth_mode(headers).map_err(|e| {
        crate::logger::error("middleware", &format!("Authentication failed: {}", e));
        e
    })?;

    // 2. Extract model ID
    let model_id = extract_model_id(payload).map_err(|e| {
        crate::logger::error(
            "middleware",
            &format!("Failed to extract model ID from payload: {}", e),
        );
        e
    })?;

    crate::logger::debug(
        "middleware",
        &format!("Building context for model: {}", model_id),
    );

    // 3. Find model configuration(s)
    let models = collect_models_for_id(&model_id, &cfg).map_err(|e| {
        crate::logger::error(
            "middleware",
            &format!("Model lookup failed: model_id='{}', error={}", model_id, e),
        );
        e
    })?;

    let model_cfg = models
        .first()
        .cloned()
        .ok_or_else(|| ForwardError::ModelNotFound("No models configured".to_string()))?;
    let routes = resolve_routes_for_models(&models);
    let routes = filter_routes_by_provider(routes, provider_hint)?;

    let enable_retry_fallback = cfg.enable_retry_fallback.unwrap_or(false);

    // 4. Extract metadata
    let meta = extract_request_meta(headers);
    let is_streaming = is_streaming_request(payload);

    // 5. Build contexts
    build_plan_from_routes(
        auth_mode,
        meta,
        is_streaming,
        model_cfg,
        routes,
        enable_retry_fallback,
        None,
    )
}

/// Legacy wrapper for callers that only need a single context.
#[allow(dead_code)]
pub fn build_forward_context(
    headers: &HeaderMap,
    payload: &Value,
) -> ForwardResult<ForwardContext> {
    build_forward_plan(headers, payload, None).map(|plan| plan.primary)
}

/// Build forward plan for Gemini (which may have model in URL path)
pub fn build_gemini_plan(
    headers: &HeaderMap,
    payload: &Value,
    endpoint_path: &str,
    api_version: &str,
) -> ForwardResult<ForwardPlan> {
    let cfg = config::load();

    // Try to extract model from payload first
    let model_id = if let Some(id) = payload.get("model").and_then(|v| v.as_str()) {
        id.to_string()
    } else {
        // Try to extract from endpoint path (e.g., /models/gemini-pro:generateContent)
        extract_model_from_gemini_path(endpoint_path)
            .unwrap_or_else(|| "gemini-pro".to_string())
    };

    // Find model config or create a default one
    let (model_cfg, routes) = match collect_models_for_id(&model_id, &cfg) {
        Ok(models) => {
            let model_cfg = models
                .first()
                .cloned()
                .ok_or_else(|| ForwardError::ModelNotFound("No models configured".to_string()))?;
            (model_cfg, resolve_routes_for_models(&models))
        }
        Err(ForwardError::ModelNotFound(_)) => {
            let model_cfg = create_default_gemini_model(&model_id);
            let routes = model_cfg.resolved_routes();
            (model_cfg, routes)
        }
        Err(e) => return Err(e),
    };

    let routes = filter_routes_by_provider(routes, Some(Provider::Gemini))?;

    // Continue with normal flow
    let auth_mode = determine_auth_mode(headers)?;
    let meta = extract_request_meta(headers);
    let is_streaming = is_gemini_streaming_request(payload, endpoint_path);
    let enable_retry_fallback = cfg.enable_retry_fallback.unwrap_or(false);

    build_plan_from_routes(
        auth_mode,
        meta,
        is_streaming,
        model_cfg,
        routes,
        enable_retry_fallback,
        Some(api_version),
    )
}

/// Legacy wrapper for callers that only need a single context.
#[allow(dead_code)]
pub fn build_gemini_context(
    headers: &HeaderMap,
    payload: &Value,
    endpoint_path: &str,
    api_version: &str,
) -> ForwardResult<ForwardContext> {
    build_gemini_plan(headers, payload, endpoint_path, api_version).map(|plan| plan.primary)
}

/// Extract model name from Gemini endpoint path
fn extract_model_from_gemini_path(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');

    let mut segments = trimmed.split('/');
    while let Some(segment) = segments.next() {
        if segment.eq_ignore_ascii_case("models") {
            if let Some(model_segment) = segments.next() {
                let model = model_segment.split(':').next().unwrap_or(model_segment);
                let model = model.trim();
                if !model.is_empty() {
                    return Some(model.to_string());
                }
            }
        }
    }

    if let Some(segment) = trimmed.split('/').find(|s| s.contains(':')) {
        let model = segment.split(':').next().unwrap_or(segment).trim();
        if !model.is_empty() {
            return Some(model.to_string());
        }
    }

    None
}

/// Create default Gemini model config for pass-through
fn create_default_gemini_model(model_id: &str) -> config::ModelCfg {
    config::ModelCfg {
        id: model_id.to_string(),
        display_name: model_id.to_string(),
        provider: "gemini".to_string(),
        upstream_id: "gemini".to_string(),
        upstream_model_id: None,
        routes: vec![config::ModelRoute {
            provider: "gemini".to_string(),
            upstream_id: "gemini".to_string(),
            upstream_model_id: None,
            priority: None,
        }],
        price_prompt_per_1k: 0.0,
        price_completion_per_1k: 0.0,
        priority: 50,
        is_temporary: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_model_from_gemini_path() {
        assert_eq!(
            extract_model_from_gemini_path("/v1beta/models/gemini-pro:generateContent"),
            Some("gemini-pro".to_string())
        );
        assert_eq!(
            extract_model_from_gemini_path("/models/gemini-1.5-pro:streamGenerateContent"),
            Some("gemini-1.5-pro".to_string())
        );
        assert_eq!(
            extract_model_from_gemini_path("/v1beta/models/glm-4.7:streamGenerateContent"),
            Some("glm-4.7".to_string())
        );
        assert_eq!(extract_model_from_gemini_path("/v1/chat/completions"), None);
    }

    #[test]
    fn test_is_streaming_request() {
        assert!(is_streaming_request(&serde_json::json!({"stream": true})));
        assert!(!is_streaming_request(&serde_json::json!({"stream": false})));
        assert!(is_streaming_request(&serde_json::json!({"stream": "true"})));
        assert!(is_streaming_request(&serde_json::json!({"stream": 1})));
        assert!(!is_streaming_request(&serde_json::json!({"stream": "false"})));
        assert!(!is_streaming_request(&serde_json::json!({"stream": 0})));
        assert!(!is_streaming_request(&serde_json::json!({})));
    }

    #[test]
    fn test_is_gemini_streaming_request() {
        let payload = serde_json::json!({});
        assert!(is_gemini_streaming_request(
            &payload,
            "/models/gemini-pro:streamGenerateContent"
        ));
        assert!(!is_gemini_streaming_request(
            &payload,
            "/models/gemini-pro:generateContent"
        ));
    }
}
