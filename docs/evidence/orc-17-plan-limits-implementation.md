# ORC-17 implementation handoff · 2026-09-30

The implementation has been revised after independent Review and Verify.
The earlier Verify result is FAIL; its findings are not closed until independent
re-verification passes. The implementer did not perform design verification or
take app/design screenshots.

**Merged onto master 2026-10-02.** Master runs every Codex step through App Server and has no
`codex exec` JSON path, so the Codex session-log reader (`crates/runner/src/codex_limits.rs`), the
session `token_count` parser, its fake session log and the runner's optional `chrono` and tokio
`fs` feature were removed. Codex limits now come only from App Server's
`account/rateLimits/updated` notification, read against the v2 schema of codex-cli 0.159.2
(`windowDurationMins` nullable, sparse updates merged into the last reading, only the `codex`
bucket). The sections below describe the run as it was before the merge.

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

The resumed run passed `tools/check.ps1` and all 22 filtered tests using the
runner-supplied absolute `CARGO_TARGET_DIR`:
`H:\projects\orchester\.orchester\target\worktrees\01a0f184-cef3-7424-b9b1-4035e8b76926`.
No manual target override was applied. The installed artifact-isolation fix
supersedes the earlier shared-target workaround. The implementation is preserved;
independent Check, Review and Verify remain pending pipeline steps.

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

## Previous checkpoint comparison

Compared checkpoint `006bf23` (`orchester/orc-17-run-1`) from this worktree.
Its correct provider parsing, hover information, connection-based visibility and
session-only aggregation behavior are retained in the current implementation.
Its IPC round-trip tests alone did not establish delivery from ordinary Codex exec;
the current matching-session reader and worker integration cover that missing path.
No entire checkpoint was cherry-picked or current functionality overwritten.
Independent Check, Review and Verify remain required pipeline steps.

## Recovery after independent Verify majors

Preserved `.tmp/verify/verification.md`, all original design/app captures, fixture
programs, and host-watcher evidence. Preserved the outside-run provider information
above. No private app was launched, no watcher was altered, and no merge or commit
was made by the implementer.

Changes addressing the findings:

- Window readings retain their individual arrival timestamps through aggregation
  and serialization. Updating Claude's five-hour window preserves the weekly
  window's old time. Mixed-age reports display a separate, window-labelled source
  line for each window; one-report snapshots retain the designed single source line.
- Known and unknown Claude windows render in the same duration-ordered sequence:
  unknown five-hour precedes weekly. A five-hour-only Claude report and Codex
  snapshots show their reported windows, matching the combined Q specimen.
- Near/reached descriptions appear in the provider header. Extra usage keeps its
  explanation row without an additional warning heading. Not read yet, Not signed
  in and No working connection have explicit header labels.
- Hints use Claude Code, localized Pro/Plus plan presentation, the six-pixel provider
  row gap and the existing eight-pixel outer gap. Normal percentage text and blocking
  reset text use the primary text role; the normal hover item keeps its bar surface.
- The unavailable body names the actual connection and project and its check time.
  Connection-check metadata is shown for signed-out entries, without an extra row
  for not-read-yet entries. Credits/extra-usage labels and values use separate runs.
- Hint dimensions moved into the theme; provider-row spacing has a named theme
  role and regression coverage.

Validation after these changes: `powershell -File tools/check.ps1` passed. All
filtered commands listed above passed: core 5, runner 3, status bar 11, locale 4,
key coverage 1, theme 1 (**25 total**). New regressions assert retained window times
and their displayed ages, ordered unknown rows, explicit state headers, connection/
project provenance, a single-row state header, and combined credits/extra usage.
These headless assertions are not a replacement for independent design verification.
No fresh macOS check/runtime, live authenticated provider smoke test, full suite,
or app/design screenshot comparison was performed during this recovery.

Next pipeline steps must independently repeat Check, Review and Verify. Verify
must recapture M–R and close the reported majors, including partial Claude reports
with distinct per-window ages. The exact combined credits/extra-usage specimen
`b3WD6` remains visually unverified despite its headless regression. Foreground
preservation remains unverified: root must supply the longer fresh host watcher
before the verifier launches a private app; retain its final conclusive judgment.

## Recovery after malformed Codex snapshot Review finding

The saved implementation, earlier UI/order/per-window-timestamp fixes, and all
Review/Verify evidence were preserved. This recovery does not close an independent
Review or Verify verdict by itself.

`codex_limits` now requires an object containing at least one validated window.
Absent/null/scalar/array/empty payloads, empty window snapshots, malformed windows,
and malformed optional reset/state/credits/plan fields produce no quota reading.
A newer such event therefore cannot replace the last valid snapshot or refresh its
source time. A credits-only payload is not treated as a replacement quota snapshot.
The new core regression exercises normal, warning and blocked states, retains the
percentage/reset/report time, asserts that age still advances, and verifies expiry
at the original reset. Missing `rate_limits` also produces no reading.

The fake Codex exec no longer emits session quota events on stdout. It writes a
fresh quota event and a newer null-payload event to the matching UUID session log
under a disposable project's explicit `CODEX_HOME`. The production scheduler/worker
is held running until the test receives the file-sourced IPC reading, then released.
The test checks the matching filename, exact report timestamp and percentage, and
that quota metadata is absent from the persisted transcript. This tests the actual
worker/session-file-reader/IPC path with a fake CLI; it is not an authenticated
Codex model turn. The captured genuine payload reader test remains in place.

The runner's optional `chrono` workspace dependency is enabled only by `test-agent`,
so the fake CLI can format fresh UTC session timestamps rather than frozen/future
fixture times. The normal runner build does not enable this optional dependency.

Validation: `powershell -File tools/check.ps1` passed; the listed filtered commands
passed core 6, runner 3, desktop status bar 11, locale 4, key coverage 1, theme 1
(**26 total**), using the supplied worktree-scoped Cargo target without overrides.
No new live provider/profile changes, full test suite, macOS runtime test, or design
verification were performed. No source reset, merge or implementer commit was made.

Outside-run account capability remains documentation-confirmed only, as recorded
above; no live account RPC or polling is claimed. Next steps must independently run
fresh Check, Review and Verify, including M–R, distinct Claude window ages, and the
combined credits/extra-usage specimen. Before any private verifier launch, the
pinned desktop/runner pair must be ready and root must then provide a fresh host
watcher. Preserve its conclusive foreground verdict alongside the new captures.
