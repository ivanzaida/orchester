# ORC-3 permission editing — Review attempt 6 corrections

Candidate: saved work based on `a842259689c3000ee446818b47b672498451f89f` in this run's worktree.
No commit made. This evidence covers these corrections, not an independent design-verification verdict.

## Corrections

- The inspector offers **Use profile commands and network** when the step has command or network
  restrictions. Workspace's `pipelines::narrowing::inherit_profile` restores inheritance while retaining
  the step's tool-approval gate. This explicitly recovers restrictions left over from a different
  profile; changing profiles still retains the user's restrictions until they choose to reset.
- Permission-edit errors use `<step key>:step-permissions`. Removing that step clears them through
  the existing step cleanup. Changing its executor clears its obsolete permission-edit error;
  current domain validation still checks the retained step permissions. Other fields and steps'
  errors remain.
- The existing save/publication rules and window-input lock are unchanged. Saving command/network
  inheritance within a profile follows the existing policy; publication of a weaker version still
  needs the person. Tool-step replacement still requires approval to save.
- The action uses the existing ghost button and a pipeline translation key; no theme values or
  deprecated-app code changed. Windows and macOS share the same implementation.

## Reproduction and tests

The first three headless regressions were added before the implementation. Once the test fixture
was corrected, all three failed against the unchanged production code for the reported reasons:
missing reset action, permission error retained after removal, and permission error retained after
selecting the original valid profile. After the fix, the same three tests passed.

Six new headless regressions now cover reset and subsequent narrowing, removal and autosave,
executor replacement and explicit save, keeping another step's error, preserving tool approvals
and protecting weaker publication, and the person-only save after replacement by a tool.
Two new workspace tests cover command/network reset, retaining the approval flag in both states,
and detecting weaker publication.

Tests ran with `TEMP` and `TMP` set to this worktree's `.tmp/orc-3-permissions`.
No installed database, app process or runner was used.

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_permissions_` | Initial fixture iterations: compile failure (provider variant), 3 fixture failures (save instead of create), compile failure (insert instead of create). Then expected RED: 3 regressions failed against unchanged production. First implementation compile failed after a refused MCP insertion left the workspace helper absent; insertion corrected. GREEN: 3 passed. |
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS — 21 tests, including all 6 new headless tests. Subsequent edits only formatted the tests and reordered their module declarations. |
| `cargo test -p orchester-workspace --test pipeline_narrowing pipeline_narrowing_` | PASS — 2 tests. |
| `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes_` | PASS — 3 tests. |
| `cargo test -p orchester-app --test i18n every_key_` | PASS — 2 tests. |
| `cargo test -p orchester-app --test i18n locales_` | PASS — 3 tests. |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace -- --check` | Initial formatting differences corrected through MCP edits; final PASS. |
| `cargo clippy -p orchester-app -p orchester-workspace --lib -- -D warnings` | PASS. |
| `git diff --check` | PASS. |

Read-only Git commands also ran successfully: `git status --short`, `git rev-parse HEAD`,
`git diff --stat`, `git diff --numstat`, and a scoped `git diff -- <changed production paths>`.
The initial status printed an inaccessible global-ignore warning but returned the clean worktree
and base revision. An environment query (`Get-Item Env:CARGO_TARGET_DIR,Env:TEMP,Env:TMP
-ErrorAction SilentlyContinue`) returned exit 1 because CARGO_TARGET_DIR is unset; builds used the
worktree's default target directory.

**Not run:** `tools/check.ps1`, explicitly delegated to the pipeline's authoritative Check step
by the person's latest instruction; unfiltered/full workspace suites; macOS builds or native
execution; native graphical/focus verification; 960-width and light-theme passes. The 31 passing
filtered tests above are not a substitute for Check or independent Verify.

## Independent Verify handoff

Design was accessed only through read-only Pencil Get operations and a screenshot, using the
person's open `data/orchester2.pen`. Layers `i4jt6V` and `vEDYY`, the selected inspector
`SVBQi` / `FXokQ`, missing pieces `Clxs2`, settings notes `D0hfv`, and ghost button
`cnlv1` were read; `K0WLeG` (Permissions section) was screenshotted. No design mutation.

Prioritize these new interaction states at 1440 × 900 dark:

1. In the selected-step inspector (`SVBQi`, Permissions `K0WLeG`), narrow profile A's
   [cargo check, cargo test] to [cargo check], then select profile B allowing only [cargo test].
   Verify the validation error and the reset action remain reachable within the scrolling inspector.
2. Press reset: the error clears, the command summary inherits the profile, and saving succeeds.
   The recovery action is newly required by Review and has no dedicated design frame; compare its
   style against ghost button `cnlv1` within the existing inspector structure.
3. Trigger a permission edit error and remove the step; verify the summary/empty state
   (`c24Tv` / `Z26ZFs`) and saved status recover. Repeat by replacing the executor with a
   valid profile or tool. Other invalid fields/steps must still block saving.
4. Reset a published narrowed step with tool approvals enabled: approvals stay enabled and weaker
   publication asks in place (`ITC0R`, `RWODj`), with the window-input lock.
5. Historical versions remain read-only (`OchSR`, `qCku8`), including the reset action.

The full ORC-3 comparison remains pending: library/new `GaNsb`, `sI5K0`, `lAQis`,
`Hgequ`; editor/summary/empty/errors `SVBQi`, `c24Tv`, `Z26ZFs`, `Clxs2`;
components `r6PIM`, `Co8O6`, `qXE7Z`; publication/history `c7g6Qi`, `ITC0R`,
`nmY08`, `OchSR`, `qCku8`; Settings and leave confirmation `x4IIjM`, `Yx32O`,
supported `fmnva`, `RWODj`.

Keep ORC-3 pending until independent Review and Verify pass. Earlier evidence is retained unchanged.
