# ORC-4 · Missing-step listing correction after Review attempt 4

Candidate: HEAD `320f2bedf8bb552b719bb184eb5a9d7a80697c10` plus this step's saved changes. No commit was made. The source identities below identify the tested changes; this report was added afterwards and changes no build inputs.

## Correction

The Steps library previously omitted instructions when their pipeline still existed but its current draft no longer contained their step. It now appends those instructions under their pipeline group after the current steps, using the existing selectable rule row. Instructions with missing pipelines retain their existing fallback. No domain lifecycle rule, string, style, locale, deprecated-app code or design file changed.

The new headless regression in `tests/instructions/library.rs` creates a published pipeline, saves a draft removing one step, and keeps another instruction selected by default. It verifies that the removed-step rule remains visible, opens from its library row, and can be reassigned to the surviving step with its draft saved and published v1 retained. It also checks that the published pipeline still has the removed step, ensuring the fixture exercises the current draft.

## Commands and outcomes

All commands below ran in this worktree. Repeated identical commands are grouped.

| Command | Outcome |
|---|---|
| `git status --short; git rev-parse HEAD` | PASS; started clean at the HEAD above. Git emitted permission warnings for the global ignore file. |
| `cargo test -p orchester-app --test instructions instructions_removed_step_rule` | Expected FAIL before the fix: 0 passed, 1 failed, 36 filtered out; failed specifically at the missing library row assertion. This is reproduction evidence for the intermediate candidate. |
| `cargo +nightly fmt -p orchester-app` | PASS. |
| `cargo test -p orchester-app --test instructions instructions_` | PASS on final saved Rust source: 37 passed, 0 failed, 0 ignored, including the new regression. |
| `git diff --check; git diff --stat` | PASS; tracked diff was limited to the grouping code and test module registration. The new regression file was untracked and is included in the identity table below. |
| `git diff -- crates/desktop-app/src/screens/instructions/library_groups.rs crates/desktop-app/tests/instructions/main.rs` | PASS; inspected the scoped change. |
| `cargo +nightly fmt -p orchester-app -- --check` | PASS on final saved Rust source. |
| `cargo clippy -p orchester-app --all-targets -- -D warnings` | PASS on final saved Rust source. |
| `git hash-object -- crates/desktop-app/src/screens/instructions/library_groups.rs crates/desktop-app/tests/instructions/main.rs crates/desktop-app/tests/instructions/library.rs` | PASS; hashes below. |
| `git status --short` | PASS; only the three intended source/test files changed before this report was added. |
| `git diff --check` | PASS after adding this report. |

Did not run `tools/check.ps1`: the authoritative pipeline Check owns it for this step. Did not run unfiltered tests, standalone workspace tests, the whole workspace suite, standalone cargo check, macOS cross-check or native execution, native app screenshots, real clipboard checks, scanner fixtures outside the Instructions test binary, or the out-of-scope 960/light passes. Earlier evidence remains unchanged and is not a verification verdict on this candidate.

## Independent Verify inventory

Read the current design through Pencil only. This step used read-only Get of Instructions `fePhH` and a Pencil screenshot of `w20Jf`; it did not modify the design or launch an app. Independent native verification and pipeline Check remain to be run on this candidate.

Compare at 1440×900, dark:

| Pencil node | State |
|---|---|
| `w20Jf` | Step overrides library, pipeline version/count, numbered current steps and role labels, step editor and preview. Additionally remove a step from an existing pipeline draft with another rule selected: confirm the removed-step rule remains visible under its pipeline, opens and can be reassigned. There is no dedicated removed-step frame; assess the row against the existing rule master `zmP3o`. Check that reassignment places it under its current step without duplicate rows. |
| `XMlEH` | Project editor, draft/version, target chips, prompt tokens and available variable detail. |
| `J7EgbM` | Unavailable variable warning, locked publication and recovery policies. |
| `ShcL9` | Effective preview, context selectors, included/excluded sources, View/Copy and context error recovery. |
| `fTuZZ` | Read-only full viewer, context/source metadata, resolved highlights, Copy and Back/Esc; context loss and recovery. |
| `udfuB` | Agent-role library/editor, injection timing/order and published preview. |
| `B5A4Ux` | Empty scope/project and repository discovery. |
| `U69ob` | Import review page, selection/source metadata, Cancel and creating drafts. |
| `GxmNs` | First draft, saved/saving/invalid/published states, retained draft after publication failure, successful inline notice and dismissal. |

The detailed existing inventory in [the previous correction report](orc-4-design-corrections.md#independent-verify-inventory) remains useful for reaching these states. Its test and source identities describe its earlier candidate only.

## Tested source identities

| File | Git blob |
|---|---|
| `crates/desktop-app/src/screens/instructions/library_groups.rs` | `f9577a63f8837827814ce215ced5c2eb3fef1ce9` |
| `crates/desktop-app/tests/instructions/main.rs` | `34da5601dc633cb9888de95df64eccd039b2c674` |
| `crates/desktop-app/tests/instructions/library.rs` | `429b90819c39daa904aeee403dfc6aed1fe51eb2` |
