# D5 design verification path (2026-09-29)

This is a read-only audit and a **proposal**, not an existing Pencil design or a completed UI
comparison. Pencil was read through its MCP (`get_editor_state` with schema, `batch_get`,
`snapshot_layout`, and a screenshot of `l2YA4`); `data/orchester.pen` was not read from disk or
edited. No app was launched.

## Closest existing design and the missing states

- New task `l2YA4` is a 1440 × 960 screen in `D4pfd` (Screens · Desktop 1440), under section
  `g7ncNr` (02 / Tasks & runs). Its right execution plan `AoFj6` is 336 wide. Create & run is
  `Hc1Cp`; Save draft is `Q4xE7q`.
- Project settings › Environment `tXahd` is where `CARGO_TARGET_DIR` is entered. It shows the
  shared-target explanation but no disk-space error state. The compact recovery component
  `iN90G` is a nearby pattern, but the D5 app displays these errors on New task, directly under
  Create & run, through `wrapping_field_error`.
- Live Pencil searches and reads found no exact New task frame for either a low-space target or
  a target whose free space cannot be measured. Neither can currently receive the required
  same-state, same-size Pencil/app comparison.

**Proposed Pencil addition:** duplicate `l2YA4` twice as full 1440 × 960 screens in a new
`States · New task preflight` group under `g7ncNr`. The section currently ends at y=20648; a
new 4896 × 1120 group at x=0, y=20648 could hold labels at (48, 68) and (1568, 68), and
screens at (48, 100) and (1568, 100). Grow the section to fit. Name the proposed states
`02.09A · New task / Low build space` and `02.09B · New task / Cannot measure build space`.
These names and coordinates specify a placement for a future design edit; they are **not node
IDs or existing frames**.

Keep the form, sidebar and pipeline from `l2YA4`, but show the saved queued state that the
verifier reached: the `Queued · run not started` header, `Retry run` in place of Create & run,
and no Save draft action. Place one wrapping error below Retry run: a 12 px `circle-alert` and
11 px normal Inter text in `$text`, with the established small theme spacing, matching the
existing field-error pattern. Use the shipped English copy templates from
`runs.preflight.buildSpaceLow` and `runs.preflight.buildSpaceUnavailable`, with an illustrative
absolute target path and, for low space, illustrative `available` and `required` GiB values.
Such fixture values must never become app defaults. Retry stays available after the target is
corrected.

**Geometry risk:** `AoFj6` currently has only a 26 px flexible spacer before the 42 px button.
A long absolute path can make either error wrap to several lines in the 308 px content width.
The designer must show the complete error and Retry run inside the 960 px screen without
clipping, then export the full-size frames for review. The app's low-space state has since been
captured, but cannot be compared with a matching Pencil frame yet.

## Safe reproduction and remaining gate

For **Cannot measure**, use a verifier-owned disposable Git project under `.tmp/verify/d5/`.
Create a regular file such as `<repo>/not-a-directory`, then configure the project's absolute
`CARGO_TARGET_DIR` as `<repo>/not-a-directory/target` in Environment. Create & run a valid task.
The runner test `environment_space_preflight_reports_an_unmeasurable_target_before_creating_a_run`
already proves this produces typed `BuildSpaceUnavailable` before a run or worktree is created.
The task may have been saved and queued before the runner rejects its start, so a design must not
claim that nothing was saved. Three independent app runs reproduced this state; see
`docs/evidence/d5-live-reproduction-2026-09-29.md`.

For **Low space**, a verifier used an empty absolute target on H: while the host had 2.643 GiB
free, below the 10 GiB cold-target threshold. This reached the real host preflight without filling
the disk or adding a test-only path. The independent exact-PID app capture shows a complete error,
one queued task, and no run or worktree; see `.tmp/verify/d5-live-low/report.md`. Its visual
comparison still requires an exact Pencil frame.

An independent verifier should export both exact Pencil frames from the live editor, run a
pinned candidate app and runner with a disposable project, drive only that app's embedded MCP,
and capture each reachable error at a 1440 × 1020 logical window size (32 px title bar + 960 px
screen + 28 px status bar). Compare the PNGs at the same pixel size, region by region: complete
error text and path wrapping, icon, spacing, button and Save draft, other form content, and
window chrome. Record screenshot paths and every mismatch. An unreached state is unverified.

Sources: `docs/design/orchester-workspace.md` (02.09, Project settings › Environment),
`docs/evidence/d5-status-2026-09-29.md`, and the D5 candidate branch
`codex/d5-free-space` (`docs/evidence/d5-free-space-2026-09-29.md` and its
`crates/desktop/src/screens/project/new_task/plan.rs`, `crates/desktop/src/tasks/run.rs`,
`crates/runner/src/execution/build_space.rs`, and `crates/runner/tests/environment.rs`).
