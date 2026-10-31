# ClaudeMeter 6.0.0 Implementation Plan

> **For agentic workers:** Executed natively in one session at the user's explicit request ("плануй і роби"). Steps use checkbox syntax.

**Goal:** Correct display for the renamed Claude plans (Max, Ultra), plans without a 5-hour session, dollar-based unknown limits, Fable shares, usage/API credits, and current Codex logs.

**Architecture:** All decisions live in pure, unit-tested functions in `providers/claude.rs` and `providers/codex.rs`; UI (`tray`, `widget`, `render`, `windows_app`, `macos_app`) only consumes them.

**Tech Stack:** Rust, Win32/Direct2D, SwiftUI (macOS), rusqlite.

**Spec:** the research conclusions in this session; live `/api/oauth/usage` response of 2026-10-31.

## Global Constraints

- Never infer "no session" from the plan name: unverified Ultra keeps a temporary limit. Drive everything from returned data.
- Unknown API keys stay visible but never drive the icon, colors or notifications.
- API credits don't cover Claude Code and have no known balance source: show a static note only, never a percentage.
- Every new i18n key goes into all locales. No `Command::new`. Codex stays local-only (no token, no network). No hardcoded colors.

## Review Focus

- Ultra at 0% weekly with an unknown metric at 99.96%: icon shows weekly, no alert fires.
- Temporary session on Ultra (`five_hour` present): session is primary again, chart and widget follow.
- Old `plan_override` "Max 20x" in an existing config.json: still works, shows Ultra.
- Codex resumed old-named rollout is the freshest: chosen by mtime, huge logs are not read whole.
- Codex event with another `limit_id`: does not overwrite the main windows.

---

### Task 1: Claude parsing and plan model (`src/providers/claude.rs`)

- [ ] `UsageMetric` gains `limit_dollars`, `used_dollars`, `locked_reason`, `inactive`, `share_of_weekly`; tests for the live fixture.
- [ ] `format_tier`/`detected_plan`: `max_20x|ultra` → Ultra, `max_5x|max` → Max; `normalize_plan_label`, `api_credits_usd`.
- [ ] `is_core_limit`, `primary_metric`, `chart_metric_key`; `max_utilization` uses core active limits only.
- [ ] `spend` and `extra_usage` amounts; `weekly_scoped_shares`; `metric_detail` data.

### Task 2: Consumers

- [ ] `db.rs`: chart queries take a metric key, tests for `seven_day`.
- [ ] `tray.rs`, `widget.rs`, `windows_app.rs`: primary metric, chart key, notifications only for core limits.
- [ ] `render.rs`: detail line, plan badge colors, API credits tooltip, sparkline key, chart label.
- [ ] `macos_app.rs` and Swift: Ultra multipliers, primary percent, chart.

### Task 3: Codex (`src/providers/codex.rs`, render)

- [ ] Newest files by mtime; bounded tail read; ignore other `limit_id`s; parse `credits`.
- [ ] Show credits in the plan pill.

### Task 4: i18n, docs, release

- [ ] New keys in all locales; README, site, CHANGELOG, config comment; `bump-version.ps1 6.0.0`.
- [ ] `cargo fmt`, clippy with `-D warnings`, `cargo test`, release build, live run against the real API and logs.
