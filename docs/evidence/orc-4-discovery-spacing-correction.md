# ORC-4 · Discovery card correction after Verify attempt 5

Candidate: HEAD `14fa0d15e23ed7fae48e4ab376880672a4de17a8` plus this step's seven saved source/test changes, identified below. No commit was made. This evidence document was added after the checks and changes no build input. Earlier verification artifacts remain unchanged and describe their earlier candidate.

## Correction

Read the returned report at `.tmp/verify/instructions-14fa0d15/report.md` and the live Instructions layer `fePhH` through read-only Pencil Get and a screenshot of `B5A4Ux`. No design mutation or native app launch occurred.

Pencil's empty-state `sG2xi` has a 32px gap between the actions and discovery card `T1w9U`. The card is 228px high: 12/16/8/16 padding, 6px gaps, a 23px heading, four 32px rows starting 38px apart and a 27px footer. The screen now follows that geometry instead of a 14px gap and rows padded only above. The unframed Instructions empty state's text gaps now follow the frame's 6px spacing.

The same card now includes the repository glyph, a right-aligned repository path through the existing truncated-text/tooltip control, file/folder row glyphs and the footer shield. Reference-only instruction copy displays the scanner's actual targets and @references; skill folder copy names the Skills destination. All copy remains localized, all added visual values live in the Instructions theme, and scanning/import rules remain in the shared domain code. The deprecated app was not changed.

The new headless discovery regression uses two instruction files and four private skills. It checks the observed card height, action gap and row rhythm, six-item count, reference/destination copy and repository path, then opens import review and cancels without creating drafts.

## Commands and outcomes

Commands ran in this worktree. Repeated identical commands are grouped.

| Command | Outcome |
|---|---|
| `git status --short; git rev-parse HEAD` | PASS; initially clean at the HEAD above. Git warned that the global ignore file was inaccessible. |
| `cargo +nightly fmt -p orchester-app` | PASS on each of four executions; final run formatted the completed source and regression. |
| `cargo test -p orchester-app --test instructions instructions_` | Initial intermediate run failed compilation because the new header required mutable Ctx while its caller still took shared Ctx; unused import/variable warnings were also corrected. Second run PASS: 37 tests. Final saved candidate PASS: 38 tests, 0 failed, including the discovery regression and removed-step listing/reassignment. |
| `git diff --check; git diff --stat; git diff -- crates/desktop-app/src/screens/instructions/empty.rs crates/desktop-app/src/screens/instructions/import.rs crates/desktop-app/src/ui/state.rs crates/desktop-app/src/theme/instructions.rs crates/desktop-app/assets/locales/en/instructions.json` | PASS; inspected the scoped production changes. |
| `git status --short` | PASS; intended scoped changes only. |
| `cargo +nightly fmt -p orchester-app -- --check` | PASS on the final saved Rust candidate. |
| `git diff --check` | PASS on each execution, including after adding this document. |
| `cargo test -p orchester-app --test i18n locales` | PASS: 3 tests, 0 failed. |
| `cargo test -p orchester-app --test i18n an_area` | PASS: 1 test, 0 failed. |
| `cargo test -p orchester-app --test i18n every_key` | PASS: 2 tests, 0 failed. |
| `cargo test -p orchester-app --test theme spacing_radii_and_borders_follow_the_design` | PASS: 1 test, 0 failed. |
| `cargo clippy -p orchester-app --all-targets -- -D warnings` | PASS. |
| `git hash-object -- crates/desktop-app/src/screens/instructions/empty.rs crates/desktop-app/src/screens/instructions/import.rs crates/desktop-app/src/theme/instructions.rs crates/desktop-app/src/ui/state.rs crates/desktop-app/assets/locales/en/instructions.json crates/desktop-app/tests/instructions/discovery.rs crates/desktop-app/tests/instructions/main.rs; git diff --check; git status --short` | PASS; source identities below and only intended changes. |

Did not run `tools/check.ps1`: the authoritative pipeline Check owns it for this step. Did not run unfiltered tests, workspace/deprecated-app tests, the whole workspace suite, standalone cargo check, macOS cross-compilation or native execution, native screenshots, real system clipboard checks, or the out-of-scope 960/light passes. No earlier check or verification was reused as a verdict on the changed candidate. The task remains pending independent design acceptance; docs/plan.md was not moved to Done.

## Independent Verify inventory

Compare the current saved candidate at 1440×900, dark, against Instructions `fePhH`:

| Pencil node | State |
|---|---|
| `B5A4Ux`, especially `sG2xi` / `T1w9U` | Empty project with AGENTS.md, a CLAUDE.md @reference and two skills in each folder. Recheck action-to-card gap, card padding/height, row rhythm, icons, repository path/truncation, references/destination copy and drafts-only footer. Also check no-findings and empty-scope states. |
| `U69ob` | Import review, selection/source metadata, reference-only copy, Cancel and Create drafts. Obtain fresh tree references before each interaction. |
| `XMlEH` | Project editor, draft/version, target chips, prompt gutter/tokens and available variable detail. |
| `J7EgbM` | Unavailable variable, publication lock and recovery policies/actions. |
| `ShcL9` | Effective preview, context selectors, included/excluded sources, View/Copy and recovery. |
| `fTuZZ` | Read-only full viewer, source/context metadata, resolved highlights, Copy and Back/Esc. |
| `udfuB` | Role library/editor and injection timing/order. |
| `w20Jf` | Step library/editor/preview; retained removed-step row under its pipeline, selection and reassignment without duplicates. |
| `GxmNs` | First draft, saved/saving/invalid/published states, publication failure retaining the draft, successful notice and dismissal. |

Also assess person-only in-place questions and the window-input lock using the established verification workflow. Attempt 5's other main states remain unverified; the passing headless tests do not establish native visual parity. Native macOS and system clipboard behavior remain unverified.

## Tested source identities

| File | Git blob |
|---|---|
| `crates/desktop-app/src/screens/instructions/empty.rs` | `b5a39e6f36e0d908c9a08eeacf84f7117dfa175f` |
| `crates/desktop-app/src/screens/instructions/import.rs` | `5ca72851389e2e525bc43a103c4d02ccf7c61a1e` |
| `crates/desktop-app/src/theme/instructions.rs` | `48a3031b5f11e4b3d7693599ef631827f2973005` |
| `crates/desktop-app/src/ui/state.rs` | `f7c763fa7147504c6022fd8b616d8370c3a4555e` |
| `crates/desktop-app/assets/locales/en/instructions.json` | `d0e72f7b8c33afe3cd7854a647626963fca81d11` |
| `crates/desktop-app/tests/instructions/discovery.rs` | `a59d52c470062972bd5115a6684b3cfd9f142c89` |
| `crates/desktop-app/tests/instructions/main.rs` | `3c181a9891a16303f15b0f987856a9b8b1ac010c` |
