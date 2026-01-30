//! Request limiting utilities (RPM, budgets, concurrency).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::response::Response;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use crate::{config, db, logger};

use super::error::{ForwardError, ForwardResult};

#[derive(Default)]
struct LimitState {
    rpm_window: VecDeque<Instant>,
    concurrent_total: u32,
    concurrent_by_session: HashMap<String, u32>,
}

static LIMIT_STATE: Lazy<Arc<Mutex<LimitState>>> =
    Lazy::new(|| Arc::new(Mutex::new(LimitState::default())));

#[derive(Clone)]
pub struct LimitGuard {
    session_id: Option<String>,
    state: Arc<Mutex<LimitState>>,
}

impl LimitGuard {
    fn new(session_id: Option<String>) -> Self {
        Self {
            session_id,
            state: Arc::clone(&LIMIT_STATE),
        }
    }
}

impl Drop for LimitGuard {
    fn drop(&mut self) {
        let session_id = self.session_id.clone();
        let state = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut guard = state.lock().await;
            if guard.concurrent_total > 0 {
                guard.concurrent_total -= 1;
            }
            if let Some(session_id) = session_id {
                if let Some(count) = guard.concurrent_by_session.get_mut(&session_id) {
                    if *count > 0 {
                        *count -= 1;
                    }
                    if *count == 0 {
                        guard.concurrent_by_session.remove(&session_id);
                    }
                }
            }
        });
    }
}

fn budget_remaining(limit: Option<f64>, spent: f64, label: &str) -> ForwardResult<()> {
    let Some(limit) = limit else {
        return Ok(());
    };
    if limit <= 0.0 {
        return Err(ForwardError::RateLimited(format!(
            "{} budget is <= 0; all requests are blocked",
            label
        )));
    }
    if spent >= limit {
        return Err(ForwardError::RateLimited(format!(
            "{} budget exceeded: spent ${:.6} / limit ${:.6}",
            label, spent, limit
        )));
    }
    Ok(())
}

fn check_budgets(limits: &config::RateLimitConfig) -> ForwardResult<()> {
    if limits.budget_daily_usd.is_some() {
        let (_, _, spent) = db::summary_for_range("daily");
        budget_remaining(limits.budget_daily_usd, spent, "Daily")?;
    }
    if limits.budget_weekly_usd.is_some() {
        let (_, _, spent) = db::summary_for_range("weekly");
        budget_remaining(limits.budget_weekly_usd, spent, "Weekly")?;
    }
    if limits.budget_monthly_usd.is_some() {
        let (_, _, spent) = db::summary_for_range("monthly");
        budget_remaining(limits.budget_monthly_usd, spent, "Monthly")?;
    }
    Ok(())
}

fn clean_rpm_window(window: &mut VecDeque<Instant>) {
    let cutoff = Instant::now() - Duration::from_secs(60);
    while matches!(window.front(), Some(ts) if *ts < cutoff) {
        window.pop_front();
    }
}

pub async fn check_and_acquire(session_id: Option<String>) -> ForwardResult<Option<LimitGuard>> {
    let cfg = config::load();
    let limits = cfg.limits;

    let has_limits = limits.rpm.is_some()
        || limits.max_concurrent.is_some()
        || limits.max_concurrent_per_session.is_some()
        || limits.budget_daily_usd.is_some()
        || limits.budget_weekly_usd.is_some()
        || limits.budget_monthly_usd.is_some();

    if !has_limits {
        return Ok(None);
    }

    check_budgets(&limits)?;

    let session_key = session_id.clone().unwrap_or_else(|| "anonymous".to_string());

    let mut state = LIMIT_STATE.lock().await;

    if let Some(rpm) = limits.rpm {
        if rpm > 0 {
            clean_rpm_window(&mut state.rpm_window);
            if state.rpm_window.len() as u32 >= rpm {
                return Err(ForwardError::RateLimited(format!(
                    "RPM limit exceeded: {} per minute",
                    rpm
                )));
            }
            state.rpm_window.push_back(Instant::now());
        } else {
            return Err(ForwardError::RateLimited(
                "RPM limit is <= 0; all requests are blocked".to_string(),
            ));
        }
    }

    if let Some(max) = limits.max_concurrent {
        if max == 0 {
            return Err(ForwardError::RateLimited(
                "Concurrency limit is 0; all requests are blocked".to_string(),
            ));
        }
        if state.concurrent_total + 1 > max {
            return Err(ForwardError::RateLimited(format!(
                "Concurrency limit exceeded: {} in-flight",
                max
            )));
        }
    }

    if let Some(max) = limits.max_concurrent_per_session {
        if max == 0 {
            return Err(ForwardError::RateLimited(
                "Session concurrency limit is 0; all requests are blocked".to_string(),
            ));
        }
        let current = state
            .concurrent_by_session
            .get(&session_key)
            .copied()
            .unwrap_or(0);
        if current + 1 > max {
            return Err(ForwardError::RateLimited(format!(
                "Session concurrency limit exceeded: {} in-flight",
                max
            )));
        }
    }

    state.concurrent_total += 1;
    if limits.max_concurrent_per_session.is_some() {
        let entry = state.concurrent_by_session.entry(session_key).or_insert(0);
        *entry += 1;
    }

    logger::debug(
        "limits",
        &format!(
            "Acquired limit guard: total_in_flight={}, session={:?}",
            state.concurrent_total,
            session_id
        ),
    );

    Ok(Some(LimitGuard::new(session_id)))
}

pub fn attach_guard(mut response: Response, guard: Option<LimitGuard>) -> Response {
    if let Some(guard) = guard {
        response.extensions_mut().insert(guard);
    }
    response
}
