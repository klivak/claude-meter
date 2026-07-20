use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const USAGE_API_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetric {
    pub utilization: f64,
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// The full usage response. Known fields are parsed directly;
/// any additional fields with the same shape are captured in `extra`.
#[derive(Debug, Clone)]
pub struct UsageResponse {
    pub five_hour: Option<UsageMetric>,
    pub seven_day: Option<UsageMetric>,
    pub seven_day_sonnet: Option<UsageMetric>,
    pub seven_day_opus: Option<UsageMetric>,
    pub seven_day_oauth_apps: Option<UsageMetric>,
    /// Any additional API fields we don't know about yet
    pub extra: HashMap<String, UsageMetric>,
    /// Subscription type from credentials (e.g. "max", "pro")
    pub subscription_type: Option<String>,
    /// Rate limit tier from credentials (e.g. "default_claude_max_5x")
    pub rate_limit_tier: Option<String>,
}

impl UsageResponse {
    /// Detected plan display name, using rate_limit_tier for specifics.
    /// Examples: "Max 5x", "Max 20x", "Pro", "Max"
    pub fn detected_plan(&self) -> String {
        // Try to parse rate_limit_tier first (most specific)
        if let Some(tier) = &self.rate_limit_tier {
            if let Some(plan) = format_tier(tier) {
                return plan;
            }
        }
        // Fall back to subscription_type
        if let Some(sub) = &self.subscription_type {
            return match sub.as_str() {
                "max" => "Max".to_string(),
                "pro" => "Pro".to_string(),
                other => other.to_string(),
            };
        }
        // Fall back to metric-based detection
        if self.seven_day_opus.is_some() {
            "Max".to_string()
        } else {
            "Pro".to_string()
        }
    }

    /// All non-null metrics as (key, metric) pairs, in display order.
    /// Known metrics come first in a fixed order, then extras alphabetically.
    pub fn all_metrics(&self) -> Vec<(String, &UsageMetric)> {
        let mut metrics = Vec::new();

        if let Some(m) = &self.five_hour {
            metrics.push(("five_hour".to_string(), m));
        }
        if let Some(m) = &self.seven_day {
            metrics.push(("seven_day".to_string(), m));
        }
        if let Some(m) = &self.seven_day_sonnet {
            metrics.push(("seven_day_sonnet".to_string(), m));
        }
        if let Some(m) = &self.seven_day_opus {
            metrics.push(("seven_day_opus".to_string(), m));
        }
        if let Some(m) = &self.seven_day_oauth_apps {
            metrics.push(("seven_day_oauth_apps".to_string(), m));
        }

        let mut extra_keys: Vec<_> = self.extra.keys().cloned().collect();
        extra_keys.sort();
        for key in extra_keys {
            if let Some(m) = self.extra.get(&key) {
                metrics.push((key, m));
            }
        }

        metrics
    }

    /// Maximum utilization across all metrics. Returns None if no metrics.
    pub fn max_utilization(&self) -> Option<f64> {
        self.all_metrics()
            .iter()
            .map(|(_, m)| m.utilization)
            .fold(None, |acc, u| Some(acc.map_or(u, |a: f64| a.max(u))))
    }
}

/// Which metrics the user has chosen to hide in the dashboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricFilter {
    pub hide_extra_usage: bool,
    /// Hide per-model weekly quotas (Opus, Sonnet, Fable, …)
    pub hide_model_limits: bool,
}

impl MetricFilter {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            hide_extra_usage: !config.show_extra_usage,
            hide_model_limits: !config.show_model_limits,
        }
    }

    /// True when `key` should be omitted from the display.
    pub fn hides(&self, key: &str) -> bool {
        if self.hide_extra_usage && key == "extra_usage" {
            return true;
        }
        self.hide_model_limits && is_model_limit(key)
    }
}

/// Per-model weekly quota, e.g. "seven_day_opus" or "seven_day_fable".
/// The overall weekly total and the OAuth-apps quota are not model-scoped.
fn is_model_limit(key: &str) -> bool {
    key.starts_with("seven_day_") && key != "seven_day_oauth_apps"
}

/// Parse raw JSON Value into a UsageResponse, handling unknown fields gracefully.
fn parse_response(value: serde_json::Value) -> Result<UsageResponse, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "API response is not a JSON object".to_string())?;

    let mut resp = UsageResponse {
        five_hour: None,
        seven_day: None,
        seven_day_sonnet: None,
        seven_day_opus: None,
        seven_day_oauth_apps: None,
        extra: HashMap::new(),
        subscription_type: None,
        rate_limit_tier: None,
    };

    for (key, val) in obj {
        if val.is_null() || key == "limits" {
            continue;
        }
        // Try to parse as UsageMetric
        let metric: Option<UsageMetric> = serde_json::from_value(val.clone()).ok();
        let Some(metric) = metric else { continue };

        match key.as_str() {
            "five_hour" => resp.five_hour = Some(metric),
            "seven_day" => resp.seven_day = Some(metric),
            "seven_day_sonnet" => resp.seven_day_sonnet = Some(metric),
            "seven_day_opus" => resp.seven_day_opus = Some(metric),
            "seven_day_oauth_apps" => resp.seven_day_oauth_apps = Some(metric),
            other => {
                resp.extra.insert(other.to_string(), metric);
            }
        }
    }

    merge_limits(&mut resp, obj.get("limits"));

    Ok(resp)
}

/// Merge the newer `limits` array into the response.
///
/// The API moved per-model quotas out of top-level `seven_day_*` keys (now null)
/// into `limits`, where each entry is
/// `{kind, percent, resets_at, scope: {model: {display_name}}}`.
/// Old keys win when both are present, so existing responses are unaffected.
fn merge_limits(resp: &mut UsageResponse, limits: Option<&serde_json::Value>) {
    let Some(entries) = limits.and_then(|v| v.as_array()) else {
        return;
    };

    for entry in entries {
        let Some(utilization) = entry.get("percent").and_then(|p| p.as_f64()) else {
            continue;
        };
        let metric = UsageMetric {
            utilization,
            resets_at: entry
                .get("resets_at")
                .and_then(|r| r.as_str())
                .map(str::to_string),
        };

        match entry.get("kind").and_then(|k| k.as_str()) {
            Some("session") => {
                if resp.five_hour.is_none() {
                    resp.five_hour = Some(metric);
                }
            }
            Some("weekly_all") => {
                if resp.seven_day.is_none() {
                    resp.seven_day = Some(metric);
                }
            }
            Some("weekly_scoped") => {
                let model = entry
                    .get("scope")
                    .and_then(|s| s.get("model"))
                    .and_then(|m| m.get("display_name"))
                    .and_then(|n| n.as_str());
                let Some(model) = model else { continue };
                let key = format!("seven_day_{}", slugify_model(model));

                // Don't shadow a known key that the old format already filled in.
                let known = match key.as_str() {
                    "seven_day_sonnet" => resp.seven_day_sonnet.is_some(),
                    "seven_day_opus" => resp.seven_day_opus.is_some(),
                    _ => false,
                };
                if known {
                    continue;
                }
                match key.as_str() {
                    "seven_day_sonnet" => resp.seven_day_sonnet = Some(metric),
                    "seven_day_opus" => resp.seven_day_opus = Some(metric),
                    _ => {
                        resp.extra.entry(key).or_insert(metric);
                    }
                }
            }
            _ => {}
        }
    }
}

/// "Fable" → "fable", "Claude Opus 4.8" → "claude_opus_4_8"
fn slugify_model(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

pub struct ClaudeClient {
    client: reqwest::Client,
}

impl ClaudeClient {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    pub async fn fetch_usage(&self, token: &str) -> Result<UsageResponse, String> {
        let url = format!("{}?t={}", USAGE_API_URL, cache_buster());
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .header("anthropic-beta", ANTHROPIC_BETA)
            .header("Cache-Control", "no-cache, no-store, max-age=0")
            .header("Pragma", "no-cache")
            .send()
            .await
            .map_err(|e| format!("[network_error] {e}"))?;

        let status = response.status();
        if status.as_u16() == 429 {
            let retry_secs: u64 = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(90)
                .max(90);
            return Err(format!("[rate_limited] Retry after {retry_secs}s"));
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err("[token_expired] OAuth token expired or revoked".to_string());
        }
        if status.is_server_error() {
            return Err(format!("[server_error] API returned {status}"));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("[api_error] API returned {status}: {body}"));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response JSON: {e}"))?;

        parse_response(value)
    }

    /// Fetch usage from claude.ai web API (fallback when OAuth is unavailable).
    /// Uses session cookie authentication.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub async fn fetch_usage_web(
        &self,
        session_key: &str,
        org_id: &str,
    ) -> Result<UsageResponse, String> {
        let url = format!(
            "https://claude.ai/api/organizations/{}/usage?t={}",
            org_id,
            cache_buster()
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .header("Cookie", format!("sessionKey={}", session_key))
            .header("Cache-Control", "no-cache, no-store, max-age=0")
            .header("Pragma", "no-cache")
            .send()
            .await
            .map_err(|e| format!("[network_error] {e}"))?;

        let status = response.status();
        if status.as_u16() == 429 {
            return Err("[rate_limited] Web API rate limited".to_string());
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err("[web_auth_failed] Session key expired or invalid".to_string());
        }
        if !status.is_success() {
            return Err(format!("[web_api_error] Web API returned {status}"));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse web API response: {e}"))?;

        parse_web_response(value)
    }
}

fn cache_buster() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Parse the claude.ai web API response into UsageResponse.
/// The web API may return a different structure than the OAuth API.
#[cfg_attr(not(windows), allow(dead_code))] // used by fetch_usage_web on the Windows app
fn parse_web_response(value: serde_json::Value) -> Result<UsageResponse, String> {
    // The web API may return the same structure as OAuth, or a wrapper.
    // Try direct parse first (same format as OAuth API).
    if let Some(obj) = value.as_object() {
        if obj.contains_key("five_hour") || obj.contains_key("seven_day") {
            return parse_response(value);
        }
    }

    // Try wrapped format: {"usage": {...}} or similar
    if let Some(usage_obj) = value.get("usage") {
        return parse_response(usage_obj.clone());
    }

    // Fall back to trying direct parse anyway
    parse_response(value)
}

/// Parse rate_limit_tier into a human-readable plan name.
/// Examples: "default_claude_max_5x" → "Max 5x", "default_claude_pro" → "Pro"
fn format_tier(tier: &str) -> Option<String> {
    let tier = tier.strip_prefix("default_claude_").unwrap_or(tier);
    match tier {
        "pro" => Some("Pro".to_string()),
        "max" => Some("Max".to_string()),
        other => {
            // "max_5x" → "Max 5x", "max_20x" → "Max 20x"
            if let Some(suffix) = other.strip_prefix("max_") {
                Some(format!("Max {}", suffix.to_uppercase()))
            } else {
                other
                    .strip_prefix("pro_")
                    .map(|suffix| format!("Pro {}", suffix.to_uppercase()))
            }
        }
    }
}

/// "fable" → "Fable", "claude_opus" → "Claude Opus"
pub fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format an API field name into a human-readable display name.
/// Examples: "five_hour" → "5-hour session", "seven_day_sonnet" → "Sonnet (7-day)"
pub fn format_metric_name(key: &str) -> String {
    match key {
        "five_hour" => "5-hour session".to_string(),
        "seven_day" => "Weekly (7-day)".to_string(),
        "seven_day_sonnet" => "Sonnet (7-day)".to_string(),
        "seven_day_opus" => "Opus (7-day)".to_string(),
        "seven_day_oauth_apps" => "OAuth Apps (7-day)".to_string(),
        "extra_usage" => "Extra Usage".to_string(),
        other => {
            // Model-scoped weekly quotas from the `limits` array: "seven_day_fable" → "Fable (7-day)"
            if let Some(model) = other.strip_prefix("seven_day_") {
                return format!("{} (7-day)", title_case(model));
            }
            // Title-case with spaces for unknown fields
            title_case(other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_known_fields() {
        let json = serde_json::json!({
            "five_hour": {"utilization": 62.0, "resets_at": "2025-11-04T04:59:59+00:00"},
            "seven_day": {"utilization": 28.0, "resets_at": null},
            "seven_day_sonnet": null,
            "seven_day_opus": {"utilization": 8.0, "resets_at": null}
        });
        let resp = parse_response(json).unwrap();
        assert_eq!(resp.five_hour.as_ref().unwrap().utilization, 62.0);
        assert_eq!(resp.seven_day.as_ref().unwrap().utilization, 28.0);
        assert!(resp.seven_day_sonnet.is_none());
        assert_eq!(resp.seven_day_opus.as_ref().unwrap().utilization, 8.0);
        assert_eq!(resp.detected_plan(), "Max");
    }

    #[test]
    fn test_parse_unknown_fields() {
        let json = serde_json::json!({
            "five_hour": {"utilization": 10.0, "resets_at": null},
            "iguana_necktie": {"utilization": 42.0, "resets_at": null}
        });
        let resp = parse_response(json).unwrap();
        assert!(resp.extra.contains_key("iguana_necktie"));
        assert_eq!(resp.extra["iguana_necktie"].utilization, 42.0);
    }

    #[test]
    fn test_max_utilization() {
        let json = serde_json::json!({
            "five_hour": {"utilization": 62.0, "resets_at": null},
            "seven_day": {"utilization": 28.0, "resets_at": null}
        });
        let resp = parse_response(json).unwrap();
        assert_eq!(resp.max_utilization(), Some(62.0));
    }

    #[test]
    fn test_parse_extra_usage() {
        let json = serde_json::json!({
            "five_hour": {"utilization": 10.0, "resets_at": null},
            "extra_usage": {"is_enabled": true, "monthly_limit": 5000, "used_credits": 1125.0, "utilization": 22.5}
        });
        let resp = parse_response(json).unwrap();
        assert!(resp.extra.contains_key("extra_usage"));
        assert_eq!(resp.extra["extra_usage"].utilization, 22.5);
        assert_eq!(resp.extra["extra_usage"].resets_at, None);
    }

    #[test]
    fn test_format_metric_name() {
        assert_eq!(format_metric_name("five_hour"), "5-hour session");
        assert_eq!(format_metric_name("seven_day"), "Weekly (7-day)");
        assert_eq!(format_metric_name("extra_usage"), "Extra Usage");
        assert_eq!(format_metric_name("iguana_necktie"), "Iguana Necktie");
    }

    #[test]
    fn test_parse_limits_array() {
        let json = serde_json::json!({
            "seven_day_opus": null,
            "seven_day_sonnet": null,
            "limits": [
                {"kind": "session", "percent": 10, "resets_at": "2026-07-20T22:49:59+00:00"},
                {"kind": "weekly_all", "percent": 13, "resets_at": null},
                {"kind": "weekly_scoped", "percent": 5, "resets_at": null,
                 "scope": {"model": {"id": null, "display_name": "Fable"}}}
            ]
        });
        let resp = parse_response(json).unwrap();
        assert_eq!(resp.five_hour.as_ref().unwrap().utilization, 10.0);
        assert_eq!(
            resp.five_hour.as_ref().unwrap().resets_at.as_deref(),
            Some("2026-07-20T22:49:59+00:00")
        );
        assert_eq!(resp.seven_day.as_ref().unwrap().utilization, 13.0);
        assert_eq!(resp.extra["seven_day_fable"].utilization, 5.0);
        assert_eq!(resp.max_utilization(), Some(13.0));
    }

    #[test]
    fn test_limits_do_not_override_legacy_keys() {
        let json = serde_json::json!({
            "five_hour": {"utilization": 62.0, "resets_at": null},
            "seven_day_opus": {"utilization": 8.0, "resets_at": null},
            "limits": [
                {"kind": "session", "percent": 99, "resets_at": null},
                {"kind": "weekly_scoped", "percent": 99, "resets_at": null,
                 "scope": {"model": {"display_name": "Opus"}}}
            ]
        });
        let resp = parse_response(json).unwrap();
        assert_eq!(resp.five_hour.as_ref().unwrap().utilization, 62.0);
        assert_eq!(resp.seven_day_opus.as_ref().unwrap().utilization, 8.0);
        assert!(resp.extra.is_empty());
    }

    #[test]
    fn test_limits_fill_legacy_model_keys_when_absent() {
        let json = serde_json::json!({
            "limits": [
                {"kind": "weekly_scoped", "percent": 42, "resets_at": null,
                 "scope": {"model": {"display_name": "Sonnet"}}}
            ]
        });
        let resp = parse_response(json).unwrap();
        assert_eq!(resp.seven_day_sonnet.as_ref().unwrap().utilization, 42.0);
        assert!(resp.extra.is_empty());
    }

    #[test]
    fn test_metric_filter() {
        let show_all = MetricFilter::default();
        assert!(!show_all.hides("seven_day_fable"));
        assert!(!show_all.hides("extra_usage"));

        let hide_models = MetricFilter {
            hide_extra_usage: false,
            hide_model_limits: true,
        };
        assert!(hide_models.hides("seven_day_fable"));
        assert!(hide_models.hides("seven_day_opus"));
        // Not model-scoped — must stay visible.
        assert!(!hide_models.hides("seven_day"));
        assert!(!hide_models.hides("seven_day_oauth_apps"));
        assert!(!hide_models.hides("five_hour"));
        assert!(!hide_models.hides("extra_usage"));
    }

    #[test]
    fn test_format_scoped_metric_name() {
        assert_eq!(format_metric_name("seven_day_fable"), "Fable (7-day)");
    }

    #[test]
    fn test_format_tier() {
        assert_eq!(
            format_tier("default_claude_max_5x"),
            Some("Max 5X".to_string())
        );
        assert_eq!(
            format_tier("default_claude_max_20x"),
            Some("Max 20X".to_string())
        );
        assert_eq!(format_tier("default_claude_pro"), Some("Pro".to_string()));
        assert_eq!(format_tier("default_claude_max"), Some("Max".to_string()));
        assert_eq!(format_tier("unknown_thing"), None);
    }
}
