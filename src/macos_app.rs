use crate::config::ConfigManager;
use crate::credentials::read_claude_token;
use crate::db::Database;
use crate::providers::claude::{ClaudeClient, UsageResponse};
use chrono::Local;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

pub fn run() {
    env_logger::init();

    let exe_dir = app_data_dir();
    let config_mgr = ConfigManager::new(&exe_dir);

    if let Err(e) = std::fs::create_dir_all(&exe_dir) {
        log::warn!("Failed to create app data directory {:?}: {e}", exe_dir);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    rt.block_on(async move {
        let client = match ClaudeClient::new() {
            Ok(client) => client,
            Err(e) => {
                log::error!("Failed to create Claude client: {e}");
                return;
            }
        };

        if config_mgr.config.show_startup_notification {
            notify(
                "ClaudeMeter",
                "Running as a low-memory macOS background agent.",
            );
        }

        loop {
            poll_once(&exe_dir, &client, config_mgr.config.token_expiry_warning).await;
            let interval = config_mgr.config.polling_interval_seconds.max(60);
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    });
}

async fn poll_once(exe_dir: &PathBuf, client: &ClaudeClient, login_warning_enabled: bool) {
    let credential = match read_claude_token() {
        Ok(credential) => credential,
        Err(e) => {
            log::warn!("Claude credentials unavailable: {e}");
            if login_warning_enabled {
                notify(
                    "ClaudeMeter",
                    "Claude login not found. Run `claude` in Terminal.",
                );
            }
            return;
        }
    };

    let mut usage = match client.fetch_usage(&credential.access_token).await {
        Ok(usage) => usage,
        Err(e) => {
            log::warn!("Usage poll failed: {e}");
            return;
        }
    };

    usage.subscription_type = credential.subscription_type;
    usage.rate_limit_tier = credential.rate_limit_tier;

    save_history(exe_dir, &usage);
    publish_status(&usage);
}

fn save_history(exe_dir: &PathBuf, usage: &UsageResponse) {
    let db = match Database::open(exe_dir) {
        Ok(db) => db,
        Err(e) => {
            log::warn!("Database unavailable: {e}");
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
            log::warn!("Failed to save metric {metric}: {e}");
        }
    }
}

fn publish_status(usage: &UsageResponse) {
    let percent = usage.max_utilization().unwrap_or(0.0).round() as u32;
    let plan = usage.detected_plan();
    let message = format!("{plan}: {percent}% max usage");

    println!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), message);

    if percent >= 90 {
        notify("ClaudeMeter: high usage", &message);
    }
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
