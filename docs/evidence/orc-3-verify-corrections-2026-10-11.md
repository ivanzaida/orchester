# ORC-3 — corrections after Verify attempt 5

Candidate: saved work on base `e8870dfd79a0bc9a05f4a360607075dadd5ea1f2`, 2026-10-11. No commit was made. This report covers the implementation and checks of these saved changes, not independent visual verification. The previous comparison in `.tmp/verify/orc-3/report.md` remains evidence for the old revision.

## Implemented

- E1/E2: constrain inspector values to the remaining column (184 px at 1440), compact editable rows and text areas, use a popup dependency checklist, section rules and aligned switches. A headless layout regression checks field bounds against the window.
- E3–E8: restore the canvas background, grouped toolbar, start/card/parallel geometry, external failure-return connectors, role-specific badges, approval ordering, selected outline, inspector close action and ghost Remove action. Wide parallel groups can scroll horizontally. Show Run with an unavailable reason in a tooltip because runs start from tasks.
- L1–L4: library display heading, state icons, version explanations in a bordered three-stage row, pipeline/published counts and parallel flow labels. Empty library also shows the actual built-in starter and creation action.
- N1–N4/Z1–Z2: page-based creation has starter chips, copy selector, metadata and creation notice; name collisions show a strip and link to the existing pipeline. Empty editor has the connected add-first-step slot and working starter action. Empty Publish is disabled with its explanation in the header.
- P1/P2: missing assignment highlights the field/card and explains the missing profile; unavailable model choices are disabled. Publication problems appear above the graph and inspector with links; publication is disabled.
- V1–V3: publication comparison cards, change count/list, pinned-run notice, graph change tags and dismissible success notice. History has draft/version rows, latest/selected treatment, dates/counts, selected details, compare and restore actions, and a read-only canvas toolbar.
- S1–S4: centered Settings form with 300 px controls, editable name and locked slug, label/help columns, section navigation that scrolls to actual bounds, planned trigger cards, changed-section marker, draft status and description Undo.
- Historical description and step/gate counts read the selected version instead of the current draft.

Name changes use workspace validation and the existing save/permission path. Storage saves the display name and body atomically and checks the name, body and latest version against the editor's snapshot. Slugs and published bodies remain unchanged. Display names are resource metadata and take effect when saved; they are not historical version fields. Stale-name rejection and rollback have a workspace regression test.

Person-only widening confirmation and the window-input lock remain in use. Ordinary autosaves do not remount fields. Existing retry/model-reset, leave protection and keyboard/caret/undo headless regressions still pass.

The shared shell's nested tab title (old report C1, minor) remains unchanged. No unrelated shell redesign or deferred Settings semantics were added.

## Final checks

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS, 15 tests on the final source; includes clipping, dependency popup, name/slug persistence, Settings Undo/section jump, differing draft/history metadata, publication notice, permission lock and existing editor regressions. |
| `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes_` | PASS, 3 tests. Workspace/store implementation unchanged since this run. |
| `cargo test -p orchester-store --test draft_matching pipeline_draft_matching` | PASS, 1 test. Store implementation unchanged since this run. |
| `cargo test -p orchester-app --test ui_kit fields::` | PASS, 11 tests; repeated after final source changes. |
| `cargo test -p orchester-app --test ui_kit choices::` | PASS, 3 tests. |
| `cargo test -p orchester-app --test connections list::` | PASS, 4 tests, covering the other screen using the shared inspector. |
| `cargo test -p orchester-app --test i18n every_key_` | PASS, 2 tests. |
| `cargo test -p orchester-app --test i18n locales_` | PASS, 3 tests. |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace -p orchester-store` | PASS. |
| `powershell -File tools/check.ps1` | PASS on the final source: rustfmt, workspace/all-target/all-feature clippy, file-size and UI-literal gates. |
| `git diff --check` | PASS. |

The following intermediate outcomes are retained as implementation history, not verdicts on the final candidate:

- `cargo test -p orchester-app --test pipelines pipelines_`: initially 10/13 passed (dependency popup interaction, budget placeholder and renamed history action expectations); subsequently 14/15 passed (Undo expected a retained draft, whereas restoring the published body deletes it); one compilation failure used the wrong element-reference builder; later 14/15 passed (empty publishing explanation moved to the header). These were corrected; the final run above is 15/15.
- `cargo test -p orchester-app --test i18n every_key_`: initially 1/2 passed because the notice used an absent `common.close` key. It now has `pipelines.dismissNotice`; final run 2/2.
- `cargo check -p orchester-app`: used during development; intermediate failures included a nonexistent spacing variant, unfinished history toolbar, absolute positioning on Element instead of a builder, closure arity, mutable Ctx, missing ChevronLeft, and Arc/Name mismatches. Those compile issues were corrected. The final quality gate compiles and lints the final source.
- `powershell -File tools/check.ps1`: intermediate function-size, redundant-clone and needless-borrow failures were corrected by extraction or simplifying the calls. Final gate passes.
- `cargo +nightly fmt -p orchester-app -p orchester-workspace`: PASS during earlier editing; superseded by the final three-crate format command.

Other commands: `git status --short`, `git diff --stat`, `git rev-parse HEAD` and scoped `git diff -- <paths>` succeeded. A process-status diagnostic (`Get-Process cargo,rustc,lld-link`) reported no lld-link process; no unrelated process was stopped. `cargo metadata --format-version 1 --offline | python -c "import json,sys; d=json.load(sys.stdin); print(next(p['manifest_path'] for p in d['packages'] if p['name']=='lurq'))"` succeeded to locate the dependency source for its read-only MCP inspection.

## Native QA and limits

`python .tmp/orc-3-fixes/launch.py` invokes `powershell -File tools/qa-app.ps1 -Name orc-3-fixes -Scenario empty -NoRunner` with data/discovery/build paths in this worktree. First attempt failed at compilation (Ctx mutability); second built and launched isolated QA PID 43204 successfully. This was an intermediate build, not the final candidate.

`python tools/lurq-mcp-client.py 43204 app_state --arguments '{}' --app orchester-app --discovery-dir .tmp/orc-3-fixes/localappdata/orchester-app-mcp` failed with PermissionError reading discovery. No token was printed, no global input or capture was attempted, and no native screenshots were obtained. `Stop-Process -Id 43204` succeeded; only this owned QA process was stopped. The subsequent PID check confirmed it is stopped. No runner was launched.

Not run: full/unfiltered test suites, unrelated subsystem suites, native graphical comparison on the final build, native focus/caret checks, macOS target/runtime checks, 960-width or light-theme passes. The code changes are shared across platforms; no new platform-specific implementation was introduced.

## Independent Verify handoff

Keep ORC-3 pending until independent Review and Verify pass. Read-only Pencil Get calls supplied component properties and frame geometry; no design mutation was made. Compare the rebuilt saved candidate with live nodes in `data/orchester2.pen`, layers Pipelines `i4jt6V` and Pipeline settings `vEDYY`:

| States | Pencil nodes |
| --- | --- |
| Populated and genuinely empty library | `GaNsb`, `sI5K0` |
| New pipeline: starter/copy/empty, description and duplicate-name strip | `lAQis`, `Hgequ` |
| Agent/tool editor, dependencies, return route, gates/narrowing, summary, empty graph, missing executor | `SVBQi`, `c24Tv`, `Z26ZFs`, `Clxs2` |
| Card, badge, parallel group geometry | `r6PIM`, `Co8O6`, `qXE7Z` |
| Blocked publish retaining the inspector, ordinary and protected review, success notice | `c7g6Qi`, `ITC0R`, `nmY08` |
| Version selection, comparison tags/details, return to editing, restore as draft | `OchSR`, `qCku8` |
| Settings form, section navigation, editable name/stable slug, description change/Undo and supported validation | `x4IIjM`, `Yx32O`, supported portions of `fmnva` |
| Leave/keep confirmation | `RWODj` |

Prioritize inspector right-edge clipping, graph/card/return-edge geometry, Settings column bounds and scroll links, history comparison, and the success overlay's dismissal and input behavior. Also recheck retry reset, agent → tool → agent model reset, historical metadata, and native host focus/caret/undo across autosave. A headless pass is not native focus or visual-parity evidence. The old report's unverified states remain unverified graphically until this next step reaches and captures them.
