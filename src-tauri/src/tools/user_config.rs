use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::logger;

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct ToolsUserConfig {
    pub version: Option<u32>,
    /// 禁用指定 id 的工具（包含内置 + 用户自定义）
    pub disabled_tools: Vec<String>,
    /// 追加/覆盖工具定义：id 相同则覆盖内置工具
    pub tools: Vec<UserTool>,
    /// 如果提供则替换默认包管理器列表（仍会应用 disabled_package_managers）
    pub package_managers: Option<Vec<UserPackageManager>>,
    /// 禁用指定 name 的包管理器
    pub disabled_package_managers: Vec<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ToolCategory {
    Ide,
    Scm,
    Language,
    AiCli,
}

impl ToolCategory {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            ToolCategory::Ide => "ide",
            ToolCategory::Scm => "scm",
            ToolCategory::Language => "language",
            ToolCategory::AiCli => "ai-cli",
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum PlatformValue<T> {
    Single(T),
    PerPlatform(PerPlatform<T>),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub(super) struct PerPlatform<T> {
    pub windows: Option<T>,
    pub mac: Option<T>,
    pub linux: Option<T>,
    pub default: Option<T>,
}

impl<T> Default for PerPlatform<T> {
    fn default() -> Self {
        Self {
            windows: None,
            mac: None,
            linux: None,
            default: None,
        }
    }
}

impl<T: Clone> PlatformValue<T> {
    pub(super) fn resolve(&self) -> Option<T> {
        match self {
            PlatformValue::Single(value) => Some(value.clone()),
            PlatformValue::PerPlatform(value) => value.resolve(),
        }
    }
}

impl<T: Clone> PerPlatform<T> {
    fn resolve(&self) -> Option<T> {
        match std::env::consts::OS {
            "windows" => self.windows.clone().or_else(|| self.default.clone()),
            "macos" => self.mac.clone().or_else(|| self.default.clone()),
            "linux" => self.linux.clone().or_else(|| self.default.clone()),
            _ => self.default.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct UserTool {
    pub id: String,
    pub name: Option<String>,
    pub category: Option<ToolCategory>,
    /// 用于检测的命令名列表（会走 PATH/which），支持按系统覆盖
    pub commands: Option<PlatformValue<Vec<String>>>,
    /// 显式路径（优先于 commands），支持模板与按系统覆盖
    pub path: Option<PlatformValue<String>>,
    pub path_regex: Option<PlatformValue<String>>,
    pub version: Option<UserVersionSpec>,
    pub config_path: Option<PlatformValue<String>>,
    pub install_hint: Option<String>,
    pub homepage: Option<String>,
    #[serde(default)]
    pub install_commands: Vec<UserInstallCommand>,
    pub cli: Option<UserCliSpec>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct UserVersionSpec {
    /// 自定义版本检测程序（默认使用检测到的工具路径），支持按系统覆盖与模板
    pub program: Option<PlatformValue<String>>,
    /// 自定义版本检测参数（默认 ["--version"]），支持按系统覆盖与模板
    pub args: Option<PlatformValue<Vec<String>>>,
    /// 自定义正则提取版本号（匹配第一个捕获组优先，否则用整段匹配）
    pub regex: Option<String>,
    /// 超时毫秒数（默认 2000）
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct UserCliSpec {
    /// 是否允许在 UI 中显示"打开 CLI"
    pub enabled: Option<bool>,
    pub label: Option<String>,
    /// 自定义启动程序（默认使用检测到的工具路径），支持按系统覆盖与模板
    pub program: Option<PlatformValue<String>>,
    /// 自定义启动参数，支持按系统覆盖与模板
    pub args: Option<PlatformValue<Vec<String>>>,
    /// 是否后台运行（不打开终端窗口），默认 false
    pub background: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct UserInstallCommand {
    pub manager: String,
    pub command: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct UserPackageManager {
    pub name: String,
    /// 用于检测的命令名列表（默认 [name]），支持按系统覆盖
    pub command: Option<PlatformValue<Vec<String>>>,
    /// 显式路径（优先于 command），支持模板与按系统覆盖
    pub path: Option<PlatformValue<String>>,
    pub install_hint: Option<String>,
}

fn _pv_string(value: &str) -> PlatformValue<String> {
    PlatformValue::Single(value.to_string())
}

fn _pv_vec(values: &[&str]) -> PlatformValue<Vec<String>> {
    PlatformValue::Single(values.iter().map(|v| v.to_string()).collect())
}

#[allow(dead_code)]
fn _per_platform_string(
    windows: Option<&str>,
    mac: Option<&str>,
    linux: Option<&str>,
    default: Option<&str>,
) -> PlatformValue<String> {
    PlatformValue::PerPlatform(PerPlatform {
        windows: windows.map(|v| v.to_string()),
        mac: mac.map(|v| v.to_string()),
        linux: linux.map(|v| v.to_string()),
        default: default.map(|v| v.to_string()),
    })
}

// 编译时内置默认配置文件
const BUILTIN_DEFAULTS_JSON: &str = include_str!("../../resources/tools.defaults.json");

fn default_tools_config() -> ToolsUserConfig {
    // 从内置的 JSON 文件解析默认配置
    match serde_json::from_str::<ToolsUserConfig>(BUILTIN_DEFAULTS_JSON) {
        Ok(cfg) => cfg,
        Err(e) => {
            // 解析失败时记录错误并返回空配置
            crate::logger::error(
                "tools",
                &format!("[tools.defaults.json] 内置配置解析失败: {}", e),
            );
            ToolsUserConfig::default()
        }
    }
}

pub(super) fn config_path() -> PathBuf {
    let mut p = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("CCR");
    let _ = fs::create_dir_all(&p);
    p.push("tools.json");
    p
}

pub(super) fn config_mtime() -> Option<SystemTime> {
    let p = config_path();
    fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

pub(super) fn load_defaults() -> ToolsUserConfig {
    default_tools_config()
}

pub(super) fn load() -> ToolsUserConfig {
    let p = config_path();
    if !p.exists() {
        return ToolsUserConfig::default();
    }

    let s = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) => {
            logger::error("tools", &format!("[tools.json] 读取失败: {} ({:?})", e, p));
            return ToolsUserConfig::default();
        }
    };

    match serde_json::from_str(&s) {
        Ok(cfg) => cfg,
        Err(e) => {
            logger::error("tools", &format!("[tools.json] 解析失败: {} ({:?})", e, p));
            ToolsUserConfig::default()
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TemplateContext {
    pub user_path: String,
    pub data_dir: String,
    pub tool_path: Option<String>,
}

impl TemplateContext {
    pub(super) fn new(tool_path: Option<String>) -> Self {
        let user_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        let mut data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data_dir.push("CCR");
        let data_dir = data_dir.to_string_lossy().to_string();

        Self {
            user_path,
            data_dir,
            tool_path,
        }
    }
}

pub(super) fn expand_template(input: &str, ctx: &TemplateContext) -> String {
    // 先处理固定占位符，再展开环境变量占位符
    let mut s = input.to_string();

    s = s.replace("{USER_PATH}", &ctx.user_path);
    s = s.replace("{CCR_DATA_DIR}", &ctx.data_dir);

    if let Some(ref tool_path) = ctx.tool_path {
        s = s.replace("{TOOL_PATH}", tool_path);
        s = s.replace("{COMMAND_PATH}", tool_path);
    }

    expand_env_vars(&s)
}

fn expand_env_vars(input: &str) -> String {
    // 支持形如 {$APPDATA} 的环境变量插值
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'{' && i + 2 < bytes.len() && bytes[i + 1] == b'$' {
            // 找到右括号
            if let Some(end) = input[i + 2..].find('}') {
                let end = i + 2 + end;
                let name = &input[i + 2..end];
                if !name.is_empty() {
                    if let Ok(val) = std::env::var(name) {
                        out.push_str(&val);
                    }
                }
                i = end + 1;
                continue;
            }
        }

        // 兜底：逐字节复制（UTF-8 安全，因为只在匹配 ASCII 模式下走特殊逻辑）
        out.push(input[i..].chars().next().unwrap());
        i += input[i..].chars().next().unwrap().len_utf8();
    }

    out
}

pub(super) fn stable_mtime_token(mtime: Option<SystemTime>) -> u64 {
    // 用于缓存对比：统一成毫秒 token（精度不足时也能稳定对比）
    mtime
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
