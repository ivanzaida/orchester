# ORC-3 corrections after Verify attempt 8

Implementation handoff, not an independent design-verification verdict.

Candidate: saved desktop-app changes on **353e2cb3643f0064eae67816bdb5a97fab83b0fd**. No commit made. The tracked desktop-app diff hashes to **5a431d2440ec9221d06de98e3796ff798841c94f** (`git diff --binary -- crates/desktop-app | git hash-object --stdin`); the new `src/ui/add_slot.rs` blob hashes to **7f55659560bbf1041809ca0869914e224edb2cb7**. This document is outside that source fingerprint.

The prior report at `.tmp/verify/orc-3-353e2cb3/report.md` remains evidence for its old candidate. It has not been rewritten or relabeled.

## Corrections

- Graph: corrected the canvas allocation and main-card center, cleared the toolbar, routed the seeded return edges outside the cards, made decorative connectors ignore pointer input, and bounded their overlay so they render. Center-clicking both parallel cards is covered by a regression test.
- Inspector: moved the editable step name into the header, restored compact 28px control rows, shortened the fixed failure-policy readback, corrected missing-executor styling and tool labels, aligned destructive actions, and retained bounded scrolling and the fixed footer. Native capture shows the seeded permissions section above the footer.
- New pipeline: inline option summaries, numbered connected starter chips, compact copy selector with placeholder, aligned slug hint and contextual collision error, titled unsaved notice, disabled-action reason and flat collision strip. New pipeline remains a page.
- Summary and empty editor: editable name, Runs/Versions grouping and explanation, unpublished save status, and the centered dashed add-step target. Explicit SVG path segments are used because native rendering did not retain SVG stroke-dasharray.
- Publication/history: versioned heading, 72px version cards and metadata, icon-bearing changes, rounded canvas tags, version chevrons and Latest badge, comparison count/toggle, inline restore action, and a fixed Cancel/Publish footer.
- Settings: transparent inactive navigation, rounded selection, outlined back action, descriptions and slug explanation, versioned main-area change strip and Undo, changed markers, contextual name error, and preservation of invalid input across remounts. The leave question names the pipeline and explains the consequences.
- Tabs now show New pipeline or the saved pipeline name, including after renaming.

The window-input lock and person-only weakening confirmation were preserved. No workspace business rule, deprecated desktop source, or design file was changed.

Minor carryover for Verify: dependency/failure selectors still show step names without the design's ordinal prefixes. The new starter preview and graph cards are numbered. No independent verdict is claimed for this or other design details.

## Checks

Every cargo test used a nonempty name filter. TEMP and TMP were set to the existing worktree directory `.tmp/orc-3-permissions`.

| Command | Result |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS, **23 tests**, on the final source fingerprint above. Earlier iterations passed 21/22 tests before adding the new regressions. |
| `cargo test -p orchester-app --test pipelines pipelines_settings_preserves_invalid_name_across_navigation` | PASS, 1 test after contextual message lookup correction; also included in the final 23-test run. |
| `cargo test -p orchester-app --test ui_kit fields::` | PASS, 11 tests. Shared field code unchanged since this run. |
| `cargo test -p orchester-app --test ui_kit blocks::` | PASS, 13 tests. Shared block code unchanged since this run. |
| `cargo test -p orchester-app --test ui_kit choices::` | PASS, 3 tests. Shared choice code unchanged since this run. |
| `cargo test -p orchester-app --test tab_labels tab_labels_` | PASS, 3 tests. Tab/feed code unchanged since this run; final pipeline tests also cover renaming the tab. |
| `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists` | PASS, 1 test on final source. First run failed on direct use of a workspace error key; corrected by using the workspace's typed error mapping. |
| `powershell -File tools/check.ps1` | **PASS on final source**: rustfmt, workspace Clippy/all targets/features, file size and UI literal checks. |
| `cargo +nightly fmt -p orchester-app` | PASS. Used after edits. |
| `cargo check -p orchester-app` | PASS during implementation, before the final changes; not used as the final verdict. Intermediate compile failures were corrected. |
| `git diff --check` | PASS after the final source changes. |

Intermediate check.ps1 failures concerned function length/redundant clone, an unnecessary SVG borrow, a string API call, and formatting during an edit. All were corrected; none is the verdict for the final candidate.

Also completed successfully: `git status --short`, `git rev-parse HEAD`, `git diff --stat`, the two Git fingerprint commands above, and the four QA builds below. A diagnostic `Get-Process cargo,rustc,link ...` printed build processes but exited 1 when no link process existed; it changed no process.

Not run: unfiltered/workspace test suites, other workspace/store/MCP test binaries, macOS builds or native checks, the 960px/light-theme passes, independent design verification, and a conclusive host-foreground-focus check. No native publication was confirmed. Existing filtered pipeline tests exercise the person-only publication guard.

## Native implementation smoke

All apps were copied, seeded and launched by:

- `powershell -File tools/qa-app.ps1 -Name orc3-corrections -Scenario empty -NoRunner` — PASS, PID 18648.
- `powershell -File tools/qa-app.ps1 -Name orc3-corrections-final -Scenario empty -NoRunner` — PASS, PID 39416.
- `powershell -File tools/qa-app.ps1 -Name orc3-corrections-ready -Scenario empty -NoRunner` — PASS, PID 37864.
- `powershell -File tools/qa-app.ps1 -Name orc3-corrections-complete -Scenario empty -NoRunner` — PASS, **final candidate**, PID 26980.

LOCALAPPDATA was the worktree's `.tmp/orc3-corrections/local`; TEMP/TMP were worktree-local. Exact-PID calls used `tools/lurq-mcp-client.py` via `.tmp/orc3-corrections/drive.ps1`. Initial sandbox discovery reads failed; approved escalation allowed access without changing ACLs or printing credentials. A first Windows PowerShell helper invocation failed to preserve JSON quotes; PowerShell 7 invocations succeeded. One guessed lurq_click call failed; subsequent interactions used the documented lurq_interact transport.

The four owned PIDs were stopped individually using `Stop-Process -Id <pid>`; all four were confirmed exited. No installed app or its data was touched.

Final-build captures are under `.tmp/orc3-corrections/`:

- `app-final-library.png`
- `app-final-new.png`
- `app-final-collision.png`
- `app-final-empty.png`
- `app-final-review.png`
- `app-final-settings.png`
- `app-final-settings-changed.png`
- `app-final-publication.png`
- `app-final-history.png`

They are 2160×1350 physical pixels at 1.5× (1440×900 logical). Files without the `app-final-` prefix are development iterations, not evidence for the final candidate. This was implementation smoke, not a scored independent comparison.

## Required independent verification

Use read-only Pencil access to layers **i4jt6V** and **vEDYY** in the person's orchester2.pen. Keep ORC-3 pending until Review and Verify pass.

| States | Pencil nodes |
| --- | --- |
| Library, empty library, new page and slug collision | GaNsb, sI5K0, lAQis, Hgequ |
| Selected step, summary, empty graph, missing executor/error/unpublished variants | SVBQi, c24Tv, Z26ZFs, Clxs2 |
| Permissions, enforcement errors and reset; bounded inspector scroll/footer | SVBQi/K0WLeG, cnlv1 |
| Blocked publication, publish review, success, history comparison/restore variants | c7g6Qi, ITC0R, nmY08, OchSR, qCku8 |
| Settings unchanged, changed, invalid name and in-place leave confirmation | x4IIjM, Yx32O, fmnva, RWODj |

Recheck actual center-click targets with both seeded return routes, rename/remount recovery, keyboard/caret/undo across autosave, protected weaker publication and refusal of synthetic approval. The native smoke did not cover every error/reset/success/leave state or establish host-focus behavior. Fixture data may differ from the illustrated version numbers, counts, platform-enforceable permissions and starter steps; inspect the actual state rather than assuming a screenshot covers it.
