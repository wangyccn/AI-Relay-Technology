use chrono::Datelike;
use dirs::data_dir;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, serde::Serialize, Clone)]
pub struct ChannelStats {
    pub channel: String,
    pub tokens: i64,
    pub price_usd: f64,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ModelStats {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub price_usd: f64,
}

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

fn optimize_connection(conn: &Connection) {
    conn.pragma_update(None, "journal_mode", &"WAL").ok();
    conn.pragma_update(None, "synchronous", &"NORMAL").ok();
    conn.pragma_update(None, "cache_size", &"-64000").ok();
    conn.pragma_update(None, "temp_store", &"MEMORY").ok();
    conn.pragma_update(None, "mmap_size", &"30000000000").ok();
}

pub fn init() {
    let conn = open_conn();
    optimize_connection(&conn);
    conn.execute("create table if not exists usage_logs (id integer primary key autoincrement, timestamp integer, channel text, tool text, model text, prompt_tokens integer, completion_tokens integer, total_tokens integer, price_usd real, upstream_id text)", []).unwrap();
    conn.execute("create table if not exists projects (id integer primary key autoincrement, name text, path text, description text, tags text, created_at integer)", []).unwrap();
    conn.execute("create table if not exists tools (id integer primary key autoincrement, name text, version text, installed integer, config_path text)", []).unwrap();
    conn.execute("create table if not exists models (id text primary key, display_name text, provider text, upstream_id text, price_prompt_per_1k real, price_completion_per_1k real)", []).unwrap();
    conn.execute("create table if not exists usage_daily (bucket text primary key, requests integer, tokens integer, price_usd real)", []).ok();
    conn.execute("create table if not exists usage_weekly (bucket text primary key, requests integer, tokens integer, price_usd real)", []).ok();
    conn.execute("create table if not exists usage_monthly (bucket text primary key, requests integer, tokens integer, price_usd real)", []).ok();

    conn.execute("create index if not exists idx_usage_logs_timestamp on usage_logs(timestamp desc)", []).ok();
    conn.execute("create index if not exists idx_usage_logs_channel_timestamp on usage_logs(channel, timestamp desc)", []).ok();
    conn.execute("create index if not exists idx_usage_logs_model_timestamp on usage_logs(model, timestamp desc)", []).ok();
    conn.execute("create index if not exists idx_usage_logs_summary on usage_logs(date(timestamp, 'unixepoch'), total_tokens, price_usd)", []).ok();
}

pub fn summary_daily() -> (i64, i64, f64) {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select count(*), ifnull(sum(total_tokens),0), ifnull(sum(price_usd),0) from usage_logs where date(timestamp,'unixepoch')=date('now')").unwrap();
    stmt.query_row([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
}

pub fn summary_since(days: i64) -> (i64, i64, f64) {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select count(*), ifnull(sum(total_tokens),0), ifnull(sum(price_usd),0) from usage_logs where timestamp>= strftime('%s','now','-'||?1||' day')").unwrap();
    stmt.query_row(params![days], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .unwrap()
}

pub fn summary_for_range(range: &str) -> (i64, i64, f64) {
    match range {
        "weekly" => summary_since(7),
        "monthly" => summary_since(30),
        _ => summary_daily(),
    }
}

pub fn log_usage(
    channel: &str,
    tool: &str,
    model: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    price_usd: f64,
    upstream_id: &str,
) {
    let conn = open_conn();
    let ts = chrono::Utc::now();
    let unix_ts = ts.timestamp();
    conn.execute("insert into usage_logs(timestamp,channel,tool,model,prompt_tokens,completion_tokens,total_tokens,price_usd,upstream_id) values(?,?,?,?,?,?,?,?,?)",
        params![unix_ts, channel, tool, model, prompt_tokens, completion_tokens, total_tokens, price_usd, upstream_id]).unwrap();
    fn bucket_day(ts: &chrono::DateTime<chrono::Utc>) -> String {
        ts.format("%Y-%m-%d").to_string()
    }
    fn bucket_week(ts: &chrono::DateTime<chrono::Utc>) -> String {
        let iso = ts.iso_week();
        format!("{}-W{:02}", iso.year(), iso.week())
    }
    fn bucket_month(ts: &chrono::DateTime<chrono::Utc>) -> String {
        ts.format("%Y-%m").to_string()
    }
    fn upsert(conn: &Connection, table: &str, bucket: &str, tokens: i64, price: f64) {
        let sql = format!("insert into {table} (bucket, requests, tokens, price_usd) values (?1,1,?2,?3) \
            on conflict(bucket) do update set requests=requests+1, tokens=tokens+excluded.tokens, price_usd=price_usd+excluded.price_usd");
        let _ = conn.execute(&sql, params![bucket, tokens, price]);
    }
    upsert(
        &conn,
        "usage_daily",
        &bucket_day(&ts),
        total_tokens,
        price_usd,
    );
    upsert(
        &conn,
        "usage_weekly",
        &bucket_week(&ts),
        total_tokens,
        price_usd,
    );
    upsert(
        &conn,
        "usage_monthly",
        &bucket_month(&ts),
        total_tokens,
        price_usd,
    );
}

pub fn series_tokens(days: i64) -> Vec<(String, i64)> {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select date(timestamp,'unixepoch'), ifnull(sum(total_tokens),0) from usage_logs where timestamp>= strftime('%s','now','-'||?1||' day') group by 1 order by 1").unwrap();
    let rows = stmt
        .query_map(params![days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

pub fn series_price(days: i64) -> Vec<(String, f64)> {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select date(timestamp,'unixepoch'), ifnull(sum(price_usd),0) from usage_logs where timestamp>= strftime('%s','now','-'||?1||' day') group by 1 order by 1").unwrap();
    let rows = stmt
        .query_map(params![days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

pub fn channels_breakdown() -> Vec<ChannelStats> {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select channel, ifnull(sum(total_tokens),0), ifnull(sum(price_usd),0) from usage_logs group by 1 order by 2 desc").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok(ChannelStats {
                channel: r.get(0)?,
                tokens: r.get(1)?,
                price_usd: r.get(2)?,
            })
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

pub fn models_cost_since(days: i64) -> Vec<ModelStats> {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select model, count(*), ifnull(sum(total_tokens),0), ifnull(sum(price_usd),0) from usage_logs where timestamp>= strftime('%s','now','-'||?1||' day') group by 1 order by 4 desc").unwrap();
    let rows = stmt
        .query_map(params![days], |r| {
            Ok(ModelStats {
                model: r.get(0)?,
                requests: r.get(1)?,
                tokens: r.get(2)?,
                price_usd: r.get(3)?,
            })
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct RequestLog {
    pub id: i64,
    pub timestamp: i64,
    pub channel: String,
    pub tool: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub price_usd: f64,
    pub upstream_id: String,
}

pub fn recent_logs(limit: i64, offset: i64) -> Vec<RequestLog> {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select id, timestamp, channel, tool, model, prompt_tokens, completion_tokens, total_tokens, price_usd, upstream_id from usage_logs order by timestamp desc limit ?1 offset ?2").unwrap();
    let rows = stmt
        .query_map(params![limit, offset], |r| {
            Ok(RequestLog {
                id: r.get(0)?,
                timestamp: r.get(1)?,
                channel: r.get(2)?,
                tool: r.get(3)?,
                model: r.get(4)?,
                prompt_tokens: r.get(5)?,
                completion_tokens: r.get(6)?,
                total_tokens: r.get(7)?,
                price_usd: r.get(8)?,
                upstream_id: r.get(9)?,
            })
        })
        .unwrap();
    rows.filter_map(|x| x.ok()).collect()
}

pub fn logs_count() -> i64 {
    let conn = open_conn();
    let mut stmt = conn.prepare_cached("select count(*) from usage_logs").unwrap();
    stmt.query_row([], |row| row.get(0)).unwrap_or(0)
}

pub fn clear_all_data() -> Result<(), String> {
    let conn = open_conn();
    conn.execute_batch(
        "DELETE FROM usage_logs;
        DELETE FROM usage_daily;
        DELETE FROM usage_weekly;
        DELETE FROM usage_monthly;
        DELETE FROM projects;
        DELETE FROM tools;
        DELETE FROM models;",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
