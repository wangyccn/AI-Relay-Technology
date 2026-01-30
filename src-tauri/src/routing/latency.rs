use reqwest::Client;
use serde::Serialize;
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Clone)]
pub struct LatencyStat {
    pub endpoint: String,
    pub ok: bool,
    pub ms: Option<u128>,
}

fn curl_sink() -> &'static str {
    if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn measure_with_curl(url: &str) -> Option<LatencyStat> {
    // Use curl to measure total time; fallback handled by caller.
    let sink = curl_sink();
    let mut cmd = Command::new("curl");
    #[cfg(target_os = "windows")]
    {
        // Avoid popping console windows on Windows GUI apps.
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .args([
            "-o",
            sink,
            "-s",
            "-w",
            "%{time_total}",
            "-m",
            "5",
            "-I",
            url,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return Some(LatencyStat {
            endpoint: url.to_string(),
            ok: false,
            ms: None,
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(seconds) = stdout.trim().parse::<f64>() {
        let ms = (seconds * 1000.0) as u128;
        return Some(LatencyStat {
            endpoint: url.to_string(),
            ok: true,
            ms: Some(ms),
        });
    }
    None
}

/// Measure latency for a list of endpoints, returning the elapsed ms (if any) for each.
pub async fn measure_all(urls: Vec<String>) -> Vec<LatencyStat> {
    let client = Client::new();
    let mut stats = Vec::new();
    for u in urls {
        if let Some(curl_result) = measure_with_curl(&u) {
            stats.push(curl_result);
            continue;
        }
        let start = Instant::now();
        match client.head(&u).timeout(Duration::from_secs(5)).send().await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_millis();
                stats.push(LatencyStat {
                    endpoint: u,
                    ok: resp.status().is_success(),
                    ms: Some(elapsed),
                });
            }
            Err(_) => stats.push(LatencyStat {
                endpoint: u,
                ok: false,
                ms: None,
            }),
        };
    }
    stats
}

/// Return the fastest available endpoint (best-effort).
#[allow(dead_code)]
pub async fn probe(urls: Vec<String>) -> Option<String> {
    let mut best: Option<(String, Duration)> = None;
    for stat in measure_all(urls).await {
        if stat.ok {
            if let Some(ms) = stat.ms {
                let dur = Duration::from_millis(ms as u64);
                if let Some((_, best_dur)) = &best {
                    if dur < *best_dur {
                        best = Some((stat.endpoint, dur));
                    }
                } else {
                    best = Some((stat.endpoint, dur));
                }
            }
        }
    }
    best.map(|b| b.0)
}
