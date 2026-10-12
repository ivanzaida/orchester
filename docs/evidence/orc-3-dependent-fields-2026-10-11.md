# ORC-3 dependent inspector fields — review attempt 4 correction

Candidate: base `c87ac64401880ef5fc67806c119bc4e909eb1846` plus the saved changes in
`crates/desktop-app/src/screens/pipelines/step_fields.rs`,
`crates/desktop-app/tests/pipelines/regressions.rs`, and this report. No commit was made.
The final results below cover these Rust changes. Earlier results describe only their stated inputs.

## Correction

The review finding reproduced in the headless window on the original production code.
After retries were set to 3, choosing Fail the run and restoring the route displayed 3
although the domain reset it to 1. Agent → tool → agent retained the old displayed model override
although the domain cleared it.

The executor watcher now reads the resulting model choice after applying the workspace transition.
A newly enabled failure route synchronizes both its retry signal and cached text to the domain's retry count.
Removing a route clears its cached retry text and validation error. Changing only a route target preserves
valid retry counts and unfinished invalid text. No domain defaults or rules were changed or duplicated.
Ordinary autosaves keep the existing component keys.

Dependent signal notifications happen after releasing the editor-state update: lurq watchers can run
synchronously and acquire that state themselves. The first implementation notified under the lock and
stalled the affected tests; that intermediate implementation was corrected before the final passing run.

Three added headless tests assert displayed values, saved database values, behavior after switching
inspector panels, preservation on same-kind changes, and clearing obsolete validation errors.
The existing focus/caret/undo autosave regression also passes.

## Commands and outcomes

| Command | Input and outcome |
| --- | --- |
| `git status --short` | Initial candidate clean. |
| `git rev-parse HEAD` | Base c87ac64401880ef5fc67806c119bc4e909eb1846. |
| `cargo test -p orchester-app --test pipelines pipelines_failure_route_reset_keeps_displayed_and_saved_retries_in_sync` | Original production code plus new test: FAIL as expected, displayed 3 instead of 1. |
| `cargo test -p orchester-app --test pipelines pipelines_executor_reset_keeps_displayed_and_saved_model_in_sync` | Original production code plus new test: FAIL as expected, selector did not show From the profile. |
| `cargo +nightly fmt -p orchester-app` | PASS on both intermediate and final Rust edits. |
| `cargo test -p orchester-app --test pipelines pipelines_` | First correction: 9 completed, 4 stalled; terminated our test process. Final correction: PASS, all 13 tests. |
| `powershell -File tools/check.ps1` | Intermediate correction: PASS (did not detect the runtime stall). Final correction: PASS, formatting, workspace/all-targets/all-features clippy, file sizes, UI literal checks. |
| `git diff --stat` | Inspected intermediate and final Rust scope: only step_fields.rs and regressions.rs. |
| `git diff --check` | PASS on intermediate and final edits, including the final report. |
| `git diff -- crates/desktop-app/src/screens/pipelines/step_fields.rs` | Inspected the production correction. |
| `Get-CimInstance Win32_Process -Filter "Name LIKE 'pipelines-%'" \| Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine` | Identified our stalled test process, PID 38020, in this worktree's target directory. |
| `Stop-Process -Id 38020` | Stopped only that test process; its cargo test command exited 1. |

Not run: full test suites; unchanged workspace, core, store, MCP, navigation, route, screen, i18n,
theme and UI-kit tests; standalone cargo check; native graphical or macOS checks; 960 or light passes.
The quality gate runs clippy across the workspace, all targets and all features on this Windows host.
No installed app, database, or process was touched.

## Independent Verify handoff

Read the Pencil layers `i4jt6V` and `vEDYY` through read-only MCP operations. Read the editor's text
and inspected its screenshot (`SVBQi`). No design edits or independent visual-parity verdict were made.

Prioritize `SVBQi`'s selected-step inspector: set retries to 3, choose Fail the run, restore a route,
and compare the displayed 1 with the saved/reopened value. Repeat after invalid retry text and an
inspector-panel switch. Change only the route target and confirm it keeps the entered retries.
Choose a model override, switch agent → tool → agent, and confirm From the profile is displayed and
saved. Same-kind agent changes should retain the override. Check native focus/caret/undo across autosave.

The remaining ORC-3 independent comparison states are unchanged:

- Library, empty library, New pipeline page and slug collision: `GaNsb`, `sI5K0`, `lAQis`, `Hgequ`.
- Selected step, no selection, empty pipeline and missing pieces: `SVBQi`, `c24Tv`, `Z26ZFs`, `Clxs2`;
  step components: `r6PIM`, `Co8O6`, `qXE7Z`.
- Publish blocked, review, published version, history comparison and history states:
  `c7g6Qi`, `ITC0R`, `nmY08`, `OchSR`, `qCku8`.
- Settings open, changed and supported states: `x4IIjM`, `Yx32O`, supported portions of `fmnva`.
- Leave confirmation: `RWODj`.

New pipeline remains a page; settings proposals remain unavailable; widening confirmations keep the
person-only input lock. Move ORC-3 to Done only after independent Review and Verify pass.
The previously recorded bare-lurq middle-insertion concern remains for native verification;
this correction does not claim to resolve it. The 960 and light passes remain out of scope.
