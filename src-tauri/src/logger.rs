//! Global Logger Module
//!
//! 提供统一的日志记录接口，支持日志持久化到 SQLite 数据库。
//! 使用异步批量写入优化性能。

use dirs::data_dir;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Once, RwLock};
use std::time::{Duration, Instant};

static INIT: Once = Once::new();

// Log message for batching
#[derive(Debug, Clone)]
struct LogMessage {
    timestamp: i64,
    level: String,
    source: String,
    message: String,
    metadata: Option<String>,
}

// Async log channel sender
static LOG_SENDER: RwLock<Option<mpsc::Sender<LogMessage>>> = RwLock::new(None);

// ============================================
// Log Level & Entry Types
// ============================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub level: Option<LogLevel>,
    pub source: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteLogsRequest {
    pub ids: Option<Vec<i64>>,
    pub level: Option<LogLevel>,
    pub source: Option<String>,
    pub before_time: Option<i64>,
}

// ============================================
// Install Log Types
// ============================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InstallStatus {
    Running,
    Success,
    Failed,
}

impl InstallStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallStatus::Running => "running",
            InstallStatus::Success => "success",
            InstallStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "running" => Some(InstallStatus::Running),
            "success" => Some(InstallStatus::Success),
            "failed" => Some(InstallStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallLog {
    pub id: i64,
    pub tool_id: String,
    pub tool_name: String,
    pub package_manager: String,
    pub command: String,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub status: InstallStatus,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallOutputEvent {
    pub event_type: String,
    pub data: String,
    pub timestamp: i64,
}

// ============================================
// Database Functions
// ============================================

fn db_path() -> PathBuf {
    let mut p = data_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("CCR");
    std::fs::create_dir_all(&p).ok();
    p.push("ccr.db");
    p
}

fn open_conn() -> Connection {
    Connection::open(db_path()).unwrap()
}

/// 初始化日志系统，创建必要的数据库表
pub fn init() {
    INIT.call_once(|| {
        let conn = open_conn();

        // 全局日志表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS global_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL,
                source TEXT NOT NULL,
                message TEXT NOT NULL,
                metadata TEXT
            )",
            [],
        ).unwrap();

        // 创建索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_global_logs_timestamp ON global_logs(timestamp DESC)",
            [],
        ).ok();
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_global_logs_level ON global_logs(level)",
            [],
        ).ok();
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_global_logs_source ON global_logs(source)",
            [],
        ).ok();

        // 安装日志表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS install_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                package_manager TEXT NOT NULL,
                command TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER,
                status TEXT NOT NULL DEFAULT 'running',
                output TEXT NOT NULL DEFAULT ''
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_install_logs_tool_id ON install_logs(tool_id)",
            [],
        ).ok();
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_install_logs_start_time ON install_logs(start_time DESC)",
            [],
        ).ok();

        // Spawn async batch writer
        spawn_batch_writer();
    });
}

/// 异步批量写入日志任务
fn spawn_batch_writer() {
    let (tx, rx) = mpsc::channel::<LogMessage>();

    // Store sender globally
    {
        let mut sender = LOG_SENDER.write().unwrap();
        *sender = Some(tx);
    }

    std::thread::spawn(move || {
        let mut buffer = Vec::with_capacity(100);
        let mut last_flush = Instant::now();
        let flush_interval = Duration::from_secs(1);

        loop {
            let timeout = flush_interval
                .checked_sub(last_flush.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            match rx.recv_timeout(timeout) {
                Ok(msg) => {
                    buffer.push(msg);
                    if buffer.len() >= 100 || last_flush.elapsed() >= flush_interval {
                        flush_logs(&mut buffer);
                        last_flush = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !buffer.is_empty() {
                        flush_logs(&mut buffer);
                    }
                    last_flush = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        if !buffer.is_empty() {
            flush_logs(&mut buffer);
        }
    });
}

/// 批量写入日志到数据库
fn flush_logs(buffer: &mut Vec<LogMessage>) {
    if buffer.is_empty() {
        return;
    }

    let mut conn = open_conn();
    let tx = conn.transaction().unwrap();

    for msg in buffer.drain(..) {
        let _ = tx.execute(
            "INSERT INTO global_logs (timestamp, level, source, message, metadata) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![msg.timestamp, msg.level, msg.source, msg.message, msg.metadata],
        );
    }

    let _ = tx.commit();
}

// ============================================
// Global Log Functions
// ============================================

/// 记录日志（内部函数）
fn log_internal(level: LogLevel, source: &str, message: &str, metadata: Option<&str>) {
    let timestamp = chrono::Utc::now().timestamp();
    let msg = LogMessage {
        timestamp,
        level: level.as_str().to_string(),
        source: source.to_string(),
        message: message.to_string(),
        metadata: metadata.map(|s| s.to_string()),
    };

    // Try to send to async channel
    if let Some(sender) = LOG_SENDER.read().unwrap().as_ref() {
        let _ = sender.send(msg);
    } else {
        // Fallback to direct write if channel not initialized
        let conn = open_conn();
        let _ = conn.execute(
            "INSERT INTO global_logs (timestamp, level, source, message, metadata) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp, level.as_str(), source, message, metadata],
        );
    }
}

/// 记录 DEBUG 级别日志
pub fn debug(source: &str, message: &str) {
    log_internal(LogLevel::Debug, source, message, None);
}

/// 记录 INFO 级别日志
pub fn info(source: &str, message: &str) {
    log_internal(LogLevel::Info, source, message, None);
}

/// 记录 WARN 级别日志
pub fn warn(source: &str, message: &str) {
    log_internal(LogLevel::Warn, source, message, None);
}

/// 记录 ERROR 级别日志
pub fn error(source: &str, message: &str) {
    log_internal(LogLevel::Error, source, message, None);
}

/// 记录带元数据的日志
pub fn log_with_metadata(level: LogLevel, source: &str, message: &str, metadata: Option<&str>) {
    log_internal(level, source, message, metadata);
}

/// 查询日志
pub fn query_logs(query: &LogQuery) -> Vec<LogEntry> {
    let conn = open_conn();
    let mut sql = String::from(
        "SELECT id, timestamp, level, source, message, metadata FROM global_logs WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref level) = query.level {
        sql.push_str(" AND level = ?");
        params_vec.push(Box::new(level.as_str().to_string()));
    }
    if let Some(ref source) = query.source {
        sql.push_str(" AND source = ?");
        params_vec.push(Box::new(source.clone()));
    }
    if let Some(start_time) = query.start_time {
        sql.push_str(" AND timestamp >= ?");
        params_vec.push(Box::new(start_time));
    }
    if let Some(end_time) = query.end_time {
        sql.push_str(" AND timestamp <= ?");
        params_vec.push(Box::new(end_time));
    }

    sql.push_str(" ORDER BY timestamp DESC");

    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            let level_str: String = row.get(2)?;
            Ok(LogEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                level: LogLevel::from_str(&level_str).unwrap_or(LogLevel::Info),
                source: row.get(3)?,
                message: row.get(4)?,
                metadata: row.get(5)?,
            })
        })
        .unwrap();

    rows.filter_map(|r| r.ok()).collect()
}

/// 获取日志总数
pub fn logs_count(query: &LogQuery) -> i64 {
    let conn = open_conn();
    let mut sql = String::from("SELECT COUNT(*) FROM global_logs WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref level) = query.level {
        sql.push_str(" AND level = ?");
        params_vec.push(Box::new(level.as_str().to_string()));
    }
    if let Some(ref source) = query.source {
        sql.push_str(" AND source = ?");
        params_vec.push(Box::new(source.clone()));
    }
    if let Some(start_time) = query.start_time {
        sql.push_str(" AND timestamp >= ?");
        params_vec.push(Box::new(start_time));
    }
    if let Some(end_time) = query.end_time {
        sql.push_str(" AND timestamp <= ?");
        params_vec.push(Box::new(end_time));
    }

    let mut stmt = conn.prepare(&sql).unwrap();
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    stmt.query_row(params_refs.as_slice(), |row| row.get(0))
        .unwrap_or(0)
}

/// 删除单条日志
pub fn delete_log(id: i64) -> Result<(), String> {
    let conn = open_conn();
    let affected = conn
        .execute("DELETE FROM global_logs WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    if affected == 0 {
        Err("Log entry not found".to_string())
    } else {
        Ok(())
    }
}

/// 批量删除日志
pub fn delete_logs(request: &DeleteLogsRequest) -> Result<i64, String> {
    let conn = open_conn();
    let mut sql = String::from("DELETE FROM global_logs WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref ids) = request.ids {
        if !ids.is_empty() {
            let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
            sql.push_str(&format!(" AND id IN ({})", placeholders.join(",")));
            for id in ids {
                params_vec.push(Box::new(*id));
            }
        }
    }
    if let Some(ref level) = request.level {
        sql.push_str(" AND level = ?");
        params_vec.push(Box::new(level.as_str().to_string()));
    }
    if let Some(ref source) = request.source {
        sql.push_str(" AND source = ?");
        params_vec.push(Box::new(source.clone()));
    }
    if let Some(before_time) = request.before_time {
        sql.push_str(" AND timestamp < ?");
        params_vec.push(Box::new(before_time));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let affected = conn
        .execute(&sql, params_refs.as_slice())
        .map_err(|e| e.to_string())?;
    Ok(affected as i64)
}

/// 清空所有日志
pub fn clear_all_logs() -> Result<i64, String> {
    let conn = open_conn();
    let affected = conn
        .execute("DELETE FROM global_logs", [])
        .map_err(|e| e.to_string())?;
    Ok(affected as i64)
}

pub fn clear_install_logs() -> Result<i64, String> {
    let conn = open_conn();
    let affected = conn
        .execute("DELETE FROM install_logs", [])
        .map_err(|e| e.to_string())?;
    Ok(affected as i64)
}

// ============================================
// Install Log Functions
// ============================================

/// 创建安装日志记录
pub fn create_install_log(
    tool_id: &str,
    tool_name: &str,
    package_manager: &str,
    command: &str,
) -> i64 {
    let conn = open_conn();
    let start_time = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO install_logs (tool_id, tool_name, package_manager, command, start_time, status, output) 
         VALUES (?1, ?2, ?3, ?4, ?5, 'running', '')",
        params![tool_id, tool_name, package_manager, command, start_time],
    ).unwrap();
    conn.last_insert_rowid()
}

/// 追加安装输出
pub fn append_install_output(log_id: i64, output: &str) {
    let conn = open_conn();
    conn.execute(
        "UPDATE install_logs SET output = output || ?1 WHERE id = ?2",
        params![output, log_id],
    )
    .ok();
}

/// 完成安装日志
pub fn complete_install_log(log_id: i64, status: InstallStatus) {
    let conn = open_conn();
    let end_time = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE install_logs SET end_time = ?1, status = ?2 WHERE id = ?3",
        params![end_time, status.as_str(), log_id],
    )
    .ok();
}

/// 获取单条安装日志
pub fn get_install_log(id: i64) -> Option<InstallLog> {
    let conn = open_conn();
    let mut stmt = conn.prepare(
        "SELECT id, tool_id, tool_name, package_manager, command, start_time, end_time, status, output 
         FROM install_logs WHERE id = ?"
    ).ok()?;

    stmt.query_row(params![id], |row| {
        let status_str: String = row.get(7)?;
        Ok(InstallLog {
            id: row.get(0)?,
            tool_id: row.get(1)?,
            tool_name: row.get(2)?,
            package_manager: row.get(3)?,
            command: row.get(4)?,
            start_time: row.get(5)?,
            end_time: row.get(6)?,
            status: InstallStatus::from_str(&status_str).unwrap_or(InstallStatus::Running),
            output: row.get(8)?,
        })
    })
    .ok()
}

/// 获取安装日志列表
pub fn list_install_logs(limit: Option<i64>, offset: Option<i64>) -> Vec<InstallLog> {
    let conn = open_conn();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT id, tool_id, tool_name, package_manager, command, start_time, end_time, status, output 
         FROM install_logs ORDER BY start_time DESC LIMIT ?1 OFFSET ?2"
    ).unwrap();

    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let status_str: String = row.get(7)?;
            Ok(InstallLog {
                id: row.get(0)?,
                tool_id: row.get(1)?,
                tool_name: row.get(2)?,
                package_manager: row.get(3)?,
                command: row.get(4)?,
                start_time: row.get(5)?,
                end_time: row.get(6)?,
                status: InstallStatus::from_str(&status_str).unwrap_or(InstallStatus::Running),
                output: row.get(8)?,
            })
        })
        .unwrap();

    rows.filter_map(|r| r.ok()).collect()
}

/// 获取安装日志总数
pub fn install_logs_count() -> i64 {
    let conn = open_conn();
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM install_logs").unwrap();
    stmt.query_row([], |row| row.get(0)).unwrap_or(0)
}
