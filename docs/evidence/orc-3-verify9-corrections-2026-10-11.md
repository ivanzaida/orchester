# ORC-3: corrections after review and Verify9

Implementation evidence for the commit containing this report, based on
`7376ba9c8cd47d60c4b99822848e785858719710`. The final handoff names the new commit.
The earlier reports remain evidence for 7376ba9c, not this candidate.
ORC-3 remains pending independent code review and Pencil verification; this is
an implementation handoff, not either independent verdict.

## Safety corrections

- **F1 / H2:** Restore asks an explicit, non-modal replacement question whenever
  a saved unpublished draft, local changes or field errors exist. Keep leaves
  body, raw inputs and errors intact. Restore is unavailable during a pending
  write; its completion updates the saved snapshot before a later replacement.
  Historical viewing and the question prevent autosave. Confirmation returns the
  restored body to the normal guarded save path. Publications and runs are not
  written by Restore.
- **F2:** The loader retains the mounted editor through Pending and Rejected.
  Only a fulfilled read accepts a new snapshot/key. Rejection shows a retry
  surface alongside the retained local editor. Save/publish completion watchers
  and creation require no error before accepting retained data as success.
  Library refetch failures prioritize an error/retry surface. History failures
  also suppress retained rows and offer retry.
- **F3:** Work::Publish carries its reviewed OpenedPipeline into the workspace
  API. The matching transaction uses before.latest, alongside the reviewed body,
  rather than the newly loaded latest version. Current executor and permission
  checks remain. An identical body recreated on a newer base is refused with a
  stale/reload error; it cannot silently publish as the next version.
- **F4 / H3:** Selected-version changes still come from the correct immutable
  VersionHistory deltas. Compare adds a separate section naming the saved draft
  or published comparison target/version. Selection, Back and History remount
  reset comparison. This addresses the hidden state when opening a historical
  step and reopening History; no domain-history rewrite was made. Invalid or
  unsaved local work must be saved before comparison.

Nine deterministic regressions were added in history_safety.rs and
async_failures.rs: saved-draft Restore, Keep of invalid/raw/local work, pending
save reconciliation, rejected Reload and recovery, rejected save after success,
rejected publish after success, failed library refetch, reviewed-body recreation
on a newer base, and immutable/history comparison separation across remount.

## Scoped visual corrections

Read the person's open orchester2.pen only through Pencil Get/Print/screenshots.
Implemented corrections include library action alignment/help copy and icons;
creation breadcrumb, disabled collision reason, presets lock, slug dash,
contextual collision sentence and text-only notice; ordinal edge choices,
select alignment, model icon, ghost Remove, summary order and metadata; empty
summary content/starter icon; historical name rendered as read-only text;
rounded return connectors with filled arrows and a rounded toolbar; publication
copy/pluralization/review-open button state; comparison provenance/counts and
outlined Back; Settings crumb, slug readback, blue versioned changed badge,
count/outlined Undo, contextual validation; primary-first leave actions.

Restored the shared inspector variable-row vertical inset flagged by the prior
code review. No deprecated-app source or unrelated routes were changed.

These are implementation changes, not claims of native visual parity. Small
details still need attention in fresh Verify, including the missing-profile
glyph, comparison canvas Changed tag, Settings unavailable-trigger layout and
changed-marker pencil, and exact notice/toast spacing. The earlier minor
measurements must be rechecked on the new build, not carried forward as current
measurements.

## Supported fixtures

`tools/qa-app.ps1` now accepts:

- `-Scenario pipelines-empty`: a registered project with no pipeline entries.
- `-Scenario pipelines`: existing catalogue plus Feature delivery v3 with a
  distinctive saved unpublished draft; Empty draft; Publication problems with
  missing profile, unpublished profile and unsupported database-tool branches;
  Narrowed permissions with a published Claude profile, a single allowed
  command, a gate and interactive tool approval retained. Alternate reviewer
  offers a different command list for executor/reset transitions.

No agents/commands are launched by these fixture constructors. They use private
QA repositories and unavailable dummy executable paths. The fixture tests read
the data through the real store/workspace APIs, count three publication problems,
and require the narrowed step to pass workspace validation.

**Capability limit:** the current provider table offers no enforced network-off
option on Windows. Claude cannot enforce network-off on either platform.
The supported narrowed fixture therefore uses AsProfile network access; it
does not seed an invalid Off value to imitate K0WLeG. Exact illustrated combined
command/network narrowing remains unavailable, not visually passed. No
capability rules were changed.

## Commands and results

Final passing test results for the corrected product sources:

| Command | Result |
|---|---|
| `cargo test -p orchester-app --test pipelines pipelines_` | PASS, 32 |
| `cargo test -p orchester-workspace --test pipeline_writes pipeline_writes_` | PASS, 3 |
| `cargo test -p orchester-store --test draft_matching pipeline_draft_matching_` | PASS, 1 |
| `cargo test -p orchester-app --test seed seed_pipelines` | PASS, 2, including final capability correction |
| `cargo test -p orchester-app --test seed seed_names_every_scenario_once` | PASS, 1 |
| `cargo test -p orchester-app --test ui_kit buttons::` | PASS, 8 |
| `cargo test -p orchester-app --test ui_kit fields::` | PASS, 11 |
| `cargo test -p orchester-app --test ui_kit blocks::` | PASS, 13 |
| `cargo test -p orchester-app --test ui_kit choices::` | PASS, 3 |
| `cargo test -p orchester-app --test tab_labels tab_labels_` | PASS, 3 |
| `cargo test -p orchester-app --test i18n every_key_named_in_code_has_its_area_file_and_exists` | PASS, 1 |
| `powershell -File tools/check.ps1` | PASS on final Rust/locale/fixture sources |
| `cargo +nightly fmt -p orchester-app -p orchester-workspace` | PASS |
| `git diff --check` | PASS |

Earlier development invocations of the same pipeline test command failed first
on icon/closure compile errors, then obsolete selector/comparison/placeholder
expectations and a wrong test navigation target; all were corrected before the
32-test passing runs. Earlier seed-test invocations failed on field/method/enum
names; final fixture validation passes. Earlier Check invocations reported
formatting, render-function length, a borrow, needless pass-by-value, clone-on-Copy
and the temporary fixture enum typo; final Check passes. One formatter invocation
found an unmatched delimiter during a History refactor; corrected and formatted.

The only changes after the last product test pass were the new QA fixture's
network choice and its added workspace-validation assertion; those have their
own fresh passing fixture test and Check. Existing scenarios used by pipeline
and tab tests were unchanged. Documentation does not change check inputs.

Git inspection commands (status, rev-parse, diff/stat/numstat, diff --check) passed.
The authorized scoped add/commit and final clean-status commands are identified
in the final handoff.

## Native smoke attempt and remaining verification

`powershell -File tools/qa-app.ps1 -Name orc3-fixes10 -Scenario pipelines -NoRunner`
first failed to build on a missing fixture borrow; corrected. The second build
and seed succeeded and started owned **PID 33172** from
`.tmp/qa/orc3-fixes10/bin/orchester-app.exe`, project
`01a12b80-f945-7035-a58b-062f79d2c10c`. Its fixture predates the final network
capability correction; do not reuse it for final narrowed-state evidence.

The app could not write
`C:/Users/lurkm/AppData/Local/orchester-app-mcp/33172.json` because sandbox
access was denied. The script exited 0 with a missing-discovery diagnostic;
this is not a successful native smoke pass.
`python tools/lurq-mcp-client.py 33172 app_state --app orchester-app --discovery-dir "$env:LOCALAPPDATA\orchester-app-mcp"`
returned FileNotFoundError. No window input or screenshot followed.
Logs: `.tmp/qa/orc3-fixes10/app.log` and `app.err.log`.
Only owned PID 33172 was stopped and its exit was confirmed. The combined
Stop-Process/Get-Process command returned 1 because the final lookup found no
process; the subsequent explicit absence assertion passed.
No installed processes, credentials, ACLs, discovery relocation, previous
refused approvals 10/13/14, global input or desktop capture were used.

Fresh independent Review and Verify remain required. Native verification needs
working authorized exact-PID discovery access; the current session did not have
it. Preserve person-only questions and the input lock. Protected acceptance
states remain **Unverified unless the person acts**; no synthetic acceptance or
bypass is authorized by this report.

Verifier comparisons at the supported dark desktop size:

- Library GaNsb, empty library sI5K0; page creation lAQis and collision Hgequ.
- Editor SVBQi; summary c24Tv; empty Z26ZFs; missing/unpublished/unsupported
  Clxs2; supported narrowing K0WLeG with the capability limitation above.
- Publication problems c7g6Qi, review ITC0R, success nmY08.
- History OchSR and states qCku8: saved/local/invalid draft replacement
  question SzRf6, Keep preservation, confirmed replacement, pending-write
  gating, first-version history, Compare provenance, step-inspector remount
  resetting Compare, loading/error/retry where supported.
- Settings x4IIjM, changed Yx32O, validation fmnva and leave component RWODj.
- Recheck shared variable-row inset in Connections and host-focus preservation.

Not run: full test suites, other MCP/domain/workspace/store tests beyond the
filters above, native screenshot comparisons, human-only permission acceptance,
conclusive host-focus checks, macOS cross-check/runtime (standing SDK deferral).
960/light passes remain excluded. No task-complete or merge verdict is claimed.
