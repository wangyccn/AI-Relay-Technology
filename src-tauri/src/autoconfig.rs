use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::config;
use crate::logger;

const CCR_BASE_URL: &str = "http://127.0.0.1:8787";
const FILE_KIND_SETTINGS: &str = "settings";
const FILE_KIND_CONFIG: &str = "config";
const FILE_KIND_AUTH: &str = "auth";
const FILE_KIND_ENV: &str = "env";

// Backup structures
#[derive(Serialize, Deserialize, Clone)]
pub struct ToolConfigBackup {
    pub id: String,
    pub tool: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_path: Option<String>,
    /// Additional files included in this backup (e.g., .env for gemini)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_files: Option<Vec<ExtraFileBackup>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ExtraFileBackup {
    pub filename: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct ToolConfigBackupList {
    pub backups: Vec<ToolConfigBackup>,
}

#[derive(Deserialize)]
pub struct CreateBackupRequest {
    pub tool: String,
    pub description: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ToolConfigStatus {
    pub configured: bool,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Serialize)]
pub struct AutoConfigStatus {
    pub claude: ToolConfigStatus,
    pub codex: ToolConfigStatus,
    pub gemini: ToolConfigStatus,
}

#[derive(Deserialize)]
pub struct AutoConfigRequest {
    pub tool: String,
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(
        rename = "fastModelId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fast_model_id: Option<String>,
    #[allow(dead_code)]
    pub global: bool,
}

#[derive(Clone)]
struct ToolFileEntry {
    kind: String,
    path: PathBuf,
    enabled: bool,
    is_primary: bool,
}

// Claude Code settings.json structure
#[derive(Serialize, Deserialize, Default)]
struct ClaudeSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    env: Option<ClaudeEnv>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

// Gemini CLI settings.json structure (new format)
#[derive(Serialize, Deserialize, Default)]
struct GeminiSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<GeminiModelConfig>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
struct GeminiModelConfig {
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "baseUrl", skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct ClaudeEnv {
    #[serde(rename = "ANTHROPIC_BASE_URL", skip_serializing_if = "Option::is_none")]
    anthropic_base_url: Option<String>,
    #[serde(
        rename = "ANTHROPIC_AUTH_TOKEN",
        skip_serializing_if = "Option::is_none"
    )]
    anthropic_auth_token: Option<String>,
    #[serde(rename = "ANTHROPIC_MODEL", skip_serializing_if = "Option::is_none")]
    anthropic_model: Option<String>,
    #[serde(
        rename = "ANTHROPIC_SMALL_FAST_MODEL",
        skip_serializing_if = "Option::is_none"
    )]
    anthropic_small_fast_model: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

// Codex config structures
#[derive(Serialize, Deserialize, Default)]
struct CodexAuth {
    #[serde(rename = "OPENAI_API_KEY", skip_serializing_if = "Option::is_none")]
    openai_api_key: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

fn get_claude_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

fn get_codex_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

fn get_gemini_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gemini"))
}

fn primary_kind(tool: &str) -> Option<&'static str> {
    match tool {
        "claude" => Some(FILE_KIND_SETTINGS),
        "codex" => Some(FILE_KIND_CONFIG),
        "gemini" => Some(FILE_KIND_SETTINGS),
        _ => None,
    }
}

fn default_tool_files(tool: &str) -> Vec<(String, PathBuf)> {
    match tool {
        "claude" => get_claude_config_path()
            .map(|p| vec![(FILE_KIND_SETTINGS.to_string(), p)])
            .unwrap_or_default(),
        "codex" => get_codex_config_dir()
            .map(|d| {
                vec![
                    (FILE_KIND_CONFIG.to_string(), d.join("config.toml")),
                    (FILE_KIND_AUTH.to_string(), d.join("auth.json")),
                ]
            })
            .unwrap_or_default(),
        "gemini" => get_gemini_config_dir()
            .map(|d| {
                vec![
                    (FILE_KIND_SETTINGS.to_string(), d.join("settings.json")),
                    (FILE_KIND_ENV.to_string(), d.join(".env")),
                ]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn resolve_tool_entries(tool: &str, _settings: &config::Settings) -> Vec<ToolFileEntry> {
    let defaults = default_tool_files(tool);
    let primary = primary_kind(tool);

    defaults
        .into_iter()
        .map(|(kind, path)| ToolFileEntry {
            enabled: true,
            is_primary: primary.map(|k| k == kind.as_str()).unwrap_or(false),
            kind,
            path,
        })
        .collect()
}

fn tool_paths_for_kind(entries: &[ToolFileEntry], kind: &str) -> Vec<PathBuf> {
    entries
        .iter()
        .filter(|e| e.kind == kind)
        .map(|e| e.path.clone())
        .collect()
}

fn get_forward_token() -> String {
    let settings = config::load();
    settings
        .forward_token
        .unwrap_or_else(|| "ccr-token".to_string())
}

fn collect_model_routes(settings: &config::Settings, model_id: &str) -> Vec<config::ModelRoute> {
    let mut routes = Vec::new();
    for model in settings
        .models
        .iter()
        .filter(|m| m.id.eq_ignore_ascii_case(model_id))
    {
        routes.extend(model.resolved_routes());
    }
    routes
}

fn ensure_model_supports_provider(
    settings: &config::Settings,
    model_id: &str,
    provider: &str,
) -> Result<(), String> {
    let routes = collect_model_routes(settings, model_id);
    if routes.is_empty() {
        return Err(format!("Model '{}' not found in configuration", model_id));
    }
    if !routes
        .iter()
        .any(|route| route.provider.eq_ignore_ascii_case(provider))
    {
        return Err(format!(
            "Model '{}' does not have provider '{}' configured",
            model_id, provider
        ));
    }
    Ok(())
}

fn select_route_for_provider(
    settings: &config::Settings,
    model_id: &str,
    provider: &str,
) -> Result<config::ModelRoute, String> {
    let mut routes: Vec<_> = collect_model_routes(settings, model_id)
        .into_iter()
        .filter(|route| route.provider.eq_ignore_ascii_case(provider))
        .collect();

    if routes.is_empty() {
        return Err(format!(
            "Model '{}' does not have provider '{}' configured",
            model_id, provider
        ));
    }

    routes.sort_by(|a, b| b.priority.unwrap_or(0).cmp(&a.priority.unwrap_or(0)));
    let route = routes.remove(0);
    if route.upstream_id.trim().is_empty() {
        return Err(format!(
            "Model '{}' route for provider '{}' is missing upstream_id",
            model_id, provider
        ));
    }
    Ok(route)
}

/// Get current auto config status for all tools
pub fn get_status() -> AutoConfigStatus {
    AutoConfigStatus {
        claude: get_claude_status(),
        codex: get_codex_status(),
        gemini: get_gemini_status(),
    }
}

fn get_claude_status() -> ToolConfigStatus {
    let settings = config::load();
    let entries = resolve_tool_entries("claude", &settings);
    let paths = tool_paths_for_kind(&entries, FILE_KIND_SETTINGS);

    for path in paths {
        if !path.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<ClaudeSettings>(&content) {
                if let Some(env) = settings.env {
                    let is_ccr = env
                        .anthropic_base_url
                        .as_ref()
                        .map(|url| url.contains("127.0.0.1:8787"))
                        .unwrap_or(false);

                    if is_ccr {
                        return ToolConfigStatus {
                            configured: true,
                            model: env.anthropic_model,
                            base_url: env.anthropic_base_url,
                        };
                    }
                }
            }
        }
    }

    ToolConfigStatus {
        configured: false,
        model: None,
        base_url: None,
    }
}

fn get_codex_status() -> ToolConfigStatus {
    let settings = config::load();
    let entries = resolve_tool_entries("codex", &settings);
    let paths = tool_paths_for_kind(&entries, FILE_KIND_CONFIG);

    for path in paths {
        if !path.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            // Simple check for CCR configuration
            let is_ccr = content.contains("127.0.0.1:8787");
            if is_ccr {
                // Extract model from config
                let model = content
                    .lines()
                    .find(|line| line.starts_with("model = "))
                    .and_then(|line| line.strip_prefix("model = "))
                    .map(|s| s.trim_matches('"').to_string());

                return ToolConfigStatus {
                    configured: true,
                    model,
                    base_url: Some(format!("{}/v1", CCR_BASE_URL)),
                };
            }
        }
    }

    ToolConfigStatus {
        configured: false,
        model: None,
        base_url: None,
    }
}

fn get_gemini_status() -> ToolConfigStatus {
    let settings = config::load();
    let entries = resolve_tool_entries("gemini", &settings);
    let settings_paths = tool_paths_for_kind(&entries, FILE_KIND_SETTINGS);

    for path in settings_paths {
        if !path.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<GeminiSettings>(&content) {
                let is_ccr = settings
                    .model
                    .as_ref()
                    .and_then(|m| m.base_url.as_ref())
                    .map(|url| url.contains("127.0.0.1:8787"))
                    .unwrap_or(false);

                if is_ccr {
                    return ToolConfigStatus {
                        configured: true,
                        model: settings.model.as_ref().and_then(|m| m.name.clone()),
                        base_url: settings.model.as_ref().and_then(|m| m.base_url.clone()),
                    };
                }
            }
        }
    }

    let env_paths = tool_paths_for_kind(&entries, FILE_KIND_ENV);
    for path in env_paths {
        if !path.exists() {
            continue;
        }
        let status = check_legacy_env(&path);
        if status.configured {
            return status;
        }
    }

    ToolConfigStatus {
        configured: false,
        model: None,
        base_url: None,
    }
}

/// Check legacy .env file for backwards compatibility
fn check_legacy_env(env_path: &Path) -> ToolConfigStatus {
    match fs::read_to_string(env_path) {
        Ok(content) => {
            // Check for CCR configuration in .env
            let is_ccr = content.contains("127.0.0.1:8787");
            if is_ccr {
                // Extract values from .env
                let mut model = None;
                let mut base_url = None;

                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("GEMINI_MODEL=") {
                        model = Some(
                            line.strip_prefix("GEMINI_MODEL=")
                                .unwrap_or("")
                                .trim_matches('"')
                                .to_string(),
                        );
                    } else if line.starts_with("GEMINI_API_BASE_URL=") {
                        base_url = Some(
                            line.strip_prefix("GEMINI_API_BASE_URL=")
                                .unwrap_or("")
                                .trim_matches('"')
                                .to_string(),
                        );
                    }
                }

                ToolConfigStatus {
                    configured: true,
                    model,
                    base_url,
                }
            } else {
                ToolConfigStatus {
                    configured: false,
                    model: None,
                    base_url: None,
                }
            }
        }
        Err(_) => ToolConfigStatus {
            configured: false,
            model: None,
            base_url: None,
        },
    }
}

/// Configure a tool with CCR proxy settings
pub fn configure(req: &AutoConfigRequest) -> Result<(), String> {
    logger::info(
        "autoconfig",
        &format!("配置工具 {} 使用模型 {}", req.tool, req.model_id),
    );
    let result = match req.tool.as_str() {
        "claude" => configure_claude(&req.model_id, req.fast_model_id.as_deref()),
        "codex" => configure_codex(&req.model_id),
        "gemini" => configure_gemini(&req.model_id),
        _ => Err(format!("Unknown tool: {}", req.tool)),
    };

    match &result {
        Ok(_) => logger::info("autoconfig", &format!("工具 {} 配置成功", req.tool)),
        Err(e) => logger::error("autoconfig", &format!("工具 {} 配置失败: {}", req.tool, e)),
    }

    result
}

fn configure_claude(model_id: &str, fast_model_id: Option<&str>) -> Result<(), String> {
    let settings = config::load();
    ensure_model_supports_provider(&settings, model_id, "anthropic")?;
    if let Some(fast_id) = fast_model_id {
        ensure_model_supports_provider(&settings, fast_id, "anthropic")?;
    }
    let entries = resolve_tool_entries("claude", &settings);
    let paths = tool_paths_for_kind(&entries, FILE_KIND_SETTINGS);

    if paths.is_empty() {
        return Err("Cannot determine Claude settings path".to_string());
    }

    let token = get_forward_token();

    for path in paths {
        // Load existing settings or create new
        let mut settings: ClaudeSettings = if path.exists() {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            ClaudeSettings::default()
        };

        // Update env section
        let mut env = settings.env.unwrap_or_default();
        env.anthropic_base_url = Some(format!("{}/anthropic", CCR_BASE_URL));
        env.anthropic_auth_token = Some(token.clone());

        // Use special reserved model names for Claude Code
        // These will be created as temporary models in CCR config
        env.anthropic_model = Some("claude-sonnet-4-5-20250929".to_string());

        // Use the reserved fast model name; it will map to the selected fast model in CCR.
        env.anthropic_small_fast_model = Some("claude-haiku-4-5-20251001".to_string());

        settings.env = Some(env);

        // Write back
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        write_file_content(&path, &content)?;
    }

    // Now create the temporary models in CCR config
    create_claude_code_special_models(model_id, fast_model_id)?;

    Ok(())
}

/// Create special reserved models for Claude Code integration
///
/// This creates two temporary models with high priority (100):
/// - claude-sonnet-4-5-20250929: Points to the selected main model
/// - claude-haiku-4-5-20251001: Points to the selected fast model (or main model if none selected)
fn create_claude_code_special_models(
    main_model_id: &str,
    fast_model_id: Option<&str>
) -> Result<(), String> {
    use crate::config;

    // Load current settings
    let mut settings = config::load();

    // Select the upstream/provider route for Claude (Anthropic)
    let main_route = select_route_for_provider(&settings, main_model_id, "anthropic")?;
    let main_upstream_model = main_route
        .upstream_model_id
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| main_model_id.to_string());

    // Find fast model id and its upstream/provider info
    let (_fast_model_id_to_use, fast_route, fast_upstream_model) = if let Some(fid) = fast_model_id {
        let route = select_route_for_provider(&settings, fid, "anthropic")?;
        let upstream_model = route
            .upstream_model_id
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| fid.to_string());
        (fid, route, upstream_model)
    } else {
        (
            main_model_id,
            main_route.clone(),
            main_upstream_model.clone(),
        )
    };

    // Remove existing special models if they exist
    settings.models.retain(|m| m.id != "claude-sonnet-4-5-20250929" && m.id != "claude-haiku-4-5-20251001");

    // Create the Sonnet special model (points to main model)
    let sonnet_model = config::ModelCfg {
        id: "claude-sonnet-4-5-20250929".to_string(),
        display_name: "Claude Code Main Model (Reserved)".to_string(),
        provider: main_route.provider.clone(),
        upstream_id: main_route.upstream_id.clone(),
        upstream_model_id: Some(main_upstream_model.clone()),
        routes: vec![config::ModelRoute {
            provider: main_route.provider.clone(),
            upstream_id: main_route.upstream_id.clone(),
            upstream_model_id: Some(main_upstream_model.clone()),
            priority: None,
        }],
        price_prompt_per_1k: 0.0,
        price_completion_per_1k: 0.0,
        priority: 100, // System reserved
        is_temporary: true,
    };

    // Create the Haiku special model (points to fast model)
    let haiku_model = config::ModelCfg {
        id: "claude-haiku-4-5-20251001".to_string(),
        display_name: "Claude Code Fast Model (Reserved)".to_string(),
        provider: fast_route.provider.clone(),
        upstream_id: fast_route.upstream_id.clone(),
        upstream_model_id: Some(fast_upstream_model.clone()),
        routes: vec![config::ModelRoute {
            provider: fast_route.provider,
            upstream_id: fast_route.upstream_id,
            upstream_model_id: Some(fast_upstream_model),
            priority: None,
        }],
        price_prompt_per_1k: 0.0,
        price_completion_per_1k: 0.0,
        priority: 100, // System reserved
        is_temporary: true,
    };

    // Add to settings
    settings.models.push(sonnet_model);
    settings.models.push(haiku_model);

    // Save config
    config::save(&settings).map_err(|e| format!("Failed to save config: {}", e))?;

    logger::info("autoconfig", "Created Claude Code special models: claude-sonnet-4-5-20250929 and claude-haiku-4-5-20251001");

    Ok(())
}

fn configure_codex(model_id: &str) -> Result<(), String> {
    let settings = config::load();
    ensure_model_supports_provider(&settings, model_id, "openai")?;
    let entries = resolve_tool_entries("codex", &settings);
    let config_paths = tool_paths_for_kind(&entries, FILE_KIND_CONFIG);
    let auth_paths = tool_paths_for_kind(&entries, FILE_KIND_AUTH);

    if config_paths.is_empty() {
        return Err("Cannot determine Codex config.toml path".to_string());
    }
    if auth_paths.is_empty() {
        return Err("Cannot determine Codex auth.json path".to_string());
    }

    let token = get_forward_token();

    // Configure auth.json
    let auth = CodexAuth {
        openai_api_key: Some(token.clone()),
        other: serde_json::Map::new(),
    };
    let auth_content = serde_json::to_string_pretty(&auth)
        .map_err(|e| format!("Failed to serialize auth: {}", e))?;
    for auth_path in auth_paths {
        write_file_content(&auth_path, &auth_content)?;
    }

    // Create a sanitized provider name from model_id
    let provider_name = model_id.replace(['-', '.', ' '], "_").to_lowercase();

    // base_url should be /v1 for Codex
    let config_content = format!(
        r#"model_provider = "{provider_name}"
model_reasoning_effort = "high"
disable_response_storage = true
model = "{model_id}"

[model_providers.{provider_name}]
name = "{model_id}"
base_url = "{base_url}/v1"
wire_api = "responses"
requires_openai_auth = true
"#,
        provider_name = provider_name,
        model_id = model_id,
        base_url = CCR_BASE_URL
    );

    for config_path in config_paths {
        write_file_content(&config_path, &config_content)?;
    }

    Ok(())
}

fn configure_gemini(model_id: &str) -> Result<(), String> {
    let settings = config::load();
    ensure_model_supports_provider(&settings, model_id, "gemini")?;
    let entries = resolve_tool_entries("gemini", &settings);
    let settings_paths = tool_paths_for_kind(&entries, FILE_KIND_SETTINGS);
    let env_paths = tool_paths_for_kind(&entries, FILE_KIND_ENV);

    if settings_paths.is_empty() && env_paths.is_empty() {
        return Err("Cannot determine Gemini config paths".to_string());
    }

    let token = get_forward_token();
    let base_url = format!("{}/gemini", CCR_BASE_URL);

    // 1. Configure settings.json with model settings
    for settings_path in settings_paths {
        let mut settings: GeminiSettings = if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)
                .map_err(|e| format!("Failed to read config: {}", e))?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            GeminiSettings::default()
        };

        // Update model configuration in settings.json
        settings.model = Some(GeminiModelConfig {
            name: Some(model_id.to_string()),
            base_url: Some(base_url.clone()),
            other: serde_json::Map::new(),
        });

        // Write settings.json
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        write_file_content(&settings_path, &content)?;
    }

    // 2. Configure .env with environment variables
    for env_path in env_paths {
        // Read existing .env content to preserve other variables
        let mut env_vars: HashMap<String, String> = HashMap::new();

        if env_path.exists() {
            if let Ok(content) = fs::read_to_string(&env_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        env_vars.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
            }
        }

        // Update CCR-related environment variables
        // Note: Gemini CLI may use different env var names, so we set both common variants
        env_vars.insert("GEMINI_API_KEY".to_string(), token.clone());
        env_vars.insert("GOOGLE_API_KEY".to_string(), token.clone());
        env_vars.insert("GEMINI_API_BASE_URL".to_string(), base_url.clone());
        // Also set the base URL for Google APIs
        env_vars.insert("GOOGLE_GEMINI_BASE_URL".to_string(), base_url.clone());

        // Write .env file
        let mut env_content = String::new();
        env_content.push_str("# Gemini CLI Configuration\n");
        env_content.push_str("# Auto-configured by CCR\n");
        env_content.push_str("# Environment variables for API access\n\n");

        let mut keys: Vec<_> = env_vars.keys().cloned().collect();
        keys.sort();
        for key in keys {
            if let Some(value) = env_vars.get(&key) {
                // Quote values that contain spaces or special characters
                let formatted_value = if value.contains(' ') || value.contains('#') {
                    format!("\"{}\"", value)
                } else {
                    value.clone()
                };
                env_content.push_str(&format!("{}={}\n", key, formatted_value));
            }
        }

        write_file_content(&env_path, &env_content)?;
    }

    Ok(())
}

// Backup functions
fn get_backups_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("ccr").join("backups"))
}

fn get_backups_file() -> Option<PathBuf> {
    get_backups_dir().map(|d| d.join("tool_backups.json"))
}

fn load_all_backups() -> Vec<ToolConfigBackup> {
    let path = match get_backups_file() {
        Some(p) => p,
        None => return Vec::new(),
    };

    if !path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_all_backups(backups: &[ToolConfigBackup]) -> Result<(), String> {
    let dir = get_backups_dir().ok_or_else(|| "Cannot determine data directory".to_string())?;

    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create backups directory: {}", e))?;

    let path =
        get_backups_file().ok_or_else(|| "Cannot determine backups file path".to_string())?;

    let content = serde_json::to_string_pretty(&backups)
        .map_err(|e| format!("Failed to serialize backups: {}", e))?;

    fs::write(&path, content).map_err(|e| format!("Failed to write backups file: {}", e))?;

    Ok(())
}

pub fn clear_all_backups() -> Result<(), String> {
    if let Some(dir) = get_backups_dir() {
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| format!("Failed to remove backups directory: {}", e))?;
        }
    }
    Ok(())
}

fn write_file_content(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    fs::write(path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn read_backup_payload(
    tool: &str,
    settings: &config::Settings,
) -> Result<(String, Option<String>, Vec<ExtraFileBackup>), String> {
    let entries = resolve_tool_entries(tool, settings);
    let mut enabled_entries: Vec<ToolFileEntry> = entries.into_iter().filter(|e| e.enabled).collect();

    if enabled_entries.is_empty() {
        return Err(format!("No enabled config paths for {}", tool));
    }

    let primary_index = enabled_entries
        .iter()
        .position(|e| e.is_primary && e.path.exists())
        .or_else(|| enabled_entries.iter().position(|e| e.path.exists()))
        .ok_or_else(|| format!("Config file does not exist for {}", tool))?;

    let primary_entry = enabled_entries.remove(primary_index);
    let primary_content = fs::read_to_string(&primary_entry.path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let primary_path = Some(primary_entry.path.to_string_lossy().to_string());

    let mut extra_files = Vec::new();
    for entry in enabled_entries {
        if !entry.path.exists() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&entry.path) {
            let filename = entry
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| entry.path.to_string_lossy().to_string());
            extra_files.push(ExtraFileBackup {
                filename,
                content,
                path: Some(entry.path.to_string_lossy().to_string()),
            });
        }
    }

    Ok((primary_content, primary_path, extra_files))
}

/// List all backups for a specific tool
pub fn list_backups(tool: &str) -> ToolConfigBackupList {
    let all_backups = load_all_backups();
    let tool_backups: Vec<ToolConfigBackup> =
        all_backups.into_iter().filter(|b| b.tool == tool).collect();

    ToolConfigBackupList {
        backups: tool_backups,
    }
}

/// Create a backup of the current tool configuration
pub fn create_backup(req: &CreateBackupRequest) -> Result<ToolConfigBackup, String> {
    let settings = config::load();
    if !settings.backup.enabled {
        return Err("Backup is disabled in settings".to_string());
    }
    let (content, primary_path, extra_files) = read_backup_payload(&req.tool, &settings)?;

    let backup = ToolConfigBackup {
        id: Uuid::new_v4().to_string(),
        tool: req.tool.clone(),
        description: req
            .description
            .clone()
            .unwrap_or_else(|| "Manual backup".to_string()),
        timestamp: Utc::now(),
        content,
        primary_path,
        extra_files: if extra_files.is_empty() { None } else { Some(extra_files) },
    };

    let mut all_backups = load_all_backups();
    all_backups.insert(0, backup.clone()); // Insert at beginning (newest first)

    // Keep only last N backups per tool
    let max_backups = settings.backup.max_backups.max(1) as usize;
    let tool = &req.tool;
    let mut tool_count = 0;
    all_backups.retain(|b| {
        if &b.tool == tool {
            tool_count += 1;
            tool_count <= max_backups
        } else {
            true
        }
    });

    save_all_backups(&all_backups)?;

    logger::info(
        "autoconfig",
        &format!("创建 {} 配置备份: {}", req.tool, backup.id),
    );

    Ok(backup)
}

/// Restore a backup by ID
pub fn restore_backup(backup_id: &str) -> Result<(), String> {
    let all_backups = load_all_backups();

    let backup = all_backups
        .iter()
        .find(|b| b.id == backup_id)
        .ok_or_else(|| "Backup not found".to_string())?;

    let settings = config::load();
    let entries = resolve_tool_entries(&backup.tool, &settings);

    let primary_path = backup
        .primary_path
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| entries.iter().find(|e| e.is_primary).map(|e| e.path.clone()))
        .or_else(|| entries.first().map(|e| e.path.clone()))
        .ok_or_else(|| "Config path not found".to_string())?;

    write_file_content(&primary_path, &backup.content)?;

    let mut path_lookup: HashMap<String, PathBuf> = HashMap::new();
    for entry in entries {
        if let Some(name) = entry.path.file_name().and_then(|n| n.to_str()) {
            path_lookup.insert(name.to_lowercase(), entry.path);
        }
    }

    // Restore extra files if present
    if let Some(ref extra_files) = backup.extra_files {
        for extra_file in extra_files {
            let target_path = extra_file
                .path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| path_lookup.get(&extra_file.filename.to_lowercase()).cloned());
            if let Some(path) = target_path {
                write_file_content(&path, &extra_file.content)?;
            }
        }
    }

    logger::info(
        "autoconfig",
        &format!("恢复 {} 配置备份: {}", backup.tool, backup_id),
    );

    Ok(())
}

/// Delete a backup by ID
pub fn delete_backup(backup_id: &str) -> Result<(), String> {
    let mut all_backups = load_all_backups();
    let original_len = all_backups.len();

    all_backups.retain(|b| b.id != backup_id);

    if all_backups.len() == original_len {
        return Err("Backup not found".to_string());
    }

    save_all_backups(&all_backups)?;

    Ok(())
}
