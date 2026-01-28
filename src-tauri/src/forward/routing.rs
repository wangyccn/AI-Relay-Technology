//! Advanced routing with priority-based model selection and automatic fallback
//!
//! This module implements intelligent routing that:
//! 1. Prioritizes models based on user-configured priority (0-100)
//! 2. Automatically falls back to lower-priority models on failure
//! 3. Supports "auto" mode for dynamic model selection
//! 4. Cleans up temporary models when configuration changes

#![allow(dead_code)]

use crate::config::{ModelCfg, Settings};

/// Configuration for retry and fallback behavior
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Enable automatic retry with fallback to other models
    pub enable_retry_fallback: bool,
    /// Enable dynamic model adjustment based on availability
    pub enable_dynamic_model: bool,
    /// Maximum number of retry attempts
    pub max_retry_attempts: u32,
}

impl RoutingConfig {
    /// Load routing configuration from settings
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            enable_retry_fallback: settings.enable_retry_fallback.unwrap_or(false),
            enable_dynamic_model: settings.enable_dynamic_model.unwrap_or(false),
            max_retry_attempts: settings.retry_max_attempts.unwrap_or(4) as u32,
        }
    }
}

/// Routing strategy for model selection
#[derive(Debug, Clone)]
pub enum RoutingStrategy {
    /// Use the specified model only
    Direct(String),
    /// Use "auto" mode - select highest priority available model
    Auto,
    /// Use priority-based fallback
    PriorityFallback(String),
}

/// Model routing result
#[derive(Debug, Clone)]
pub struct ModelRoute {
    /// Model configuration to use
    pub model: ModelCfg,
    /// Routing strategy used
    pub strategy: RoutingStrategy,
    /// Alternative models for fallback (ordered by priority)
    pub fallback_models: Vec<ModelCfg>,
}

/// Router for intelligent model selection
pub struct ModelRouter {
    config: RoutingConfig,
}

impl ModelRouter {
    /// Create a new router from settings
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            config: RoutingConfig::from_settings(settings),
        }
    }

    /// Route a request to the appropriate model(s)
    pub fn route(&self, requested_model: &str, settings: &Settings) -> Option<ModelRoute> {
        // Handle "auto" mode
        if requested_model.eq_ignore_ascii_case("auto") {
            return self.route_auto(settings);
        }

        // Find all models with matching ID (supports duplicates with different priorities)
        let models: Vec<_> = settings
            .models
            .iter()
            .filter(|m| m.id.eq_ignore_ascii_case(requested_model))
            .cloned()
            .collect();

        if models.is_empty() {
            return None;
        }

        // Sort by priority (highest first)
        let mut sorted_models = models;
        sorted_models.sort_by(|a, b| b.priority.cmp(&a.priority));

        let primary = sorted_models[0].clone();
        let strategy = if self.config.enable_retry_fallback && sorted_models.len() > 1 {
            RoutingStrategy::PriorityFallback(requested_model.to_string())
        } else {
            RoutingStrategy::Direct(requested_model.to_string())
        };

        let fallback_models = if self.config.enable_retry_fallback {
            sorted_models.into_iter().skip(1).collect()
        } else {
            Vec::new()
        };

        Some(ModelRoute {
            model: primary,
            strategy,
            fallback_models,
        })
    }

    /// Route to auto mode (highest priority non-temporary model)
    fn route_auto(&self, settings: &Settings) -> Option<ModelRoute> {
        let mut models: Vec<_> = settings
            .models
            .iter()
            .filter(|m| !m.is_temporary)
            .cloned()
            .collect();

        if models.is_empty() {
            return None;
        }

        models.sort_by(|a, b| b.priority.cmp(&a.priority));

        let primary = models[0].clone();
        let fallback_models = if self.config.enable_dynamic_model {
            models.into_iter().skip(1).collect()
        } else {
            Vec::new()
        };

        Some(ModelRoute {
            model: primary,
            strategy: RoutingStrategy::Auto,
            fallback_models,
        })
    }

    /// Clean up temporary models (marked with is_temporary=true)
    ///
    /// Returns the number of models removed
    pub fn cleanup_temporary_models(settings: &mut Settings) -> usize {
        let original_count = settings.models.len();
        settings.models.retain(|m| !m.is_temporary);
        original_count - settings.models.len()
    }

    /// Check if temporary models should be cleaned up
    ///
    /// Temporary models should be removed if:
    /// 1. User manually edited Claude Code config (detected by config hash change)
    /// 2. User restored from backup
    pub fn should_cleanup_temp_models() -> bool {
        // This would be triggered by external events (backup restore, manual config edit)
        // For now, return false - this will be called by specific events
        false
    }
}

/// Create temporary Opus/Sonnet forwarding models for Claude Code
///
/// This creates 3 high-priority models (priority=100) that forward to Opus/Sonnet
/// These are marked as temporary and will be cleaned up automatically
pub fn create_claude_code_temp_models(
    settings: &mut Settings,
    upstream_id: &str,
    _base_url: &str,
) -> Vec<ModelCfg> {
    let temp_models = vec![
        ModelCfg {
            id: "claude-3-5-sonnet-20241022-temp".to_string(),
            display_name: "Claude 3.5 Sonnet (Temp)".to_string(),
            provider: "anthropic".to_string(),
            upstream_id: upstream_id.to_string(),
            upstream_model_id: Some("claude-3-5-sonnet-20241022".to_string()),
            routes: Vec::new(),
            price_prompt_per_1k: 0.003,
            price_completion_per_1k: 0.015,
            priority: 100,
            is_temporary: true,
        },
        ModelCfg {
            id: "claude-3-5-sonnet-20240620-temp".to_string(),
            display_name: "Claude 3.5 Sonnet (Temp)".to_string(),
            provider: "anthropic".to_string(),
            upstream_id: upstream_id.to_string(),
            upstream_model_id: Some("claude-3-5-sonnet-20240620".to_string()),
            routes: Vec::new(),
            price_prompt_per_1k: 0.003,
            price_completion_per_1k: 0.015,
            priority: 100,
            is_temporary: true,
        },
        ModelCfg {
            id: "claude-3-opus-20240229-temp".to_string(),
            display_name: "Claude 3 Opus (Temp)".to_string(),
            provider: "anthropic".to_string(),
            upstream_id: upstream_id.to_string(),
            upstream_model_id: Some("claude-3-opus-20240229".to_string()),
            routes: Vec::new(),
            price_prompt_per_1k: 0.015,
            price_completion_per_1k: 0.075,
            priority: 100,
            is_temporary: true,
        },
    ];

    // Only add if they don't already exist
    for model in &temp_models {
        if !settings.models.iter().any(|m| m.id == model.id) {
            settings.models.push(model.clone());
        }
    }

    temp_models
}

/// Detect if Claude Code configuration was manually changed
///
/// This compares the current config hash with a stored hash
pub fn detect_claude_code_config_change(config_path: &str) -> bool {
    use std::fs;
    use std::path::Path;

    let path = Path::new(config_path);
    if !path.exists() {
        return false;
    }

    // Read current config and calculate hash
    let current_content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return false,
    };

    let current_hash = hash_config(&current_content);

    // Read stored hash
    let stored_hash = get_stored_config_hash();

    current_hash != stored_hash.unwrap_or_default()
}

/// Calculate a simple hash of the configuration content
fn hash_config(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Get the stored configuration hash
fn get_stored_config_hash() -> Option<String> {
    // This would read from a persistent store
    // For now, return None
    None
}

/// Store the configuration hash
pub fn store_config_hash(_hash: String) {
    // This would persist the hash
    // For now, do nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_config() {
        let settings = Settings::default();
        let config = RoutingConfig::from_settings(&settings);
        assert!(!config.enable_retry_fallback);
        assert!(!config.enable_dynamic_model);
    }

    #[test]
    fn test_model_router() {
        let settings = Settings::default();
        let router = ModelRouter::from_settings(&settings);

        // Test routing with no models
        let route = router.route("gpt-4", &settings);
        assert!(route.is_none());

        // Test auto routing with no models
        let route = router.route_auto(&settings);
        assert!(route.is_none());
    }
}
