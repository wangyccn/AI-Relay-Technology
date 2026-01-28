use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

use crate::{autoconfig, config, db, forward, logger, projects, tools};

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

#[derive(Deserialize)]
struct SummaryQ {
    range: Option<String>,
}

async fn stats_summary(Query(q): Query<SummaryQ>) -> Json<Value> {
    let range = q.range.unwrap_or_else(|| "daily".to_string());
    let (reqs, tokens, price) = db::summary_for_range(&range);
    Json(json!({
        "range": range,
        "requests": reqs,
        "tokens": tokens,
        "price_usd": price
    }))
}

#[derive(Deserialize)]
struct SeriesQ {
    metric: Option<String>,
    days: Option<i64>,
}

async fn stats_series(Query(q): Query<SeriesQ>) -> Json<Value> {
    let days = q.days.unwrap_or(30);
    if q.metric.as_deref() == Some("price") {
        let s = db::series_price(days);
        return Json(
            json!({"days": s.iter().map(|(d,_)| d).cloned().collect::<Vec<_>>(), "price": s.iter().map(|(_,v)| v).cloned().collect::<Vec<_>>() }),
        );
    }
    let s = db::series_tokens(days);
    Json(
        json!({"days": s.iter().map(|(d,_)| d).cloned().collect::<Vec<_>>(), "tokens": s.iter().map(|(_,v)| v).cloned().collect::<Vec<_>>() }),
    )
}

async fn stats_channels() -> Json<Value> {
    let s = db::channels_breakdown();
    Json(json!({"channels": s}))
}

#[derive(Deserialize)]
struct ModelsQ {
    range: Option<String>,
}

async fn stats_models(Query(q): Query<ModelsQ>) -> Json<Value> {
    let days = match q.range.as_deref() {
        Some("weekly") => 7,
        Some("monthly") => 30,
        _ => 1,
    };
    let s = db::models_cost_since(days);
    Json(json!({"models": s}))
}

#[derive(Deserialize)]
struct LogsQ {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn stats_logs(Query(q): Query<LogsQ>) -> Json<Value> {
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    let logs = db::recent_logs(limit, offset);
    let total = db::logs_count();
    Json(json!({
        "logs": logs,
        "total": total,
        "limit": limit,
        "offset": offset
    }))
}

async fn list_projects() -> Json<Vec<projects::Project>> {
    Json(projects::list())
}

async fn create_project(Json(input): Json<projects::ProjectInput>) -> impl IntoResponse {
    if let Some(project) = projects::create(input) {
        (StatusCode::CREATED, Json(project)).into_response()
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}

async fn update_project(
    Path(id): Path<i64>,
    Json(input): Json<projects::ProjectInput>,
) -> impl IntoResponse {
    if let Some(project) = projects::update(id, input) {
        Json(project).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn delete_project(Path(id): Path<i64>) -> impl IntoResponse {
    if projects::remove(id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

#[derive(Deserialize)]
struct ProjectOpenQ {
    #[serde(rename = "where")]
    r#where: Option<String>,
}

async fn open_project(Path(id): Path<i64>, Query(q): Query<ProjectOpenQ>) -> impl IntoResponse {
    let target = q.r#where.as_deref().unwrap_or("folder");
    let action = projects::ProjectOpenTarget::from_str(target);
    match projects::open_project(id, action) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn detect_project_type(Path(id): Path<i64>) -> Json<Value> {
    let types = projects::detect_project_types(id);
    Json(json!({"types": types}))
}

async fn list_editors() -> Json<Value> {
    let editors = projects::detect_available_editors();
    Json(json!({"editors": editors}))
}

async fn list_tools() -> Json<Vec<tools::ToolInfo>> {
    Json(tools::list())
}

#[derive(Deserialize)]
struct ToolInstallRequest {
    id: String,
}

async fn install_tool(Json(req): Json<ToolInstallRequest>) -> impl IntoResponse {
    if let Some(plan) = tools::install_plan(&req.id) {
        Json(plan).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Deserialize)]
struct ExecuteInstallRequest {
    id: String,
    manager: String,
}

async fn execute_install(Json(req): Json<ExecuteInstallRequest>) -> impl IntoResponse {
    match tools::execute_install(&req.id, &req.manager) {
        Ok(result) => Json(result).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn open_tool_homepage(Path(id): Path<String>) -> impl IntoResponse {
    match tools::open_homepage(&id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn open_tool_config(Path(id): Path<String>) -> impl IntoResponse {
    match tools::open_config(&id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn open_tool_config_path(Path(id): Path<String>) -> impl IntoResponse {
    match tools::open_config_folder(&id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn open_tool_cli(Path(id): Path<String>) -> impl IntoResponse {
    match tools::launch_cli(&id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn environment_report() -> Json<tools::EnvironmentReport> {
    Json(tools::environment_report())
}

async fn get_config() -> Json<config::Settings> {
    Json(config::load())
}

async fn put_config(Json(body): Json<config::Settings>) -> impl IntoResponse {
    eprintln!(
        "Received config update with {} models, {} upstreams",
        body.models.len(),
        body.upstreams.len()
    );
    match config::save(&body) {
        Ok(_) => {
            eprintln!("Config saved successfully via API");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            eprintln!("Config save failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

async fn list_providers() -> Json<Value> {
    forward::list_api_styles().await
}

async fn upstream_latency(Path(id): Path<String>) -> Json<Value> {
    forward::upstream_latency(Path(id)).await
}

async fn test_latency_urls(Json(urls): Json<Vec<String>>) -> Json<Value> {
    forward::test_latency_urls(Json(urls)).await
}

async fn get_forward_token() -> Json<Value> {
    forward::get_forward_token().await
}

async fn refresh_forward_token() -> Json<Value> {
    forward::refresh_forward_token().await
}

async fn export_backup() -> Json<Value> {
    let cfg = config::load();
    let projects = projects::list();
    let tools = tools::list();
    let daily = db::summary_for_range("daily");
    let weekly = db::summary_for_range("weekly");
    let monthly = db::summary_for_range("monthly");
    Json(json!({
        "settings": cfg,
        "projects": projects,
        "tools": tools,
        "usage": {
            "daily": { "requests": daily.0, "tokens": daily.1, "price_usd": daily.2 },
            "weekly": { "requests": weekly.0, "tokens": weekly.1, "price_usd": weekly.2 },
            "monthly": { "requests": monthly.0, "tokens": monthly.1, "price_usd": monthly.2 },
        }
    }))
}

async fn clear_all_data() -> impl IntoResponse {
    match clear_all_data_inner() {
        Ok(payload) => Json(payload).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err})),
        )
            .into_response(),
    }
}

fn clear_all_data_inner() -> Result<Value, String> {
    db::clear_all_data()?;
    let logs_deleted = logger::clear_all_logs()?;
    let install_logs_deleted = logger::clear_install_logs()?;
    autoconfig::clear_all_backups()?;
    config::reset()?;
    Ok(json!({
        "ok": true,
        "logs_deleted": logs_deleted,
        "install_logs_deleted": install_logs_deleted
    }))
}

// Auto config handlers
async fn get_auto_config_status() -> Json<autoconfig::AutoConfigStatus> {
    Json(autoconfig::get_status())
}

async fn configure_auto_config(
    Json(req): Json<autoconfig::AutoConfigRequest>,
) -> impl IntoResponse {
    match autoconfig::configure(&req) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

// Backup handlers
async fn list_tool_backups(Path(tool): Path<String>) -> Json<autoconfig::ToolConfigBackupList> {
    Json(autoconfig::list_backups(&tool))
}

async fn create_tool_backup(Json(req): Json<autoconfig::CreateBackupRequest>) -> impl IntoResponse {
    match autoconfig::create_backup(&req) {
        Ok(backup) => (StatusCode::CREATED, Json(backup)).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn restore_tool_backup(Path(backup_id): Path<String>) -> impl IntoResponse {
    match autoconfig::restore_backup(&backup_id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn delete_tool_backup(Path(backup_id): Path<String>) -> impl IntoResponse {
    match autoconfig::delete_backup(&backup_id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

// ============================================
// Global Logs API Handlers
// ============================================

#[derive(Deserialize)]
struct GlobalLogsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    level: Option<String>,
    source: Option<String>,
    start_time: Option<i64>,
    end_time: Option<i64>,
}

async fn get_global_logs(Query(q): Query<GlobalLogsQuery>) -> Json<Value> {
    let query = logger::LogQuery {
        limit: q.limit,
        offset: q.offset,
        level: q.level.as_ref().and_then(|l| logger::LogLevel::from_str(l)),
        source: q.source,
        start_time: q.start_time,
        end_time: q.end_time,
    };
    let logs = logger::query_logs(&query);
    let total = logger::logs_count(&query);
    Json(json!({
        "logs": logs,
        "total": total,
        "limit": q.limit.unwrap_or(100),
        "offset": q.offset.unwrap_or(0)
    }))
}

async fn get_global_logs_count(Query(q): Query<GlobalLogsQuery>) -> Json<Value> {
    let query = logger::LogQuery {
        limit: None,
        offset: None,
        level: q.level.as_ref().and_then(|l| logger::LogLevel::from_str(l)),
        source: q.source,
        start_time: q.start_time,
        end_time: q.end_time,
    };
    let count = logger::logs_count(&query);
    Json(json!({ "count": count }))
}

async fn delete_global_log(Path(id): Path<i64>) -> impl IntoResponse {
    match logger::delete_log(id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::NOT_FOUND, Json(json!({"error": err}))).into_response(),
    }
}

async fn delete_global_logs_batch(Json(req): Json<logger::DeleteLogsRequest>) -> impl IntoResponse {
    match logger::delete_logs(&req) {
        Ok(count) => Json(json!({ "deleted": count })).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    }
}

async fn clear_global_logs() -> impl IntoResponse {
    match logger::clear_all_logs() {
        Ok(count) => Json(json!({ "deleted": count })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err})),
        )
            .into_response(),
    }
}

// ============================================
// Install Logs API Handlers
// ============================================

#[derive(Deserialize)]
struct InstallLogsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn get_install_logs(Query(q): Query<InstallLogsQuery>) -> Json<Value> {
    let logs = logger::list_install_logs(q.limit, q.offset);
    let total = logger::install_logs_count();
    Json(json!({
        "logs": logs,
        "total": total,
        "limit": q.limit.unwrap_or(50),
        "offset": q.offset.unwrap_or(0)
    }))
}

async fn get_install_log(Path(id): Path<i64>) -> impl IntoResponse {
    match logger::get_install_log(id) {
        Some(log) => Json(json!(log)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Install log not found"})),
        )
            .into_response(),
    }
}

pub fn app() -> Router {
    let cors = CorsLayer::permissive();
    Router::new()
        // Health check
        .route("/health", get(health))
        // ============================================
        // Unified API Endpoints (main entry points for editors)
        // ============================================
        // OpenAI-compatible unified endpoint (auto-routes based on model)
        .route(
            "/v1/chat/completions",
            post(forward::unified_chat_completions),
        )
        // Model listing (OpenAI-compatible)
        .route("/v1/models", get(forward::list_models))
        .route("/v1/models/:model_id", get(forward::get_model))
        // API health check
        .route("/v1/health", get(forward::api_health))
        // ============================================
        // Provider-Specific Endpoints
        // ============================================
        // OpenAI-style
        .route("/openai/v1/chat/completions", post(forward::openai_chat))
        .route("/openai/v1/models", get(forward::list_models))
        // Anthropic-style
        .route("/anthropic/v1/messages", post(forward::anthropic_messages))
        // Gemini-style (wildcard for all endpoints)
        .route("/gemini/v1beta/*endpoint", post(forward::gemini_generate))
        .route("/gemini/v1/*endpoint", post(forward::gemini_generate_v1))
        // ============================================
        // Stats & Analytics API
        // ============================================
        .route("/api/stats/summary", get(stats_summary))
        .route("/api/stats/series", get(stats_series))
        .route("/api/stats/channels", get(stats_channels))
        .route("/api/stats/models", get(stats_models))
        .route("/api/stats/logs", get(stats_logs))
        // ============================================
        // Projects API
        // ============================================
        .route("/api/projects", get(list_projects).post(create_project))
        .route(
            "/api/projects/:id",
            put(update_project).delete(delete_project),
        )
        .route("/api/projects/:id/open", post(open_project))
        .route("/api/projects/:id/detect-type", get(detect_project_type))
        .route("/api/editors", get(list_editors))
        // ============================================
        // Tools API
        // ============================================
        .route("/api/tools", get(list_tools))
        .route("/api/tools/install", post(install_tool))
        .route("/api/tools/execute-install", post(execute_install))
        .route("/api/tools/:id/open-homepage", post(open_tool_homepage))
        .route("/api/tools/:id/open-config", post(open_tool_config))
        .route(
            "/api/tools/:id/open-config-path",
            post(open_tool_config_path),
        )
        .route("/api/tools/:id/open-cli", post(open_tool_cli))
        .route("/api/environment", get(environment_report))
        // ============================================
        // Config API
        // ============================================
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/providers", get(list_providers))
        .route("/api/upstreams/:id/latency", get(upstream_latency))
        .route("/api/latency/test", post(test_latency_urls))
        .route(
            "/api/forward/token",
            get(get_forward_token).post(refresh_forward_token),
        )
        .route("/api/export/backup", get(export_backup))
        .route("/api/data/clear", post(clear_all_data))
        // ============================================
        // Auto Config & Backup API
        // ============================================
        .route("/api/auto-config/status", get(get_auto_config_status))
        .route("/api/auto-config/configure", post(configure_auto_config))
        .route("/api/auto-config/backups/:tool", get(list_tool_backups))
        .route("/api/auto-config/backup", post(create_tool_backup))
        .route(
            "/api/auto-config/backup/:backup_id/restore",
            post(restore_tool_backup),
        )
        .route(
            "/api/auto-config/backup/:backup_id",
            axum::routing::delete(delete_tool_backup),
        )
        // ============================================
        // Global Logs API
        // ============================================
        .route("/api/logs", get(get_global_logs).delete(clear_global_logs))
        .route("/api/logs/count", get(get_global_logs_count))
        .route("/api/logs/:id", axum::routing::delete(delete_global_log))
        .route("/api/logs/delete", post(delete_global_logs_batch))
        // ============================================
        // Install Logs API
        // ============================================
        .route("/api/install-logs", get(get_install_logs))
        .route("/api/install-logs/:id", get(get_install_log))
        .layer(cors)
}

pub async fn serve() {
    db::init();
    let app = app();
    let addr: SocketAddr = "127.0.0.1:8787".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

pub fn spawn() {
    tauri::async_runtime::spawn(async move { serve().await });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn health_ok() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let a = app();
        let h = tokio::spawn(async move { axum::serve(listener, a).await.unwrap() });
        let url = format!("http://{}", addr);
        let r = reqwest::get(format!("{}/health", url)).await.unwrap();
        let s = r.json::<serde_json::Value>().await.unwrap();
        assert_eq!(s["status"], "ok");
        drop(h);
    }
}
