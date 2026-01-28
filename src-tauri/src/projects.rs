use dirs::data_dir;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Command};

use crate::tools;

fn db_path() -> PathBuf {
    let mut p = data_dir().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&p).ok();
    p.push("CCR");
    std::fs::create_dir_all(&p).ok();
    p.push("ccr.db");
    p
}

fn open_conn() -> Connection {
    Connection::open(db_path()).unwrap()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

fn serialize_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_tags(raw: String) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn list() -> Vec<Project> {
    let conn = open_conn();
    let mut stmt = conn.prepare("select id,name,path,description,ifnull(tags,''),ifnull(created_at,0) from projects order by id desc").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                description: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                tags: parse_tags(r.get(4)?),
                created_at: r.get(5)?,
            })
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

pub fn get(id: i64) -> Option<Project> {
    let conn = open_conn();
    let mut stmt = conn.prepare("select id,name,path,description,ifnull(tags,''),ifnull(created_at,0) from projects where id=?").ok()?;
    stmt.query_row(params![id], |r| {
        Ok(Project {
            id: r.get(0)?,
            name: r.get(1)?,
            path: r.get(2)?,
            description: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            tags: parse_tags(r.get(4)?),
            created_at: r.get(5)?,
        })
    })
    .ok()
}

pub fn create(input: ProjectInput) -> Option<Project> {
    let conn = open_conn();
    let ts = chrono::Utc::now().timestamp();
    let ProjectInput {
        name,
        path,
        description,
        tags,
    } = input;
    let tags_vec = tags.unwrap_or_default();
    let tags = serialize_tags(&tags_vec);
    let desc = description.unwrap_or_default();
    conn.execute(
        "insert into projects(name,path,description,tags,created_at) values(?,?,?,?,?)",
        params![name, path, desc, tags, ts],
    )
    .ok()?;
    let id = conn.last_insert_rowid();
    get(id)
}

pub fn update(id: i64, input: ProjectInput) -> Option<Project> {
    let conn = open_conn();
    let ProjectInput {
        name,
        path,
        description,
        tags,
    } = input;
    let tags_vec = tags.unwrap_or_default();
    let tags = serialize_tags(&tags_vec);
    let desc = description.unwrap_or_default();
    conn.execute(
        "update projects set name=?, path=?, description=?, tags=? where id=?",
        params![name, path, desc, tags, id],
    )
    .ok()?;
    get(id)
}

pub fn remove(id: i64) -> bool {
    let conn = open_conn();
    conn.execute("delete from projects where id=?", params![id])
        .map(|n| n > 0)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
pub enum ProjectOpenTarget {
    Folder,
    Terminal,
    Vscode,
    Cursor,
    Windsurf,
    Zed,
    Sublime,
    Webstorm,
    Idea,
    Claude,
    Gemini,
    Codex,
}

impl ProjectOpenTarget {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "terminal" => Self::Terminal,
            "vscode" => Self::Vscode,
            "cursor" => Self::Cursor,
            "windsurf" => Self::Windsurf,
            "zed" => Self::Zed,
            "sublime" | "subl" => Self::Sublime,
            "webstorm" => Self::Webstorm,
            "idea" => Self::Idea,
            "claude" => Self::Claude,
            "gemini" => Self::Gemini,
            "codex" => Self::Codex,
            _ => Self::Folder,
        }
    }
}

fn open_folder(path: &str) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    } else {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

fn open_editor(cmd: &str, path: &str) -> Result<(), String> {
    Command::new(cmd)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开编辑器 {}: {}", cmd, e))
}

pub fn open_project(id: i64, target: ProjectOpenTarget) -> Result<(), String> {
    let proj = get(id).ok_or_else(|| "project_not_found".to_string())?;
    let path = proj.path;
    match target {
        ProjectOpenTarget::Folder => open_folder(&path),
        ProjectOpenTarget::Terminal => tools::open_terminal_in_dir(&path),
        ProjectOpenTarget::Vscode => open_editor("code", &path),
        ProjectOpenTarget::Cursor => open_editor("cursor", &path),
        ProjectOpenTarget::Windsurf => open_editor("windsurf", &path),
        ProjectOpenTarget::Zed => open_editor("zed", &path),
        ProjectOpenTarget::Sublime => open_editor("subl", &path),
        ProjectOpenTarget::Webstorm => open_editor("webstorm", &path),
        ProjectOpenTarget::Idea => open_editor("idea", &path),
        ProjectOpenTarget::Claude => tools::launch_cli_in_dir("claude-code", Some(&path)),
        ProjectOpenTarget::Gemini => tools::launch_cli_in_dir("gemini-cli", Some(&path)),
        ProjectOpenTarget::Codex => tools::launch_cli_in_dir("codex", Some(&path)),
    }
}

/// 检测项目目录中的配置文件类型
pub fn detect_project_types(id: i64) -> Vec<String> {
    let proj = match get(id) {
        Some(p) => p,
        None => return vec![],
    };

    let path = std::path::Path::new(&proj.path);
    if !path.exists() || !path.is_dir() {
        return vec![];
    }

    // 常见的项目配置文件
    let config_files = [
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pyproject.toml",
        "requirements.txt",
        "pom.xml",
        "build.gradle",
        "composer.json",
        "Gemfile",
        "CMakeLists.txt",
        "Makefile",
        "pubspec.yaml",
        "deno.json",
        "tsconfig.json",
    ];

    let mut found = Vec::new();
    for file in &config_files {
        if path.join(file).exists() {
            found.push(file.to_string());
        }
    }

    // 检查 .csproj 文件 (可能有不同名称)
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".csproj") {
                    found.push(".csproj".to_string());
                    break;
                }
            }
        }
    }

    found
}

/// 检测系统中安装的编辑器
pub fn detect_available_editors() -> Vec<String> {
    let editors = [
        ("vscode", "code"),
        ("cursor", "cursor"),
        ("windsurf", "windsurf"),
        ("zed", "zed"),
        ("sublime", "subl"),
        ("webstorm", "webstorm"),
        ("idea", "idea"),
    ];

    let mut available = Vec::new();
    for (name, cmd) in &editors {
        if which::which(cmd).is_ok() {
            available.push(name.to_string());
        }
    }

    // 如果没有检测到任何编辑器，默认返回 vscode
    if available.is_empty() {
        available.push("vscode".to_string());
    }

    available
}
