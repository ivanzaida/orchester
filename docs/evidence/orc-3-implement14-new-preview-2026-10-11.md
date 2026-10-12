# ORC-3 Implement14 — New pipeline preview

Scoped correction for Verify14 V14-M1. Read the original report at
`.tmp/verify/orc3-verify14-6b290ddf/report.md`; earlier evidence remains unchanged.

Candidate: uncommitted work over `6b290ddf5af92f12788ce8b772910f4161532b88`.
Implementation/test Git blob identities:

- `crates/desktop-app/src/screens/pipelines/new_pipeline.rs`: `19735898548641d2ad706fc39584d9fcd30b5712`
- `crates/desktop-app/src/screens/pipelines/library.rs`: `f9807e097094add1537ff0b62cddea529afd5df0`
- `crates/desktop-app/tests/pipelines/layout.rs`: `cb673431df7b6afb9bf4dfe4f6317700c99bc01b`

## Change

The shared starter preview now takes a compact presentation flag. New pipeline uses inline
Run checks and Review chips with the existing translated parallel marker; the empty library
retains the exact grouped branch. Both continue to use actual starter dependency levels.
Existing theme roles and i18n keys are reused. No domain, permission, recovery, history,
publication-link, graph-card, profile or shared UI-kit behavior changed.

Read-only Pencil Get and screenshots covered `lAQis` / `Iewpp` and `sI5K0` in the
person's open `orchester2.pen`. The design was not changed.

## Checks on this candidate

- `cargo test -p orchester-app --test pipelines pipelines_new_starter_preview_stays_compact_across_creation_states`:
  PASS, 1 test. Covers default, collision and Start empty; starter width 560 and height within
  3 logical pixels of 103, inline parallel marker present, grouped container absent.
  An initial invocation before the new test was included ran 0 tests (41 filtered out);
  it is not counted as coverage. The subsequent invocation above ran and passed the test.
- `cargo test -p orchester-app --test pipelines pipelines_empty_library_entries_select_the_requested_start_on_the_creation_page`:
  PASS, 1 test. Existing grouped preview and both entry choices remain covered.
- `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists`:
  PASS, 1 test.
- `powershell -File tools/check.ps1`: PASS, exit 0, `check: OK`.
  Includes nightly formatting, workspace/all-target/all-feature clippy, file size and UI literals.
- `git diff --check`: PASS.
- Git status, HEAD, diff/stat and hash-object inspections: succeeded; no commit created.

Not run: full suites, workspace domain tests (domain inputs unchanged), native QA,
protected native decisions, host-focus checks, macOS checks/runtime, 960 or light.
Headless layout assertions do not establish native design acceptance.
No app was started, controlled or stopped; no classifier retry or alternative cleanup occurred.

## Handoff to Review and Verify

Review only this scoped correction. ORC-3 remains pending the separate independent Verify step.

Fresh same-size native/Pencil comparison on the exact candidate must cover:

- New pipeline default, empty name and Starter selected: `lAQis`, option `Iewpp`
  (560 × 103), compact parallel chips and the following choices' vertical positions.
- Collision: `Hgequ`, retaining the same compact starter preview.
- Start empty selected, both from the page and the empty-library entry: `lAQis`
  as the page reference; confirm the starter preview stays compact when unselected.
- Empty-library regression: `sI5K0`, grouped parallel preview, both creation actions
  and draft-only reassurance unchanged.

Retain all prior domain/reset/recovery safeguards, replacement guards, F1–F4 and H3;
200-character publication-link bounds, graph-card separation, compact permissions and
first-v1 history are outside this diff. Open profile remains explicitly unavailable pending ORC-2.
Person-only, no-frame and unsupported states remain Unverified; carry forward Verify14's other
explicit Unverified limits. Do not relabel earlier screenshots as evidence for this candidate.
