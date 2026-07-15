//! Local OpenAI Codex CLI session-log parser.
//!
//! Codex CLI writes "rollout" JSONL files under `~/.codex/sessions/YYYY/MM/DD/`.
//! Unlike the ChatGPT web product, these logs contain a `rate_limits` block with
//! `used_percent` + `resets_at` for the rolling 5-hour ("primary") and weekly
//! ("secondary") windows — the same information the Claude usage API exposes.
//! OpenAI never shipped a public subscription-usage API, which is why earlier
//! attempts failed; reading these local logs works.
//!
//! Everything here is local-only: no network, no token.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One rolling rate-limit window from a Codex `token_count` event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodexRateLimit {
    pub used_percent: f64,
    pub window_minutes: u64,
    /// Reset time as a Unix epoch (seconds), if provided.
    pub resets_at_epoch: Option<i64>,
}

impl CodexRateLimit {
    /// Reset time formatted as an RFC3339 string (to match the Claude pipeline).
    pub fn resets_at_rfc3339(&self) -> Option<String> {
        self.resets_at_epoch
            .and_then(|e| DateTime::<Utc>::from_timestamp(e, 0))
            .map(|dt| dt.to_rfc3339())
    }
}

/// A snapshot of Codex usage parsed from the most recent `token_count` event.
///
/// Codex reports two rolling windows in `primary`/`secondary` slots, but which
/// slot holds which window is NOT stable — a fresh session can put the weekly
/// window in `primary`. So windows are classified by `window_minutes`, not slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodexStatus {
    pub timestamp: DateTime<Utc>,
    pub plan_type: Option<String>,
    /// Rolling ~5-hour window (`window_minutes` < 1 day).
    pub five_hour: Option<CodexRateLimit>,
    /// Rolling weekly window (`window_minutes` >= 1 day).
    pub weekly: Option<CodexRateLimit>,
}

impl CodexStatus {
    /// Number of live rolling windows Codex is currently reporting (0-2). Used
    /// to reserve the correct dashboard height — Codex may report only one.
    pub fn window_count(&self) -> i32 {
        self.five_hour.is_some() as i32 + self.weekly.is_some() as i32
    }
}

/// A rate-limit window counts as "weekly" when its window spans a day or more;
/// otherwise it's treated as the short rolling (~5-hour) window.
fn is_weekly(rl: &CodexRateLimit) -> bool {
    rl.window_minutes >= 1440
}

fn parse_rate_limit(v: &serde_json::Value) -> Option<CodexRateLimit> {
    let obj = v.as_object()?;
    let used_percent = obj.get("used_percent").and_then(|x| x.as_f64())?;
    let window_minutes = obj
        .get("window_minutes")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let resets_at_epoch = obj.get("resets_at").and_then(|x| x.as_i64());
    Some(CodexRateLimit {
        used_percent,
        window_minutes,
        resets_at_epoch,
    })
}

/// Parse a single Codex rollout line. Returns the rate-limit status if this is
/// a `token_count` event carrying a `rate_limits` block.
pub fn parse_status_line(line: &str) -> Option<CodexStatus> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
        return None;
    }
    let payload = v.get("payload")?;
    if payload.get("type").and_then(|t| t.as_str()) != Some("token_count") {
        return None;
    }
    let rl = payload.get("rate_limits")?;

    let timestamp = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))?;

    let plan_type = rl
        .get("plan_type")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());

    // Classify each window by its duration, not by its primary/secondary slot.
    let mut five_hour = None;
    let mut weekly = None;
    for slot in ["primary", "secondary"] {
        if let Some(limit) = rl.get(slot).and_then(parse_rate_limit) {
            if is_weekly(&limit) {
                weekly = Some(limit);
            } else {
                five_hour = Some(limit);
            }
        }
    }

    // A status with neither window is useless.
    if five_hour.is_none() && weekly.is_none() {
        return None;
    }

    Some(CodexStatus {
        timestamp,
        plan_type,
        five_hour,
        weekly,
    })
}

/// Default Codex sessions directory: `~/.codex/sessions`.
pub fn default_sessions_dir() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(Path::new(&home).join(".codex").join("sessions"))
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Find the current Codex usage snapshot across recent session files.
///
/// Rollout files are named `rollout-<ISO timestamp>-...jsonl`, so the newest
/// file sorts last lexicographically. Each window (primary/secondary) is taken
/// from the **most recent event that actually carried it**, so a fresh event
/// that only reports the 5-hour window doesn't wipe the weekly figure (which is
/// exactly what happens right after a new session's first turn). Returns `None`
/// if no logs contain a rate-limit block.
pub fn latest_status(root: &Path, now: DateTime<Utc>) -> Option<CodexStatus> {
    let mut files = Vec::new();
    collect_jsonl(root, &mut files);
    files.sort();
    files.reverse();

    // Collect statuses from the newest few files, then merge per-field.
    let mut statuses: Vec<CodexStatus> = Vec::new();
    for f in files.iter().take(5) {
        let Ok(content) = std::fs::read_to_string(f) else {
            continue;
        };
        for line in content.lines() {
            if let Some(status) = parse_status_line(line) {
                statuses.push(status);
            }
        }
        // Once we have events covering both windows, no need to read older files.
        if statuses.iter().any(|s| s.five_hour.is_some())
            && statuses.iter().any(|s| s.weekly.is_some())
        {
            break;
        }
    }

    if statuses.is_empty() {
        return None;
    }

    // Newest first; take each window from the most recent event that carried it.
    statuses.sort_by_key(|s| std::cmp::Reverse(s.timestamp));
    let newest_ts = statuses[0].timestamp;
    let five_hour = statuses.iter().find_map(|s| s.five_hour.clone());
    let weekly = statuses.iter().find_map(|s| s.weekly.clone());
    let plan_type = statuses.iter().find_map(|s| s.plan_type.clone());

    // Drop any window whose reset time is already in the past: it has rolled
    // over and we have no current data for it, so showing a (stale or fake-zero)
    // value would be misleading. Only display windows Codex is currently
    // reporting with a live reset time.
    let now_epoch = now.timestamp();
    let live = |rl: Option<CodexRateLimit>| -> Option<CodexRateLimit> {
        rl.filter(|w| w.resets_at_epoch.map_or(true, |e| e > now_epoch))
    };
    let five_hour = live(five_hour);
    let weekly = live(weekly);

    // If nothing live remains, there's no meaningful status to show.
    if five_hour.is_none() && weekly.is_none() {
        return None;
    }

    Some(CodexStatus {
        timestamp: newest_ts,
        plan_type,
        five_hour,
        weekly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT: &str = r#"{"timestamp":"2026-07-11T12:42:48.215Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":16168,"cached_input_tokens":11008,"output_tokens":229,"reasoning_output_tokens":39,"total_tokens":16397},"last_token_usage":{"input_tokens":16168,"cached_input_tokens":11008,"output_tokens":229,"reasoning_output_tokens":39,"total_tokens":16397},"model_context_window":353400},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":42.0,"window_minutes":300,"resets_at":1783791754},"secondary":{"used_percent":5.0,"window_minutes":10080,"resets_at":1784378554},"credits":null,"individual_limit":null,"plan_type":"plus","rate_limit_reached_type":null}}}"#;

    #[test]
    fn test_parse_status_line() {
        let s = parse_status_line(EVENT).unwrap();
        assert_eq!(s.plan_type.as_deref(), Some("plus"));
        let p = s.five_hour.unwrap();
        assert_eq!(p.used_percent, 42.0);
        assert_eq!(p.window_minutes, 300);
        assert_eq!(p.resets_at_epoch, Some(1783791754));
        assert!(p.resets_at_rfc3339().is_some());
        let sec = s.weekly.unwrap();
        assert_eq!(sec.used_percent, 5.0);
        assert_eq!(sec.window_minutes, 10080);
    }

    #[test]
    fn test_window_classified_by_duration_not_slot() {
        // A fresh session can put the WEEKLY window in the `primary` slot.
        let line = r#"{"timestamp":"2026-07-13T15:31:27.975Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":0.0,"window_minutes":10080,"resets_at":1784561486},"plan_type":"plus"}}}"#;
        let s = parse_status_line(line).unwrap();
        // The 10080-minute window must be classified as weekly, not 5-hour.
        assert!(s.five_hour.is_none());
        let w = s.weekly.unwrap();
        assert_eq!(w.window_minutes, 10080);
    }

    #[test]
    fn test_parse_status_line_skips_non_events() {
        assert!(parse_status_line(r#"{"type":"session_meta","payload":{}}"#).is_none());
        assert!(
            parse_status_line(r#"{"type":"event_msg","payload":{"type":"task_started"}}"#)
                .is_none()
        );
        assert!(parse_status_line("garbage").is_none());
    }

    #[test]
    fn test_parse_status_line_five_hour_only() {
        // A status with only the 5-hour window still parses.
        let line = r#"{"timestamp":"2026-07-11T12:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{},"rate_limits":{"primary":{"used_percent":10.0,"window_minutes":300,"resets_at":100},"plan_type":"pro"}}}"#;
        let s = parse_status_line(line).unwrap();
        assert_eq!(s.plan_type.as_deref(), Some("pro"));
        assert_eq!(s.five_hour.unwrap().used_percent, 10.0);
        assert!(s.weekly.is_none());
    }

    #[test]
    fn test_rate_limit_epoch_to_rfc3339() {
        let rl = CodexRateLimit {
            used_percent: 5.0,
            window_minutes: 300,
            resets_at_epoch: Some(1_700_000_000),
        };
        let s = rl.resets_at_rfc3339().unwrap();
        assert!(s.starts_with("2023-11-14T"));
    }

    #[test]
    fn test_latest_status_missing_dir() {
        assert!(latest_status(Path::new("/nonexistent/codex/xyz"), Utc::now()).is_none());
    }

    // A `now` before all the reset epochs used in the tests below.
    fn test_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-13T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    // Future reset epochs (well after test_now) so they aren't "freshened".
    const FUTURE_5H: i64 = 1_784_000_000; // ~2026-07-13
    const FUTURE_WK: i64 = 1_784_500_000;

    #[test]
    fn test_latest_status_carries_forward_secondary() {
        // Newest event has only the 5-hour window; the weekly window must be
        // carried forward from the previous event instead of disappearing.
        let dir = std::env::temp_dir().join("claudemeter_codex_merge_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("rollout-2026-07-13T09-00-00-merge.jsonl");
        let older = format!(
            r#"{{"timestamp":"2026-07-13T09:00:00.000Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":50.0,"window_minutes":300,"resets_at":{FUTURE_5H}}},"secondary":{{"used_percent":30.0,"window_minutes":10080,"resets_at":{FUTURE_WK}}},"plan_type":"plus"}}}}}}"#
        );
        let newer = format!(
            r#"{{"timestamp":"2026-07-13T09:30:00.000Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":0.0,"window_minutes":300,"resets_at":{FUTURE_5H}}},"plan_type":"plus"}}}}}}"#
        );
        std::fs::write(&file, format!("{older}\n{newer}\n")).unwrap();

        let s = latest_status(&dir, test_now()).unwrap();
        // 5-hour from the newest event (0%), weekly carried from older (30%).
        assert_eq!(s.five_hour.unwrap().used_percent, 0.0);
        let sec = s.weekly.expect("weekly should be carried forward");
        assert_eq!(sec.used_percent, 30.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_latest_status_drops_expired_window() {
        // A window whose reset time is already in the past has rolled over and
        // has no current data → it must be dropped, not shown as a fake 0%.
        let dir = std::env::temp_dir().join("claudemeter_codex_expired_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("rollout-2026-07-13T08-00-00-expired.jsonl");
        // 5-hour with a reset time in the PAST (epoch 100 = 1970), weekly fresh.
        let line = format!(
            r#"{{"timestamp":"2026-07-13T08:00:00.000Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":6.0,"window_minutes":300,"resets_at":100}},"secondary":{{"used_percent":1.0,"window_minutes":10080,"resets_at":{FUTURE_WK}}},"plan_type":"plus"}}}}}}"#
        );
        std::fs::write(&file, format!("{line}\n")).unwrap();

        let s = latest_status(&dir, test_now()).unwrap();
        // Expired 5-hour window is dropped entirely.
        assert!(s.five_hour.is_none(), "expired window should be dropped");
        // Weekly (future reset) keeps its real value.
        assert_eq!(s.weekly.unwrap().used_percent, 1.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_latest_status_none_when_all_expired() {
        // If every window has already reset, there's nothing live to show.
        let dir = std::env::temp_dir().join("claudemeter_codex_allexpired_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("rollout-2026-07-13T08-00-00-allexpired.jsonl");
        let line = r#"{"timestamp":"2026-07-13T08:00:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":6.0,"window_minutes":300,"resets_at":100},"secondary":{"used_percent":1.0,"window_minutes":10080,"resets_at":200},"plan_type":"plus"}}}"#;
        std::fs::write(&file, format!("{line}\n")).unwrap();

        assert!(latest_status(&dir, test_now()).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Live sanity check against this machine's real ~/.codex/sessions logs.
    // Run with: cargo test live_codex_status -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_codex_status() {
        let Some(dir) = default_sessions_dir() else {
            return;
        };
        match latest_status(&dir, Utc::now()) {
            Some(s) => {
                println!("plan: {:?}", s.plan_type);
                println!("5h: {:?}", s.five_hour);
                println!("weekly: {:?}", s.weekly);
                println!("as of: {}", s.timestamp);
            }
            None => println!("no codex status found"),
        }
    }
}
