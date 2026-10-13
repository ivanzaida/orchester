# ORC-4 · Corrections after Verify attempt 3

Implementation candidate: HEAD `28ce0bc4efc421abc90569c963f087cbced1433f` plus the saved worktree changes identified below. Base remains `934b85b92f05296b7e30552073eea438136010f0`. No commit was made by this Implement step.

The implementation and covering checks pass. This is **not** a native design-verification verdict. Pipeline Check must run `tools/check.ps1`; independent Verify must build this changed candidate and compare it with live Pencil. The earlier failed report in `.tmp/verify/instructions-28ce0bc4/report.md` remains evidence for that earlier binary only and was not edited.

## Implemented corrections

- Import is a dedicated in-place page: return breadcrumb, grouped instruction/skill rows, selected backgrounds, source metadata and bordered read-only source, notice, selection summary, Cancel and right-aligned Create drafts.
- Library rows have draft/new-draft/published glyphs. Ownership follows the list. Role groups and pipeline/numbered-step groups expose counts, role labels and versions read from workspace data.
- Editor headers distinguish library and editor version wording, include draft scope/version, and offer Preview prompt after publication. Target chips remove the named target, and the shared form error marks an empty Apply to field. Role/project/step event widths use Pencil's respective 220/180/150px geometry. Prompt gutter padding, tab separator and scope explanations are restored.
- Variables have context, compact summaries, chevrons/counts, a detail/value box, availability, recovery policies and a footer outside the scroll region. A searched selected variable precedes the other search results.
- Required-variable publication checks live in `orchester-workspace::instructions::publication`. The editor rechecks its chosen context through `editor::publish_for_in` immediately before publishing. The existing context-free domain publishing API and MCP schema are unchanged; this check concerns publication from the editor's selected preview, not a new global lifecycle invariant.
- Recovery replaces grammar references only, preserving escaped examples. Preview ranges identify actual substituted values with UTF-8 ranges from the same prompt grammar.
- Preview selectors are horizontal; source rows identify this rule and excluded sources. Actions share one visible row, and privacy stays outside source scrolling.
- Full viewer has breadcrumb/back, exact task/agent/step/event context, source/version/event headings, selected blocks, read-only highlighted source, resolved-value summary and Esc. If context becomes unavailable while viewing, a return action and task picker remain available; restoring context recovers the viewer.
- Save/publish states use state glyphs, an in-place write-failure notice, Preview prompt after success, and a dismissible inline publication result with active run/version details when the catalog has them.
- No deprecated-app files, scanner behavior, install, permissions, design file, or unrelated screen was changed.

## Commands and outcomes

Repeated identical commands are grouped. Earlier failures below describe intermediate inputs and were corrected; they are not verdicts on the saved candidate.

| Command | Outcome |
|---|---|
| `cargo test -p orchester-app --test instructions instructions_` | **Final PASS: 36 tests.** Earlier runs: compile failures while constructing the UI/test helpers; 32/34 with old filename/step-label assertions; then passes at 34 and 35 tests. The final 36-test run includes viewer context loss/recovery and named-chip removal. |
| `cargo test -p orchester-workspace --test instructions instructions_` | **PASS: 4 tests**, rerun after the final workspace changes. |
| `cargo test -p orchester-workspace --test instructions_tokens instructions_` | **PASS: 4 tests**, including escaped-reference recovery and exact UTF-8 rendered ranges; rerun after final workspace changes. |
| `cargo test -p orchester-app --test i18n every_key` | **Final PASS: 2 tests.** Earlier failure exposed a workspace message-key literal in UI code; it now routes through the workspace form error. An intermediate compile retry also failed before the new method was successfully inserted. |
| `cargo test -p orchester-app --test i18n locales` | **PASS: 3 tests.** Locale assets did not change after this final locale pass. |
| `cargo test -p orchester-app --test i18n an_area_file` | **PASS: 1 test.** Area ownership/locale inputs did not change afterwards. |
| `cargo test -p orchester-app --test ui_kit icons_are_sorted` | **Final PASS: 1 test.** Earlier failure found Braces before Bot; the order was corrected. One intermediate compile attempt failed while the form-error helper insertion was incomplete. Rerun after the final CornerDownRight addition. |
| `cargo test -p orchester-app --test screens screens_each_route` | **PASS: 1 test**, rerun for the final application source. Instructions remains in BUILT. |
| `cargo check -p orchester-app` | **Final PASS.** Earlier iterative checks found incomplete wiring, invalid toolkit role/type names, a missing method and a run-number type mismatch. Two intermediate successful checks still had warnings that were removed. Later strict Clippy also passes on the final source. |
| `cargo clippy -p orchester-app -p orchester-workspace --all-targets -- -D warnings` | **Final PASS on saved Rust source.** Earlier failures: unused imports/dead variant, long functions, a redundant clone, a collapsible if, test accessor/type issues, a transient parse error, and unconsumed TaskUpdate results in the new recovery fixture. All were corrected without warning allowances. |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace` | Final runs **PASS**. One intermediate run found a parse error during a function extraction; corrected before later successful formatting/checks. |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace -- --check` | **PASS** on final source. |
| `git diff --check` | **PASS**, including after the final edits. |
| `git status --short` | **PASS**; began clean and later listed this step's saved changes. |
| `git rev-parse HEAD` | **PASS**: `28ce0bc4efc421abc90569c963f087cbced1433f`, unchanged. |
| `git diff --stat` | **PASS**; inspected the correction's scope. Untracked new modules are additionally listed in the identity table. |
| `git diff -- crates/workspace/src/instructions/editor.rs crates/workspace/src/instructions/data.rs` | **PASS**; inspected shared workspace changes. |
| `git hash-object -- <candidate files below>` | **Final PASS: 42 source/locale/test identities.** Initial command failed because trimming the status output removed the first path's leading c; no file changed. Corrected capture kept status columns. Exact successful command is recorded below. |

Did **not** run: `tools/check.ps1` (the authoritative pipeline Check owns it), unfiltered cargo tests, the whole workspace test suite, macOS cross-check/native execution, native app launch/screenshots, real system-clipboard checks, 960 or light UI passes. The earlier four skipped Skills scanner-link fixtures are not newly verified here; this correction does not alter scanner inputs. The Instructions external-junction and bounded-root-file regression fixtures did run and passed in the 36-test suite.

The 36 app tests and final strict Clippy cover the saved Rust source. Workspace tests cover unchanged final workspace source; later changes were app/test-only. Locale parity/area tests are reused only for unchanged locale/ownership inputs. This report itself was added after code checks and does not alter their code inputs.

## Independent Verify inventory

Read-only live Pencil exports are under `.tmp/implement-instructions/design/`. They were read through Pencil Get/Export and inspected, with no design mutation. They are implementation references, **not** paired native verification captures.

Verify at **1440×900, dark**, layer **Instructions `fePhH`**, against the fresh binary:

| Pencil node | States to compare |
|---|---|
| `XMlEH` | Project editor; scope/draft/version state; removable target chips; prompt gutter/tokens; available task.title detail, compact groups and pinned footer. |
| `J7EgbM` | Existing run with base revision but no checkpoint; run.revision selected/search; amber detail and publication warning; locked Publish; resume policy and base-revision replacement. Confirm required values are rechecked and no publication happens while unavailable. |
| `ShcL9` | Draft/published effective preview; four horizontal context controls; this-rule selection, exclusions, View/Copy row and privacy footer. Also task-selection error/recovery, covered headlessly but without its own dedicated Pencil frame. |
| `fTuZZ` | Full viewer context, included source index, source/version/event metadata, selected source, actual resolved-value highlight, read-only input, Copy, source/variable footer and Esc/Back. Also context disappearing while open, returning and recovering (no dedicated error frame). |
| `udfuB` | Published agent-role editor, grouped library, role/injection field proportions, injection-order note and Preview prompt. |
| `w20Jf` | Pipeline version/count and numbered step hierarchy with roles; exactly three step-setting fields, injection-order note and preview. |
| `B5A4Ux` | Empty scope/project, including discovered repository files; recapture with fixtures present rather than inferring a card from absent files. |
| `U69ob` | Dedicated import page, grouped/selected/skipped rows, selected instruction/skill metadata, read-only source and line count, drafts-only notice, selection summary and both footer actions. Cancel/return; creating selected drafts. |
| `GxmNs` | Published; saved draft on published version; saving; invalid-target/not-saved; first draft; typed publish failure with retained draft; successful inline notice and dismissal with active-run/version details when available. |

Also inspect reusable masters `zmP3o`, `Mz2gq`, `kov6Z`, `H5rZOl`, `zRiUA`, `x5WE6i`, `BYHVZ`, `S9rsK`, `zNzKN` and `LcwgT`. Check the new Braces/CornerDownRight/EyeOff/ArrowLeft glyphs visually in the bundled font. Protected person-only decisions remain for the person; no synthetic protected answer was attempted.

## Tested saved-source identities

Git blob hashes, obtained before writing this report. This report is the only subsequent added file; no source, locale or test changes followed the recorded checks.

| File | Blob |
|---|---|
| `crates/desktop-app/assets/locales/en/instructions.json` | `d113868b72eb38884ccf115d8aabd1ef92e9959f` |
| `crates/desktop-app/src/screens/instructions/editor.rs` | `c25b4da3c2c6a19ab46a4a4d0c3a9e89d0d8eb9f` |
| `crates/desktop-app/src/screens/instructions/fields.rs` | `87cf40c3ef3c6497247f346c7a1ffd58ac2b3203` |
| `crates/desktop-app/src/screens/instructions/frame.rs` | `72e2c55156c5a17f3df56c53b63a7fddb6e1a531` |
| `crates/desktop-app/src/screens/instructions/import.rs` | `0c6fe741bd15d97d028245e7353d04a0fe74948f` |
| `crates/desktop-app/src/screens/instructions/library.rs` | `80491cced10a79a424cc6273221cfe825cdda3fe` |
| `crates/desktop-app/src/screens/instructions/mod.rs` | `12574d75f8289cd862704dea35fce0950d3eef3f` |
| `crates/desktop-app/src/screens/instructions/panels.rs` | `0606e4d9cd6e5b7676f3b0c66f264c5fffc69790` |
| `crates/desktop-app/src/screens/instructions/preview.rs` | `eb7eed409cd155b1bf856a379dd05b19285c9ff5` |
| `crates/desktop-app/src/screens/instructions/sources.rs` | `2601467e30b7af85eb9c1386b68c3d5d8ab94081` |
| `crates/desktop-app/src/screens/instructions/variables.rs` | `24f762abacc08d29576b80bb746de6b8c7bffcb8` |
| `crates/desktop-app/src/screens/instructions/words.rs` | `d095eb49deba902aedc699a2e137257db51839dc` |
| `crates/desktop-app/src/screens/instructions/work.rs` | `ee31cad4df534cb665d689f402a090ff20b021b8` |
| `crates/desktop-app/src/theme/instructions.rs` | `5762ab8061ecf6a7f8982a400cdf0dd9f1e8aa45` |
| `crates/desktop-app/src/ui/icon.rs` | `40001101f76351efe66019f5994c4f18596bcd1b` |
| `crates/desktop-app/src/ui/mod.rs` | `c9910a19132cef172a8bb6cffaa31d9ae7ebdbc3` |
| `crates/desktop-app/src/ui/notice.rs` | `36f4db1b44f2e78660e5ae0c673dd35fb711dda2` |
| `crates/desktop-app/src/ui/rule_item.rs` | `f045f35b3772af9241866e8f7e39a8e292c0eaff` |
| `crates/desktop-app/tests/instructions/insertion.rs` | `95b5c85ac645542beab55a765a0c1e8a67278bdc` |
| `crates/desktop-app/tests/instructions/main.rs` | `faa16e2b1be1095e8afd9b4291112f4a5d26a2d6` |
| `crates/desktop-app/tests/instructions/recovery.rs` | `244a9126fc87bb987742c4fab8de6969ca0c3122` |
| `crates/desktop-app/tests/instructions/states.rs` | `0ca1000faf0ba3f33e181cefa10a644b2d58942f` |
| `crates/workspace/assets/locales/en.json` | `a3a3f689eb80a381c735a8f48a2fe9cfa4235b4e` |
| `crates/workspace/src/instructions/data.rs` | `7cd96278690c946096d137d4bbe7ddf6b9dddeab` |
| `crates/workspace/src/instructions/editor.rs` | `914897c9657b878be70282c86323382d6d909d58` |
| `crates/workspace/src/instructions/form.rs` | `a765cbbc6b8ce29120c95efda81a0a5802cd82bb` |
| `crates/workspace/src/instructions/mod.rs` | `df18c77c5b001af30a85214bd3cb82197d9edeae` |
| `crates/workspace/src/instructions/preview.rs` | `22fab163c74ca98ab977edc481e5b394f67e5db3` |
| `crates/workspace/src/instructions/tokens.rs` | `414ed3e835ef7e44fab9cb1b53a55a392904dbf0` |
| `crates/workspace/tests/instructions_tokens.rs` | `c13f7c3f9d5770d59b2f1190a8eeec6e1f6a60e7` |
| `crates/desktop-app/src/screens/instructions/editor_notes.rs` | `9ec2382f422cd62e0836027f23ba8454a071eec9` |
| `crates/desktop-app/src/screens/instructions/import_preview.rs` | `2da234c0fd8d275ddb0b19b9608b9fd62d1de77c` |
| `crates/desktop-app/src/screens/instructions/import_rows.rs` | `db650b079a0b1c8afc6d7174dd0d8003a2b0151e` |
| `crates/desktop-app/src/screens/instructions/library_groups.rs` | `36b472319b3debc4bf86babd5dd9be2a8021f5c5` |
| `crates/desktop-app/src/screens/instructions/variable_detail.rs` | `485c47899d1ee2f6ec19b34f132a546e458f8e54` |
| `crates/desktop-app/src/screens/instructions/viewer.rs` | `2e94cb6354e1addc2b499e2d8947f59e96ccbec5` |
| `crates/desktop-app/src/ui/prompt_source.rs` | `8c4c1a1e510c35d0cf80256bb453f495a244e63b` |
| `crates/desktop-app/src/ui/resolved_source.rs` | `2f3b20ecc545518e3c0ef7528d96e7a2dd8e417e` |
| `crates/desktop-app/src/ui/source_box.rs` | `3c6339fc8fcb4b7daa82b54ac5624cd75ad196da` |
| `crates/desktop-app/tests/instructions/design_states.rs` | `3fe2f079ab71139940129dbdcbac8940ba268a79` |
| `crates/workspace/src/instructions/preview_tokens.rs` | `a05a3ddc19c9b136e7627d12f5aebaeda821faae` |
| `crates/workspace/src/instructions/publication.rs` | `63001265e594e530135156e1664d7b9869d0d05a` |

Exact successful identity command:

```powershell
git hash-object -- 'crates/desktop-app/assets/locales/en/instructions.json' 'crates/desktop-app/src/screens/instructions/editor.rs' 'crates/desktop-app/src/screens/instructions/fields.rs' 'crates/desktop-app/src/screens/instructions/frame.rs' 'crates/desktop-app/src/screens/instructions/import.rs' 'crates/desktop-app/src/screens/instructions/library.rs' 'crates/desktop-app/src/screens/instructions/mod.rs' 'crates/desktop-app/src/screens/instructions/panels.rs' 'crates/desktop-app/src/screens/instructions/preview.rs' 'crates/desktop-app/src/screens/instructions/sources.rs' 'crates/desktop-app/src/screens/instructions/variables.rs' 'crates/desktop-app/src/screens/instructions/words.rs' 'crates/desktop-app/src/screens/instructions/work.rs' 'crates/desktop-app/src/theme/instructions.rs' 'crates/desktop-app/src/ui/icon.rs' 'crates/desktop-app/src/ui/mod.rs' 'crates/desktop-app/src/ui/notice.rs' 'crates/desktop-app/src/ui/rule_item.rs' 'crates/desktop-app/tests/instructions/insertion.rs' 'crates/desktop-app/tests/instructions/main.rs' 'crates/desktop-app/tests/instructions/recovery.rs' 'crates/desktop-app/tests/instructions/states.rs' 'crates/workspace/assets/locales/en.json' 'crates/workspace/src/instructions/data.rs' 'crates/workspace/src/instructions/editor.rs' 'crates/workspace/src/instructions/form.rs' 'crates/workspace/src/instructions/mod.rs' 'crates/workspace/src/instructions/preview.rs' 'crates/workspace/src/instructions/tokens.rs' 'crates/workspace/tests/instructions_tokens.rs' 'crates/desktop-app/src/screens/instructions/editor_notes.rs' 'crates/desktop-app/src/screens/instructions/import_preview.rs' 'crates/desktop-app/src/screens/instructions/import_rows.rs' 'crates/desktop-app/src/screens/instructions/library_groups.rs' 'crates/desktop-app/src/screens/instructions/variable_detail.rs' 'crates/desktop-app/src/screens/instructions/viewer.rs' 'crates/desktop-app/src/ui/prompt_source.rs' 'crates/desktop-app/src/ui/resolved_source.rs' 'crates/desktop-app/src/ui/source_box.rs' 'crates/desktop-app/tests/instructions/design_states.rs' 'crates/workspace/src/instructions/preview_tokens.rs' 'crates/workspace/src/instructions/publication.rs'
```
