# TODO / Roadmap — ClaudeMeter

Working notes for future work. Not user-facing. Keep entries short and honest
about what is shipped vs. planned.

Legend: `[ ]` planned · `[~]` partially done · `[x]` shipped

---

## Requested / next up

- [ ] **Show the active Claude account on click (when multi-account is enabled).**
  - Depends on **multi-account profiles** landing first (see Deferred below) — the
    concept of an "active account" doesn't exist until profiles do.
  - Idea: when `profiles` is non-empty and an `active_profile` is set, surface the
    active account somewhere obvious:
    - Dashboard: show the active profile name (e.g. "Work" / "Personal") next to
      the Claude plan header, and/or in the tray tooltip.
    - On click: either (a) show a small menu listing profiles to switch between,
      or (b) cycle to the next profile and re-poll.
  - Open questions: how to label an account (profile name vs. email — email would
    need to be read from the token/usage response); where the click target lives
    (plan badge? a dedicated account chip?); whether switching should re-poll
    immediately vs. on next interval.

---

## Deferred from the v5 branch (`feature/v5-codex-usage-cost`, NOT shipped in 5.0.0)

These were implemented on the branch but intentionally left out of 5.0.0. Code
exists to reference/port later.

- [ ] **Multi-account profiles** — named credential profiles (`profiles` +
  `active_profile`) pointing at alternate `~/.claude`-style credential dirs, to
  switch between work/personal accounts. Prerequisite for the "active account"
  feature above. (Branch: `config.rs` `Profile`/`active_profile_dir`,
  `credentials.rs` `read_claude_token_from`, `windows_app.rs` poll plumbing.)
- [ ] **Local token & cost tracking** — parse `~/.claude/projects/**/*.jsonl`
  on-device to estimate tokens + USD spend (per-model/per-project), "Cost today"
  line in dashboard + tray tooltip. Opt-in `show_cost_tracking`. Also Codex cost
  from `~/.codex` logs. (Branch: `providers/jsonl.rs`, `codex.rs` cost fns,
  `draw_cost_line`.)
- [ ] **Burn-rate / time-to-limit forecast** — project current rate of change to
  estimate ETA to 100% ("~47m to limit"), shown in tooltip/statusline. Opt-in
  `show_burn_rate`. (Branch: `forecast.rs`, `build_tooltip_ext`.)
- [ ] **Claude Code statusline export** — write `~/.claude/claudemeter-status.{json,txt}`
  for a Claude Code `statusLine` command. Opt-in `statusline_export`.
  (Branch: `statusline.rs`.)
- [ ] **macOS menu bar ring gauge** — color-coded circular progress ring next to
  the percentage, matching the Windows tray "ring" style. (Branch: macOS files.)

---

## Possible improvements / ideas

- [ ] **Status-page URLs in config** — currently hardcoded
  (`status.openai.com`, `status.claude.com`, `claude.ai/settings/usage`). Make
  them configurable like `chatgpt_usage_url`.
- [ ] **Verify localization quality** — 6 new keys were machine-translated into
  34 locales in 5.0.0 (CODEX/Show extra usage/Export History (JSON)/Usage link
  icons/Open usage/Service status). Have native speakers sanity-check the
  non-Latin scripts (ar/fa/he/hi/th/bn/ja/ko/zh).
- [ ] **Codex bar color from theme** — the Codex teal (`#14b8a6`) is hardcoded in
  `render.rs`. Consider moving it into the `colors.rs` palette so it respects
  custom/high-contrast themes.
- [ ] **`show_usage_links` read via global `APP_STATE`** in `render.rs`
  (`draw_section_header`) is a bit hacky — thread the bool through cleanly.
- [ ] **Runtime/visual QA** — the tray icon, dashboard popup, Codex panel, icon
  buttons, and redrawn gear were only build/clippy/test-verified, not visually
  confirmed. Do a manual pass per the CLAUDE.md testing checklist (especially at
  150%/200% DPI).
- [ ] **Memory check** — confirm private-bytes footprint still meets the <10 MB
  target after the Codex/local-log reads (WorkingSet looked higher post-poll,
  likely shared DLLs/TLS, but worth measuring private bytes).

---

## Shipped in 5.0.0

- [x] Live Codex (OpenAI) **subscription** usage from local `~/.codex/sessions`
  logs (5-hour + weekly bars), independent of Claude API polls.
- [x] Distinct **teal** Codex progress bars (`override_color` on `draw_metric`).
- [x] Compact **usage + service-status icon buttons** per section (Claude +
  Codex), toggle "Usage link icons" (`show_usage_links`, default on).
- [x] "Show extra usage" replaces "Hide Extra Usage" (`show_extra_usage`).
- [x] Redrawn gear/cog settings icon.
- [x] JSON history export (alongside CSV).
- [x] Full localization of the above across all locales.
