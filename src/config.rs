use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// User correction for one API metric, so a renamed or new limit can be handled
/// from config.json without waiting for a release.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricOverride {
    /// Display name shown instead of the built-in one.
    #[serde(default)]
    pub label: Option<String>,
    /// Whether it drives the icon and notifications.
    #[serde(default)]
    pub alerts: Option<bool>,
    #[serde(default)]
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub thresholds: Vec<u8>,
    pub sound: bool,
}

pub const NOTIFICATION_THRESHOLD_PRESETS: &[&[u8]] =
    &[&[50, 75, 90], &[75, 90, 100], &[90, 100], &[100]];

pub fn cycle_notification_thresholds(current: &[u8]) -> Vec<u8> {
    let index = NOTIFICATION_THRESHOLD_PRESETS
        .iter()
        .position(|preset| *preset == current)
        .unwrap_or(0);
    NOTIFICATION_THRESHOLD_PRESETS[(index + 1) % NOTIFICATION_THRESHOLD_PRESETS.len()].to_vec()
}

pub fn format_notification_thresholds(thresholds: &[u8]) -> String {
    thresholds
        .iter()
        .map(|threshold| format!("{threshold}%"))
        .collect::<Vec<_>>()
        .join(" / ")
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: vec![50, 75, 90],
            sound: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuietHoursConfig {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".to_string(),
            end: "08:00".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomColors {
    pub background: Option<String>,
    pub surface: Option<String>,
    pub text_primary: Option<String>,
    pub text_secondary: Option<String>,
    pub progress_bg: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub red: Option<String>,
    pub accent: Option<String>,
    pub separator: Option<String>,
    pub hover: Option<String>,
    pub border: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub polling_interval_seconds: u64,
    pub notifications: NotificationConfig,
    pub autostart: bool,
    pub compact_mode: bool,
    pub theme: String,
    pub language: String,
    pub show_chatgpt_section: bool,
    pub chatgpt_usage_url: String,
    pub claude_install_url: String,
    #[serde(default)]
    pub show_widget: bool,
    #[serde(default = "default_true")]
    pub check_updates: bool,
    #[serde(default)]
    pub accessibility_patterns: bool,
    #[serde(default = "default_icon_style")]
    pub tray_icon_style: String,
    #[serde(default = "default_dashboard_layout")]
    pub dashboard_layout: String,
    /// Show the `extra_usage` metric in the dashboard (off by default — it does
    /// not fit the standard rolling-limit concept).
    #[serde(default)]
    pub show_extra_usage: bool,
    /// Show per-model weekly quotas (Opus, Sonnet, Fable, …) in the dashboard.
    /// On by default; turn off to keep only the session and overall weekly bars.
    #[serde(default = "default_true")]
    pub show_model_limits: bool,
    /// Metric keys the user collapsed to a single compact row in the dashboard.
    #[serde(default)]
    pub collapsed_metrics: Vec<String>,
    #[serde(default)]
    pub token_expiry_warning: bool,
    #[serde(default = "default_true")]
    pub show_startup_notification: bool,
    #[serde(default)]
    pub custom_colors: CustomColors,
    #[serde(default)]
    pub quiet_hours: QuietHoursConfig,
    /// Session key for claude.ai web API fallback (optional)
    #[serde(default)]
    pub web_api_session_key: Option<String>,
    /// Organization ID for claude.ai web API fallback (optional)
    #[serde(default)]
    pub web_api_org_id: Option<String>,
    /// Manual plan label override, e.g. "Ultra". The usage API does not
    /// return the plan tier, so set this for a correct label and the
    /// downgrade comparison. Recognized: "Pro", "Max", "Ultra" (old "Max 5x" and "Max 20x" still work).
    #[serde(default)]
    pub plan_override: Option<String>,
    #[serde(default = "default_true")]
    pub show_usage_links: bool,
    /// Corrections per API metric key, e.g. {"amber_ladder": {"label": "Promo", "alerts": false}}.
    #[serde(default)]
    pub metric_overrides: std::collections::HashMap<String, MetricOverride>,
    /// Plan names by raw tier or subscription value, e.g. {"default_claude_new": "New"}.
    #[serde(default)]
    pub plan_aliases: std::collections::HashMap<String, String>,
    /// Show a local usage estimate on accounts without a session limit.
    #[serde(default = "default_true")]
    pub session_estimate: bool,
    /// Length of the estimate window in hours.
    #[serde(default = "default_session_estimate_hours")]
    pub session_estimate_hours: u32,
    /// Weekly percent that fills the estimate bar. Empty means an even share of
    /// the week: 100% / 168 hours * window.
    #[serde(default)]
    pub session_estimate_budget: Option<f64>,
}

fn default_session_estimate_hours() -> u32 {
    24
}

/// Choices offered in settings for the estimate window, in hours.
pub const SESSION_ESTIMATE_HOURS: &[u32] = &[5, 12, 24, 72];
/// Choices offered in settings for a manual budget, in weekly percent.
pub const SESSION_ESTIMATE_BUDGETS: &[f64] = &[5.0, 10.0, 20.0, 30.0, 50.0];

pub const CODEX_USAGE_URL: &str = "https://chatgpt.com/settings/usage?tab=overview";
const OLD_CODEX_USAGE_URL: &str = "https://chatgpt.com/codex/settings/usage";

impl Config {
    /// Move saved values that a newer release replaced. A custom URL is kept.
    fn migrate(&mut self) {
        if self.chatgpt_usage_url == OLD_CODEX_USAGE_URL {
            self.chatgpt_usage_url = CODEX_USAGE_URL.to_string();
        }
    }

    /// Next estimate window after the current one; the cycle passes through off.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn cycle_session_estimate_window(&mut self) {
        if !self.session_estimate {
            self.session_estimate = true;
            self.session_estimate_hours = SESSION_ESTIMATE_HOURS[0];
            return;
        }
        match SESSION_ESTIMATE_HOURS
            .iter()
            .position(|h| *h == self.session_estimate_hours)
        {
            Some(i) if i + 1 < SESSION_ESTIMATE_HOURS.len() => {
                self.session_estimate_hours = SESSION_ESTIMATE_HOURS[i + 1];
            }
            _ => self.session_estimate = false,
        }
    }

    /// Next manual budget; after the largest it returns to the automatic share.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn cycle_session_estimate_budget(&mut self) {
        self.session_estimate_budget = match self.session_estimate_budget {
            None => Some(SESSION_ESTIMATE_BUDGETS[0]),
            Some(current) => SESSION_ESTIMATE_BUDGETS
                .iter()
                .copied()
                .find(|b| *b > current),
        };
    }
}

fn default_dashboard_layout() -> String {
    "standard".to_string()
}

fn default_icon_style() -> String {
    "number".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            polling_interval_seconds: 120,
            notifications: NotificationConfig::default(),
            autostart: false,
            compact_mode: false,
            theme: "auto".to_string(),
            language: "auto".to_string(),
            show_chatgpt_section: false,
            chatgpt_usage_url: CODEX_USAGE_URL.to_string(),
            claude_install_url: "https://claude.ai/download".to_string(),
            show_widget: false,
            check_updates: true,
            accessibility_patterns: false,
            tray_icon_style: "number".to_string(),
            dashboard_layout: "standard".to_string(),
            show_extra_usage: false,
            show_model_limits: true,
            collapsed_metrics: Vec::new(),
            token_expiry_warning: false,
            show_startup_notification: true,
            custom_colors: CustomColors::default(),
            quiet_hours: QuietHoursConfig::default(),
            web_api_session_key: None,
            web_api_org_id: None,
            plan_override: None,
            metric_overrides: Default::default(),
            plan_aliases: Default::default(),
            session_estimate: true,
            session_estimate_hours: default_session_estimate_hours(),
            session_estimate_budget: None,
            show_usage_links: true,
        }
    }
}

impl Config {
    /// Generate a random polling interval.
    /// Within 15 minutes before the top of the hour: 120-180 seconds.
    /// Otherwise: 120-300 seconds.
    /// Uses system time nanoseconds for simple randomness without external crate.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn random_polling_interval() -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let minute_of_hour = (secs % 3600) / 60;
        let nanos = now.subsec_nanos() as u64;

        if minute_of_hour >= 45 {
            // Last 15 minutes of the hour: 120-180s
            120 + (nanos % 61)
        } else {
            // Normal: 120-300s
            120 + (nanos % 181)
        }
    }

    /// Validate and fix config values to safe ranges.
    pub fn validate(&mut self) {
        // Thresholds: keep only 1..=100, remove duplicates, sort
        self.notifications
            .thresholds
            .retain(|&t| (1..=100).contains(&t));
        self.notifications.thresholds.sort();
        self.notifications.thresholds.dedup();
        if self.notifications.thresholds.is_empty() {
            self.notifications.thresholds = vec![50, 75, 90];
        }

        // Validate theme
        if !crate::theme::ThemeMode::is_valid(&self.theme) {
            self.theme = "auto".to_string();
        }

        // Validate tray icon style
        if !["number", "ring", "bar", "pie"].contains(&self.tray_icon_style.as_str()) {
            self.tray_icon_style = "number".to_string();
        }

        // Validate dashboard layout
        if !["minimal", "standard", "detailed"].contains(&self.dashboard_layout.as_str()) {
            self.dashboard_layout = "standard".to_string();
        }

        // Validate language
        if ![
            "auto", "en", "uk", "es", "de", "fr", "pt", "ja", "ko", "zh", "it", "hi", "tr", "nl",
            "pl", "vi", "ru", "th", "id", "sv", "cs", "ar", "ro", "da", "fi", "hu", "bg", "el",
            "he", "ms", "no", "ca", "hr", "et", "lv", "lt",
        ]
        .contains(&self.language.as_str())
        {
            self.language = "auto".to_string();
        }
    }
}

pub struct ConfigManager {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    pub config: Config,
}

impl ConfigManager {
    pub fn new(exe_dir: &Path) -> Self {
        let path = exe_dir.join("config.json");
        let mut mgr = Self {
            path,
            last_modified: None,
            config: Config::default(),
        };
        mgr.load();
        mgr
    }

    fn load(&mut self) {
        if self.path.exists() {
            match fs::read_to_string(&self.path) {
                Ok(content) => match serde_json::from_str::<Config>(&content) {
                    Ok(mut cfg) => {
                        cfg.migrate();
                        cfg.validate();
                        self.config = cfg;
                        self.last_modified = fs::metadata(&self.path)
                            .ok()
                            .and_then(|m| m.modified().ok());
                    }
                    Err(e) => {
                        log::warn!("Failed to parse config.json: {e}. Using defaults.");
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read config.json: {e}. Using defaults.");
                }
            }
        } else {
            self.save();
        }
    }

    /// Check if config file changed on disk and reload if so.
    #[cfg_attr(not(windows), allow(dead_code))] // consumed by the Windows app
    pub fn reload_if_changed(&mut self) {
        let mtime = fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok());
        if mtime != self.last_modified {
            log::debug!("Config file changed, reloading.");
            self.load();
        }
    }

    pub fn save(&self) {
        match serde_json::to_string_pretty(&self.config) {
            Ok(content) => {
                if let Err(e) = fs::write(&self.path, content) {
                    log::error!("Failed to write config.json: {e}");
                }
            }
            Err(e) => log::error!("Failed to serialize config: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = Config::default();
        assert_eq!(cfg.dashboard_layout, "standard");
        assert_eq!(cfg.tray_icon_style, "number");
        assert_eq!(cfg.theme, "auto");
        assert!(!cfg.compact_mode);
        assert!(!cfg.token_expiry_warning);
    }

    #[test]
    fn test_validate_dashboard_layout() {
        let mut cfg = Config::default();

        // Valid values should be preserved
        for valid in ["minimal", "standard", "detailed"] {
            cfg.dashboard_layout = valid.to_string();
            cfg.validate();
            assert_eq!(cfg.dashboard_layout, valid);
        }

        // Invalid value resets to default
        cfg.dashboard_layout = "invalid".to_string();
        cfg.validate();
        assert_eq!(cfg.dashboard_layout, "standard");
    }

    #[test]
    fn test_validate_tray_icon_style_includes_pie() {
        let mut cfg = Config::default();

        for valid in ["number", "ring", "bar", "pie"] {
            cfg.tray_icon_style = valid.to_string();
            cfg.validate();
            assert_eq!(cfg.tray_icon_style, valid);
        }

        cfg.tray_icon_style = "invalid".to_string();
        cfg.validate();
        assert_eq!(cfg.tray_icon_style, "number");
    }

    #[test]
    fn test_validate_theme() {
        let mut cfg = Config::default();
        for valid in ["auto", "dark", "light", "midnight", "sunset"] {
            cfg.theme = valid.to_string();
            cfg.validate();
            assert_eq!(cfg.theme, valid);
        }
        cfg.theme = "neon".to_string();
        cfg.validate();
        assert_eq!(cfg.theme, "auto");
    }

    #[test]
    fn test_validate_language_includes_new_locales() {
        let mut cfg = Config::default();
        for valid in ["ca", "hr", "et", "lv", "lt"] {
            cfg.language = valid.to_string();
            cfg.validate();
            assert_eq!(cfg.language, valid);
        }
    }

    #[test]
    fn test_validate_thresholds() {
        let mut cfg = Config::default();
        // Out-of-range and duplicate thresholds cleaned up
        cfg.notifications.thresholds = vec![0, 50, 50, 101, 75];
        cfg.validate();
        assert_eq!(cfg.notifications.thresholds, vec![50, 75]);
    }

    #[test]
    fn test_validate_empty_thresholds_reset() {
        let mut cfg = Config::default();
        cfg.notifications.thresholds = vec![0, 200]; // all invalid
        cfg.validate();
        assert_eq!(cfg.notifications.thresholds, vec![50, 75, 90]);
    }

    #[test]
    fn test_cycle_notification_threshold_presets() {
        assert_eq!(
            cycle_notification_thresholds(&[50, 75, 90]),
            vec![75, 90, 100]
        );
        assert_eq!(cycle_notification_thresholds(&[75, 90, 100]), vec![90, 100]);
        assert_eq!(cycle_notification_thresholds(&[90, 100]), vec![100]);
        assert_eq!(cycle_notification_thresholds(&[100]), vec![50, 75, 90]);
    }

    #[test]
    fn test_format_notification_thresholds() {
        assert_eq!(
            format_notification_thresholds(&[75, 90, 100]),
            "75% / 90% / 100%"
        );
    }

    #[test]
    fn test_random_polling_interval_range() {
        for _ in 0..100 {
            let interval = Config::random_polling_interval();
            assert!((120..=300).contains(&interval));
        }
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.dashboard_layout, cfg.dashboard_layout);
        assert_eq!(parsed.tray_icon_style, cfg.tray_icon_style);
        assert_eq!(parsed.theme, cfg.theme);
    }

    #[test]
    fn test_config_deserialize_missing_new_fields() {
        // Simulates loading old config without new v2.0 fields
        let json = r#"{
            "version": "1.10.0",
            "polling_interval_seconds": 120,
            "notifications": {"enabled": true, "thresholds": [50, 75, 90], "sound": true},
            "autostart": false,
            "compact_mode": false,
            "theme": "auto",
            "language": "en",
            "show_chatgpt_section": false,
            "chatgpt_usage_url": "https://chatgpt.com/codex/settings/usage",
            "claude_install_url": "https://claude.ai/download"
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        // New fields should get defaults
        assert_eq!(cfg.dashboard_layout, "standard");
        assert_eq!(cfg.tray_icon_style, "number");
        assert!(!cfg.show_widget);
        assert!(!cfg.token_expiry_warning);
    }

    #[test]
    fn test_old_codex_usage_url_is_migrated() {
        let mut config = Config {
            chatgpt_usage_url: OLD_CODEX_USAGE_URL.to_string(),
            ..Config::default()
        };
        config.migrate();
        assert_eq!(config.chatgpt_usage_url, CODEX_USAGE_URL);

        config.chatgpt_usage_url = "https://example.com/usage".to_string();
        config.migrate();
        assert_eq!(config.chatgpt_usage_url, "https://example.com/usage");
    }

    #[test]
    fn test_session_estimate_setting_cycles() {
        let mut config = Config::default();
        assert_eq!(config.session_estimate_hours, 24);
        config.cycle_session_estimate_window();
        assert_eq!(config.session_estimate_hours, 72);
        config.cycle_session_estimate_window();
        assert!(!config.session_estimate);
        config.cycle_session_estimate_window();
        assert!(config.session_estimate);
        assert_eq!(config.session_estimate_hours, 5);

        assert_eq!(config.session_estimate_budget, None);
        config.cycle_session_estimate_budget();
        assert_eq!(config.session_estimate_budget, Some(5.0));
        config.session_estimate_budget = Some(50.0);
        config.cycle_session_estimate_budget();
        assert_eq!(config.session_estimate_budget, None);
    }
}
