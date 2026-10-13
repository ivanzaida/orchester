# ORC-4: corrections for Verify attempt 6

Implement step only. Candidate: HEAD `4a888c7de27322b9bfb9082fd797b00ee8859c3c` plus the saved changes identified below.
The task is not accepted, merged, or moved to Done. Pipeline Check, independent Review and independent native Verify remain required.

The original failed report and captures at `.tmp/verify/instructions-4a888c7d/report.md` are unchanged and cover only their original build.
Read that full report and its paired step-library/publication-failure screenshots. Read live Pencil Instructions through read-only Get and screenshots of `w20Jf` and `NPZYy` (the failure sample within `GxmNs`). No design mutation occurred.
The skills registry exposed only codebase-index; the Rust/UI skills and both lurq-ui references were read through the worktree read tool after the named-skill lookup failed.

## Scope and changes

Only the two reported major findings were corrected.

- M1: the step library renders headings/rows only for steps with retained instruction records. Pipelines without overrides keep their heading and No overrides count. Removed-step and removed-owner records remain reachable, and current steps retain their original ordinals.
- M2: publication failures have a notice inset 20px at each side and 12px below the editor header. The existing shared Notice has a stacked action mode matching SSbRV: explanation and retry align with the title, after the icon; its sides are 14px and its top/bottom are 12px. Ordinary publication failure presents Retry without the separate Reload action. Conflicts and input changed after failure retain explicit Reload recovery.
- Save/create/reload error presentation, domain write checks, i18n/theme values, person-only policy, deprecated app and the discovery-card fix were preserved.
- Generic failure copy and the other recorded minor findings remain unchanged under the person's precise two-major-finding scope.

Four regressions were added: empty-step filtering with empty pipeline headings, failure inset/inner alignment and successful retry preserving the draft/version, publication-conflict recovery, and recovery after editing a rejected publication. The existing removed-step/reassignment and save/reload tests still pass.

## Checks and commands

Commands ran in this existing worktree. No new worktree or commit was made.

| Command | Outcome |
| --- | --- |
| `git status --short; git rev-parse HEAD` | Passed; initially clean 4a888c7d. Git warned that its per-user ignore file was inaccessible. |
| `cargo fmt --all -- --check` | Failed before formatting: Windows command length, OS error 206. |
| `cargo fmt -p orchester-app -- --check` | Failed pre-format check; stable rustfmt also warned about nightly-only settings. |
| `rustfmt +nightly --edition 2024 crates/desktop-app/src/screens/instructions/editor.rs crates/desktop-app/src/screens/instructions/work.rs crates/desktop-app/src/screens/instructions/library_groups.rs crates/desktop-app/src/ui/notice.rs crates/desktop-app/tests/instructions/library.rs crates/desktop-app/tests/instructions/publication_layout.rs` | Passed. |
| `cargo test -p orchester-app --test instructions instructions_` | Interim candidate: 40 passed. Repeated after the recovery refinements: final candidate, 42 passed, 0 failed. No native windows, runner or domain runs started. |
| `git diff --stat; git diff --check` | Passed; interim diff. |
| `cargo clippy -p orchester-app --all-targets --all-features -- -D warnings` | Passed on interim candidate; repeated and passed on final candidate. |
| `git diff -- crates/desktop-app/src/ui/notice.rs crates/desktop-app/src/screens/instructions/editor.rs` | Passed; reviewed the scoped changes. |
| `rustfmt +nightly --edition 2024 crates/desktop-app/src/screens/instructions/editor.rs crates/desktop-app/tests/instructions/publication_layout.rs` | Passed after recovery refinements. |
| `cargo test -p orchester-app --test ui_kit a_notice` | Final candidate: 1 passed, 0 failed, 51 filtered out. Confirms the existing inline Notice layout. |
| `cargo +nightly fmt -p orchester-app -- --check` | Passed on final candidate. |
| `git diff --check; git status --short` | Passed; the scoped saved changes and new regression file listed. |
| `git hash-object crates/desktop-app/src/screens/instructions/editor.rs crates/desktop-app/src/screens/instructions/library_groups.rs crates/desktop-app/src/screens/instructions/work.rs crates/desktop-app/src/ui/notice.rs crates/desktop-app/tests/instructions/library.rs crates/desktop-app/tests/instructions/main.rs crates/desktop-app/tests/instructions/publication_layout.rs` | Passed; final source identities below. |
| `git diff --check` | Passed after this evidence file was added. |

Not run: `tools/check.ps1` (the pipeline Check owns it, per this step's instruction), independent source review, native QA launch/input/capture, system clipboard, native Windows/macOS verification, macOS cross-check/clippy, 960/light passes, full workspace/test suites, separate i18n/theme/screen-registration tests (those files are unchanged), installed-app checks or new domain runs. No app was launched or stopped, and no QA lease was consumed here.

## Final source identities

Git blob hashes of all seven changed/new Rust files after final formatting; the 42-test result, UI-kit result, strict Clippy and nightly fmt result cover these inputs.

| File (under crates/desktop-app) | Git blob |
| --- | --- |
| src/screens/instructions/editor.rs | 54ae8a7cee4e276f5354bc64ad1823ffeaf2f395 |
| src/screens/instructions/library_groups.rs | c16ba938cc743a5cc2c22381693ecf7dfadf6db1 |
| src/screens/instructions/work.rs | e33d5635b5f5f16d57ff5212607afaed553c820b |
| src/ui/notice.rs | a3b1331ed7bf7dc58ce5333ac67c3bec375ace5a |
| tests/instructions/library.rs | a2beb9a93a3de7e6a5d520bdd659d5b902966a32 |
| tests/instructions/main.rs | acbe5297569a05368f58b8930455ee960d363b46 |
| tests/instructions/publication_layout.rs | 1ab295dc495d1dcd24183051fb80cbbf1b1cd665 |

## Independent verification handoff

Follow the direct HUMAN instruction: this is the one additional retry of the same original run. Before any QA launch/input/capture, obtain the one global QA lease with a fresh root reservation and the required exact PID/path/hash/literal discovery/source/window prospective acknowledgement. No parallel/automatic relaunch, alternative context, refusal bypass, person decisions, installed changes or new domain runs. Any further retry needs a new direct HUMAN decision.

Primary comparisons at 1440x900 dark, Instructions `fePhH`:

- `w20Jf`: Step overrides with two retained step rules and unrelated empty steps; the empty Quick fix pipeline heading must have no step rows. Recheck selection, counts, original step numbering and ownership-note position.
- `GxmNs`, failure sample `NPZYy`, notice `SSbRV` inside `f5Nkf`: rejected publication, notice side/top/bottom inset and title/body/retry alignment, absence of the extra Reload action for an ordinary retained draft, retained draft/version and retry success. Also compare first unpublished draft failure, published-version failure, saved/unsaved/invalid/published/success states.
- `XMlEH` and `J7EgbM`: editor normal/available-variable and unavailable-variable states; ensure the unchanged normal editor did not move.
- `ShcL9` and `fTuZZ`: preview and full viewer.
- `udfuB`: role editor.
- `B5A4Ux` and `U69ob`: discovery card and import review; preserve prior corrected discovery spacing.

Removed-step reassignment, publication conflicts and changed-input/save/reload recovery have headless coverage; the removed-step visual state has no Pencil frame. Keep prior Unverified entries and person-only states unresolved unless legitimately checked under the current authority. This implementation report establishes no native PASS and supersedes no historical failed/unverified evidence.
