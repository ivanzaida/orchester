# ORC-39 implementation handoff

Implementation is complete in the existing run worktree. Independent Review and app/Pencil Verify are still required; this is not a visual-verification report. No commit, merge, stable install change, or canonical Pencil change was made.

The approved defaults are Backlog, No priority, no tags, Blocked off. Reason-only Blocked is informational. Unfinished dependency links prevent Run; resolved dependencies restore readiness while retaining the manual flag/reason. Existing review/merge completion gates remain authoritative. Priority does not alter scheduler order.

## Implemented behavior

- Core organization types, validation, dependency-cycle checks, combined filters and deterministic sorting; separate workflow and run states.
- Store migrations and atomic task/content/organization saves, manual and automatic change history, cancellation guard, latest-run dependency readiness, and project tag uniqueness/rename/merge/delete with assignment cleanup and retained name snapshots.
- Runner readiness enforcement and automatic run-start/review/merge transitions through store rules.
- Tasks tabs, status/priority icons and tag chips in Tasks and Overview; task-header pickers and tags; New/Edit organization fields and blocking links; active-run cancellation confirmation using the existing stop path and preserving branch work.
- Bulk status/priority/tag actions with individual outcomes, filters and sorting, tag overflow, project tag management page and confirmations, and task Activity organization history. Existing unrelated form controls and run snapshots are preserved.

## Passed checks

All commands use worktree-local `CARGO_TARGET_DIR=.tmp/orc39-check/target`. Every cargo test has a name filter.

- `powershell -File tools/check.ps1`: passed after the final source changes (formatting, workspace Clippy with warnings denied, file-size and UI-literal checks).
- `cargo test -p orchester-core --test task_organization organization_`: 5 passed.
- `cargo test -p orchester-store --test task_organization organization_`: 5 passed, including automatic transition history, retained tag-name history, atomic validation failures and bulk cancellation rollback.
- `cargo test -p orchester-runner --features test-agent --test task_organization organization_`: 1 passed.
- `cargo test -p orchester-desktop --test form_keyboard organization_`: 6 original/expanded cases passed; the subsequently added `organization_bulk_add` case passed separately. Together these cover new organization fields and keyboard save, inline tags, bulk status/priority/filtering, active-run cancellation/Escape and branch retention, blocked/resolved readiness, tag rename/merge/delete and bulk Add/Remove tags.
- `cargo test -p orchester-desktop --test form_keyboard new_task_`: 10 passed.
- `cargo test -p orchester-desktop --test form_keyboard edit_task_`: 4 passed.
- `cargo test -p orchester-desktop --test form_keyboard history::history_`: 14 passed after updating the selected-tab test to the available Backlog tab.
- `cargo test -p orchester-desktop --test form_keyboard window::`: 11 passed (filter also matches tabs_window).
- `cargo test -p orchester-desktop --test tasks tabs_`: 1 passed; `task_`: 8 passed.
- `cargo test -p orchester-desktop --test navigation route_`: 18 passed.
- `cargo test -p orchester-desktop --test i18n locale`: 4 passed; `every_key`: 1 passed after adding Retry and correcting the pipeline-not-published key.
- `cargo check -p orchester-core --target aarch64-apple-darwin`: passed.
- `git diff --check`: passed.

The desktop/runner macOS cross-check was attempted with `cargo check -p orchester-desktop -p orchester-runner --target aarch64-apple-darwin`; native dependency compilation stopped because the cross C compiler `cc` is unavailable. It did not complete. No Mac runtime test, complete workspace test suite, or implementer app screenshots were run. Some test builds emitted incremental-cache access warnings; the Windows quality gate itself passed with warnings denied.

## Corrections after independent Review

The same run/worktree and approved design were retained. Independent Check passed the previous candidate; Review then identified two major restart defects and one Add link interaction defect. All three are corrected:

- Core `RunEventKind::starts_execution` identifies Started, Resumed, Continued (explicit and reconnect), Reopened and RecoveryRequested. Store uses this common rule to record In progress with Orchester/when/from/to history. A failed follow-up returns to In review because its original result remains ready for review.
- Store checks dependency readiness inside the run-event transaction before updating run state, attempts, events or organization history. Run creation uses the same readiness function. Runner continuation preparation checks readiness too, and follow-up validates preparation before persisting its restart. This preserves typed unavailable behavior and avoids committing a restart before a preparation failure.
- Add link applies `enabled` after its click handler. Empty and cyclic choices are skipped by keyboard focus and cannot add a link; a valid choice can be added and saved.

Fresh implementation checks passed:

- `powershell -File tools/check.ps1`: passed after the final source corrections.
- `cargo test -p orchester-core --test task_organization organization_`: 5 passed.
- `cargo test -p orchester-store --test task_organization organization_`: 7 passed. Restart coverage includes follow-up, recovery, pause/resume, explicit continuation and reconnect; blocked persistence leaves run/events/history unchanged. Successful restarts preserve priority/manual reason and record transitions. Failed follow-up restores review with history.
- `cargo test -p orchester-runner --features test-agent --test task_organization organization_`: 2 passed. Both follow-up and recovery refuse new unfinished blockers without changing durable run state, queued messages, history or uncommitted worktree files. Removing links permits continued work on the same branch and preserves informational Blocked/priority.
- `cargo test -p orchester-runner --features test-agent --test failed_recovery failed_recovery_`: 6 passed.
- `cargo test -p orchester-runner --features test-agent --test conversation conversation_ask_agent_`: 3 passed, including Codex/Claude follow-ups and removed-worktree refusal.
- `cargo test -p orchester-desktop --test form_keyboard organization_`: 8 passed, including empty/cyclic Add link unavailable behavior and valid keyboard add/save.
- `cargo check -p orchester-core --target aarch64-apple-darwin`: passed.
- `git diff --check`: passed.

No full workspace test run, desktop/runner macOS cross-check or app/Pencil verification was repeated in this correction step. The prior desktop/runner cross-compiler limitation remains. No commits, merges, root task recreation or stable runtime changes were made.

Fresh independent Check, Review and Verify must run on these corrections. In addition to the 24 frame states below, inspect restart history from In review/manually Done, refusal after a linked unfinished blocker is added to a succeeded/failed run, retained work/messages after refusal, and the empty/cyclic Add link states in `tUwII`.

## Required next steps

Run fresh Check and independent Review. The independent Verify step must launch its pinned app when ready and compare it to canonical Pencil frames at 1440 x 960, region by region. Host PNG exports were authoritative design input only.

Verify all 24 frame states listed in `docs/design/orchester-workspace.md`: Tasks/Overview rows and tabs (`qVMJh`, `DtpHJ`); header status/priority (`gNExS`, `Sd5qt`); tag picker, inline creation, validation and overflow (`IOWwP`, `yrT73`); New/Edit organization fields, reason-only flag, unfinished/finished links and readiness (`ixENv`, `ZLhiM`, `tUwII`, `Ncjvt`); single and bulk active-run cancellation, pending/failure/Escape and retained work (`N1vwaM`, `Fhzcc`, `hFH4D`); bulk status/priority/tags and mixed outcomes (`Ayo8q`, `HI91q`); combined filters, sorting, no matches and load errors (`l7kBSq`, `rVJoA`); project tag list and create/rename/merge/delete confirmations and errors (`y3ohgK`, `YBfjV`, `ZWSYj`, `nOOi1`, `VvhOQ`); history and independent system labels (`D1DC6T`, `k3YiJ`). Explicitly check keyboard focus, chips wrapping/overflow, menu positioning, confirmation dismissal, resolved-readiness refresh, and preservation of unrelated controls. Report inaccessible states rather than treating unit tests as design verification.

Only move the complete product task to Done after independent verification passes.


## Corrections after independent Verify of a68b9b9 - 2026-10-02

This section covers the current saved work based on `d535ff6dc6c159ab766d56f439c986a4605f8f31` plus the preserved working-tree corrections. The source-input SHA256 is `6fbd60368fda2c40e2ba4c159dbe566753258a4f5909216751db90de7de7d663`. The 859-file input list and hashing algorithm are in `.tmp/orc39-verify-corrections-20261002/source-inputs.json`. Orchester checkpointed the first corrective changes; this implementing agent made no commit or merge. Earlier sections and their checks describe earlier candidates.

The negative Verify report at `.tmp/verify/report.md`, its screenshot pairs, old checkpoints and inconclusive foreground timeline remain unchanged. The approved host PNGs and handoff were used as design input; no canonical design or stable installation was changed.

Implemented corrections:

- Task titles retain a real line inside their table column instead of taking vertical flex space. Headless coverage resizes the existing route repeatedly and checks visible title bounds without navigation or route reconstruction. The independent verifier must still reproduce the original GPU-rendered resize scenario.
- Organization status/priority menus have the approved 280px width and selected-option surface. Shared tag, filter and bulk options occupy the full menu width with trailing checks and allow long names to wrap. Header tag controls, compact chips, Tasks heading rhythm, toolbar alignment, labels and secondary New task were corrected.
- Tag pickers have persistent search/create labeling, conditional inline creation and a project management entry. Filters expose selected counts and summaries, searchable tags, all/any matching, descriptive sorting and an explanatory no-results notice. Bulk removal stages its selected tags before a separate apply action; membership names both the assigned and selected-task counts.
- Organization forms retain every unrelated criteria/context/execution control. Blocked uses the checkbox/helper, a multiline reason and linked-task title/status/navigation. Dependency/readiness explanations wrap. The task page shows a prominent flag and a full reason notice. Reason-only Blocked remains informational; execution readiness rules did not change.
- Overview uses two compact summary panels and full-width task rows, with priority beside the second line, combined workflow/run labels and explicit blocked notices. Blocked preserves the run label. Existing run tabs, attention actions, runner details, tokens and budget remain accessible through Run activity and attention; no capability was removed.
- Project tags have neutral chips, count columns, 64px separated rows and a shared action row. Create/rename and merge use the approved focused page panels; delete uses a named-impact modal. Editors identify source/target tags, preview and task impact, provide action-specific confirmation copy, and preserve invalid/no-target/pending disabled behavior. History has page padding, event icons, separate change/attribution lines and complete localized dates.
- The exact Attach regression now compares against the durable snapshot taken after the runner settled and queued replies were persisted. It still asserts preservation of blocked run state/events/history/messages and uncommitted worktree content while unrelated work resumes. Production Attach/restart/readiness behavior is unchanged by this visual correction.

Fresh checks passed with `CARGO_TARGET_DIR=.tmp/orc39-check/target`:

- `powershell -File tools/check.ps1`: `check: OK`; final log `.tmp/orc39-verify-corrections-20261002/check.log`.
- `cargo test -p orchester-core --test task_organization organization_`: 5 passed.
- `cargo test -p orchester-store --test task_organization organization_`: 7 passed.
- `cargo test -p orchester-runner --features test-agent --test task_organization organization_`: 3 passed, including blocked offline queued-reply attachment isolation.
- `cargo test -p orchester-desktop --test form_keyboard organization_`: 13 passed; final log `.tmp/orc39-verify-corrections-20261002/organization-tests.log`.
- `cargo test -p orchester-desktop --test form_keyboard new_task_`: 10 passed; `edit_task_`: 4 passed.
- `cargo test -p orchester-desktop --test i18n locale`: 4 passed; `every_key`: 1 passed.
- `cargo test -p orchester-desktop --test theme roles_`: 14 passed. Theme configuration and these test inputs are unchanged by the subsequent row-layout edit; this check covers roles, not layout.
- `git diff --check`: passed.

No full workspace tests, macOS cross-check/runtime, private desktop launch or implementer app/Pencil verification were run in this continuation. Incremental-cache access warnings appeared in test builds; tests and the warnings-denied quality gate passed. No process belonging to the stable installation was touched.

Next agents must run fresh independent Check, Review and app/Pencil Verify against this saved candidate. Compare all 24 approved frames, including the previously unreachable active-run, cancellation, pending/failure, merge/deletion, restart/history and resolved-readiness states. Specifically inspect resize/title rendering without route reconstruction, menu dimensions/options, tag search/filters and staged bulk apply, blocked reasons/links, Overview composition and retained activity controls, tag editors/named impacts and history. Retained New/Edit criteria/context/execution controls remain the approved exception.

Before every private Verify launch, build and pin the current executable pair, announce that current pin READY, then obtain a NEW Root host watcher. Reuse neither the old captures nor the inconclusive timeline as proof. Root does not perform task QA. Move the product task to Done only after independent Verify passes.
