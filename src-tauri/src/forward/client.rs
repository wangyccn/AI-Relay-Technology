//! HTTP client utilities for forwarding requests
//!
//! Provides common functionality for making HTTP requests to upstream providers.

use std::time::{Duration, Instant};

use rand::{rngs::OsRng, RngCore};
use reqwest::{header::HeaderMap, Client, NoProxy, Proxy, Response};
use serde_json::Value;

use super::context::RetryConfig;
use super::error::{ForwardError, ForwardResult};
use crate::config;

#[derive(Default)]
struct SystemProxySettings {
    http: Option<String>,
    https: Option<String>,
    bypass: Option<String>,
}

fn normalize_proxy_url(raw: &str, default_scheme: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("{}://{}", default_scheme, trimmed)
    }
}

fn push_no_proxy_entry(entries: &mut Vec<String>, entry: &str) {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return;
    }
    if trimmed.eq_ignore_ascii_case("<local>") {
        entries.push("localhost".to_string());
        entries.push("127.0.0.1".to_string());
        entries.push("::1".to_string());
        return;
    }
    entries.push(trimmed.to_string());
}

fn build_no_proxy(cfg: &config::ProxyConfig, extra_bypass: Option<&str>) -> Option<NoProxy> {
    let mut entries = Vec::new();

    if let Some(bypass) = cfg.bypass.as_ref() {
        for item in bypass {
            push_no_proxy_entry(&mut entries, item);
        }
    }

    if let Some(extra) = extra_bypass {
        for item in extra.split(&[';', ','][..]) {
            push_no_proxy_entry(&mut entries, item);
        }
    }

    if entries.is_empty() {
        None
    } else {
        NoProxy::from_string(&entries.join(","))
    }
}

fn system_proxy_from_env() -> Option<SystemProxySettings> {
    let http = std::env::var("HTTP_PROXY")
        .or_else(|_| std::env::var("http_proxy"))
        .ok();
    let https = std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .ok();
    let bypass = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .ok();

    if http.is_none() && https.is_none() {
        return None;
    }

    Some(SystemProxySettings { http, https, bypass })
}

#[cfg(target_os = "windows")]
fn read_registry_string(path: &str, value: &str) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, REG_VALUE_TYPE,
    };

    let path_w: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    let value_w: Vec<u16> = OsStr::new(value).encode_wide().chain(Some(0)).collect();
    let mut data_type = REG_VALUE_TYPE(0);
    let mut data_len: u32 = 0;
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;

    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            flags,
            Some(&mut data_type),
            None,
            Some(&mut data_len),
        )
    };
    if status != WIN32_ERROR(0) || data_len == 0 {
        return None;
    }

    let mut buffer = vec![0u16; (data_len as usize + 1) / 2];
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            flags,
            Some(&mut data_type),
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut data_len),
        )
    };
    if status != WIN32_ERROR(0) {
        return None;
    }

    let len = (data_len as usize / 2).saturating_sub(1);
    let value = String::from_utf16_lossy(&buffer[..len]);
    let trimmed = value.trim_end_matches('\0').trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(target_os = "windows")]
fn read_registry_dword(path: &str, value: &str) -> Option<u32> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD, REG_VALUE_TYPE,
    };

    let path_w: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    let value_w: Vec<u16> = OsStr::new(value).encode_wide().chain(Some(0)).collect();
    let mut data_type = REG_VALUE_TYPE(0);
    let mut data: u32 = 0;
    let mut data_len: u32 = std::mem::size_of::<u32>() as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_DWORD,
            Some(&mut data_type),
            Some(&mut data as *mut _ as *mut _),
            Some(&mut data_len),
        )
    };

    if status != WIN32_ERROR(0) {
        None
    } else {
        Some(data)
    }
}

#[cfg(target_os = "windows")]
fn system_proxy_from_registry() -> Option<SystemProxySettings> {
    let path = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let enabled = read_registry_dword(path, "ProxyEnable")?;
    if enabled == 0 {
        return None;
    }

    let proxy_server = read_registry_string(path, "ProxyServer")?;
    let bypass = read_registry_string(path, "ProxyOverride");

    let mut settings = SystemProxySettings::default();
    settings.bypass = bypass;

    if proxy_server.contains('=') {
        for part in proxy_server.split(';') {
            let Some((scheme, addr)) = part.split_once('=') else {
                continue;
            };
            let scheme = scheme.trim().to_ascii_lowercase();
            let addr = addr.trim();
            if addr.is_empty() {
                continue;
            }
            match scheme.as_str() {
                "http" => settings.http = Some(addr.to_string()),
                "https" => settings.https = Some(addr.to_string()),
                "socks" | "socks5" => {
                    crate::logger::warn(
                        "client",
                        "System proxy uses SOCKS, but socks support is disabled; ignoring.",
                    );
                }
                _ => {}
            }
        }
    } else {
        let trimmed = proxy_server.trim();
        if !trimmed.is_empty() {
            settings.http = Some(trimmed.to_string());
            settings.https = Some(trimmed.to_string());
        }
    }

    if settings.http.is_none() && settings.https.is_none() {
        None
    } else {
        Some(settings)
    }
}

#[cfg(not(target_os = "windows"))]
fn system_proxy_from_registry() -> Option<SystemProxySettings> {
    None
}

fn create_proxies(cfg: &config::ProxyConfig) -> Vec<Proxy> {
    if !cfg.enabled {
        return Vec::new();
    }

    match cfg.proxy_type.as_str() {
        "none" => Vec::new(),
        "custom" => {
            let Some(url) = cfg.url.as_ref().map(|u| u.trim()).filter(|u| !u.is_empty()) else {
                crate::logger::warn(
                    "client",
                    "Proxy type is 'custom' but no URL configured, ignoring proxy",
                );
                return Vec::new();
            };
            let mut proxy = match Proxy::all(normalize_proxy_url(url, "http")) {
                Ok(proxy) => proxy,
                Err(_) => {
                    crate::logger::warn("client", "Failed to parse custom proxy URL, ignoring proxy");
                    return Vec::new();
                }
            };

            if let (Some(username), Some(password)) = (&cfg.username, &cfg.password) {
                proxy = proxy.basic_auth(username, password);
            }

            if let Some(no_proxy) = build_no_proxy(cfg, None) {
                proxy = proxy.no_proxy(Some(no_proxy));
            }

            crate::logger::debug("client", &format!("Using custom proxy: {}", url));
            vec![proxy]
        }
        "system" | _ => {
            let settings = system_proxy_from_env().or_else(system_proxy_from_registry);
            let Some(settings) = settings else {
                return Vec::new();
            };

            let no_proxy = build_no_proxy(cfg, settings.bypass.as_deref());
            let mut proxies = Vec::new();

            if let Some(http) = settings.http.as_ref() {
                if let Ok(proxy) = Proxy::http(normalize_proxy_url(http, "http")) {
                    proxies.push(proxy);
                }
            }

            if let Some(https) = settings.https.as_ref() {
                if let Ok(proxy) = Proxy::https(normalize_proxy_url(https, "http")) {
                    proxies.push(proxy);
                }
            }

            if proxies.is_empty() {
                if let Some(url) = settings.https.as_ref().or(settings.http.as_ref()) {
                    if let Ok(proxy) = Proxy::all(normalize_proxy_url(url, "http")) {
                        proxies.push(proxy);
                    }
                }
            }

            if let Some(no_proxy) = no_proxy {
                proxies = proxies
                    .into_iter()
                    .map(|proxy| proxy.no_proxy(Some(no_proxy.clone())))
                    .collect();
            }

            if !proxies.is_empty() {
                crate::logger::debug("client", "Using system proxy settings");
            }

            proxies
        }
    }
}

/// Create a new HTTP client with standard configuration
pub fn create_client(timeout_secs: u64) -> ForwardResult<Client> {
    let cfg = config::load();
    let builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10));

    // Add proxy if configured
    let builder = if let Some(proxy_cfg) = cfg.proxy.as_ref() {
        let proxies = create_proxies(proxy_cfg);
        if proxies.is_empty() && proxy_cfg.enabled {
            crate::logger::warn(
                "client",
                &format!(
                    "Proxy enabled but no proxy could be resolved (type: {}).",
                    proxy_cfg.proxy_type
                ),
            );
        }
        let mut builder = builder;
        for proxy in proxies {
            builder = builder.proxy(proxy);
        }
        builder
    } else {
        builder
    };

    builder
        .build()
        .map_err(|e| ForwardError::Internal(format!("Failed to create HTTP client: {}", e)))
}

/// Create a default client for non-streaming requests
pub fn default_client() -> ForwardResult<Client> {
    create_client(120)
}

/// Create a client for streaming requests (longer timeout)
pub fn streaming_client() -> ForwardResult<Client> {
    create_client(300)
}

/// Determine if an HTTP status code should trigger a retry
pub fn should_retry(status: u16) -> bool {
    // Only retry on server errors (5xx) and rate limiting (429)
    // Do NOT retry on client errors (4xx) as they indicate bad requests
    matches!(status, 500 | 502 | 503 | 504 | 429)
}

/// Calculate delay with exponential backoff and jitter
pub fn calculate_retry_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let base_delay = config.initial_delay_ms;
    let max_delay = config.max_delay_ms;

    // Exponential backoff: 2^attempt * base_delay
    let exp_delay = (1u64 << attempt.min(10)) * base_delay;
    let delay = exp_delay.min(max_delay);

    // Add jitter (up to 25% of delay)
    let mut jitter_bytes = [0u8; 8];
    OsRng.fill_bytes(&mut jitter_bytes);
    let jitter = u64::from_le_bytes(jitter_bytes) % (delay / 4 + 1);

    Duration::from_millis(delay + jitter)
}

/// Result of a single request attempt
pub struct RequestAttemptResult {
    pub response: Response,
    #[allow(dead_code)]
    pub latency_ms: u64,
}

/// Make a single POST request attempt
pub async fn make_request(
    client: &Client,
    url: &str,
    headers: HeaderMap,
    body: &Value,
) -> ForwardResult<RequestAttemptResult> {
    let start = Instant::now();

    crate::logger::debug("client", &format!("Sending request to: {}", url));

    let response = client
        .post(url)
        .headers(headers)
        .json(body)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("client", &format!("Request failed: {}", e));
            if e.is_timeout() {
                ForwardError::Timeout("Request timeout".to_string())
            } else if e.is_connect() {
                ForwardError::RequestFailed(format!("Connection failed: {}", e))
            } else {
                ForwardError::RequestFailed(format!("Request error: {}", e))
            }
        })?;

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status();

    crate::logger::debug("client", &format!("Response status: {} ({}ms)", status, latency_ms));

    Ok(RequestAttemptResult {
        response,
        latency_ms,
    })
}

/// Send request with retry using exponential backoff
pub async fn send_with_retry(
    client: &Client,
    endpoints: &[String],
    path: &str,
    headers: HeaderMap,
    body: &Value,
    config: &RetryConfig,
) -> ForwardResult<RequestAttemptResult> {
    if endpoints.is_empty() {
        return Err(ForwardError::UpstreamNotFound(
            "No endpoints configured".to_string(),
        ));
    }

    let mut attempt = 0u32;
    let mut endpoint_idx = 0usize;
    let mut last_error: Option<String> = None;

    loop {
        if attempt >= config.max_attempts {
            return Err(ForwardError::RequestFailed(format!(
                "Max retries ({}) exceeded. Last error: {}",
                config.max_attempts,
                last_error.unwrap_or_else(|| "Unknown".to_string())
            )));
        }

        let endpoint = &endpoints[endpoint_idx];
        let url = format!("{}{}", endpoint.trim_end_matches('/'), path);

        match make_request(client, &url, headers.clone(), body).await {
            Ok(result) => {
                let status = result.response.status();

                if status.is_success() {
                    return Ok(result);
                } else if !should_retry(status.as_u16()) {
                    // Don't retry on client errors (4xx except 429)
                    let error_body = result.response.text().await.unwrap_or_default();
                    return Err(ForwardError::RequestFailed(format!(
                        "Upstream returned {}: {}",
                        status, error_body
                    )));
                } else {
                    last_error = Some(format!("HTTP {}", status));
                }
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }

        // Prepare for retry
        attempt += 1;
        endpoint_idx = (endpoint_idx + 1) % endpoints.len();

        // Wait before retrying
        let delay = calculate_retry_delay(attempt, config);
        tokio::time::sleep(delay).await;
    }
}

/// Parse SSE (Server-Sent Events) data line
pub fn parse_sse_data(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix("data:") {
        return Some(rest.strip_prefix(' ').unwrap_or(rest));
    }
    None
}

/// Check if SSE line indicates stream end
pub fn is_sse_done(data: &str) -> bool {
    data.trim() == "[DONE]"
}

/// Normalize stream flag to a boolean if present.
pub fn normalize_stream_flag(payload: &mut Value) -> bool {
    let Some(obj) = payload.as_object_mut() else {
        return false;
    };

    let is_streaming = match obj.get("stream") {
        Some(Value::Bool(stream)) => *stream,
        Some(Value::Number(value)) => value.as_i64().map(|v| v != 0).unwrap_or(false),
        Some(Value::String(value)) => {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "true" | "1" | "yes" | "on")
        }
        _ => false,
    };

    if obj.contains_key("stream") {
        obj.insert("stream".to_string(), Value::Bool(is_streaming));
    }

    is_streaming
}

/// Parse JSON response text with a fallback for SSE `[DONE]` payloads.
pub fn parse_json_response(response_text: &str) -> Result<Value, serde_json::Error> {
    let trimmed = response_text.trim();
    let parse_err = match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => return Ok(value),
        Err(err) => err,
    };

    if response_text.contains("data:") {
        let mut last_value = None;
        for line in response_text.lines() {
            if let Some(data) = parse_sse_data(line) {
                let data = data.trim();
                if data.is_empty() || is_sse_done(data) {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    last_value = Some(value);
                }
            }
        }
        if let Some(value) = last_value {
            return Ok(value);
        }
    }

    if trimmed.contains("[DONE]") {
        let cleaned = trimmed.replace("[DONE]", "");
        if let Ok(value) = serde_json::from_str::<Value>(cleaned.trim()) {
            return Ok(value);
        }
    }

    Err(parse_err)
}

/// Drain complete SSE lines from a byte buffer.
///
/// This handles chunked responses where line breaks may split across reads.
pub fn drain_sse_lines(buffer: &mut Vec<u8>, chunk: &[u8]) -> Vec<String> {
    if !chunk.is_empty() {
        buffer.extend_from_slice(chunk);
    }

    let mut lines = Vec::new();
    loop {
        let Some(pos) = buffer.iter().position(|&b| b == b'\n') else {
            break;
        };

        let mut line = buffer.drain(..=pos).collect::<Vec<u8>>();
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }

        lines.push(String::from_utf8_lossy(&line).to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_retry() {
        assert!(should_retry(500));
        assert!(should_retry(502));
        assert!(should_retry(503));
        assert!(should_retry(504));
        assert!(should_retry(429));

        assert!(!should_retry(400));
        assert!(!should_retry(401));
        assert!(!should_retry(403));
        assert!(!should_retry(404));
        assert!(!should_retry(200));
    }

    #[test]
    fn test_calculate_retry_delay() {
        let config = RetryConfig::default();

        let delay0 = calculate_retry_delay(0, &config);
        let delay1 = calculate_retry_delay(1, &config);
        let delay2 = calculate_retry_delay(2, &config);

        // Each delay should be larger (exponential backoff)
        assert!(delay1 >= delay0);
        assert!(delay2 >= delay1);

        // Should not exceed max delay
        let delay_max = calculate_retry_delay(20, &config);
        assert!(delay_max.as_millis() <= (config.max_delay_ms + config.max_delay_ms / 4) as u128);
    }

    #[test]
    fn test_parse_sse_data() {
        assert_eq!(parse_sse_data("data: hello"), Some("hello"));
        assert_eq!(parse_sse_data("data:hello"), Some("hello"));
        assert_eq!(parse_sse_data("data: [DONE]"), Some("[DONE]"));
        assert_eq!(parse_sse_data("event: message"), None);
        assert_eq!(parse_sse_data("hello"), None);
    }

    #[test]
    fn test_is_sse_done() {
        assert!(is_sse_done("[DONE]"));
        assert!(is_sse_done("  [DONE]  "));
        assert!(!is_sse_done("{}"));
        assert!(!is_sse_done("data"));
    }

    #[test]
    fn test_drain_sse_lines_partial() {
        let mut buffer = Vec::new();
        let lines = drain_sse_lines(&mut buffer, b"data: {\"id\":");
        assert!(lines.is_empty());

        let lines = drain_sse_lines(&mut buffer, b"1}\n");
        assert_eq!(lines, vec!["data: {\"id\":1}"]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_drain_sse_lines_crlf() {
        let mut buffer = Vec::new();
        let lines = drain_sse_lines(&mut buffer, b"data: ok\r\n");
        assert_eq!(lines, vec!["data: ok"]);
        assert!(buffer.is_empty());
    }
}
