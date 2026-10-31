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
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
    /// Purchased credits, reported alongside the windows when available.
    pub credits: Option<CodexCredits>,
    /// The latest event says a rate limit or spend control has been hit.
    pub limit_reached: bool,
}

/// The `credits` block of a `rate_limits` event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodexCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<f64>,
}

impl CodexCredits {
    /// Text worth showing, or None when the account has no credits.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn display_balance(&self) -> Option<String> {
        if self.unlimited {
            return Some("\u{221E}".to_string());
        }
        let balance = self.balance.filter(|b| self.has_credits && *b > 0.0)?;
        Some(format!("{}", balance.round() as i64))
    }
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

/// Null, missing and `false` all mean the condition isn't active.
fn is_set(v: Option<&serde_json::Value>) -> bool {
    !matches!(
        v,
        None | Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false))
    )
}

fn parse_credits(v: &serde_json::Value) -> Option<CodexCredits> {
    let obj = v.as_object()?;
    let flag = |key: &str| obj.get(key).and_then(|x| x.as_bool()).unwrap_or(false);
    // The balance arrives as a string ("0"), but accept a number too.
    let balance = obj.get("balance").and_then(|b| {
        b.as_f64()
            .or_else(|| b.as_str().and_then(|s| s.parse().ok()))
    });
    Some(CodexCredits {
        has_credits: flag("has_credits"),
        unlimited: flag("unlimited"),
        balance,
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

    // Model-specific buckets carry their own `limit_id`; only the main "codex"
    // bucket describes the plan-wide windows shown here.
    if rl
        .get("limit_id")
        .and_then(|i| i.as_str())
        .unwrap_or("codex")
        != "codex"
    {
        return None;
    }

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
        credits: rl.get("credits").and_then(parse_credits),
        limit_reached: is_set(rl.get("rate_limit_reached_type"))
            || is_set(rl.get("spend_control_reached")),
    })
}

/// Default Codex sessions directory: `~/.codex/sessions`.
pub fn default_sessions_dir() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(Path::new(&home).join(".codex").join("sessions"))
}

fn collect_with_suffix(dir: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_with_suffix(&path, suffix, out);
        } else if path.to_string_lossy().ends_with(suffix) {
            out.push(path);
        }
    }
}

/// How much of each rollout to read. Logs can reach tens of MB, and the latest
/// rate-limit events are always near the end.
const ROLLOUT_TAIL_BYTES: u64 = 1024 * 1024;

/// Last `ROLLOUT_TAIL_BYTES` of a log, without the partial first line.
fn read_tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(ROLLOUT_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((len - start) as usize);
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    if start == 0 {
        return Some(text.into_owned());
    }
    text.split_once('\n').map(|(_, rest)| rest.to_string())
}

/// Find the current Codex usage snapshot across recent session files.
///
/// Files are ordered by modification time, not by name: the name holds the
/// session's creation date, and a resumed old session is often the live one.
/// Finished logs get compressed to `.jsonl.zst`; the newest one is only read
/// when plain logs hold nothing live. Each window (primary/secondary) is taken
/// from the **most recent event that actually carried it**, so a fresh event
/// that only reports the 5-hour window doesn't wipe the weekly figure (which is
/// exactly what happens right after a new session's first turn). Returns `None`
/// if no logs contain a rate-limit block.
pub fn latest_status(root: &Path, now: DateTime<Utc>) -> Option<CodexStatus> {
    let mut files = Vec::new();
    collect_with_suffix(root, ".jsonl", &mut files);
    let modified = |path: &PathBuf| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    files.sort_by_cached_key(|path| std::cmp::Reverse(modified(path)));

    // Collect statuses from the newest few files, then merge per-field.
    let mut statuses: Vec<CodexStatus> = Vec::new();
    for f in files.iter().take(5) {
        let Some(content) = read_tail(f) else {
            continue;
        };
        for line in content.lines() {
            if let Some(status) = parse_status_line(line) {
                statuses.push(status);
            }
        }
        // Once both windows are live, no need to read older files. An expired
        // window keeps looking: a resumed session may only hold old events.
        let is_live = |w: &CodexRateLimit| w.resets_at_epoch.is_some_and(|e| e > now.timestamp());
        if statuses
            .iter()
            .any(|s| s.five_hour.as_ref().is_some_and(is_live))
            && statuses
                .iter()
                .any(|s| s.weekly.as_ref().is_some_and(is_live))
        {
            break;
        }
    }

    let any_live = statuses.iter().any(|s| {
        [&s.five_hour, &s.weekly]
            .into_iter()
            .flatten()
            .any(|w| w.resets_at_epoch.map_or(true, |e| e > now.timestamp()))
    });
    if !any_live {
        statuses.extend(newest_compressed_statuses(root));
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
    let credits = statuses.iter().find_map(|s| s.credits.clone());
    let limit_reached = statuses[0].limit_reached;

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
        credits,
        limit_reached,
    })
}

type CompressedCache = Option<(PathBuf, std::time::SystemTime, Vec<CodexStatus>)>;
static COMPRESSED_CACHE: Mutex<CompressedCache> = Mutex::new(None);
/// Enough events to find both windows, without keeping a whole session.
const COMPRESSED_KEEP: usize = 4;

/// Last events of the newest compressed log. Decompression walks the whole file,
/// so the result is cached until a newer file appears.
fn newest_compressed_statuses(root: &Path) -> Vec<CodexStatus> {
    let mut files = Vec::new();
    collect_with_suffix(root, ".jsonl.zst", &mut files);
    let newest = files
        .into_iter()
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(modified, _)| *modified);
    let Some((modified, path)) = newest else {
        return Vec::new();
    };
    let Ok(mut cache) = COMPRESSED_CACHE.lock() else {
        return Vec::new();
    };
    if let Some((cached_path, cached_modified, statuses)) = cache.as_ref() {
        if *cached_path == path && *cached_modified == modified {
            return statuses.clone();
        }
    }
    let statuses = read_compressed(&path).unwrap_or_default();
    *cache = Some((path, modified, statuses.clone()));
    statuses
}

fn read_compressed(path: &Path) -> Option<Vec<CodexStatus>> {
    let file = BufReader::new(std::fs::File::open(path).ok()?);
    let decoder = ruzstd::decoding::StreamingDecoder::new(file).ok()?;
    let mut statuses = Vec::new();
    for line in BufReader::new(decoder).split(b'\n').map_while(Result::ok) {
        // Cheap filter: most lines are huge and don't hold rate limits.
        if !line.windows(13).any(|w| w == b"\"rate_limits\"") {
            continue;
        }
        if let Some(status) = parse_status_line(&String::from_utf8_lossy(&line)) {
            if statuses.len() == COMPRESSED_KEEP {
                statuses.remove(0);
            }
            statuses.push(status);
        }
    }
    Some(statuses)
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

    fn rate_event(ts: &str, percent: f64, reset: i64) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{percent},"window_minutes":10080,"resets_at":{reset}}},"plan_type":"pro"}}}}}}"#
        )
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn set_age(path: &Path, seconds: u64) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn test_resumed_old_session_is_read() {
        // Lives in an old-named file, so name order would never reach it first.
        let dir = fresh_dir("claudemeter_codex_resumed_test");
        let old_name = dir.join("rollout-2026-01-01T00-00-00-a.jsonl");
        let new_name = dir.join("rollout-2026-07-01T00-00-00-b.jsonl");
        std::fs::write(
            &old_name,
            rate_event("2026-07-13T09:00:00.000Z", 77.0, FUTURE_WK),
        )
        .unwrap();
        std::fs::write(
            &new_name,
            rate_event("2026-07-01T09:00:00.000Z", 11.0, FUTURE_WK),
        )
        .unwrap();
        set_age(&new_name, 3600);

        let s = latest_status(&dir, test_now()).unwrap();
        assert_eq!(s.weekly.unwrap().used_percent, 77.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_expired_recent_file_does_not_hide_live_data() {
        // The newest-modified file holds only expired windows.
        let dir = fresh_dir("claudemeter_codex_keepreading_test");
        let stale = dir.join("rollout-stale.jsonl");
        let live = dir.join("rollout-live.jsonl");
        let both = r#"{"timestamp":"2026-06-01T09:00:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":2.0,"window_minutes":300,"resets_at":100},"secondary":{"used_percent":1.0,"window_minutes":10080,"resets_at":200}}}}"#;
        std::fs::write(&stale, both).unwrap();
        std::fs::write(
            &live,
            rate_event("2026-07-13T09:00:00.000Z", 40.0, FUTURE_WK),
        )
        .unwrap();
        set_age(&live, 3600);

        let s = latest_status(&dir, test_now()).unwrap();
        assert_eq!(s.weekly.unwrap().used_percent, 40.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_large_log_reads_only_complete_tail_lines() {
        let dir = fresh_dir("claudemeter_codex_tail_test");
        let file = dir.join("rollout-big.jsonl");
        let filler = format!("{{\"pad\":\"{}\"}}\n", "x".repeat(2 * 1024 * 1024));
        let event = rate_event("2026-07-13T09:00:00.000Z", 9.0, FUTURE_WK);
        std::fs::write(&file, format!("{filler}{event}\n")).unwrap();

        let tail = read_tail(&file).unwrap();
        assert!(tail.len() <= ROLLOUT_TAIL_BYTES as usize);
        let status = latest_status(&dir, test_now()).unwrap();
        assert_eq!(status.weekly.unwrap().used_percent, 9.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_other_limit_id_is_ignored() {
        let line = r#"{"timestamp":"2026-07-13T09:00:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex_other","primary":{"used_percent":99.0,"window_minutes":300,"resets_at":1784000000}}}}"#;
        assert!(parse_status_line(line).is_none());
    }

    #[test]
    fn test_weekly_only_pro_plan() {
        let s = parse_status_line(&rate_event("2026-07-13T09:00:00.000Z", 3.0, FUTURE_WK)).unwrap();
        assert!(s.five_hour.is_none());
        assert_eq!(s.window_count(), 1);
    }

    #[test]
    fn test_credits_block() {
        let parse = |json: serde_json::Value| parse_credits(&json).unwrap().display_balance();
        assert_eq!(
            parse(serde_json::json!({"has_credits": false, "unlimited": false, "balance": "0"})),
            None
        );
        assert_eq!(
            parse(serde_json::json!({"has_credits": true, "unlimited": false, "balance": "120.6"})),
            Some("121".to_string())
        );
        assert_eq!(
            parse(serde_json::json!({"has_credits": true, "unlimited": true, "balance": null})),
            Some("\u{221E}".to_string())
        );
        assert!(parse_credits(&serde_json::Value::Null).is_none());
    }

    #[test]
    fn test_compressed_log_is_used_when_plain_logs_are_gone() {
        let dir = fresh_dir("claudemeter_codex_zst_test");
        let event = rate_event("2026-07-13T09:00:00.000Z", 21.0, FUTURE_WK);
        let compressed = ruzstd::encoding::compress_to_vec(
            format!("{{\"pad\":1}}\n{event}\n").as_bytes(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        std::fs::write(dir.join("rollout-old.jsonl.zst"), compressed).unwrap();

        let s = latest_status(&dir, test_now()).unwrap();
        assert_eq!(s.weekly.unwrap().used_percent, 21.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_limit_reached_flags() {
        let line = |extra: &str| {
            format!(
                r#"{{"timestamp":"2026-07-13T09:00:00.000Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":100.0,"window_minutes":300,"resets_at":1784000000}}{extra}}}}}}}"#
            )
        };
        assert!(
            !parse_status_line(&line(r#","rate_limit_reached_type":null"#))
                .unwrap()
                .limit_reached
        );
        assert!(
            parse_status_line(&line(r#","rate_limit_reached_type":"primary""#))
                .unwrap()
                .limit_reached
        );
        assert!(
            parse_status_line(&line(r#","spend_control_reached":true"#))
                .unwrap()
                .limit_reached
        );
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
