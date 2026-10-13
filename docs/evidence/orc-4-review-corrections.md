# ORC-4 returned-review corrections

This report covers source freeze 5d1d536c0078dbf9b5c3cede5eeebd42262721e9, tree affdcd693143f2d6f6a774c735b7dd6c49de0154. The report-only descendant and final HEAD/tree/disk manifest are recorded in .tmp/instructions/review-corrections/final-manifest.json. source-freeze-manifest.json binds all 110 combined changed paths against base 934b85b92f05296b7e30552073eea438136010f0; every normalized disk blob matches HEAD. No source edit followed this freeze. Temporary Skills diagnostic assertions were restored exactly before the final Instructions test and clippy completion.

## Corrections and provenance

- Check: reproduced nightly formatter failures in task imports, then applied only those formatter hunks through the file tools. Stable rustfmt had ignored nightly grouping settings. The complete reproduced failures and correction are retained in .tmp/instructions/check-return-1/. Formatting checkpoint: 85df4356.
- Shared scanner: ordinary merge 2a166f56 integrates accepted ORC5 master 2fef3eb3b034f62d8f6097c9ddf61cff0ef2f0e7, source 6cf15fae055cc00a91a346a3f2fb12d028195943. The scanner subtree and its tests are byte-identical in Git to the accepted master (git diff --exit-code returned 0). No duplicate scanner was added. Both screen registrations, theme/type roles and UI exports were retained when resolving the two merge conflicts. The merge only adds the existing windows-sys 0.61.2 workspace dependency reference and accepted platform feature declarations; modern central lurq 0.44 and legacy 0.42 stay unchanged.
- Reload: captures raw fields when requested and replaces only fields that still equal that request. Later prompt input, invalid raw input, picker insertion, targets, timing and scope survive. Pre-request dirty input is discarded when the person chose Reload and did not subsequently change that field.
- Create: every mutable field, already-open selection, retained input callback, variable insertion and timing helper is locked while the first Create is pending. Ordinary autosave remains editable and acknowledges its saved baseline without erasing later input.
- Create identity: the actual committed creation receipt bridges the list refresh until that new ID appears in the catalog. This prevents mounting a different existing rule while the refresh waits. A new regression proved the old behavior (Existing text instead of Created text); the corrected test also publishes the new rule and verifies the existing publication is unchanged.
- Retry: retains the failed operation kind and expected entry. Failed Save plus revert retries Save; failed Reload retries a read; neither can turn into Publish. A Publish retry is offered only for the same expected body.
- Copied: binds the receipt to composition, selected preview context/event and edited entry, and clears it when the visible payload changes. Tests use a private clipboard boundary around the real screen and do not alter the system clipboard.

Correction commits: f2a2f7c7567b1fa3284f19d731006d2058eebf00 and 5d1d536c0078dbf9b5c3cede5eeebd42262721e9. Root's independent review covered ccda208e and excluded later edits; its REQUEST CHANGES is preserved, not relabeled as a review of this freeze. Review SHA256: 8d00f5febdeaea2a32617d7c83f1a7ef4694e17b700daf009d9295aa11c424d8; proof SHA256: 538fe36b40c07ef11ae6d90b3ad33dba950aed53a2f215b3571b8e144919a51b.

## Named checks and actual outcomes

All commands below were run in this worktree. Complete collected output, command text and exit status are in the named JSON files under .tmp/instructions/review-corrections/. Earlier successes are not native acceptance.

| Command | Outcome | Raw evidence |
| --- | --- | --- |
| cargo test -p orchester-app --test instructions instructions_ --locked | PASS, exit 0, 29 tests on exact source freeze | final-instructions-frozen.json |
| cargo test -p orchester-workspace --test instructions instructions_ --locked | PASS, exit 0, 4 | final-workspace-instructions.json |
| cargo test -p orchester-workspace --test skills_repository skills_repository_ --locked -- --skip skills_repository_refuses_external_skill_reference_and_agent_file_links --skip skills_repository_refuses_protected_targets_and_other_skill_roots --skip skills_repository_never_opens_a_special_file --skip skills_repository_accepts_in_project_reference_file_links | PASS, exit 0, 9; four fixtures excluded and Unverified | scanner.json |
| cargo test -p orchester-core --test native native_scope_repository_discovery_ --locked | PASS, exit 0, 1 on f2a2f7c; core inputs unchanged in final source | frozen-native-scope.json |
| cargo test -p orchester-app --test ui_kit prompt_input_ --locked | PASS, exit 0, 4 | final-prompt-input.json |
| cargo test -p orchester-app --test screens screens_ --locked | PASS, exit 0, 2; both registrations built | final-screens.json |
| cargo test -p orchester-app --test skills_theme skills_ --locked | PASS, exit 0, 1 | final-skills-theme.json |
| cargo test -p orchester-app --test theme roles --locked | PASS, exit 0, 6 | final-theme.json |
| cargo test -p orchester-app --test i18n every_key --locked | PASS, exit 0, 2 | final-i18n-keys.json |
| cargo test -p orchester-app --test i18n locales --locked | PASS, exit 0, 3 | final-i18n-locales.json |
| cargo check --workspace --all-targets --all-features --locked | PASS, exit 0 | final-workspace-check.json |
| cargo clippy -p orchester-app -p orchester-workspace --all-targets --all-features --locked -- -D warnings | PASS, exit 0 | final-clippy.json |
| cargo test -p orchester-app --test skills skills_empty_states_ --locked | FAIL, exit 1, 0/1; repeated once, same page timeout | final-skills-screen.json, skills-screen-repeat.json |
| Same Skills filter with temporary error diagnostic | FAIL, exit 1: actual screen reports Skills could not be read | skills-screen-diagnostic.json |
| Same Skills filter with temporary private-fixture scanner preflight | FAIL, exit 1: scanner Read error on C:/Users/lurkm/AppData/Local/Temp/.tmpHY54UZ/repo-kontur, OS code 5 PermissionDenied, Access is denied | skills-screen-raw-diagnostic.json |

The extra Skills test did not pass. The exact failing component/permission cause was not isolated beyond this scanner error; it is reported without changing the accepted scanner, its fixtures, the sandbox or execution context. No alternate retry followed the observed access denial. Temporary diagnostic source was restored and git status was clean. The Instructions owned-tree import, outside-directory refusal, byte bound and drafts-only regressions passed on the final freeze.

Nightly formatting commands, each exit 0: cargo +nightly fmt -p orchester-agent -- --check; cargo +nightly fmt -p orchester-core -- --check; cargo +nightly fmt -p orchester-exec -- --check; cargo +nightly fmt -p orchester-api -- --check; cargo +nightly fmt -p orchester-server -- --check; cargo +nightly fmt -p orchester-desktop -- --check; cargo +nightly fmt -p orchester-index -- --check; cargo +nightly fmt -p orchester-ipc -- --check; cargo +nightly fmt -p orchester-store -- --check; cargo +nightly fmt -p orchester-workspace -- --check; cargo +nightly fmt -p orchester-vault -- --check; cargo +nightly fmt -p orchester-app -- --check; cargo +nightly fmt -p orchester-runner -- --check. Raw: final-format-results.json.

Git checks: git diff --check, git status --short, quoted git rev-parse HEAD 'HEAD^{tree}', git ls-tree -r HEAD -- <manifest paths>, git hash-object --no-filters <paths>, git hash-object <paths>, scanner git diff --exit-code against accepted master: all exit 0. An unquoted git rev-parse HEAD HEAD^{tree} failed with PowerShell argument interpretation; it was corrected by quoting. Scoped git add/commit and the accepted ordinary merge succeeded. Earlier diagnostic/build/test/formatter failures and intermediate cohorts are preserved in development-failures.md, development-format.json, development-instructions-26.json, corrected-instructions.json, frozen-instructions.json and create-identity-before.json. An interrupted scanner session had unknown completion, never a PASS; the separately completed scanner run above supplies the current result.

## What was not run and remaining gates

- tools/check.ps1 was not run by this implementer: the explicit step override reserves it for authoritative pipeline Check. Original Check failed on ccda208e (exit 1, 102.1s, step_outputs1791840015203). Its supplied tail and complete fresh formatter reproduction remain under .tmp/instructions/check-return-1/. The full original raw output was requested but not supplied; no fabricated full artifact or authoritative PASS is asserted.
- Four accepted OS1314 file-link fixtures remain Unverified by explicit direction. No elevation, protected-fixture retry, permission change or blanket scanner bypass was attempted.
- No full/unfiltered Cargo tests, fresh Apple cross-check or macOS execution. The historical Apple native header/SDK failure remains in orc-4-implementation.md and has not been relabeled.
- No native QA launch/capture, independent source re-review or app/Pencil fidelity verdict. Root must provide the new-freeze QA lease and PID identity acknowledgment before a verifier launches. Independent combined-source review and native Verify still must pass, together with actual authoritative Check.
- No 960/light passes, toolkit version/publication changes, path SDK override, check weakening/exemptions, deprecated-app edits, installed-app/database changes or root plan/handover mutations.

## Independent Verify states and Pencil nodes

Read only Pencil data/orchester2.pen, Instructions layer fePhH, at the frames' 1440 x 900.

- XMlEH: p6Bsv heading, p7ztk scope tabs, b1i8f library and zmP3o rule item; ilieH/f4Slyn/qSn5T editor; cCFTK/kov6Z raw prompt; Un13s/blqcd toolbar/gutter; r8Ipz variables and jNHQ3 detail. Compare scope, targets and timing in editable and pending first-Create states, including already-open menus and retained controller insertion.
- J7EgbM: unavailable run.revision, k3WFT/OV84i warning token, Resume helper and explicit unavailable fallback; compare normal oA91D/CX02m tokens too. The published SDK lacks the drawn 1px token outline; independent actual-app/Pencil verification must classify that visible gap. No toolkit change or workaround is authorized.
- ShcL9: task/step/run/event preview, effective composition, included/excluded sources, unavailable variable and disabled Copy. Check Copied resets after composition, task, step, run, event or draft changes and that the clipboard still holds only the last copied raw payload.
- fTuZZ: XXiGS in-place full viewer, CKzcD header (52), IAt1E source rail (300), QI125 prompt, breadcrumbs/back/read-only/copy; no modal.
- udfuB: Agent roles and selected role editor. w20Jf: step overrides, pipeline/step selection and pipeline tab.
- B5A4Ux: sG2xi/yxIJ7 empty state and actual findings T1w9U. U69ob: import selection, raw preview, valid/invalid, pending/failure and resulting drafts, with unchanged repository sources.
- GxmNs: published v3, saved draft with v3, unsaved/invalid/new draft, failed publication and published v4.
- Additional interaction scenarios: delayed Reload preserves only post-request fields including invalid raw prompt; ordinary delayed autosave preserves later input; first Create locks all mutations; new rule identity stays correct while catalog refresh waits; failed Save/revert Retry and failed Reload Retry never publish; outside conflict and stale draft, loading/error/retry, leave/keep, Unicode/CRLF/soft wrap, selection insertion, focus/undo/redo, long-list scrolling and raw clipboard bytes. Preserve actual person-only/consent/refusal/secret boundaries; unreached human-only states are Unverified.

No task-level acceptance or native PASS is claimed by this implementation handoff.
