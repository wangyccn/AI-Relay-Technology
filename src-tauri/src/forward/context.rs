//! Forward context structures
//!
//! Defines the context structures passed between middleware and handlers.

use serde::{Deserialize, Serialize};

/// Supported API providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Gemini,
}

impl Provider {
    /// Parse provider from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(Provider::OpenAI),
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "gemini" => Some(Provider::Gemini),
            _ => None,
        }
    }

    /// Get provider name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::OpenAI => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Authentication mode for the request
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// Use the API key configured in upstream settings
    UseConfiguredKey,
    /// Use the token from the request as API key (passthrough mode)
    UseRequestToken(String),
}

/// Upstream endpoint information
#[derive(Debug, Clone)]
pub struct UpstreamInfo {
    /// Upstream ID
    pub id: String,
    /// List of endpoint URLs (for load balancing/failover)
    pub endpoints: Vec<String>,
    /// API style (openai/anthropic/gemini)
    pub api_style: Option<String>,
    /// API key for this upstream (if configured)
    pub api_key: Option<String>,
}

/// Model configuration information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model ID (as used in requests)
    pub id: String,
    /// Display name for UI
    #[allow(dead_code)]
    pub display_name: String,
    /// Provider type
    pub provider: Provider,
    /// Associated upstream ID
    #[allow(dead_code)]
    pub upstream_id: String,
    /// Model name to use when forwarding to upstream (if different from id)
    pub upstream_model_id: Option<String>,
    /// Price per 1k prompt tokens
    pub price_prompt_per_1k: f64,
    /// Price per 1k completion tokens
    pub price_completion_per_1k: f64,
}

impl ModelInfo {
    /// Get the actual model name to use for upstream requests
    pub fn upstream_model(&self) -> &str {
        self.upstream_model_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .unwrap_or(&self.id)
    }
}

/// Request metadata extracted from headers
#[derive(Debug, Clone, Default)]
pub struct RequestMeta {
    /// Channel identifier (e.g., "web", "cli", "api")
    pub channel: String,
    /// Tool identifier (e.g., "dashboard", "claude-code")
    pub tool: String,
}

/// Forward context containing all information needed for request forwarding
///
/// This context is built by the middleware and passed to the appropriate handler.
#[derive(Debug, Clone)]
pub struct ForwardContext {
    /// Authentication mode
    pub auth_mode: AuthMode,
    /// Model configuration
    pub model: ModelInfo,
    /// Upstream configuration
    pub upstream: UpstreamInfo,
    /// Optional Gemini API version override (e.g., "v1" or "v1beta")
    pub gemini_api_version: Option<String>,
    /// Request metadata
    pub meta: RequestMeta,
    /// Whether this is a streaming request
    pub is_streaming: bool,
    /// Optional override for max retry attempts (used for upstream fallback)
    pub retry_max_attempts_override: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ForwardPlan {
    pub primary: ForwardContext,
    pub fallbacks: Vec<ForwardContext>,
}

impl ForwardContext {
    /// Get the effective API key based on auth mode and upstream configuration
    pub fn get_api_key(&self) -> Option<String> {
        match &self.auth_mode {
            AuthMode::UseRequestToken(token) => Some(token.clone()),
            AuthMode::UseConfiguredKey => {
                // First try upstream configured key
                if let Some(ref key) = self.upstream.api_key {
                    if !key.is_empty() {
                        return Some(key.clone());
                    }
                }
                // Fallback to environment variables
                self.get_env_api_key()
            }
        }
    }

    /// Get API key from environment variables based on provider
    fn get_env_api_key(&self) -> Option<String> {
        let env_var = match self.model.provider {
            Provider::Anthropic => "CCR_ANTHROPIC_KEY",
            Provider::Gemini => "CCR_GEMINI_KEY",
            Provider::OpenAI => "CCR_OPENAI_KEY",
        };
        std::env::var(env_var).ok()
    }

    /// Get the primary endpoint URL
    pub fn primary_endpoint(&self) -> Option<&str> {
        self.upstream.endpoints.first().map(|s| s.as_str())
    }

    /// Get all endpoints for failover
    pub fn all_endpoints(&self) -> &[String] {
        &self.upstream.endpoints
    }

    /// Get retry configuration for this request (with optional override).
    pub fn retry_config(&self) -> RetryConfig {
        let mut config = RetryConfig::from_config();
        if let Some(max_attempts) = self.retry_max_attempts_override {
            config.max_attempts = max_attempts;
        }
        config
    }

    /// Get Gemini API version for this request.
    pub fn gemini_version(&self) -> &str {
        self.gemini_api_version
            .as_deref()
            .unwrap_or("v1beta")
    }

    /// Calculate cost for given token usage
    pub fn calculate_cost(&self, usage: &TokenUsage) -> f64 {
        crate::pricing::cost_usd(
            usage.prompt_tokens,
            usage.completion_tokens,
            self.model.price_prompt_per_1k,
            self.model.price_completion_per_1k,
        )
    }

    /// Log usage to database
    ///
    /// For temporary/reserved models (like claude-sonnet-4-5-20250929),
    /// we log the actual upstream model ID instead of the temporary model ID
    /// to ensure correct statistics aggregation.
    pub fn log_usage(&self, usage: &TokenUsage) {
        let cost = self.calculate_cost(usage);

        // Use upstream_model_id for statistics if available (for temporary models)
        // This ensures temporary models are counted under their actual target model
        let model_for_stats = self.model.upstream_model_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .unwrap_or(&self.model.id);

        crate::db::log_usage(
            &self.meta.channel,
            &self.meta.tool,
            model_for_stats,
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total(),
            cost,
            &self.upstream.id,
        );

        // Log to system logger for visibility
        crate::logger::info(
            "forward",
            &format!(
                "API request completed: model={}, tokens={}/{}, cost=${:.6}",
                model_for_stats,
                usage.prompt_tokens,
                usage.completion_tokens,
                cost
            ),
        );
    }
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            initial_delay_ms: 300,
            max_delay_ms: 3000,
        }
    }
}

impl RetryConfig {
    /// Load from global config
    pub fn from_config() -> Self {
        let cfg = crate::config::load();
        Self {
            max_attempts: cfg.retry_max_attempts.unwrap_or(4),
            initial_delay_ms: cfg.retry_initial_ms.unwrap_or(300),
            max_delay_ms: cfg.retry_max_ms.unwrap_or(3000),
        }
    }
}

/// Response from upstream provider
#[derive(Debug)]
pub struct UpstreamResponse {
    /// Response body
    pub body: serde_json::Value,
    /// Latency in milliseconds
    #[allow(dead_code)]
    pub latency_ms: u64,
    /// HTTP status code
    #[allow(dead_code)]
    pub status: u16,
    /// Token usage extracted from response
    #[allow(dead_code)]
    pub usage: TokenUsage,
}

/// Token usage information
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Number of prompt/input tokens
    pub prompt_tokens: i64,
    /// Number of completion/output tokens
    pub completion_tokens: i64,
}

impl TokenUsage {
    /// Create new token usage
    pub fn new(prompt: i64, completion: i64) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
        }
    }

    /// Get total tokens
    pub fn total(&self) -> i64 {
        self.prompt_tokens + self.completion_tokens
    }

    /// Add another usage to this one
    #[allow(dead_code)]
    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
    }
}

/// Estimate tokens from text (rough approximation: ~3.5 chars per token)
pub fn estimate_tokens(text: &str) -> i64 {
    let char_count = text.chars().count();
    (char_count as f64 / 3.5).round() as i64
}
