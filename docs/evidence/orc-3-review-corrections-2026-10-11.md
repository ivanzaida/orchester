# ORC-3 review corrections — 2026-10-11

Candidate: checkpoint `4e6db3b3` plus this worktree's uncommitted changes to
`crates/desktop-app/tests/pipelines/regressions.rs`, `crates/desktop-app/tests/pipelines_support/mod.rs`
and this report. Production code is unchanged from that checkpoint. Earlier evidence in
`orc-3-pipelines-implementation.md` remains historical, not a fresh verdict on this candidate.

## Findings checked

1. **Unassigned starter drafts:** the saved correction in workspace's `narrowing::check_step` allows
   Write on an unassigned starter while retaining checks on assigned profiles. Store publication still
   rejects missing executors. The inspector exposes the existing Write toggle so it can be removed.
   Both the UI regression and workspace write test cover editing the starter and refusing publication.
2. **Autosave component identity:** `Editing::saved` updates the saved snapshot without incrementing
   `epoch`; summary, settings and step input mount keys therefore survive ordinary saves. The revised
   keyboard regression types distinct values in all three surfaces, requires a pending save, checks the
   database value and focused element after completion, inserts at the pre-save caret without refocusing,
   and undoes both the post-save insertion and a pre-save character.
3. **Leaving pending work:** the saved `Navigation` guard holds Back, sidebar/history navigation and tab
   departures while edits, invalid fields or a write are pending. The in-place question keeps editing or
   explicitly discards and leaves. The existing regressions cover those paths and the person-only save lock.

## Why the caret test changed

The original test failed with `abxcy` instead of `abxyc`. The caret had already moved before autosave:
`a b c ArrowLeft x q` produced `abxcq` immediately. This also reproduces with a bare
`TextInput::styled(...).textarea().rows(1, 12)` mounted as the root, with no pipeline components,
watchers or saves. Using read-only value inspection, putting a headless pass between key-down/up,
omitting key-up, and using pointer focus did not remove it. This observation is limited to lurq 0.42's
headless path; no conclusion about a running native window is claimed.

The regression now establishes the caret with ArrowLeft immediately before autosave. It still verifies
focus, caret position and undo across a real save. Distinct values prevent the settings case from
accidentally matching the previously saved description and skipping the save. The value reader uses
immutable element text inspection; an empty field has no text content.

As a negative control, temporarily restoring `self.epoch += 1` in `Editing::saved` made the revised test
fail at its focus assertion (`None` instead of `pipeline-description`). That mutation was reverted.
No dependency internals or other files outside the worktree were read. The unrelated headless
middle-insertion behavior is reported for follow-up, not repaired by an app workaround. Its diagnostic
source is `.tmp/orc-3-caret/standalone-repro.rs` (scratch, not committed); the reproduction is:

```rust
// Mount a component holding ctx.signal(String::new()) and render only:
TextInput::styled(value.clone(), resolved_description_style)
  .id("input").textarea().rows(1, 12)
// Focus input. For each key below, call Tree::key_down_with_meta and pass_headless:
// "a", "b", "c", "ArrowLeft", "x", "q"
// Immutable input.text_content(): actual "abxcq", expected "abxqc".
```

## Validation

These commands ran in this attempt. After the final helper-only `unwrap` → `expect` correction,
the pipeline tests and full gate ran again; the other six test commands' inputs were unchanged and their
passing results are retained. No older attempt's checks are used as a verdict here:

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS, 10 tests. The first run after changing the value reader was 9/10: empty input text was `None`; the reader was corrected and all 10 passed. |
| `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes` | PASS, 2 tests |
| `cargo test -p orchester-app --test navigation history` | PASS, 5 tests |
| `cargo test -p orchester-app --test routes pipelines_` | PASS, 1 test |
| `cargo test -p orchester-app --test screens screens_` | PASS, 2 tests |
| `cargo test -p orchester-app --test i18n locales_` | PASS, 3 tests |
| `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists` | PASS, 1 test |
| `cargo +nightly fmt -p orchester-app` | PASS |
| `powershell -File tools/check.ps1` | PASS on final Rust inputs: formatting, workspace clippy, file sizes and UI literals. The earlier run rejected two `unwrap()` calls in the new helper; descriptive `expect()` calls fixed them. The pipeline tests and complete gate were rerun and passed. |
| `git diff --check` | PASS |
| `git status --short`, `git log -1 --oneline`, `git diff --stat` | PASS, inspection only |
| `git diff -- crates/desktop-app/tests/pipelines/regressions.rs crates/desktop-app/tests/pipelines_support/mod.rs` | PASS, inspected the saved test changes |

Diagnostic commands, before final validation (failures describe their stated inputs only):

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_keyboard_focus_caret_and_undo_survive_autosave` | Three failures with the original/pre-save-insertion scenario (baseline, immutable reader, explicit pre-save assertion); two passes with the caret positioned immediately before saving (first the revised scenario, then distinct values plus database/focus assertions); expected failure with the old epoch-increment bug temporarily restored. The restored production fix subsequently passes in the final `pipelines_` run. |
| `cargo test -p orchester-app --test pipelines pipelines_keyboard_harness_preserves_middle_edits` | Failed with a standalone shared field and again with bare lurq TextInput. Diagnostic code retained only in scratch and described above. |
| `cargo test -p orchester-app --test pipelines pipelines_keyboard_` | Three diagnostic runs failed: adding a pass between key-down/up, removing key-up, then pointer focus. These harness experiments were reverted. |

Not run in this attempt: full/unfiltered suites, graphical app tests or independent screenshot comparison,
macOS compilation/runtime, 960/light verification, separate cargo check, or the unchanged MCP/store/UI-kit/theme
suites. The pipeline's later Check is authoritative; independent review and Verify still follow this Implement step.

## Verify handoff

Pencil was accessed only through read operations: `Get` for layers `i4jt6V` and `vEDYY`, and screenshots
of `SVBQi` and `Yx32O`. The person's design was not changed. This is not independent visual verification.
The later Verify step must compare dark desktop states at each frame's dimensions:

- Library/empty/new/collision: `GaNsb`, `sI5K0`, `lAQis`, `Hgequ`.
- Selected step, summary, empty graph and executor states: `SVBQi`, `c24Tv`, `Z26ZFs`, `Clxs2`.
- Cards, badges, parallel groups: `r6PIM`, `Co8O6`, `qXE7Z`.
- Publish blocked/review/published/history: `c7g6Qi`, `ITC0R`, `nmY08`, `OchSR`, `qCku8`.
- Settings and edited description: `x4IIjM`, `Yx32O`, supported parts of `fmnva`.
- Pending navigation's in-place question: confirmation master `RWODj` within the editor. Verify Keep
  retains pending/invalid text and explicit Leave discards it; verify the person-only widening lock.
- Exercise actual native keyboard focus, caret and undo over autosave on all three edited fields;
  also investigate repeated middle insertion in the native window before attributing the headless finding.

New pipeline remains a page. `D0hfv` proposals remain visibly unavailable. Do not move ORC-3 to Done
until independent review and Verify pass. The 960 and light passes remain out of scope.
