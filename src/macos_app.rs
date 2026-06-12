use crate::config::ConfigManager;
use crate::credentials::read_claude_token;
use crate::db::Database;
use crate::providers::claude::{format_metric_name, ClaudeClient, UsageResponse};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricEntry {
    /// Raw API key, e.g. "five_hour" — used by the UI to pick the session metric.
    key: String,
    /// Human-readable name, e.g. "Weekly (7-day)"
    name: String,
    percent: u32,
    resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MacStatus {
    state: String,
    title: String,
    detail: String,
    plan: Option<String>,
    percent: Option<u32>,
    /// Per-limit breakdown (5-hour, weekly, Sonnet, Opus, ...). Empty until first fetch.
    #[serde(default)]
    metrics: Vec<MetricEntry>,
    /// Downgrade comparison, e.g. "On Max 5x: weekly ~96%, session ~124%".
    #[serde(default)]
    tier_note: Option<String>,
    last_api_update: Option<String>,
    data_age_seconds: Option<u64>,
    error: Option<String>,
}

impl MacStatus {
    fn refreshing() -> Self {
        Self {
            state: "refreshing".to_string(),
            title: "Refreshing...".to_string(),
            detail: "Requesting fresh Claude usage data".to_string(),
            plan: None,
            percent: None,
            metrics: Vec::new(),
            tier_note: None,
            last_api_update: None,
            data_age_seconds: None,
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            state: "error".to_string(),
            title: "API error".to_string(),
            detail: message.clone(),
            plan: None,
            percent: None,
            metrics: Vec::new(),
            tier_note: None,
            last_api_update: None,
            data_age_seconds: None,
            error: Some(message),
        }
    }
}

pub fn run() {
    env_logger::init();

    let exe_dir = app_data_dir();
    if let Err(e) = std::fs::create_dir_all(&exe_dir) {
        log::warn!("Failed to create app data directory {:?}: {e}", exe_dir);
    }

    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|arg| arg == "--once" || arg == "--refresh");
    let status_only = args.iter().any(|arg| arg == "--status");

    if status_only {
        print_status(&exe_dir);
        return;
    }

    let config_mgr = ConfigManager::new(&exe_dir);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    rt.block_on(async move {
        let client = match ClaudeClient::new() {
            Ok(client) => client,
            Err(e) => {
                let message = format!("Failed to create Claude client: {e}");
                append_log(&exe_dir, &message);
                write_error(&exe_dir, message);
                return;
            }
        };

        let plan_override = config_mgr.config.plan_override.clone();
        let login_warning = config_mgr.config.token_expiry_warning;

        if once {
            poll_once(&exe_dir, &client, login_warning, plan_override.as_deref()).await;
            return;
        }

        if config_mgr.config.show_startup_notification {
            notify("ClaudeMeter", "Running in the macOS menu bar.");
        }

        loop {
            poll_once(&exe_dir, &client, login_warning, plan_override.as_deref()).await;
            let interval = config_mgr.config.polling_interval_seconds.max(60);
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    });
}

async fn poll_once(
    exe_dir: &Path,
    client: &ClaudeClient,
    login_warning_enabled: bool,
    plan_override: Option<&str>,
) {
    mark_refreshing(exe_dir);

    let credential = match read_claude_token() {
        Ok(credential) => credential,
        Err(e) => {
            let message = format!("Claude credentials unavailable: {e}");
            append_log(exe_dir, &message);
            if login_warning_enabled {
                notify(
                    "ClaudeMeter",
                    "Claude login not found. Run `claude` in Terminal.",
                );
            }
            write_error(exe_dir, message);
            return;
        }
    };

    let mut usage = match client.fetch_usage(&credential.access_token).await {
        Ok(usage) => usage,
        Err(e) => {
            let message = format!("Usage poll failed: {e}");
            append_log(exe_dir, &message);
            write_error(exe_dir, message);
            return;
        }
    };

    usage.subscription_type = credential.subscription_type;
    usage.rate_limit_tier = credential.rate_limit_tier;

    save_history(exe_dir, &usage);
    publish_status(exe_dir, &usage, plan_override);
}

fn save_history(exe_dir: &Path, usage: &UsageResponse) {
    let db = match Database::open(exe_dir) {
        Ok(db) => db,
        Err(e) => {
            append_log(exe_dir, &format!("Database unavailable: {e}"));
            return;
        }
    };

    for (metric, value) in usage.all_metrics() {
        if let Err(e) = db.insert(
            "claude",
            &metric,
            value.utilization,
            value.resets_at.as_deref(),
        ) {
            append_log(exe_dir, &format!("Failed to save metric {metric}: {e}"));
        }
    }
}

fn publish_status(exe_dir: &Path, usage: &UsageResponse, plan_override: Option<&str>) {
    let percent = usage.max_utilization().unwrap_or(0.0).round() as u32;
    let plan = plan_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| usage.detected_plan());
    let now = Local::now();
    let last_api_update = now.to_rfc3339();
    let message = format!("{plan}: {percent}% max usage");

    let metrics = usage
        .all_metrics()
        .into_iter()
        .map(|(key, m)| MetricEntry {
            name: format_metric_name(&key),
            percent: m.utilization.round() as u32,
            resets_at: m.resets_at.clone(),
            key,
        })
        .collect();

    let tier_note = plan_override.and_then(|p| build_tier_note(p, usage));

    let status = MacStatus {
        state: "live".to_string(),
        title: format!("{percent}%"),
        detail: message.clone(),
        plan: Some(plan),
        percent: Some(percent),
        metrics,
        tier_note,
        last_api_update: Some(last_api_update),
        data_age_seconds: Some(0),
        error: None,
    };

    append_log(
        exe_dir,
        &format!("[{}] {}", now.format("%Y-%m-%d %H:%M:%S"), message),
    );
    write_status(exe_dir, &status);

    if percent >= 90 {
        notify("ClaudeMeter: high usage", &message);
    }
}

/// Plan tier as a multiple of the Pro base allowance. Used to estimate what
/// usage would look like on a smaller plan. Recognized labels: Pro, Max 5x,
/// Max 20x (bare "Max" assumed 5x, its entry tier).
fn plan_multiplier(plan: &str) -> Option<f64> {
    let p = plan.to_lowercase();
    if p.contains("20x") {
        Some(20.0)
    } else if p.contains("5x") {
        Some(5.0)
    } else if p.contains("max") {
        Some(5.0)
    } else if p.contains("pro") {
        Some(1.0)
    } else {
        None
    }
}

/// The next cheaper tier and its multiplier, or None if already at the bottom.
fn lower_tier(mult: f64) -> Option<(&'static str, f64)> {
    if mult >= 20.0 {
        Some(("Max 5x", 5.0))
    } else if mult >= 5.0 {
        Some(("Pro", 1.0))
    } else {
        None
    }
}

/// "On Max 5x: weekly ~96%, session ~124% — would throttle": estimate the
/// session and weekly usage if the same work ran on the next tier down.
/// Assumes limits scale linearly with the tier multiplier.
fn build_tier_note(plan: &str, usage: &UsageResponse) -> Option<String> {
    let mult = plan_multiplier(plan)?;
    let (lower_name, lower_mult) = lower_tier(mult)?;
    let factor = mult / lower_mult;

    let week = usage.seven_day.as_ref().map(|m| m.utilization);
    let five = usage.five_hour.as_ref().map(|m| m.utilization);

    let mut parts = Vec::new();
    if let Some(w) = week {
        parts.push(format!("weekly ~{}%", (w * factor).round() as i64));
    }
    if let Some(f) = five {
        parts.push(format!("session ~{}%", (f * factor).round() as i64));
    }
    if parts.is_empty() {
        return None;
    }

    let over = week.is_some_and(|w| w * factor > 100.0) || five.is_some_and(|f| f * factor > 100.0);
    let verdict = if over {
        " — would throttle"
    } else {
        " — would fit"
    };
    Some(format!("On {}: {}{}", lower_name, parts.join(", "), verdict))
}

fn write_status(exe_dir: &Path, status: &MacStatus) {
    let path = exe_dir.join("status.json");
    match serde_json::to_string_pretty(status) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                log::warn!("Failed to write macOS status: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize macOS status: {e}"),
    }
}

/// Begin a refresh without blanking the menu. If a prior good reading exists,
/// keep its numbers on screen (and clear any stale error) instead of flashing
/// an empty "refreshing" state every poll. Only show the blank refreshing
/// placeholder on the very first run, when there is no data yet.
fn mark_refreshing(exe_dir: &Path) {
    let path = exe_dir.join("status.json");
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(mut prev) = serde_json::from_str::<MacStatus>(&contents) {
            if prev.percent.is_some() {
                prev.error = None;
                write_status(exe_dir, &prev);
                return;
            }
        }
    }
    write_status(exe_dir, &MacStatus::refreshing());
}

/// Record a fetch failure without discarding the last good reading.
/// If a prior live status exists, keep its data and metrics (so the menu
/// keeps showing the numbers with a staleness indicator) and just attach
/// the error. Only blank out when there is no prior data to show.
fn write_error(exe_dir: &Path, message: String) {
    let path = exe_dir.join("status.json");
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(mut prev) = serde_json::from_str::<MacStatus>(&contents) {
            if prev.percent.is_some() {
                prev.error = Some(message);
                write_status(exe_dir, &prev);
                return;
            }
        }
    }
    write_status(exe_dir, &MacStatus::error(message));
}

fn print_status(exe_dir: &Path) {
    let path = exe_dir.join("status.json");
    match std::fs::read_to_string(path) {
        Ok(status) => println!("{status}"),
        Err(_) => println!(
            "{}",
            serde_json::to_string(&MacStatus::refreshing()).unwrap()
        ),
    }
}

fn append_log(exe_dir: &Path, message: &str) {
    let path = exe_dir.join("claudemeter.log");
    let line = format!("{}\n", message);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(line.as_bytes())
        });
}

fn notify(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        escape_applescript(message),
        escape_applescript(title)
    );

    let _ = Command::new("osascript").arg("-e").arg(script).status();
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn app_data_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("ClaudeMeter");
    }

    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refreshing_status_shape() {
        let status = MacStatus::refreshing();
        assert_eq!(status.state, "refreshing");
        assert_eq!(status.title, "Refreshing...");
        assert!(status.percent.is_none());
    }

    #[test]
    fn test_escape_applescript() {
        assert_eq!(escape_applescript(r#"a\b"c"#), r#"a\\b\"c"#);
    }
}
