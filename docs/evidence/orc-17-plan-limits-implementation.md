# ORC-17 implementation handoff · 2026-09-30

Implementation is complete. Independent design verification belongs to the later Verify step;
the implementer did not perform it or take app/design screenshots.

## Behavior

- The runner parses Codex session `token_count.rate_limits`, Codex App Server
  `account/rateLimits/updated`, and Claude `rate_limit_event` messages.
- Ordinary Codex exec invocations follow only their matching session log, respecting
  `CODEX_HOME` and the invocation start time. Partial lines wait for completion and
  pre-invocation readings are skipped. Only validated limit metadata reaches IPC.
- Readings are delivered live and never added to the project transcript/database.
  The desktop session retains the newest reading per CLI across project runners;
  Claude's individually reported windows merge with previously reported windows.
- The selected project's agent connections determine which CLI entries appear.
  Multiple connections of one CLI produce one entry. No agent connections means no item.
- The fullest live window supplies the percentage. Provider warnings, 80% usage,
  rejection, 100% usage, and Claude extra usage determine emphasis in core.
  A provider block names the responsible window's reset. Expired readings become
  unknown, with the reset and next-turn explanation in the hint.
- The hover-only item and its 360px hint use i18n and theme roles. The new
  `ExtraTypography::MonoLimit` role is Geist Mono 11/500 for emphasized percentages.
  Version disappears below 1280px, Tokens today below 1120px, then count labels
  for reached readings below 1120px (or all readings below 960px). Limits remain.

## Codex reads outside a run

The [official App Server documentation](https://developers.openai.com/codex/app-server#6-rate-limits-chatgpt)
documents `account/rateLimits/read` independently of any thread or turn. It returns
ChatGPT account windows, reset timestamps, plan type and credits when available.
Thus outside-run reads are supported by the documented protocol. This task retains
the approved turn-sourced behavior; it does not add a polling process or sign-in flow.
An outside-run request against the installed CLI was not executed.

Fixture provenance is recorded in `crates/core/tests/fixtures/plan-limits.md`.

## Validation

`powershell -File tools/check.ps1`: **check: OK**, including all-target/all-feature
clippy, rustfmt, source-size limits and UI literal checks.

Filtered test commands:

All passed: core 5, runner 3, status bar 8, locale 4, key coverage 1, theme 1
(22 tests total). The status-bar tests cover the 960px geometry and hover states;
they are not independent design verification.

```powershell
cargo test -p orchester-core --test plan_limits plan_limits
cargo test -p orchester-runner --features test-agent --test plan_limits plan_limits
cargo test -p orchester-desktop --test status_bar status_bar
cargo test -p orchester-desktop --test i18n locale
cargo test -p orchester-desktop --test i18n every_key
cargo test -p orchester-desktop --test theme plan_limits
```

Builds use `CARGO_TARGET_DIR=H:\projects\orchester\.orchester\target\orc17`.
The shared parent target initially supplied incompatible metadata from another worktree;
isolating this task's artifacts resolved those errors.

`cargo check -p orchester-core --target aarch64-apple-darwin` passed.
Runner/desktop cross-check was attempted but stopped in native dependencies because
the target C compiler `cc` is unavailable. No Mac runtime test, complete test suite,
outside-run live request, release installation, or design verification was performed.

## Verify must check

Use Pencil `poPso` in `Tg1WX`, status item `XCXvw` in `pmOpt`, and the same-size
app captures through the embedded MCP server. App elements are `status-plan-limits`
and `status-plan-limits-hint`.

- M `t8nGQ`: normal bar and open hint, window meters, missing-window row,
  plan type, turn time/age, footnote, separators and hint placement.
- N `e1TK9`: 80% or provider warning, grayscale emphasis, emphasized percentage
  and the correct warning window even when another window has a higher percentage.
- O `WuIp9`: blocking fill/icon, reset replacing percentage, full window name
  and provider refusal explanation. Confirm no new run-blocking behavior was added.
- P `mMpm5`: not read yet and signed out, dashes and connection-check time.
- Q `YlYEY`: no working connection, expired window, reported credits,
  unlimited credits and Claude extra usage (near, never reached).
- R `VTnmr`: 960px normal and reached bars, hide order, intact limits/MCP items,
  readable activity, and an in-window hover hint.
- Switch projects/tabs while another project's agent reports limits; the newest
  CLI reading must appear everywhere that has a connection for that CLI.
  Restart the app: readings must begin unknown. Check a connection after sign-in;
  its status must refresh without reopening the project.

All verification is pending; no screenshot comparison is claimed here.
