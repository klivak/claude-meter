use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

const USAGE_API_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageMetric {
    pub utilization: f64,
    #[serde(default)]
    pub resets_at: Option<String>,
    /// Dollar budget, when the limit is denominated in money.
    #[serde(default)]
    pub limit_dollars: Option<f64>,
    #[serde(default)]
    pub used_dollars: Option<f64>,
    /// Why the API locked this limit, if it did.
    #[serde(default)]
    pub locked_reason: Option<String>,
    /// `is_active: false` in the `limits` array: shown, but never alerts.
    #[serde(default)]
    pub inactive: bool,
    /// Share of the overall weekly limit this model may use (0-100).
    #[serde(default)]
    pub share_of_weekly: Option<f64>,
    /// Weekly percent spent in the window, on the local estimate.
    #[serde(default)]
    pub estimated_weekly_spend: Option<f64>,
    /// Weekly percent that fills the local estimate.
    #[serde(default)]
    pub estimated_budget: Option<f64>,
}

/// Key of the local usage estimate. Never sent by the API and never stored.
pub const SESSION_ESTIMATE_KEY: &str = "session_estimate";
const HOURS_PER_WEEK: f64 = 168.0;

#[derive(Default)]
struct UserOverrides {
    metrics: HashMap<String, crate::config::MetricOverride>,
    plans: HashMap<String, String>,
    estimate_hours: u32,
}

static USER_OVERRIDES: RwLock<Option<UserOverrides>> = RwLock::new(None);

/// Apply the config's corrections. Called on startup and after every poll, so
/// editing config.json takes effect without a restart.
pub fn set_user_overrides(config: &crate::config::Config) {
    if let Ok(mut slot) = USER_OVERRIDES.write() {
        *slot = Some(UserOverrides {
            metrics: config.metric_overrides.clone(),
            plans: config.plan_aliases.clone(),
            estimate_hours: config.session_estimate_hours,
        });
    }
}

fn metric_override<T>(
    key: &str,
    read: impl Fn(&crate::config::MetricOverride) -> Option<T>,
) -> Option<T> {
    let slot = USER_OVERRIDES.read().ok()?;
    read(slot.as_ref()?.metrics.get(key)?)
}

fn plan_alias(raw: &str) -> Option<String> {
    let slot = USER_OVERRIDES.read().ok()?;
    slot.as_ref()?.plans.get(raw).cloned()
}

/// Configured estimate window in hours, for its display name.
pub fn session_estimate_hours() -> u32 {
    USER_OVERRIDES
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().map(|o| o.estimate_hours))
        .unwrap_or(24)
}

/// Weekly percent that fills the estimate: the manual value, or an even share
/// of the week. The week is 100%, so one day is 100/7 = 14.3%.
pub fn session_estimate_budget(hours: u32, manual: Option<f64>) -> f64 {
    manual
        .filter(|b| *b > 0.0)
        .unwrap_or(hours.max(1) as f64 * 100.0 / HOURS_PER_WEEK)
}

/// Local stand-in for the session limit on accounts without one: weekly percent
/// spent over the last `hours` against `budget`. `samples` are (epoch seconds,
/// weekly percent) in time order. Drops from a weekly reset don't count.
pub fn estimate_session(samples: &[(i64, f64)], now: i64, hours: u32, budget: f64) -> UsageMetric {
    let window_secs = hours.max(1) as i64 * 3600;
    let mut spent = 0.0;
    let mut first_rise = None;
    for pair in samples.windows(2) {
        let (earlier, later) = (pair[0], pair[1]);
        if later.0 > now - window_secs && later.1 > earlier.1 {
            spent += later.1 - earlier.1;
            first_rise.get_or_insert(later.0);
        }
    }
    UsageMetric {
        utilization: spent / budget * 100.0,
        resets_at: first_rise
            .and_then(|t| chrono::DateTime::from_timestamp(t + window_secs, 0))
            .map(|t| t.to_rfc3339()),
        estimated_weekly_spend: Some(spent),
        estimated_budget: Some(budget),
        ..Default::default()
    }
}

/// Saved value that must not be shown at startup: a past window of a session or
/// a promotion that may be over. The next poll restores whatever is still live.
#[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
pub fn is_expired_cache(
    key: &str,
    resets_at: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    if key == SESSION_ESTIMATE_KEY {
        return true;
    }
    if key != "five_hour" && is_core_limit(key) {
        return false;
    }
    resets_at
        .and_then(|r| chrono::DateTime::parse_from_rfc3339(r).ok())
        .is_some_and(|reset| reset < now)
}

impl UsageMetric {
    /// True when `detail_text` has something to show.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn has_detail(&self) -> bool {
        self.locked_reason.is_some()
            || self.estimated_weekly_spend.is_some()
            || (self.used_dollars.is_some() && self.limit_dollars.is_some())
            || self.share_of_weekly.is_some()
    }

    /// Secondary line under the bar: lock reason, dollar amounts, weekly share.
    pub fn detail_text(&self, i18n: &crate::i18n::I18n) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(reason) = &self.locked_reason {
            parts.push(format!("{}: {}", i18n.t("Limit locked"), reason));
        }
        if let (Some(used), Some(limit)) = (self.used_dollars, self.limit_dollars) {
            parts.push(format!(
                "{} / {}",
                format_dollars(used),
                format_dollars(limit)
            ));
        }
        if let Some(share) = self.share_of_weekly {
            parts.push(
                i18n.t("share_of_weekly")
                    .replace("{0}", &format!("{share:.0}")),
            );
        }
        if let (Some(spent), Some(budget)) = (self.estimated_weekly_spend, self.estimated_budget) {
            parts.push(
                i18n.t("estimate_note")
                    .replace("{0}", &format!("{spent:.0}"))
                    .replace("{1}", &format!("{budget:.0}")),
            );
        }
        (!parts.is_empty()).then(|| parts.join(" \u{00B7} "))
    }
}

/// "$249.89", and "$250" for whole amounts.
fn format_dollars(amount: f64) -> String {
    if (amount - amount.round()).abs() < 0.005 {
        format!("${}", amount.round() as i64)
    } else {
        format!("${amount:.2}")
    }
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
    /// Credentials may still carry the pre-rename tiers, so `max_20x` is Ultra
    /// and `max_5x` is Max. Examples: "Pro", "Max", "Ultra"
    pub fn detected_plan(&self) -> String {
        let raw_values = [&self.rate_limit_tier, &self.subscription_type];
        if let Some(alias) = raw_values
            .into_iter()
            .flatten()
            .find_map(|raw| plan_alias(raw))
        {
            return alias;
        }
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
                other => title_case(other),
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

        if let Some(m) = self.extra.get(SESSION_ESTIMATE_KEY) {
            metrics.push((SESSION_ESTIMATE_KEY.to_string(), m));
        }
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
            if key == SESSION_ESTIMATE_KEY {
                continue;
            }
            if let Some(m) = self.extra.get(&key) {
                metrics.push((key, m));
            }
        }

        metrics
    }

    /// Metric shown on the tray icon and widget: the running session, or the
    /// weekly limit on plans without a session window. None while a session
    /// window exists but is idle (no reset time yet).
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn display_metric(&self) -> Option<&UsageMetric> {
        match &self.five_hour {
            Some(session) => session.resets_at.is_some().then_some(session),
            None => self.seven_day.as_ref(),
        }
    }

    /// Metric whose history the charts plot. Follows the data, not the plan,
    /// because an unverified Ultra account still has a temporary session limit.
    pub fn chart_metric_key(&self) -> &'static str {
        if self.five_hour.is_some() {
            "five_hour"
        } else {
            "seven_day"
        }
    }

    /// Add the local estimate when the API reports no session window. Follows
    /// the data, so a temporary session limit replaces it automatically.
    pub fn apply_session_estimate(
        &mut self,
        samples: &[(i64, f64)],
        now: i64,
        hours: u32,
        budget: Option<f64>,
    ) {
        if self.five_hour.is_some() {
            return;
        }
        let Some(weekly) = &self.seven_day else {
            return;
        };
        let mut samples = samples.to_vec();
        samples.push((now, weekly.utilization));
        let budget = session_estimate_budget(hours, budget);
        let estimate = estimate_session(&samples, now, hours, budget);
        self.extra
            .insert(SESSION_ESTIMATE_KEY.to_string(), estimate);
    }

    /// Maximum utilization across the limits that can actually block the user.
    /// Returns None if there are none.
    pub fn max_utilization(&self) -> Option<f64> {
        self.all_metrics()
            .iter()
            .filter(|(key, m)| is_core_limit(key) && !m.inactive)
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
        if metric_override(key, |o| o.hidden) == Some(true) {
            return true;
        }
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

/// Session, weekly and per-model limits. Unknown keys (the API uses codenames
/// such as `iguana_necktie`) and `extra_usage` are displayed, but they don't
/// drive the icon or notifications because their meaning is not documented.
pub fn is_core_limit(key: &str) -> bool {
    if let Some(alerts) = metric_override(key, |o| o.alerts) {
        return alerts;
    }
    key == "five_hour" || key == "seven_day" || key.starts_with("seven_day_")
}

/// Monthly API credits included with a plan, in USD. Team is pooled per seat,
/// so it has no single figure. These credits don't cover Claude Code.
#[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
pub fn api_credits_usd(plan: &str) -> Option<u32> {
    match plan {
        "Max" => Some(100),
        "Ultra" => Some(200),
        _ => None,
    }
}

/// Map pre-rename plan labels from existing configs to the current names.
#[cfg_attr(windows, allow(dead_code))] // consumed by the macOS app
pub fn normalize_plan_label(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.contains("20x") {
        "Ultra".to_string()
    } else if lower.contains("5x") {
        "Max".to_string()
    } else {
        label.to_string()
    }
}

/// Read one metric leniently: only `utilization` is required, so a surprising
/// type in a newer optional field can't hide a limit.
fn parse_metric(val: &serde_json::Value) -> Option<UsageMetric> {
    let money = |key: &str| val.get(key).and_then(|v| v.as_f64());
    Some(UsageMetric {
        utilization: val.get("utilization")?.as_f64()?,
        resets_at: val
            .get("resets_at")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        limit_dollars: money("limit_dollars"),
        used_dollars: money("used_dollars"),
        locked_reason: val.get("locked_reason").and_then(json_text),
        ..Default::default()
    })
}

fn json_text(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn is_usd(currency: Option<&serde_json::Value>) -> bool {
    currency.and_then(|c| c.as_str()).unwrap_or("USD") == "USD"
}

/// `extra_usage` amounts are minor units: 1125 with 2 decimals is $11.25.
fn fill_extra_usage_amounts(metric: &mut UsageMetric, val: &serde_json::Value) {
    if !is_usd(val.get("currency")) {
        return;
    }
    let decimals = val
        .get("decimal_places")
        .and_then(|d| d.as_i64())
        .unwrap_or(2) as i32;
    let scale = 10f64.powi(decimals);
    metric.used_dollars = val
        .get("used_credits")
        .and_then(|v| v.as_f64())
        .map(|v| v / scale);
    metric.limit_dollars = val
        .get("monthly_limit")
        .and_then(|v| v.as_f64())
        .map(|v| v / scale);
}

/// Usage credits from the newer `spend` block, for responses that no longer
/// fill `extra_usage`.
fn merge_spend(resp: &mut UsageResponse, spend: Option<&serde_json::Value>) {
    let Some(spend) = spend else { return };
    if resp.extra.contains_key("extra_usage")
        || spend.get("enabled").and_then(|e| e.as_bool()) != Some(true)
    {
        return;
    }
    let Some(utilization) = spend.get("percent").and_then(|p| p.as_f64()) else {
        return;
    };
    let amount = |key: &str| -> Option<f64> {
        let money = spend.get(key)?;
        if !is_usd(money.get("currency")) {
            return None;
        }
        let minor = money.get("amount_minor")?.as_f64()?;
        let exponent = money.get("exponent").and_then(|e| e.as_i64()).unwrap_or(2) as i32;
        Some(minor / 10f64.powi(exponent))
    };
    let metric = UsageMetric {
        utilization,
        used_dollars: amount("used"),
        limit_dollars: amount("limit"),
        ..Default::default()
    };
    resp.extra.insert("extra_usage".to_string(), metric);
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
        // Objects without a numeric utilization (spend, pace, ...) are not metrics.
        let Some(mut metric) = parse_metric(val) else {
            continue;
        };
        if key == "extra_usage" {
            fill_extra_usage_amounts(&mut metric, val);
        }

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

    merge_limits(
        &mut resp,
        obj.get("limits"),
        obj.get("weekly_scoped_shares"),
    );
    merge_spend(&mut resp, obj.get("spend"));

    Ok(resp)
}

/// Merge the newer `limits` array into the response.
///
/// The API moved per-model quotas out of top-level `seven_day_*` keys (now null)
/// into `limits`, where each entry is
/// `{kind, percent, resets_at, scope: {model: {display_name}}}`.
/// Old keys win when both are present, so existing responses are unaffected.
///
/// `weekly_scoped_shares` points back into `limits` by `limit_index`.
fn merge_limits(
    resp: &mut UsageResponse,
    limits: Option<&serde_json::Value>,
    shares: Option<&serde_json::Value>,
) {
    let Some(entries) = limits.and_then(|v| v.as_array()) else {
        return;
    };
    let share_for = |index: usize| -> Option<f64> {
        shares?.as_array()?.iter().find_map(|share| {
            if share.get("limit_index")?.as_u64()? == index as u64 {
                share.get("allowance_percent_of_weekly")?.as_f64()
            } else {
                None
            }
        })
    };

    for (index, entry) in entries.iter().enumerate() {
        let Some(utilization) = entry.get("percent").and_then(|p| p.as_f64()) else {
            continue;
        };
        let metric = UsageMetric {
            utilization,
            resets_at: entry
                .get("resets_at")
                .and_then(|r| r.as_str())
                .map(str::to_string),
            inactive: entry.get("is_active").and_then(|a| a.as_bool()) == Some(false),
            share_of_weekly: share_for(index),
            ..Default::default()
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
                // Scoped to a model, or to a surface in newer responses.
                let scope = entry.get("scope");
                let display_name = |part: &str| {
                    scope
                        .and_then(|s| s.get(part))
                        .and_then(|m| m.get("display_name"))
                        .and_then(|n| n.as_str())
                };
                let Some(model) = display_name("model").or_else(|| display_name("surface")) else {
                    continue;
                };
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
            // A new kind stays visible as an additional limit until it is known.
            Some(kind) => {
                resp.extra.entry(slugify_model(kind)).or_insert(metric);
            }
            None => {}
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
/// Examples: "default_claude_max_20x" → "Ultra", "default_claude_max_5x" → "Max"
fn format_tier(tier: &str) -> Option<String> {
    let tier = tier.strip_prefix("default_claude_").unwrap_or(tier);
    match tier {
        "pro" => Some("Pro".to_string()),
        "max" | "max_5x" => Some("Max".to_string()),
        "ultra" | "max_20x" => Some("Ultra".to_string()),
        other => {
            if let Some(suffix) = other.strip_prefix("max_") {
                Some(format!("Max {}", suffix.to_uppercase()))
            } else if let Some(suffix) = other.strip_prefix("ultra_") {
                Some(format!("Ultra {}", suffix.to_uppercase()))
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
    if let Some(label) = metric_override(key, |o| o.label.clone()) {
        return label;
    }
    match key {
        SESSION_ESTIMATE_KEY => format!("{}h (estimate)", session_estimate_hours()),
        // Claude Code cloud sessions promotion, a one-time dollar credit.
        "iguana_necktie" => "Cloud credit".to_string(),
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
            // Unknown fields are internal codenames; their names mean nothing to users.
            "Additional limit".to_string()
        }
    }
}

/// Parsed response for tests in other modules.
#[cfg(test)]
pub fn test_usage(value: serde_json::Value) -> UsageResponse {
    parse_response(value).unwrap()
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
        assert_eq!(format_metric_name("iguana_necktie"), "Cloud credit");
        assert_eq!(format_metric_name("amber_ladder"), "Additional limit");
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
            Some("Max".to_string())
        );
        assert_eq!(
            format_tier("default_claude_max_20x"),
            Some("Ultra".to_string())
        );
        assert_eq!(
            format_tier("default_claude_ultra"),
            Some("Ultra".to_string())
        );
        assert_eq!(format_tier("default_claude_pro"), Some("Pro".to_string()));
        assert_eq!(format_tier("default_claude_max"), Some("Max".to_string()));
        assert_eq!(format_tier("unknown_thing"), None);
    }

    /// Trimmed copy of a live response from an Ultra account (2026-10-31).
    fn ultra_response() -> serde_json::Value {
        serde_json::json!({
            "five_hour": null,
            "seven_day": {"utilization": 0.0, "resets_at": "2026-11-06T19:00:00.336491+00:00",
                "limit_dollars": null, "used_dollars": null, "remaining_dollars": null, "locked_reason": null},
            "seven_day_opus": null,
            "tangelo": null,
            "iguana_necktie": {"utilization": 99.95605, "resets_at": "2026-11-05T07:59:00+00:00",
                "limit_dollars": 250, "used_dollars": 249.890125, "remaining_dollars": 0.109875, "locked_reason": null},
            "pace": null,
            "extra_usage": {"is_enabled": false, "monthly_limit": null, "used_credits": null, "utilization": null,
                "currency": null, "decimal_places": null, "user_disabled": true},
            "limits": [
                {"kind": "weekly_all", "group": "weekly", "percent": 0, "severity": "normal",
                 "resets_at": "2026-11-06T19:00:00.336491+00:00", "scope": null, "is_active": true},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 0, "severity": "normal",
                 "resets_at": "2026-11-06T19:00:00+00:00", "is_active": false,
                 "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null}}
            ],
            "spend": {"used": {"amount_minor": 0, "currency": "USD", "exponent": 2}, "limit": null,
                "percent": 0, "severity": "normal", "enabled": false, "balance": null},
            "member_dashboard_available": false,
            "seven_day_breakdown": null,
            "weekly_scoped_shares": [
                {"limit_index": 1, "used_percent_of_weekly": 0, "allowance_percent_of_weekly": 50}
            ]
        })
    }

    fn english() -> crate::i18n::I18n {
        crate::i18n::I18n::new(crate::i18n::Locale::En)
    }

    #[test]
    fn test_ultra_response_is_not_alarmed_by_unknown_limit() {
        let resp = parse_response(ultra_response()).unwrap();
        assert!(resp.five_hour.is_none());
        assert_eq!(resp.extra["iguana_necktie"].utilization, 99.95605);
        // The undocumented 99.96% limit must not drive the icon.
        assert_eq!(resp.max_utilization(), Some(0.0));
        assert_eq!(resp.display_metric().unwrap().utilization, 0.0);
        assert_eq!(resp.chart_metric_key(), "seven_day");
        assert!(!resp.extra.contains_key("extra_usage"));
    }

    #[test]
    fn test_ultra_response_details() {
        let resp = parse_response(ultra_response()).unwrap();
        let i18n = english();
        assert_eq!(
            resp.extra["iguana_necktie"].detail_text(&i18n).as_deref(),
            Some("$249.89 / $250")
        );
        let fable = &resp.extra["seven_day_fable"];
        assert!(fable.inactive);
        assert_eq!(fable.share_of_weekly, Some(50.0));
        assert!(fable.detail_text(&i18n).unwrap().contains("50%"));
        assert!(!resp.seven_day.as_ref().unwrap().has_detail());
    }

    #[test]
    fn test_inactive_limit_does_not_alert() {
        let resp = parse_response(serde_json::json!({
            "limits": [
                {"kind": "weekly_all", "percent": 10, "is_active": true},
                {"kind": "weekly_scoped", "percent": 95, "is_active": false,
                 "scope": {"model": {"display_name": "Fable"}}}
            ]
        }))
        .unwrap();
        assert_eq!(resp.max_utilization(), Some(10.0));
    }

    #[test]
    fn test_temporary_session_limit_stays_primary() {
        // An unverified Ultra account still reports a session window.
        let resp = parse_response(serde_json::json!({
            "five_hour": {"utilization": 40.0, "resets_at": "2026-11-01T10:00:00+00:00"},
            "seven_day": {"utilization": 5.0, "resets_at": "2026-11-06T10:00:00+00:00"}
        }))
        .unwrap();
        assert_eq!(resp.display_metric().unwrap().utilization, 40.0);
        assert_eq!(resp.chart_metric_key(), "five_hour");
    }

    #[test]
    fn test_idle_session_shows_no_primary_value() {
        let resp = parse_response(serde_json::json!({
            "five_hour": {"utilization": 0.0, "resets_at": null},
            "seven_day": {"utilization": 30.0, "resets_at": "2026-11-06T10:00:00+00:00"}
        }))
        .unwrap();
        assert!(resp.display_metric().is_none());
        assert_eq!(resp.chart_metric_key(), "five_hour");
    }

    #[test]
    fn test_surprising_optional_field_keeps_the_metric() {
        let resp = parse_response(serde_json::json!({
            "seven_day": {"utilization": 12.0, "resets_at": 123, "limit_dollars": "n/a",
                "locked_reason": {"code": "fraud"}}
        }))
        .unwrap();
        let weekly = resp.seven_day.unwrap();
        assert_eq!(weekly.utilization, 12.0);
        assert_eq!(weekly.resets_at, None);
        assert_eq!(weekly.limit_dollars, None);
        assert_eq!(weekly.locked_reason.as_deref(), Some(r#"{"code":"fraud"}"#));
    }

    #[test]
    fn test_extra_usage_amounts_in_minor_units() {
        let resp = parse_response(serde_json::json!({
            "extra_usage": {"is_enabled": true, "monthly_limit": 5000, "used_credits": 1125.0,
                "utilization": 22.5, "currency": "USD", "decimal_places": 2}
        }))
        .unwrap();
        assert_eq!(
            resp.extra["extra_usage"].detail_text(&english()).as_deref(),
            Some("$11.25 / $50")
        );
        // Usage credits are not a plan limit.
        assert_eq!(resp.max_utilization(), None);
    }

    #[test]
    fn test_non_usd_amounts_are_not_shown_as_dollars() {
        let resp = parse_response(serde_json::json!({
            "extra_usage": {"monthly_limit": 5000, "used_credits": 1125.0,
                "utilization": 22.5, "currency": "EUR", "decimal_places": 2}
        }))
        .unwrap();
        assert!(!resp.extra["extra_usage"].has_detail());
    }

    #[test]
    fn test_spend_fills_extra_usage_when_enabled() {
        let resp = parse_response(serde_json::json!({
            "spend": {"enabled": true, "percent": 25,
                "used": {"amount_minor": 1250, "currency": "USD", "exponent": 2},
                "limit": {"amount_minor": 5000, "currency": "USD", "exponent": 2}}
        }))
        .unwrap();
        let credits = &resp.extra["extra_usage"];
        assert_eq!(credits.utilization, 25.0);
        assert_eq!(credits.used_dollars, Some(12.5));
        assert_eq!(credits.limit_dollars, Some(50.0));
    }

    #[test]
    fn test_plan_names_and_api_credits() {
        let plan = |tier: Option<&str>, sub: Option<&str>| {
            let mut resp = parse_response(serde_json::json!({})).unwrap();
            resp.rate_limit_tier = tier.map(str::to_string);
            resp.subscription_type = sub.map(str::to_string);
            resp.detected_plan()
        };
        assert_eq!(plan(Some("default_claude_max_20x"), Some("max")), "Ultra");
        assert_eq!(plan(Some("default_claude_max_5x"), Some("max")), "Max");
        assert_eq!(plan(None, Some("ultra")), "Ultra");
        assert_eq!(plan(None, Some("team")), "Team");
        assert_eq!(api_credits_usd("Ultra"), Some(200));
        assert_eq!(api_credits_usd("Max"), Some(100));
        assert_eq!(api_credits_usd("Pro"), None);
        assert_eq!(normalize_plan_label("Max 20x"), "Ultra");
        assert_eq!(normalize_plan_label("Max 5x"), "Max");
        assert_eq!(normalize_plan_label("Pro"), "Pro");
    }

    #[test]
    fn test_session_estimate_counts_only_rises_inside_the_window() {
        let now = 100_000;
        let hour = 3600;
        let samples = [
            (now - 7 * hour, 10.0),
            (now - 6 * hour, 12.0), // before the window
            (now - 4 * hour, 13.0), // +1
            (now - 2 * hour, 16.0), // +3
            (now - hour, 1.0),      // weekly reset
            (now, 2.0),             // +1
        ];
        let estimate = estimate_session(&samples, now, 5, 10.0);
        assert_eq!(estimate.estimated_weekly_spend, Some(5.0));
        assert_eq!(estimate.utilization, 50.0);
        // Frees up 5 hours after the oldest rise that still counts.
        let reset = chrono::DateTime::parse_from_rfc3339(estimate.resets_at.as_deref().unwrap());
        assert_eq!(reset.unwrap().timestamp(), now - 4 * hour + 5 * hour);
    }

    #[test]
    fn test_session_estimate_budget_is_an_even_share_of_the_week() {
        let day = session_estimate_budget(24, None);
        assert!((day - 100.0 / 7.0).abs() < 1e-9);
        assert!((session_estimate_budget(168, None) - 100.0).abs() < 1e-9);
        assert_eq!(session_estimate_budget(24, Some(30.0)), 30.0);
        // 14.3% of the week within 24 hours fills the bar exactly.
        let samples = [(0, 0.0), (3600, day)];
        let estimate = estimate_session(&samples, 7200, 24, day);
        assert!((estimate.utilization - 100.0).abs() < 1e-9);
        // A longer window keeps rises that a shorter one forgets.
        let old_rise = [(0, 0.0), (3600, 5.0)];
        let now = 10 * 3600;
        assert_eq!(estimate_session(&old_rise, now, 5, 10.0).utilization, 0.0);
        assert_eq!(estimate_session(&old_rise, now, 24, 10.0).utilization, 50.0);
    }

    #[test]
    fn test_session_estimate_idle() {
        let estimate = estimate_session(&[(1, 5.0), (2, 5.0)], 2, 5, 10.0);
        assert_eq!(estimate.utilization, 0.0);
        assert_eq!(estimate.resets_at, None);
    }

    #[test]
    fn test_estimate_only_without_a_session_and_never_alerts() {
        let mut ultra = parse_response(ultra_response()).unwrap();
        ultra.apply_session_estimate(&[(0, 0.0)], 10, 24, None);
        assert_eq!(ultra.all_metrics()[0].0, SESSION_ESTIMATE_KEY);
        assert!(!is_core_limit(SESSION_ESTIMATE_KEY));
        assert_eq!(ultra.max_utilization(), Some(0.0));
        assert_eq!(
            ultra
                .all_metrics()
                .iter()
                .filter(|(k, _)| k == SESSION_ESTIMATE_KEY)
                .count(),
            1
        );

        let mut session = parse_response(serde_json::json!({
            "five_hour": {"utilization": 40.0, "resets_at": null},
            "seven_day": {"utilization": 5.0, "resets_at": null}
        }))
        .unwrap();
        session.apply_session_estimate(&[(0, 0.0)], 10, 24, None);
        assert!(!session.extra.contains_key(SESSION_ESTIMATE_KEY));
    }

    #[test]
    fn test_expired_cache_rules() {
        let now = chrono::Utc::now();
        let past = (now - chrono::Duration::hours(1)).to_rfc3339();
        let future = (now + chrono::Duration::hours(1)).to_rfc3339();
        assert!(is_expired_cache("iguana_necktie", Some(&past), now));
        assert!(!is_expired_cache("iguana_necktie", Some(&future), now));
        assert!(is_expired_cache("five_hour", Some(&past), now));
        assert!(!is_expired_cache("seven_day", Some(&past), now));
    }

    #[test]
    fn test_new_limit_kinds_stay_visible() {
        let resp = parse_response(serde_json::json!({
            "limits": [
                {"kind": "monthly_all", "percent": 77, "resets_at": null},
                {"kind": "weekly_scoped", "percent": 3,
                 "scope": {"model": null, "surface": {"display_name": "Cowork"}}}
            ]
        }))
        .unwrap();
        assert_eq!(resp.extra["monthly_all"].utilization, 77.0);
        assert_eq!(resp.extra["seven_day_cowork"].utilization, 3.0);
        assert_eq!(resp.max_utilization(), Some(3.0));
    }

    #[test]
    fn test_user_overrides_rename_and_promote() {
        let mut config = crate::config::Config::default();
        config.metric_overrides.insert(
            "unit_test_limit".to_string(),
            crate::config::MetricOverride {
                label: Some("My limit".to_string()),
                alerts: Some(true),
                hidden: Some(true),
            },
        );
        config
            .plan_aliases
            .insert("unit_test_tier".to_string(), "Galaxy".to_string());
        set_user_overrides(&config);
        assert_eq!(format_metric_name("unit_test_limit"), "My limit");
        assert!(is_core_limit("unit_test_limit"));
        assert!(MetricFilter::default().hides("unit_test_limit"));
        let mut resp = parse_response(serde_json::json!({})).unwrap();
        resp.rate_limit_tier = Some("unit_test_tier".to_string());
        assert_eq!(resp.detected_plan(), "Galaxy");
    }

    // Live check against this machine's account; prints no token.
    // Run with: cargo test live_claude_usage -- --ignored --nocapture
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn live_claude_usage() {
        let cred = crate::credentials::read_claude_token().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = ClaudeClient::new().unwrap();
        let mut resp = runtime
            .block_on(client.fetch_usage(&cred.access_token))
            .unwrap();
        resp.subscription_type = cred.subscription_type;
        resp.rate_limit_tier = cred.rate_limit_tier;
        let i18n = english();
        println!("plan: {}", resp.detected_plan());
        println!("max: {:?}", resp.max_utilization());
        println!(
            "display: {:?}",
            resp.display_metric().map(|m| m.utilization)
        );
        println!("chart: {}", resp.chart_metric_key());
        for (key, m) in resp.all_metrics() {
            println!(
                "{key} -> {}: {:.2}% | {:?} | core={} inactive={}",
                format_metric_name(&key),
                m.utilization,
                m.detail_text(&i18n),
                is_core_limit(&key),
                m.inactive
            );
        }
    }
}
