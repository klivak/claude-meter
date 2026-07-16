# Changelog

All notable changes to ClaudeMeter will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]




## [5.3.2] - 2026-07-16

### Fixed
- **Windows usage notifications** — restored the standard system balloon notification mode instead of relying on a custom balloon icon that some Windows notification hosts silently ignored.
- **Notification diagnostics** — log a warning when Windows rejects a balloon notification request.

## [5.3.1] - 2026-07-16

### Added
- **Actionable authentication errors** — error panels now offer a one-click `claude login` command copy action when credentials are missing or expired.
- **Retry action** — transient network, API, server, and rate-limit errors can be retried directly from the error panel.

### Tests
- Added coverage for error action selection and auth/retry behavior.

## [5.3.0] - 2026-07-16

### Added
- **One-click Windows updates** — downloads the release executable, verifies its SHA-256 checksum, and safely installs it after ClaudeMeter exits with rollback on failure.
- **Version automation** — added PowerShell bump and consistency-check scripts, plus CI guards for release and website versions.
- **Centralized error classification** — popup and tray now share typed error categories and retry-after parsing.

### Fixed
- **macOS update detection** — compares against the actual bundle version instead of a stale hardcoded release number.

## [5.2.1] - 2026-07-16

### Added
- **Visitor map** — added a responsive MapMyVisitors widget to the project website's analytics section, including a link to detailed visitor statistics.

### Fixed
- **Small progress fills** — kept low-percentage Claude and Codex indicators inside the progress bar bounds.

## [5.2.0] - 2026-07-15

### Added
- **Project website** — launched a responsive GitHub Pages landing page with platform badges, feature explanations, theme previews, direct downloads, FAQ, and clear Windows 10/11 and macOS 12+ support.
- **Website Project Pulse** — added a separate privacy-friendly long-term page-load counter plus public stars, release, and issue metrics.
- **Automatic Pages deployment** — added a dedicated GitHub Actions workflow that builds and deploys the static site and refreshes public project metrics every day.
- **Optional Codex website showcase** — the landing page now highlights the additional Codex panel alongside Claude usage, including its Settings-based activation and compact-mode support.

### Changed
- **Simpler README counter** — the Project Pulse badge now shows one clearly labeled approximate `README loads` total instead of a today/total pair.
- **README website documentation** — added the live project URL, a current website screenshot, analytics behavior, platform coverage, and Pages deployment details.
- **Refined website hero** — reduced headline scale, removed the redundant release pill, and resized dashboard previews so both screenshots remain fully visible.

## [5.1.0] - 2026-07-15

### Added
- **Midnight and Sunset themes** — added two complete palettes with theme-specific surfaces, accents, thresholds, and progress gradients.
- **Chart percentage scale** — added a dedicated 0–100% Y-axis panel and more breathing room above the history plot.
- **Five new languages** — added complete Catalan, Croatian, Estonian, Latvian, and Lithuanian localizations, bringing the total to 40 languages.

### Changed
- **Codex naming** — renamed the visible ChatGPT section, menu action, tooltip, documentation, and empty-state copy to Codex while retaining legacy config keys for compatibility.
- **Extensible theme registry** — centralized selectable theme metadata and moved progress gradients into each palette so future themes require fewer UI changes.
- **README feature gallery** — refreshed the main and Settings screenshots, added a four-theme gallery, and documented all current dashboard, Codex, icon, notification, and theme behavior.
- **README Project Pulse** — added live views, stars, latest release, forks, issues, and language-count badges, with a privacy-friendly traffic dashboard behind the views counter.

### Fixed
- **Codex in compact mode** — compact dashboards now include enabled Codex rolling-window usage rows with their distinct teal styling.
- **Codex activation hint** — settings now explains that the tray popup should be reopened after enabling the Codex section.
- **Compact provider labels** — Claude and Codex rows are now explicitly labeled when both providers are visible in compact mode.
- **Chart axis alignment** — percentage labels now use evenly spaced centers with symmetric top and bottom padding.
- **Balloon notification icon** — Windows notifications now receive the embedded ClaudeMeter app icon through `NIIF_USER` and `hBalloonIcon`.

## [5.0.3] - 2026-07-15

### Changed
- **New application icon** — replaced the generic executable icon with a polished ClaudeMeter usage-gauge mark, including transparent corners and Windows sizes from 16×16 through 256×256.
- **README branding** — added the new ClaudeMeter icon to the project header.

## [5.0.2] - 2026-07-15

### Added
- **Release checksums** — Windows releases now include a `claudemeter.exe.sha256` file and display the executable's SHA-256 hash in the release notes.

## [5.0.1] - 2026-07-15

### Added
- **Automatic Rust formatting before commits** — a repository pre-commit hook now runs `cargo fmt` and stages formatting updates for tracked Rust files.

### Fixed
- **Release formatting check** — applied the Rust formatting required by CI.

## [5.0.0] - 2026-07-15

### Added
- **Live Codex (OpenAI) usage** — reads local `~/.codex` logs to show your ChatGPT/Codex subscription usage directly in the dashboard, with a "CODEX · Plan" header and rolling-window progress bars. No API key required.
- **Distinct Codex progress bars** — Codex windows render in a teal hue so they read as a different provider from Claude's green→amber bars.
- **Compact usage & status icon buttons** — each section header now shows small "open usage" and "service status" icon buttons; toggle them with the new "Usage link icons" setting.
- **JSON history export** — right-click tray → "Export History (JSON)" to save full usage history as JSON, alongside the existing CSV export.

### Changed
- **"Show extra usage" setting** — replaces the previous "Hide Extra Usage" toggle (inverted meaning; off by default).
- **Redrawn settings gear icon** — the gear/cog icon is now a proper filled cog outline with a punched-out center hole instead of a sunburst.
- **Localization** — all new UI strings translated across every supported language.

### Fixed
- **Codex panel height** — the dashboard now reserves space for exactly the number of rolling windows Codex actually reports (it may send only the weekly window), instead of always assuming two. This removes the empty gap that appeared below the history chart when only one Codex window was present.

## [4.2.2] - 2026-07-14

### Added
- **macOS menu-bar usage history chart** — 24h bar chart in the dropdown (ported from the Windows popup), with 25/50/75% grid lines, threshold-colored bars, and dashed 5-hour session reset lines.
- **macOS per-limit breakdown** — 5-hour / weekly / Sonnet / Opus entries in the menu, matching the Windows layout.
- **macOS weekly pace projection** — flags when usage is on track to exhaust the weekly budget before reset, with a 🔥 indicator in the menu-bar title on overshoot.
- **Plan override + downgrade comparison** — new `plan_override` config field (`"Pro"`, `"Max 5x"`, `"Max 20x"`) for a correct plan label and a "what usage would look like on a smaller tier" comparison.
- **Compact macOS menu-bar title** — severity dot + session % + time-left, plus weekly %.

### Fixed
- **macOS app never finished launching** (NSApplication lifecycle).
- **macOS LaunchAgent relaunch loop / unquittable app** — launch the bundle executable directly with `KeepAlive=false` instead of via `/usr/bin/open`.
- **Orphaned macOS agents** — kill leftover `--agent` pollers on startup so their combined poll rate no longer trips the API rate limiter.
- **macOS keeps last-known usage** on a failed poll instead of blanking the menu.
- **macOS autostart applies immediately** — Enable/Disable now run `launchctl load/unload -w` for the current session, not just next login.

### Credits
- macOS improvements contributed by @kenschwartz (#1).

## [4.1.1] - 2026-07-13

### Fixed
- **CI dependency check** — replaced `cargo-deny-action` with a direct `cargo install cargo-deny` + `cargo deny check`, fixing the failing job caused by the action passing an unrecognized `--log-level` flag

## [4.1.0] - 2026-06-30

### Added
- **Stale data warning** — the popup now flags cached numbers as stale (with a ⚠ marker) when the OAuth token has expired or no live poll has succeeded this session, instead of showing them as if current

### Security
- **quinn-proto 0.11.15** — bumped from 0.11.14 to fix RUSTSEC-2026-0185 (remote memory exhaustion via unbounded out-of-order stream reassembly)

## [4.0.1] - 2026-05-31

### Fixed
- **macOS app bundle build** - compile the Swift menu bar app with -parse-as-library for GitHub macOS runners

## [4.0.0] - 2026-05-31

### Added
- **Native macOS menu bar app** — added a Swift/AppKit `NSStatusItem` app with live percentage, freshness state, Refresh, Open Claude, update check, config import/export, logs, and autostart controls
- **macOS `.app` bundle release** — CI now packages `ClaudeMeter.app` with the Rust polling agent embedded as `claudemeter-agent`
- **macOS data freshness status** — the agent writes `status.json` with `Live`, `Refreshing`, cached/no-data, and API error states for the menu UI
- **Portable macOS logs** — the macOS agent writes `claudemeter.log` under `~/Library/Application Support/ClaudeMeter`
- **Optional macOS notarization hook** — release workflow notarizes the app zip when Apple Developer secrets are configured

### Changed
- **Dependency refresh** — updated compatible Rust dependencies in `Cargo.lock`

## [3.1.4] - 2026-05-31

### Fixed
- **macOS CI build script** — gated Windows resource compilation in `build.rs` so macOS builds no longer require the Windows-only `winres` build dependency

## [3.1.3] - 2026-05-31

### Fixed
- **CI clippy compatibility** — restored crate-wide lint allowances that were lost when the Windows entrypoint was split out of `main.rs`, so `cargo clippy -- -D warnings` passes again

## [3.1.2] - 2026-05-31

### Added
- **macOS low-memory background agent** — added a native Rust macOS entrypoint, LaunchAgent installer script, and GitHub Actions builds for Apple Silicon without Electron/WebView overhead

### Changed
- **Login expiry warnings disabled by default** — `token_expiry_warning` now defaults to `false`, including old config files that do not have the field

### Fixed
- **Manual refresh no longer gets dropped** — user-triggered refreshes queue one follow-up poll when another poll is already running, avoiding stale cached display after opening the dashboard or pressing Refresh/F5
- **Usage API cache bypass** — usage requests now include cache-busting query parameters and no-cache headers for fresher responses
- **Cross-platform credentials module** — Windows Credential Manager access is now gated to Windows, while file-based Claude credentials work cross-platform

## [3.1.1] - 2026-04-26

### Fixed
- **Security: rustls-webpki CRL parsing panic** — updated rustls-webpki 0.103.12 → 0.103.13 to fix RUSTSEC-2026-0104 (reachable panic when parsing certificate revocation lists with an empty `BIT STRING` in `onlySomeReasons` of an `IssuingDistributionPoint` extension)

## [3.1.0] - 2026-04-26

### Added
- **Toggle for startup tray notification** — new `show_startup_notification` config option (default: true) and a settings-panel toggle to silence the "Running in system tray" balloon shown on every launch
- **Settings toggle for token-expiry warning** — the existing `token_expiry_warning` flag is now exposed in the settings panel as "Show login expiry warning" (was previously only editable via config.json)

### Fixed
- **Update-available balloon now opens the GitHub release page** — clicking the toast/balloon for a newer version opens the release URL in the default browser instead of doing nothing
- **`token_expiry_warning` config flag was ignored** — the option existed since v2.2.6 but wasn't actually checked when firing the notification; now properly suppresses the balloon when set to false

## [3.0.1] - 2026-04-20

### Fixed
- **Security: rustls-webpki name constraints vulnerabilities** — updated rustls-webpki 0.103.10 → 0.103.12 to fix RUSTSEC-2026-0098 (URI names incorrectly accepted) and RUSTSEC-2026-0099 (wildcard certificate name constraints)

## [3.0.0] - 2026-04-04

### Added
- **Credential file watcher** — monitors `~/.claude/` directory for changes and triggers immediate re-poll when credentials are updated, instead of waiting for the next polling cycle
- **Network connectivity monitor** — detects network interface changes (e.g., WiFi reconnect) via Windows IP Helper API and triggers an immediate poll when connectivity is restored
- **Sleep/wake progressive retry** — after resuming from sleep/hibernate, retries at 2s, 5s, 15s, 30s intervals instead of a single immediate poll; stops retrying on first successful response
- **Subscription type badge** — the plan name (Pro, Max, Max 5X, Max 20X) is now displayed as a colored pill-shaped badge in the dashboard header instead of plain text
- **Web API fallback** — when OAuth credentials are unavailable or expired, ClaudeMeter can optionally fall back to the claude.ai web API using session cookies; configure `web_api_session_key` and `web_api_org_id` in config.json
- **Aggregated notifications** — when multiple usage thresholds are crossed simultaneously, a single batched notification is shown instead of multiple separate alerts

### Fixed
- **Security: rustls-webpki CRL vulnerability** — updated rustls-webpki 0.103.9 → 0.103.10 to fix RUSTSEC-2026-0049

### Changed
- **CI: Node.js 24 migration** — updated `actions/checkout` v4 → v6 across all workflows to resolve Node.js 20 deprecation warnings
- **CI: audit workflow** — replaced `rustsec/audit-check@v2` (which required issue-creation permissions) with direct `cargo audit` for simpler, more reliable security scanning

## [2.2.6] - 2026-03-16

### Fixed
- **Security: quinn-proto DoS vulnerability** — updated quinn-proto 0.11.13 → 0.11.14 to fix RUSTSEC-2026-0037 (high severity)

### Added
- **Token expiry warning** — new `token_expiry_warning` config option (default: true)

## [2.2.5] - 2026-03-09

### Fixed
- **UI thread blocking** — removed synchronous file I/O (config reload) and registry reads (theme detection) from the paint handler that ran every 16ms frame; config and theme are now refreshed on a 5-second timer instead
- **Popup open delay** — removed blocking config reload from popup show path
- **DB blocking on main thread** — moved SQLite insert/query operations to a background thread so the UI stays responsive during database writes after each poll

## [2.2.4] - 2026-03-09

### Changed
- **Adaptive polling interval** — normal interval widened to 120–300s (was 90–180s); tightens to 120–180s in the last 15 minutes of each hour when limits are about to reset

## [2.2.3] - 2026-03-08

### Fixed
- **Autostart broken after update** — removed version number from release binary filename so users can replace `claudemeter.exe` in-place without breaking the autostart registry path

## [2.2.2] - 2026-03-07

### Fixed
- **Autostart not surviving exe relocation** — autostart registry entry is now synced on every launch, so moving the exe to a new folder (e.g., after downloading a new release) no longer silently breaks startup with Windows

## [2.2.1] - 2026-03-05

### Fixed
- **Plan header spacing** — restored visible space between `Plan` label and Max tier text in the dashboard header
- **CI format check** — applied `cargo fmt` formatting for newly added i18n/render code so `cargo fmt --check` passes

## [2.2.0] - 2026-03-05

### Added
- **Slide animation** — smooth horizontal slide transition between Dashboard and Settings views
- **Dashboard layouts** — three layout modes selectable in Settings: Minimal (single largest metric, large percentage), Standard (existing view), Detailed (metrics with inline sparkline charts)
- **Gradient progress bars** — full-spectrum green→amber→coral gradient across progress bars reflecting utilization position; falls back to solid colors when custom color overrides are active
- **Pie chart tray icon** — new "Pie" icon style showing all metrics as proportional colored sectors in a mini pie chart
- **Rate of change indicators** — arrow symbols (↑↗→↘↓) next to each metric percentage showing utilization trend based on hourly rate of change from historical data
- **Focus Assist integration** — automatically suppresses all notifications when Windows Focus Assist / Do Not Disturb is active (uses SHQueryUserNotificationState API)

## [1.10.4] - 2026-03-05

### Added
- **30-day chart view** — usage history chart now has a "24h | 7d | 30d" toggle to switch between 24-hour, 7-day, and 30-day views with daily bucket intervals

## [1.10.3] - 2026-03-05

### Fixed
- **Polling interval randomization** — API polling now uses a random interval between 90-180 seconds instead of a fixed interval, reducing the risk of rate limiting and looking more natural to the API
- **Rate limit retry floor** — minimum retry-after on 429 responses raised from 1s to 90s

## [1.10.2] - 2026-03-05

### Fixed
- **Tooltip plan name** — compact plan display: "Max 5X" → "5x", "Max 20X" → "20x" for cleaner tooltip
- **Tooltip extra metrics** — single extra metric now shown inline with name and reset time instead of "+1 extra"

## [1.10.1] - 2026-03-05

### Fixed
- **Tooltip format** — restored reset times and spacing in tray tooltip; extra metrics shown as "+N extra" summary; removed redundant app name header

## [1.10.0] - 2026-03-05

### Added
- **7-day chart toggle** — usage history chart now has a "24h | 7d" toggle to switch between 24-hour and 7-day views with 4-hour bucket intervals
- **Copy button** — clipboard icon in the popup footer copies all current usage metrics to the clipboard for easy sharing

### Fixed
- **Tooltip truncation** — tray hover tooltip now uses a compact format (no empty lines or reset times) to fit all metrics within the 127-character Win32 limit

## [1.9.0] - 2026-03-05

### Added
- **Tooltip with error info** — hovering the tray icon now shows the error type (token expired, connection error, rate limited, etc.) when data is unavailable, and a warning indicator when stale data is shown
- **Token expiry warning** — proactive balloon notification when the OAuth token is about to expire within 1 hour, with guidance to run `claude login` to refresh
- **Wake-from-sleep polling** — app immediately polls the API when the system resumes from sleep/hibernate, instead of waiting for the next timer tick (up to 10 min with backoff)

## [1.8.3] - 2026-03-05

### Added
- **Cached data on startup** — last known usage data is loaded from the local database at launch, so the dashboard shows data immediately even if the first API poll fails (e.g. rate limited)

### Fixed
- **Popup clipping on error screen** — the "Status ↗" link added in v1.8.0 was cut off because the popup height calculation didn't account for it
- **Concurrent poll requests** — multiple simultaneous API requests (e.g. from timer + manual refresh) could trigger 429 rate limits; added a guard to prevent overlapping polls

## [1.8.2] - 2026-03-05

### Fixed
- **"Retry after 0s"** — rate limit retry-after header with value 0 now shows minimum 1s; non-numeric values default to 60s

## [1.8.1] - 2026-03-05

### Fixed
- **Extra Usage not displayed** — `extra_usage` field from the API (monthly credits utilization) was silently ignored because `resets_at` was missing from the response; added `#[serde(default)]` so missing optional fields parse correctly

## [1.8.0] - 2026-03-05

### Added
- **Detailed error diagnostics** — instead of a generic "Claude Code not detected" for all failures, the app now shows specific error messages: token expired, network error, rate limited, server error, or credentials not found
- **Error blink** — tray icon blinks when a poll error occurs and there is no cached data, stops when the user opens the popup
- **Status link on error screen** — "Status ↗" link to status.claude.com is now shown on the error/not-detected screen, not just on the main dashboard

### Fixed
- **Data loss on transient errors** — previously, a single failed API poll would wipe all cached usage data and show "not detected"; now previous data is preserved until a successful poll replaces it

## [1.7.2] - 2026-03-03

### Added
- **5 new languages** — Arabic, Romanian, Danish, Finnish, Hungarian (25 languages total)

### Fixed
- **Tray icon color mismatch** — icon color now matches the displayed value (5-hour session %) instead of using the max across all limits, which caused a yellow icon even at 3% session usage

## [1.7.1] - 2026-03-03

### Added
- **Quiet hours** — suppress notifications during configurable time window (default 22:00–08:00, disabled by default). Set `quiet_hours.enabled: true` in config.json
- **Theme popup picker** — click Theme in settings to choose from a popup menu instead of cycling through options

### Fixed
- **Chart time alignment** — 24h usage chart now always renders 48 fixed 30-minute slots aligned with x-axis labels. Previously, sparse data was stretched across the full chart width, misaligning bars with time labels
- **Config language validation** — added 5 missing languages (ru, th, id, sv, cs) to validator; previously selecting these languages via config.json would reset to "auto" on next load

## [1.7.0] - 2026-03-03

### Added
- **5 new languages** — Russian, Thai, Indonesian, Swedish, Czech (20 languages total)
- **Popup language picker** — click Language in settings to open a popup menu with all 20 languages instead of cycling through them one by one
- **Widget hover tooltip** — hover the mini-widget to see full usage breakdown (all metrics, reset times, plan info)

### Changed
- Widget now shows 5-hour session utilization instead of max across all metrics (fixes misleading orange color)
- Version strings now use `CARGO_PKG_VERSION` — no more hardcoded version numbers in source code

### Fixed
- Widget color mismatch — widget showed orange (high %) when only the weekly limit was high, even though the 5-hour session was low

## [1.6.1] - 2026-03-03

### Fixed
- **Single notification per threshold jump** — when usage crosses multiple thresholds at once (e.g., 0% → 96%), only the highest applicable threshold fires a notification instead of all three

## [1.6.0] - 2026-03-03

### Added
- **Configurable tray icon style** — choose between Number (default), Ring (circular progress), or Bar (vertical fill) via Settings or `tray_icon_style` in config.json
- **i18n for icon style** — "Icon style", "Number", "Ring", "Bar" translated in all 15 languages
- **Missing i18n keys for Ukrainian** — added Show widget, Check for updates, Accessibility patterns, Update available translations

### Changed
- Settings panel expanded from 8 to 9 rows (added icon style selector)

## [1.5.0] - 2026-03-03

### Added
- **5 new languages** — Hindi, Turkish, Dutch, Polish, Vietnamese (15 languages total)
- **Notification screenshot** in README for Smart Notifications section

### Changed
- Language cycling now covers all 15 languages in order

## [1.4.0] - 2026-03-03

### Added
- **5 new languages** — Portuguese, Japanese, Korean, Chinese (Simplified), Italian (10 languages total)
- **Better language picker** — settings panel shows full native language name (e.g. "Português", "日本語") instead of language code
- **Mini floating widget** — always-on-top PiP window showing current usage % with color-coded background. Draggable, click to open dashboard. Disabled by default; enable in Settings
- **Auto-update checker** — checks GitHub Releases on startup for newer versions, shows balloon notification if available. Enabled by default; toggle in Settings
- **Accessibility patterns** — colorblind-friendly overlays on progress bars: dots (green), diagonal stripes (yellow), cross-hatch (red). Disabled by default; enable in Settings
- **Custom color themes** — override any theme color via `custom_colors` in config.json (hex values like `"#ff0000"`)

### Changed
- Settings panel expanded from 5 to 8 rows (added widget toggle, update checker toggle, accessibility patterns toggle)
- Language cycling now covers all 10 languages in order

## [1.3.6] - 2026-03-03

### Added
- **VirusTotal scanning** — every release binary is automatically scanned by VirusTotal (60+ antivirus engines) with a link to the full report in release notes
- **VirusTotal badge** in README for transparency and trust

## [1.3.0] - 2026-03-03

### Added
- **Claude Status link** — "Status" link on the Claude section header opens https://status.claude.com/

### Fixed
- **Tray icon shows session %, not weekly max** — when a 5-hour session is active, the tray icon now shows the session utilization (e.g. 7%) instead of the weekly maximum (e.g. 41%)
- **Tray icon shows "..." when no active session** — instead of showing the weekly limit number, the icon displays "..." when no 5-hour session is running
- **Chart no longer shows phantom activity** — usage history chart now filters out records with no active session; old invalid data is cleaned up on startup
- **Tray icon text readability** — white text on green background (was black, hard to read)
- **Notifications use native balloon tips** — replaced unreliable PowerShell toast notifications with Win32 balloon tips that always show both title and body text

## [1.2.0] - 2026-03-03

### Added
- **Dynamic tray icon with % number** — shows actual utilization percentage on the tray icon
- **Gradient progress bars** — smooth color gradients on metric bars
- **Animated progress bars** — bars smoothly fill on popup open (~60fps lerp)
- **Popup fade-in animation** — smooth opacity transition when opening dashboard
- **Chart bar hover tooltip** — hover chart bars to see exact % and time
- **CSV export** — export full usage history from context menu
- **Mica backdrop** — Windows 11 translucent Mica effect on popup
- **Keyboard shortcuts** — ESC to close popup, F5 to refresh
- **Notification sound** — system beep with notifications (configurable)
- **Informative notifications** — shows metric name, current %, exceeded threshold, and reset time
- **Startup notification** — "Running in system tray" toast on launch
- **Auto-refresh on popup open** — triggers poll if data is older than 60 seconds
- **Tray icon blink** — icon blinks when usage exceeds 90% until popup is opened
- **Idle detection** — pauses API polling when PC is idle for 5+ minutes (saves bandwidth)
- **Retry with exponential backoff** — on API errors, poll interval doubles (up to 10 min cap)
- **Rate-limit (429) handling** — graceful retry-after parsing for Anthropic API
- **Config validation** — sanitizes polling interval, thresholds, theme, and language on load

### Fixed
- PowerShell notification window no longer flashes on startup (CREATE_NO_WINDOW flag)
- Tray icon text contrast — black text on green/yellow, white text on red/gray for readability

## [1.1.0] - 2026-03-02

### Added
- Direct2D + DirectWrite hardware-accelerated rendering (replaces GDI)
- DWM dark title bar integration
- Session reset lines (dashed vertical) on 24h usage chart
- Tooltip spacing between metric values
- Screenshots in README (dashboard, tooltip, settings)

### Fixed
- DPI scaling at 125%/150% — popup no longer clips content
- Memory reclaimed when popup is closed (D2D resources released + working set trimmed)
- Settings gear and close button visibility at non-100% DPI
- Credential error display improvements

## [1.0.0] - 2026-02-25

### Added
- Initial release
- Claude usage monitoring (5-hour, 7-day, Sonnet, Opus + dynamic metrics)
- Auto-detection of Claude Pro/Max plan
- Future-proof API parsing (unknown metrics auto-displayed)
- OAuth token retrieval from Windows Credential Manager
- System tray with dynamic color-coded icons (green/yellow/red/gray)
- Rich tooltip with full usage summary on hover
- Dashboard popup with progress bars and countdown timers
- ChatGPT/Codex info section with link to usage page (hidden by default)
- 24-hour usage history chart from SQLite database
- Windows toast notifications at configurable thresholds (50%, 75%, 90%)
- Auto-start with Windows (registry + batch script)
- Compact mode toggle
- Theme: Dark / Light / Auto (follows Windows system theme)
- Languages: English, Українська, Español, Deutsch, Français
- Auto language detection from Windows settings
- Portable config.json next to .exe
- Single .exe, zero dependencies, under 10 MB RAM
- Built with Rust for minimal memory footprint
