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

        if once {
            poll_once(&exe_dir, &client, config_mgr.config.token_expiry_warning).await;
            return;
        }

        if config_mgr.config.show_startup_notification {
            notify("ClaudeMeter", "Running in the macOS menu bar.");
        }

        loop {
            poll_once(&exe_dir, &client, config_mgr.config.token_expiry_warning).await;
            let interval = config_mgr.config.polling_interval_seconds.max(60);
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    });
}

async fn poll_once(exe_dir: &Path, client: &ClaudeClient, login_warning_enabled: bool) {
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
    publish_status(exe_dir, &usage);
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

fn publish_status(exe_dir: &Path, usage: &UsageResponse) {
    let percent = usage.max_utilization().unwrap_or(0.0).round() as u32;
    let plan = usage.detected_plan();
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

    let status = MacStatus {
        state: "live".to_string(),
        title: format!("{percent}%"),
        detail: message.clone(),
        plan: Some(plan),
        percent: Some(percent),
        metrics,
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
