use regex::Regex;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::logger;

mod user_config;

// 环境检测缓存
static ENV_CACHE: OnceLock<Mutex<EnvCache>> = OnceLock::new();

// 命令路径缓存，避免重复检测
static CMD_CACHE: OnceLock<Mutex<std::collections::HashMap<String, Option<PathBuf>>>> =
    OnceLock::new();

struct EnvCache {
    report: Option<EnvironmentReport>,
    last_update: Option<Instant>,
    tools_config_token: u64,
}

impl EnvCache {
    fn new() -> Self {
        Self {
            report: None,
            last_update: None,
            tools_config_token: 0,
        }
    }

    fn is_valid(&self, tools_config_token: u64) -> bool {
        match self.last_update {
            Some(t) => {
                t.elapsed() < Duration::from_secs(120) && self.tools_config_token == tools_config_token
            } // 120秒缓存，提高到 2 分钟
            None => false,
        }
    }
}

fn get_cache() -> &'static Mutex<EnvCache> {
    ENV_CACHE.get_or_init(|| Mutex::new(EnvCache::new()))
}

fn get_cmd_cache() -> &'static Mutex<std::collections::HashMap<String, Option<PathBuf>>> {
    CMD_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[derive(Serialize, Clone)]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub installed: bool,
    pub version: Option<String>,
    pub command_path: Option<String>,
    pub config_path: Option<String>,
    pub install_hint: String,
    pub homepage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launcher: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub install_commands: Vec<InstallCommand>,
}

#[derive(Serialize)]
pub struct ToolInstallPlan {
    pub id: String,
    pub instructions: String,
    pub url: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub commands: Vec<InstallCommand>,
}

#[derive(Serialize, Clone)]
pub struct InstallCommand {
    pub manager: String,
    pub command: String,
}

#[derive(Clone)]
struct ToolCliDef {
    enabled: bool,
    program_template: Option<String>,
    args: Vec<String>,
    label: String,
    background: bool,
}

#[derive(Clone)]
struct ToolRuntimeDef {
    id: String,
    name: String,
    category: String,
    /// 用于检测的命令名（会走 PATH/which）；允许是绝对路径或带模板
    commands: Vec<String>,
    /// 显式路径（优先于 commands）；允许带模板
    path_template: Option<String>,
    path_regex: Option<String>,

    version_program_template: Option<String>,
    version_args: Vec<String>,
    version_regex: Option<String>,
    version_timeout_ms: u64,

    config_resolver: Option<fn() -> Option<String>>,
    config_path_template: Option<String>,

    install_hint: String,
    homepage: String,
    installers: Vec<InstallCommand>,

    cli: Option<ToolCliDef>,
}

#[derive(Clone)]
struct PackageManagerDef {
    name: String,
    commands: Vec<String>,
    path_template: Option<String>,
    install_hint: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct PackageManagerInfo {
    pub name: String,
    pub installed: bool,
    pub command_path: Option<String>,
    pub install_hint: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct EnvironmentReport {
    pub os: String,
    pub arch: String,
    pub package_managers: Vec<PackageManagerInfo>,
    pub ide_tools: Vec<ToolInfo>,
    pub languages: Vec<ToolInfo>,
    pub ai_tools: Vec<ToolInfo>,
}

struct RuntimeCatalog {
    tools: Vec<ToolRuntimeDef>,
    package_managers: Vec<PackageManagerDef>,
}

fn tools_config_token() -> u64 {
    user_config::stable_mtime_token(user_config::config_mtime())
}

fn load_runtime_catalog() -> RuntimeCatalog {
    let cfg = user_config::load();
    let tools = merge_tool_defs(&cfg);
    let package_managers = merge_package_manager_defs(&cfg);
    RuntimeCatalog {
        tools,
        package_managers,
    }
}

fn builtin_tool_defs() -> Vec<ToolRuntimeDef> {
    use std::collections::HashSet;

    let cfg = user_config::load_defaults();
    let disabled: HashSet<String> = cfg
        .disabled_tools
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    cfg.tools
        .iter()
        .filter_map(|ut| {
            let id = ut.id.trim();
            if id.is_empty() || disabled.contains(id) {
                return None;
            }
            let mut def = default_user_tool_def(id);
            apply_user_tool_overrides(&mut def, ut);
            Some(def)
        })
        .collect()
}

fn default_user_tool_def(id: &str) -> ToolRuntimeDef {
    ToolRuntimeDef {
        id: id.to_string(),
        name: id.to_string(),
        category: "ide".to_string(),
        commands: vec![id.to_string()],
        path_template: None,
        path_regex: None,
        version_program_template: None,
        version_args: vec!["--version".to_string()],
        version_regex: None,
        version_timeout_ms: 2000,
        config_resolver: None,
        config_path_template: None,
        install_hint: String::new(),
        homepage: String::new(),
        installers: Vec::new(),
        cli: Some(ToolCliDef {
            enabled: true,
            program_template: None,
            args: Vec::new(),
            label: id.to_string(),
            background: false,
        }),
    }
}

fn merge_tool_defs(cfg: &user_config::ToolsUserConfig) -> Vec<ToolRuntimeDef> {
    use std::collections::{HashMap, HashSet};

    let disabled: HashSet<String> = cfg
        .disabled_tools
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut defs: Vec<ToolRuntimeDef> = Vec::new();
    let mut index_by_id: HashMap<String, usize> = HashMap::new();

    for def in builtin_tool_defs() {
        if disabled.contains(&def.id) {
            continue;
        }
        index_by_id.insert(def.id.clone(), defs.len());
        defs.push(def);
    }

    for ut in &cfg.tools {
        let id = ut.id.trim();
        if id.is_empty() || disabled.contains(id) {
            continue;
        }

        let mut merged = if let Some(idx) = index_by_id.get(id).copied() {
            defs[idx].clone()
        } else {
            default_user_tool_def(id)
        };

        apply_user_tool_overrides(&mut merged, ut);

        if let Some(idx) = index_by_id.get(id).copied() {
            defs[idx] = merged;
        } else {
            index_by_id.insert(id.to_string(), defs.len());
            defs.push(merged);
        }
    }

    // 二次过滤：允许用户在 tools 里覆盖后仍通过 disabled_tools 一键禁用
    defs.into_iter().filter(|d| !disabled.contains(&d.id)).collect()
}

fn apply_user_tool_overrides(def: &mut ToolRuntimeDef, ut: &user_config::UserTool) {
    if let Some(ref name) = ut.name {
        if !name.trim().is_empty() {
            def.name = name.trim().to_string();
        }
    }
    if let Some(cat) = ut.category {
        def.category = cat.as_str().to_string();
    }

    if let Some(ref commands) = ut.commands {
        if let Some(resolved) = commands.resolve() {
            def.commands = resolved;
        }
    }
    if let Some(ref path) = ut.path {
        if let Some(resolved) = path.resolve() {
            def.path_template = Some(resolved);
        }
    }
    if let Some(ref path_regex) = ut.path_regex {
        if let Some(resolved) = path_regex.resolve() {
            def.path_regex = Some(resolved);
        }
    }

    if let Some(ref v) = ut.version {
        if let Some(ref program) = v.program {
            if let Some(resolved) = program.resolve() {
                def.version_program_template = Some(resolved);
            }
        }
        if let Some(ref args) = v.args {
            if let Some(resolved) = args.resolve() {
                def.version_args = resolved;
            }
        }
        if let Some(ref r) = v.regex {
            def.version_regex = Some(r.to_string());
        }
        if let Some(ms) = v.timeout_ms {
            if ms > 0 {
                def.version_timeout_ms = ms;
            }
        }
    }

    if let Some(ref p) = ut.config_path {
        if let Some(resolved) = p.resolve() {
            def.config_path_template = Some(resolved);
            def.config_resolver = None;
        }
    }

    if let Some(ref homepage) = ut.homepage {
        def.homepage = homepage.trim().to_string();
    }
    if let Some(ref hint) = ut.install_hint {
        def.install_hint = hint.trim().to_string();
    }

    if !ut.install_commands.is_empty() {
        def.installers = ut
            .install_commands
            .iter()
            .filter(|c| !c.manager.trim().is_empty() && !c.command.trim().is_empty())
            .map(|c| InstallCommand {
                manager: c.manager.trim().to_string(),
                command: c.command.trim().to_string(),
            })
            .collect();
    }

    if let Some(ref cli) = ut.cli {
        let enabled = cli.enabled.unwrap_or(true);
        if !enabled {
            def.cli = None;
        } else {
            let mut merged = def.cli.clone().unwrap_or(ToolCliDef {
                enabled: true,
                program_template: None,
                args: Vec::new(),
                label: def.id.clone(),
                background: false,
            });
            merged.enabled = true;
            if let Some(ref label) = cli.label {
                if !label.trim().is_empty() {
                    merged.label = label.trim().to_string();
                }
            }
            if let Some(ref program) = cli.program {
                if let Some(resolved) = program.resolve() {
                    merged.program_template = Some(resolved);
                }
            }
            if let Some(ref args) = cli.args {
                if let Some(resolved) = args.resolve() {
                    merged.args = resolved;
                }
            }
            if let Some(ref background) = cli.background {
                merged.background = *background;
            }
            def.cli = Some(merged);
        }
    }

    // 兜底：如果用户定义了 homepage 但没写 install_hint，则默认使用 homepage
    if def.install_hint.is_empty() && !def.homepage.is_empty() {
        def.install_hint = def.homepage.clone();
    }
}

fn default_package_manager_defs() -> Vec<PackageManagerDef> {
    vec![
        PackageManagerDef {
            name: "winget".to_string(),
            commands: vec!["winget".to_string()],
            path_template: None,
            install_hint: Some("https://learn.microsoft.com/windows/package-manager/winget".to_string()),
        },
        PackageManagerDef {
            name: "brew".to_string(),
            commands: vec!["brew".to_string()],
            path_template: None,
            install_hint: Some("https://brew.sh".to_string()),
        },
        PackageManagerDef {
            name: "npm".to_string(),
            commands: vec!["npm".to_string()],
            path_template: None,
            install_hint: Some("https://nodejs.org/en/download".to_string()),
        },
        PackageManagerDef {
            name: "pip".to_string(),
            commands: vec!["pip".to_string()],
            path_template: None,
            install_hint: Some("https://pip.pypa.io".to_string()),
        },
        PackageManagerDef {
            name: "apt".to_string(),
            commands: vec!["apt".to_string()],
            path_template: None,
            install_hint: None,
        },
        PackageManagerDef {
            name: "dnf".to_string(),
            commands: vec!["dnf".to_string()],
            path_template: None,
            install_hint: None,
        },
        PackageManagerDef {
            name: "pacman".to_string(),
            commands: vec!["pacman".to_string()],
            path_template: None,
            install_hint: None,
        },
        PackageManagerDef {
            name: "cargo".to_string(),
            commands: vec!["cargo".to_string()],
            path_template: None,
            install_hint: Some("https://rustup.rs".to_string()),
        },
    ]
}

fn merge_package_manager_defs(cfg: &user_config::ToolsUserConfig) -> Vec<PackageManagerDef> {
    use std::collections::HashSet;

    let disabled: HashSet<String> = cfg
        .disabled_package_managers
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let base: Vec<PackageManagerDef> = cfg
        .package_managers
        .as_ref()
        .map(|pms| {
            pms.iter()
                .filter_map(|pm| {
                    let name = pm.name.trim();
                    if name.is_empty() || disabled.contains(name) {
                        return None;
                    }
                    let commands = pm
                        .command
                        .as_ref()
                        .and_then(|c| c.resolve())
                        .unwrap_or_else(|| vec![name.to_string()]);
                    Some(PackageManagerDef {
                        name: name.to_string(),
                        commands,
                        path_template: pm.path.as_ref().and_then(|p| p.resolve()),
                        install_hint: pm.install_hint.as_ref().map(|s| s.trim().to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_else(default_package_manager_defs);

    base.into_iter().filter(|pm| !disabled.contains(&pm.name)).collect()
}

fn detect_command(commands: &[&str]) -> Option<PathBuf> {
    // 检查缓存
    let cache_key = commands.join(",");
    {
        let cache = get_cmd_cache().lock().unwrap();
        if let Some(result) = cache.get(&cache_key) {
            return result.clone();
        }
    }

    let result = detect_command_uncached(commands);

    // 存入缓存
    {
        let mut cache = get_cmd_cache().lock().unwrap();
        cache.insert(cache_key, result.clone());
    }

    result
}

fn detect_command_uncached(commands: &[&str]) -> Option<PathBuf> {
    for cmd in commands {
        if let Ok(path) = which::which(cmd) {
            return Some(path);
        }
    }
    if cfg!(windows) {
        // 首先检查 npm 全局路径 (%APPDATA%\npm)
        // 这是 npm 全局安装包的默认位置
        if let Ok(appdata) = std::env::var("APPDATA") {
            let npm_path = Path::new(&appdata).join("npm");
            for cmd in commands {
                // 检查 .cmd 文件 (npm 在 Windows 上创建的批处理文件)
                let cmd_file = npm_path.join(format!("{}.cmd", cmd));
                if cmd_file.exists() {
                    return Some(cmd_file);
                }
                // 检查 .ps1 文件 (PowerShell 脚本)
                let ps1_file = npm_path.join(format!("{}.ps1", cmd));
                if ps1_file.exists() {
                    return Some(ps1_file);
                }
                // 检查 .exe 文件
                let exe_file = npm_path.join(format!("{}.exe", cmd));
                if exe_file.exists() {
                    return Some(exe_file);
                }
                // 检查无扩展名文件
                let plain_file = npm_path.join(cmd);
                if plain_file.exists() {
                    return Some(plain_file);
                }
            }
        }

        // 然后检查其他常见路径
        for cmd in commands {
            let exe = if cmd.to_ascii_lowercase().ends_with(".exe") {
                cmd.to_string()
            } else {
                format!("{cmd}.exe")
            };
            let guesses = [
                std::env::var("LOCALAPPDATA").ok().map(|p| {
                    Path::new(&p)
                        .join("Microsoft")
                        .join("WindowsApps")
                        .join(&exe)
                }),
                std::env::var("ProgramFiles")
                    .ok()
                    .map(|p| Path::new(&p).join("WindowsApps").join(&exe)),
                Some(PathBuf::from(r"C:\Windows\System32").join(&exe)),
            ];
            for g in guesses.into_iter().flatten() {
                if g.exists() {
                    return Some(g);
                }
            }
        }
    }
    None
}

fn detect_winget() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    if let Some(found) = detect_command(&["winget"]) {
        return Some(found);
    }

    let mut candidates = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(Path::new(&local).join("Microsoft/WindowsApps/winget.exe"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        let base = Path::new(&program_files).join("WindowsApps");
        candidates.push(base.join("winget.exe"));
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path().join("winget.exe");
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        let base = Path::new(&program_files_x86).join("WindowsApps");
        candidates.push(base.join("winget.exe"));
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path().join("winget.exe");
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    candidates.push(PathBuf::from(r"C:\Windows\System32\winget.exe"));
    candidates.into_iter().find(|p| p.exists())
}

fn read_version(
    path: &Path,
    args: &[&str],
    regex: Option<&Regex>,
    timeout_ms: u64,
) -> Option<String> {
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::thread;

    let path = path.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let timeout_ms = timeout_ms.max(1);

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut cmd = Command::new(&path);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 在 Windows 上隐藏 CMD 窗口
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output().ok();
        let _ = tx.send(output);
    });

    let output = rx.recv_timeout(Duration::from_millis(timeout_ms)).ok()??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    if let Some(regex) = regex {
        if let Some(caps) = regex.captures(&combined) {
            if let Some(m) = caps.get(1).or_else(|| caps.get(0)) {
                let value = m.as_str().trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }

    let line = stdout.lines().next().unwrap_or_else(|| stdout.trim());
    if line.is_empty() {
        let err_line = stderr.lines().next().unwrap_or("").trim();
        if err_line.is_empty() {
            None
        } else {
            Some(err_line.to_string())
        }
    } else {
        Some(line.trim().to_string())
    }
}

fn template_ctx(tool_path: Option<&Path>) -> user_config::TemplateContext {
    user_config::TemplateContext::new(tool_path.map(|p| p.to_string_lossy().to_string()))
}

fn expand_args(args: &[String], ctx: &user_config::TemplateContext) -> Vec<String> {
    args.iter()
        .map(|a| user_config::expand_template(a, ctx))
        .collect()
}

fn looks_like_path(value: &str) -> bool {
    value.contains('\\') || value.contains('/') || value.contains(':')
}

fn resolve_cli_program(
    cli: &ToolCliDef,
    tool_path: &Path,
    ctx: &user_config::TemplateContext,
) -> String {
    if let Some(ref tpl) = cli.program_template {
        let expanded = user_config::expand_template(tpl, ctx);
        let trimmed = expanded.trim();
        if !trimmed.is_empty() {
            if looks_like_path(trimmed) {
                let candidate = PathBuf::from(trimmed);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
            } else if which::which(trimmed).is_ok() {
                return trimmed.to_string();
            }
        }
    }

    tool_path.to_string_lossy().to_string()
}

fn regex_matches_path(regex: &Regex, path: &Path) -> bool {
    let raw = path.to_string_lossy();
    if regex.is_match(&raw) {
        return true;
    }
    if raw.contains('\\') {
        let normalized = raw.replace('\\', "/");
        return regex.is_match(&normalized);
    }
    false
}

fn expand_regex(
    template: &str,
    ctx: &user_config::TemplateContext,
    label: &str,
) -> Option<Regex> {
    let expanded = user_config::expand_template(template, ctx);
    let pattern = expanded.trim();
    if pattern.is_empty() {
        return None;
    }
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(err) => {
            logger::warn(
                "tools",
                &format!("invalid {label} regex: {pattern} ({err})"),
            );
            None
        }
    }
}

fn detect_command_by_regex(
    commands: &[String],
    ctx: &user_config::TemplateContext,
    regex: &Regex,
) -> Option<PathBuf> {
    for cmd in commands {
        let expanded = user_config::expand_template(cmd, ctx);
        let trimmed = expanded.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cmd_name = trimmed.to_string();
        if let Ok(paths) = which::which_all(cmd_name) {
            for path in paths {
                if regex_matches_path(regex, &path) {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn collect_path_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(Path::new(&appdata).join("npm"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(Path::new(&local).join("Microsoft").join("WindowsApps"));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            dirs.push(Path::new(&program_files).join("WindowsApps"));
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            dirs.push(Path::new(&program_files_x86).join("WindowsApps"));
        }
        dirs.push(PathBuf::from(r"C:\Windows\System32"));
    }

    dirs
}

fn find_path_by_regex(regex: &Regex) -> Option<PathBuf> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    for dir in collect_path_dirs() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        if !dir.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if regex_matches_path(regex, &path) {
                return Some(path);
            }
        }
    }
    None
}

fn resolve_tool_command_path(def: &ToolRuntimeDef) -> Option<PathBuf> {
    let ctx = template_ctx(None);

    if let Some(ref tpl) = def.path_template {
        let p = PathBuf::from(user_config::expand_template(tpl, &ctx));
        if p.exists() {
            return Some(p);
        }
    }

    if let Some(ref path_regex) = def.path_regex {
        if let Some(regex) = expand_regex(path_regex, &ctx, "tool path") {
            if let Some(found) = detect_command_by_regex(&def.commands, &ctx, &regex) {
                return Some(found);
            }
            if let Some(found) = find_path_by_regex(&regex) {
                return Some(found);
            }
        }
    }

    if def.commands.is_empty() {
        return None;
    }

    let cmds = def
        .commands
        .iter()
        .map(|c| user_config::expand_template(c, &ctx))
        .collect::<Vec<_>>();
    let borrowed = cmds.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    detect_command(&borrowed)
}

fn resolve_version_program(
    def: &ToolRuntimeDef,
    tool_path: Option<&Path>,
    ctx: &user_config::TemplateContext,
) -> Option<PathBuf> {
    if let Some(ref tpl) = def.version_program_template {
        let expanded = user_config::expand_template(tpl, ctx);
        let trimmed = expanded.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.exists() {
                return Some(candidate);
            }
            if let Some(found) = detect_command(&[trimmed]) {
                return Some(found);
            }
        }
    }
    tool_path.map(|p| p.to_path_buf())
}

fn resolve_config_path(def: &ToolRuntimeDef, tool_path: Option<&Path>) -> Option<String> {
    let ctx = template_ctx(tool_path);
    if let Some(ref tpl) = def.config_path_template {
        return Some(user_config::expand_template(tpl, &ctx));
    }
    if let Some(resolver) = def.config_resolver {
        return resolver().map(|p| user_config::expand_template(&p, &ctx));
    }
    None
}

pub fn list() -> Vec<ToolInfo> {
    let report = environment_report();
    report
        .ide_tools
        .into_iter()
        .chain(report.languages)
        .chain(report.ai_tools)
        .collect()
}

pub fn install_plan(id: &str) -> Option<ToolInstallPlan> {
    let tool = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)?;

    let commands = tool.installers.clone();
    let url = if !tool.install_hint.is_empty() {
        tool.install_hint.clone()
    } else {
        tool.homepage.clone()
    };

    let cmds = if commands.is_empty() {
        tool.commands
            .first()
            .cloned()
            .unwrap_or_else(|| tool.id.clone())
    } else {
        commands
            .iter()
            .map(|c| format!("{} {}", c.manager, c.command))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let instructions = if url.is_empty() {
        format!("Install via:\n{cmds}", cmds = cmds)
    } else {
        format!(
            "Visit {url} or install via:\n{cmds}",
            url = url,
            cmds = cmds
        )
    };

    Some(ToolInstallPlan {
        id: tool.id.clone(),
        instructions,
        url,
        commands,
    })
}

fn ensure_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::File::create(path)?;
    }
    Ok(())
}

pub fn open_config(id: &str) -> Result<(), String> {
    let tool = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "tool_not_found".to_string())?;

    let tool_path = resolve_tool_command_path(&tool);
    let path = resolve_config_path(&tool, tool_path.as_deref())
        .ok_or_else(|| "config_path_unknown".to_string())?;

    let path_buf = PathBuf::from(&path);
    if path_buf.exists() && path_buf.is_dir() {
        return open::that(path_buf).map_err(|e| e.to_string());
    }

    ensure_file(&path_buf).map_err(|e| e.to_string())?;
    open::that(path_buf).map_err(|e| e.to_string())
}

pub fn open_config_folder(id: &str) -> Result<(), String> {
    let tool = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "tool_not_found".to_string())?;

    let tool_path = resolve_tool_command_path(&tool);
    let path = resolve_config_path(&tool, tool_path.as_deref())
        .ok_or_else(|| "config_path_unknown".to_string())?;

    let folder = PathBuf::from(path);
    let dir = if folder.is_dir() {
        folder
    } else {
        folder.parent().map(|p| p.to_path_buf()).unwrap_or(folder)
    };
    open::that(dir).map_err(|e| e.to_string())
}

fn normalize_workdir(cwd: Option<&str>) -> Result<Option<String>, String> {
    match cwd {
        Some(dir) => {
            // 直接使用原始路径，不进行 canonicalize 转换
            // 因为 canonicalize 在 Windows 下会返回 \\?\ 前缀的路径，导致某些命令无法识别
            let path = Path::new(dir);
            
            logger::debug("tools", &format!("[normalize_workdir] 输入路径: {}", dir));
            
            if !path.exists() {
                let err = format!("工作目录不存在: {}", dir);
                logger::error("tools", &err);
                return Err(err);
            }
            if !path.is_dir() {
                let err = format!("路径不是目录: {}", dir);
                logger::error("tools", &err);
                return Err(err);
            }

            // 直接使用原始路径字符串，避免转换问题
            let result = dir.to_string();
            logger::debug("tools", &format!("[normalize_workdir] 输出路径: {}", result));
            
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn powershell_executable() -> PathBuf {
    std::env::var("SystemRoot")
        .map(|root| {
            PathBuf::from(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .ok()
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("powershell.exe"))
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ps_escape(s: &str) -> String {
    s.replace('`', "``").replace('"', "`\"")
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ps_quote(s: &str) -> String {
    format!("\"{}\"", ps_escape(s))
}

#[cfg(target_os = "windows")]
fn cmd_quote_arg(s: &str) -> String {
    if s.contains(' ') || s.contains('\t') {
        // 对于 Windows 路径,直接用引号包裹即可,不需要额外转义反斜杠
        // 只有当字符串本身包含引号时才需要转义
        if s.contains('"') {
            format!("\"{}\"", s.replace('"', "\\\""))
        } else {
            format!("\"{}\"", s)
        }
    } else {
        s.to_string()
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn build_cmd_line(program: &str, args: &[String]) -> String {
    let mut parts = vec![cmd_quote_arg(program)];
    for arg in args {
        parts.push(cmd_quote_arg(arg));
    }
    parts.join(" ")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn shell_escape_single(s: &str) -> String {
    s.replace('\'', "'\\\\''")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", shell_escape_single(s))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn build_unix_command(program: &str, args: &[String]) -> String {
    let mut cmd = String::new();
    cmd.push_str(&shell_quote(program));
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(arg));
    }
    cmd
}

#[cfg(target_os = "windows")]
fn needs_cmd_start(program: &str) -> bool {
    let lower = program.to_ascii_lowercase();
    lower.ends_with("\\git-bash.exe") || lower.ends_with("/git-bash.exe")
}

fn launch_terminal(
    cwd: Option<&str>,
    program_and_args: Option<(&str, &[String])>,
    background: bool,
) -> Result<(), String> {
    let normalized = normalize_workdir(cwd)?;

    logger::debug(
        "tools",
        &format!(
            "[launch_terminal] cwd={:?}, program={:?}, normalized={:?}, background={}",
            cwd,
            program_and_args.map(|(p, _)| p),
            normalized,
            background
        ),
    );

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        if let Some((program, args)) = program_and_args {
            // 对于 Git Bash 等图形化终端,直接启动而不通过 CMD
            let is_git_bash = needs_cmd_start(&program);

            if is_git_bash && !background {
                // 直接启动 Git Bash,不使用 CMD
                logger::debug(
                    "tools",
                    &format!("[launch_terminal] 直接启动: {}", program),
                );

                let mut cmd = Command::new(&program);
                cmd.args(args);
                cmd.creation_flags(CREATE_NEW_CONSOLE);

                if let Some(ref dir_str) = normalized {
                    cmd.current_dir(dir_str);
                }

                return cmd.spawn().map(|_| ()).map_err(|e| {
                    let err = format!("启动程序失败: {} - {}", program, e);
                    logger::error("tools", &err);
                    err
                });
            }

            // 后台运行或其他程序,使用 Command 直接执行
            if background {
                logger::debug(
                    "tools",
                    &format!("[launch_terminal] 后台执行: {} {:?}", program, args),
                );

                let mut cmd = Command::new(&program);
                cmd.args(args);
                cmd.creation_flags(CREATE_NO_WINDOW);

                if let Some(ref dir_str) = normalized {
                    cmd.current_dir(dir_str);
                }

                return cmd.spawn().map(|_| ()).map_err(|e| {
                    let err = format!("后台执行失败: {} - {}", program, e);
                    logger::error("tools", &err);
                    err
                });
            }

            // 前台运行其他程序,在 CMD 中执行
            let cmd_line = build_cmd_line(&program, args);
            logger::debug(
                "tools",
                &format!("[launch_terminal] CMD 执行: {}", cmd_line),
            );

            let mut cmd = Command::new("cmd");
            cmd.args(["/K", &cmd_line]);
            cmd.creation_flags(CREATE_NEW_CONSOLE);

            if let Some(ref dir_str) = normalized {
                cmd.current_dir(dir_str);
            }

            return cmd.spawn().map(|_| ()).map_err(|e| {
                let err = format!("启动 CMD 失败: {} - {}", cmd_line, e);
                logger::error("tools", &err);
                err
            });
        } else if let Some(ref dir_str) = normalized {
            // 打开文件夹: 使用 start /D 在指定目录打开 cmd，避免复杂引号嵌套
            logger::debug(
                "tools",
                &format!(
                    "[launch_terminal] 执行命令: cmd /C start \"\" /D {:?} cmd",
                    dir_str
                ),
            );

            return Command::new("cmd")
                .args(["/C", "start", "", "/D", dir_str.as_str(), "cmd"])
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    let err = format!("打开终端失败: {}", e);
                    logger::error("tools", &err);
                    err
                });
        } else {
            // 仅打开新的 CMD 窗口
            logger::debug("tools", "[launch_terminal] 打开新的 CMD 窗口");

            return Command::new("cmd")
                .args(["/C", "start cmd"])
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    let err = format!("打开终端失败: {}", e);
                    logger::error("tools", &err);
                    err
                });
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mut script_line = String::new();
        if let Some(ref dir_str) = normalized {
            script_line.push_str(&format!("cd {}", shell_quote(dir_str)));
        }
        if let Some((program, args)) = program_and_args {
            if !script_line.is_empty() {
                script_line.push_str("; ");
            }
            script_line.push_str(&build_unix_command(program, args));
        }

        let script = format!(
            r#"tell application "Terminal"
                activate
                do script "{}"
            end tell"#,
            script_line.replace('"', "\\\"")
        );

        logger::debug("tools", &format!("[launch_terminal] AppleScript: {}", script));

        return Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map(|_| ())
            .map_err(|e| {
                let err = format!("打开终端失败: {}", e);
                logger::error("tools", &err);
                err
            });
    }

    #[cfg(target_os = "linux")]
    {
        let mut shell_cmd = String::new();
        if let Some(ref dir_str) = normalized {
            shell_cmd.push_str(&format!("cd {} && ", shell_quote(dir_str)));
        }

        if let Some((program, args)) = program_and_args {
            shell_cmd.push_str(&build_unix_command(program, args));
            shell_cmd.push_str("; exec $SHELL");
        } else {
            shell_cmd.push_str("exec $SHELL");
        }

        let terminals = [
            ("gnome-terminal", vec!["--", "bash", "-c"]),
            ("konsole", vec!["-e", "bash", "-c"]),
            ("xfce4-terminal", vec!["-e", "bash -c"]),
            ("xterm", vec!["-e", "bash", "-c"]),
        ];

        for (term, prefix) in terminals.iter() {
            if which::which(term).is_ok() {
                let mut cmd = Command::new(term);
                for p in prefix {
                    cmd.arg(p);
                }
                cmd.arg(&shell_cmd);
                logger::debug("tools", &format!("[launch_terminal] 使用终端: {}", term));
                return cmd
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| {
                        let err = format!("打开终端失败: {}", e);
                        logger::error("tools", &err);
                        err
                    });
            }
        }

        return Err("未找到可用的终端模拟器".to_string());
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".to_string())
}

pub fn open_terminal_in_dir(cwd: &str) -> Result<(), String> {
    launch_terminal(Some(cwd), None, false)
}

pub fn launch_cli(id: &str) -> Result<(), String> {
    launch_cli_in_dir(id, None)
}

/// 检查指定工具是否已安装
#[allow(dead_code)]
pub fn is_tool_installed(id: &str) -> bool {
    load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .and_then(|t| resolve_tool_command_path(&t))
        .is_some()
}

/// 获取工具的安装状态和路径信息
#[derive(Serialize)]
#[allow(dead_code)]
pub struct ToolStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub install_hint: String,
    pub install_commands: Vec<InstallCommand>,
}

#[allow(dead_code)]
pub fn get_tool_status(id: &str) -> Option<ToolStatus> {
    let def = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)?;
    let path = resolve_tool_command_path(&def);
    Some(ToolStatus {
        id: def.id.clone(),
        name: def.name.clone(),
        installed: path.is_some(),
        path: path.map(|p| p.to_string_lossy().to_string()),
        install_hint: def.install_hint.clone(),
        install_commands: def.installers.clone(),
    })
}

pub fn launch_cli_in_dir(id: &str, cwd: Option<&str>) -> Result<(), String> {
    let def = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("工具 '{}' 未定义", id))?;

    let cli = def
        .cli
        .clone()
        .ok_or_else(|| format!("工具 '{}' 不支持打开 CLI", def.name))?;

    // 首先检测工具是否已安装
    let tool_path = resolve_tool_command_path(&def).ok_or_else(|| {
        let install_cmds = def
            .installers
            .iter()
            .map(|i| format!("  {} {}", i.manager, i.command))
            .collect::<Vec<_>>()
            .join("\n");
        let hint = if def.install_hint.is_empty() {
            def.homepage.clone()
        } else {
            def.install_hint.clone()
        };
        format!(
            "工具 '{}' 未安装。\n请通过以下方式安装：\n{}\n或访问: {}",
            def.name, install_cmds, hint
        )
    })?;

    let ctx = template_ctx(Some(tool_path.as_path()));
    let program = resolve_cli_program(&cli, tool_path.as_path(), &ctx);
    let args = expand_args(&cli.args, &ctx);

    launch_terminal(cwd, Some((program.as_str(), &args)), cli.background)
}

fn resolve_package_manager_path(def: &PackageManagerDef) -> Option<PathBuf> {
    let ctx = template_ctx(None);

    if let Some(ref tpl) = def.path_template {
        let p = PathBuf::from(user_config::expand_template(tpl, &ctx));
        if p.exists() {
            return Some(p);
        }
    }

    if def.name == "winget" {
        return detect_winget();
    }

    if def.commands.is_empty() {
        return None;
    }

    let cmds = def
        .commands
        .iter()
        .map(|c| user_config::expand_template(c, &ctx))
        .collect::<Vec<_>>();
    let borrowed = cmds.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    detect_command(&borrowed)
}

fn package_managers(defs: &[PackageManagerDef]) -> Vec<PackageManagerInfo> {
    std::thread::scope(|s| {
        let handles: Vec<_> = defs
            .iter()
            .map(|def| {
                s.spawn(move || {
                    let path = resolve_package_manager_path(def);
                    PackageManagerInfo {
                        name: def.name.clone(),
                        installed: path.is_some(),
                        command_path: path.map(|p| p.to_string_lossy().to_string()),
                        install_hint: def.install_hint.clone(),
                    }
                })
            })
            .collect();

        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect()
    })
}

fn build_tool_info(def: &ToolRuntimeDef) -> ToolInfo {
    let path = resolve_tool_command_path(def);
    let installed = path.is_some();

    let ctx = template_ctx(path.as_deref());
    let version_args = expand_args(&def.version_args, &ctx);
    let version_regex = def
        .version_regex
        .as_ref()
        .and_then(|r| expand_regex(r, &ctx, "version"));
    let version_program = resolve_version_program(def, path.as_deref(), &ctx);
    let version = version_program.as_ref().and_then(|p| {
        let borrowed = version_args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        read_version(p, &borrowed, version_regex.as_ref(), def.version_timeout_ms)
    });

    let config_path = resolve_config_path(def, path.as_deref());

    ToolInfo {
        id: def.id.clone(),
        name: def.name.clone(),
        category: def.category.clone(),
        installed,
        version,
        command_path: path.map(|p| p.to_string_lossy().to_string()),
        config_path,
        install_hint: def.install_hint.clone(),
        homepage: def.homepage.clone(),
        launcher: def
            .cli
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| c.label.clone()),
        install_commands: def.installers.clone(),
    }
}

/// 并行检测所有工具
fn list_parallel(defs: &[ToolRuntimeDef]) -> Vec<ToolInfo> {
    std::thread::scope(|s| {
        let handles: Vec<_> = defs
            .iter()
            .map(|def| s.spawn(move || build_tool_info(def)))
            .collect();

        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect()
    })
}

pub fn environment_report() -> EnvironmentReport {
    let start = Instant::now();
    logger::debug("tools", "[环境检测] 开始...");

    let cfg_token = tools_config_token();

    // 检查缓存
    {
        let cache = get_cache().lock().unwrap();
        if cache.is_valid(cfg_token) {
            if let Some(ref report) = cache.report {
                logger::debug(
                    "tools",
                    &format!("[环境检测] 使用缓存，耗时: {:?}", start.elapsed()),
                );
                return report.clone();
            }
        }
    }

    let catalog = load_runtime_catalog();

    // 并行获取工具和包管理器信息
    let (tools, package_managers) = std::thread::scope(|s| {
        let tools_defs = &catalog.tools;
        let pm_defs = &catalog.package_managers;

        let tools_handle = s.spawn(move || list_parallel(tools_defs));
        let pm_handle = s.spawn(move || package_managers(pm_defs));

        let tools = tools_handle.join().unwrap_or_default();
        let pms = pm_handle.join().unwrap_or_default();
        (tools, pms)
    });

    let mut ide_tools = Vec::new();
    let mut languages = Vec::new();
    let mut ai_tools = Vec::new();
    for t in tools {
        match t.category.as_str() {
            "ide" | "scm" => ide_tools.push(t),
            "language" => languages.push(t),
            _ => ai_tools.push(t),
        }
    }

    let report = EnvironmentReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        package_managers,
        ide_tools,
        languages,
        ai_tools,
    };

    // 更新缓存
    {
        let mut cache = get_cache().lock().unwrap();
        cache.report = Some(report.clone());
        cache.last_update = Some(Instant::now());
        cache.tools_config_token = cfg_token;
    }

    logger::debug(
        "tools",
        &format!("[环境检测] 完成，耗时: {:?}", start.elapsed()),
    );

    report
}

/// 清除环境缓存，强制下次重新检测
#[allow(dead_code)]
pub fn invalidate_cache() {
    {
        let mut cache = get_cache().lock().unwrap();
        cache.report = None;
        cache.last_update = None;
        cache.tools_config_token = 0;
    }
    {
        let mut cmd_cache = get_cmd_cache().lock().unwrap();
        cmd_cache.clear();
    }
    logger::debug("tools", "缓存已清除");
}

/// 打开工具的官方网站
pub fn open_homepage(id: &str) -> Result<(), String> {
    let tool = load_runtime_catalog()
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("工具 '{}' 未定义", id))?;

    if tool.homepage.trim().is_empty() {
        return Err("homepage_unknown".to_string());
    }

    open::that(tool.homepage).map_err(|e| format!("无法打开网站: {}", e))
}

/// 执行安装命令结果
#[derive(Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub output: Option<String>,
}

/// 使用包管理器执行安装命令
pub fn execute_install(id: &str, manager: &str) -> Result<InstallResult, String> {
    let catalog = load_runtime_catalog();
    let tool = catalog
        .tools
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("工具 '{}' 未定义", id))?;

    // 查找对应的安装命令
    let installer = tool
        .installers
        .iter()
        .find(|i| i.manager == manager)
        .ok_or_else(|| format!("未找到 {} 的 {} 安装命令", tool.name, manager))?;

    // 检查包管理器是否已安装
    let pm_path = catalog
        .package_managers
        .iter()
        .find(|pm| pm.name == manager)
        .and_then(resolve_package_manager_path)
        .or_else(|| match manager {
            "winget" => detect_winget(),
            _ => detect_command(&[manager]),
        })
        .ok_or_else(|| format!("包管理器 '{}' 未安装", manager))?;

    // 解析命令参数（这里保持简单 split_whitespace；如需复杂引号参数可改为数组配置）
    let args: Vec<&str> = installer.command.split_whitespace().collect();
    let full_command = format!("{} {}", manager, installer.command);

    // 记录安装开始日志
    logger::info("tools", &format!("开始安装 {} (使用 {})", tool.name, manager));
    let install_log_id = logger::create_install_log(id, &tool.name, manager, &full_command);

    // 执行安装命令
    let mut cmd = Command::new(&pm_path);
    cmd.args(&args);

    // 在 Windows 上隐藏 CMD 窗口
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|e| {
        let err_msg = format!("执行安装命令失败: {}", e);
        logger::error("tools", &err_msg);
        logger::complete_install_log(install_log_id, logger::InstallStatus::Failed);
        err_msg
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined_output = if stderr.is_empty() {
        stdout
    } else if stdout.is_empty() {
        stderr
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    // 追加输出到安装日志
    logger::append_install_output(install_log_id, &combined_output);

    if output.status.success() {
        logger::info("tools", &format!("{} 安装成功", tool.name));
        logger::complete_install_log(install_log_id, logger::InstallStatus::Success);
        Ok(InstallResult {
            success: true,
            message: format!("{} 安装成功", tool.name),
            output: Some(combined_output),
        })
    } else {
        logger::error("tools", &format!("{} 安装失败", tool.name));
        logger::complete_install_log(install_log_id, logger::InstallStatus::Failed);
        Ok(InstallResult {
            success: false,
            message: format!("{} 安装失败", tool.name),
            output: Some(combined_output),
        })
    }
}
