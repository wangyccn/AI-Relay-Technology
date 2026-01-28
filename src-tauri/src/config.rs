use dirs::data_dir;
use std::{fs, path::PathBuf};

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(default)]
pub struct Settings {
    pub upstreams: Vec<Upstream>,
    pub models: Vec<ModelCfg>,
    pub retry_max_attempts: Option<u32>,
    pub retry_initial_ms: Option<u64>,
    pub retry_max_ms: Option<u64>,
    /// Forward token used to protect proxy endpoints.
    pub forward_token: Option<String>,
    /// UI / upstream preference hints (e.g. "openai", "anthropic", "gemini").
    pub preferred_api_style: Option<String>,
    /// Optional accent color so the frontend can switch to a light blue theme.
    pub accent_color: Option<String>,
    /// Proxy configuration for HTTP/HTTPS requests
    pub proxy: Option<ProxyConfig>,
    /// Enable automatic retry with fallback to other models
    pub enable_retry_fallback: Option<bool>,
    /// Enable dynamic model adjustment based on availability
    pub enable_dynamic_model: Option<bool>,
    /// Theme configuration for the UI
    pub theme: ThemeConfig,
    /// Backup configuration
    pub backup: BackupConfig,
}

/// Proxy configuration
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ProxyConfig {
    /// Enable proxy for HTTP/HTTPS requests
    pub enabled: bool,

    /// Proxy type: "system", "custom", or "none"
    #[serde(rename = "type")]
    pub proxy_type: String,

    /// Custom proxy URL (e.g., "http://127.0.0.1:8080")
    /// Only used when proxy_type is "custom"
    pub url: Option<String>,

    /// Proxy username (optional)
    pub username: Option<String>,

    /// Proxy password (optional, stored in plain text - use with caution)
    pub password: Option<String>,

    /// List of hosts/patterns to bypass proxy (e.g., ["localhost", "127.0.0.1"])
    pub bypass: Option<Vec<String>>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: "system".to_string(),
            url: None,
            username: None,
            password: None,
            bypass: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ToolConfigPath {
    pub tool: String,
    pub config_path: String,
    pub enabled: bool,
    /// Optional file kind hint (e.g., "settings", "config", "auth", "env")
    pub file_kind: Option<String>,
}

impl Default for ToolConfigPath {
    fn default() -> Self {
        Self {
            tool: String::new(),
            config_path: String::new(),
            enabled: true,
            file_kind: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BackupConfig {
    pub enabled: bool,
    pub max_backups: u32,
    pub auto_backup_on_config: bool,
    pub tool_paths: Vec<ToolConfigPath>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_backups: 20,
            auto_backup_on_config: true,
            tool_paths: Vec::new(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    Auto,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Light
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub light_preset: Option<String>,
    pub dark_preset: Option<String>,
    pub light_custom: Option<String>,
    pub dark_custom: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Light,
            light_preset: Some("default".to_string()),
            dark_preset: Some("default".to_string()),
            light_custom: None,
            dark_custom: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(default)]
pub struct Upstream {
    pub id: String,
    pub endpoints: Vec<String>,
    /// Optional API style for this upstream (openai/anthropic/gemini).
    pub api_style: Option<String>,
    /// Optional API key for this upstream. If not set, will use client headers or environment variables.
    pub api_key: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct ModelRoute {
    /// Provider type for this route (openai/anthropic/gemini).
    pub provider: String,
    /// Upstream ID to use for this route.
    pub upstream_id: String,
    /// Optional upstream model override for this route.
    pub upstream_model_id: Option<String>,
    /// Optional route priority (higher = preferred). If all routes omit priority, selection is random.
    pub priority: Option<u32>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct ModelCfg {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub upstream_id: String,
    /// Model name to use when forwarding to upstream API. If empty, uses `id`.
    pub upstream_model_id: Option<String>,
    /// Multi-route support: each route can target a provider + upstream.
    #[serde(default)]
    pub routes: Vec<ModelRoute>,
    pub price_prompt_per_1k: f64,
    pub price_completion_per_1k: f64,
    /// Priority for model selection (0-100, where 100 is highest priority)
    /// Priority 100 is reserved for temporary auto-generated models
    pub priority: u32,
    /// Mark this model as temporary (auto-generated, should be cleaned up)
    #[serde(default)]
    pub is_temporary: bool,
}

impl ModelCfg {
    pub fn resolved_routes(&self) -> Vec<ModelRoute> {
        if !self.routes.is_empty() {
            return self.routes.clone();
        }
        if self.provider.trim().is_empty() || self.upstream_id.trim().is_empty() {
            return Vec::new();
        }
        vec![ModelRoute {
            provider: self.provider.clone(),
            upstream_id: self.upstream_id.clone(),
            upstream_model_id: self.upstream_model_id.clone(),
            priority: None,
        }]
    }
}

fn gen_forward_token() -> String {
    use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
    let token: String = OsRng
        .sample_iter(&Alphanumeric)
        .take(42)
        .map(char::from)
        .collect();
    format!("ccr_{token}")
}

fn settings_path() -> PathBuf {
    let mut p = data_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("CCR");
    fs::create_dir_all(&p).ok();
    p.push("settings.toml");
    p
}

pub fn load() -> Settings {
    let p = settings_path();
    eprintln!("Loading config from: {:?}", p);
    let mut cfg = if p.exists() {
        let s = fs::read_to_string(&p).unwrap_or_default();
        eprintln!("Config file size: {} bytes", s.len());
        toml::from_str(&s).unwrap_or_else(|e| {
            eprintln!("Failed to parse config: {}", e);
            Settings::default()
        })
    } else {
        eprintln!("Config file does not exist, using default");
        Settings::default()
    };
    eprintln!(
        "Loaded {} models, {} upstreams",
        cfg.models.len(),
        cfg.upstreams.len()
    );

    let mut changed = false;
    if cfg
        .forward_token
        .as_deref()
        .map(|t| t.is_empty())
        .unwrap_or(true)
    {
        cfg.forward_token = Some(gen_forward_token());
        changed = true;
    }

    if cfg
        .accent_color
        .as_deref()
        .map(|c| c.is_empty())
        .unwrap_or(true)
    {
        cfg.accent_color = Some("#c4e1ff".to_string()); // light blue default
        changed = true;
    }

    if cfg
        .theme
        .light_preset
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        cfg.theme.light_preset = Some("default".to_string());
        changed = true;
    }

    if cfg
        .theme
        .dark_preset
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        cfg.theme.dark_preset = Some("default".to_string());
        changed = true;
    }

    if cfg
        .theme
        .light_custom
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(false)
    {
        cfg.theme.light_custom = None;
        changed = true;
    }

    if cfg
        .theme
        .dark_custom
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(false)
    {
        cfg.theme.dark_custom = None;
        changed = true;
    }

    if changed {
        let _ = save(&cfg); // Ignore errors during initial load
    }

    cfg
}

pub fn save(cfg: &Settings) -> Result<(), String> {
    let p = settings_path();
    eprintln!("Saving config to: {:?}", p);
    eprintln!(
        "Saving {} models, {} upstreams",
        cfg.models.len(),
        cfg.upstreams.len()
    );

    // Ensure the directory exists
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let s =
        toml::to_string_pretty(cfg).map_err(|e| format!("Failed to serialize config: {}", e))?;
    eprintln!("Config serialized to {} bytes", s.len());
    fs::write(&p, &s).map_err(|e| format!("Failed to write config file to {:?}: {}", p, e))?;
    eprintln!("Config saved successfully");
    Ok(())
}

pub fn reset() -> Result<(), String> {
    let p = settings_path();
    if p.exists() {
        fs::remove_file(&p)
            .map_err(|e| format!("Failed to remove config file {:?}: {}", p, e))?;
    }
    Ok(())
}

/// Force refresh of the forward token and persist the new value.
pub fn refresh_forward_token() -> String {
    let mut cfg = load();
    cfg.forward_token = Some(gen_forward_token());
    let _ = save(&cfg); // Ignore errors for this operation
    cfg.forward_token.clone().unwrap_or_default()
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
mod platform_security {
    use std::{ffi::c_void, ptr::null_mut};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{LocalFree, HLOCAL},
            Security::Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        },
    };

    unsafe fn vec_from_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if blob.pbData.is_null() || blob.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec()
        }
    }

    unsafe fn blob_from_slice(slice: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: slice.len() as u32,
            pbData: slice.as_ptr() as *mut u8,
        }
    }

    fn free_blob(blob: &mut CRYPT_INTEGER_BLOB) {
        unsafe {
            if !blob.pbData.is_null() {
                let _ = LocalFree(HLOCAL(blob.pbData as *mut c_void));
                blob.pbData = null_mut();
                blob.cbData = 0;
            }
        }
    }

    pub fn protect(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        unsafe {
            let in_blob = blob_from_slice(data);
            let mut out_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: null_mut(),
            };
            if CryptProtectData(
                &in_blob,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )
            .is_err()
            {
                return data.to_vec();
            }
            let result = vec_from_blob(&out_blob);
            free_blob(&mut out_blob);
            result
        }
    }

    pub fn unprotect(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        unsafe {
            let in_blob = blob_from_slice(data);
            let mut out_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: null_mut(),
            };
            if CryptUnprotectData(
                &in_blob,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )
            .is_err()
            {
                return data.to_vec();
            }
            let result = vec_from_blob(&out_blob);
            free_blob(&mut out_blob);
            result
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unused_imports)] // exported for consumers even if not used internally
pub use platform_security::{protect, unprotect};

#[cfg(not(target_os = "windows"))]
pub fn protect(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
#[cfg(not(target_os = "windows"))]
pub fn unprotect(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
