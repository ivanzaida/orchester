# ORC-7: return from Verify attempt 6

Candidate: saved changes in this run's worktree based on 5e0d586a99df2ed1d72a771616eabf8abde72011. No commit by the implementer. The earlier consolidated report .tmp/verify/orc7-current/continuation/report.md covers only that unchanged base, and remains preserved.

## Corrections

V5-M1: the five-row table now requests its complete 272px content height instead of sharing flex growth with the following spacer. It can still shrink to the available height for long virtualized lists. Its 32px header does not shrink. A headless regression reproduced the original 259.3px height, then passed with all five complete 48px rows and the 14px gap before the handling notice.

V5-M2: Saving dims Name, Type, Secret value (including its hint), Project and Environment groups to Pencil's 0.6 opacity. Replacing dims only the New value group and its hint. Their loading primary action uses the same theme value, drawn in ZO3gD and fEyY0. The controls retain their actual locks, loading spinner, failure/retry behavior and existing person-only input gate. No domain logic or credential handling changed.

The nine minor findings in the earlier report remain for Verify to assess; this return addresses the two major findings. In particular pending select chevrons, archive loading treatment and the other minor rhythm differences are not claimed fixed.

## Checks

Commands ran from this worktree:

| Command | Result |
| --- | --- |
| git status --short | PASS: initial clean base. |
| git rev-parse HEAD | PASS: 5e0d586a99df2ed1d72a771616eabf8abde72011. |
| cargo test -p orchester-app --test secrets secrets_five_rows_fit_before_the_handling_notice | Expected FAIL against original layout: 259.3px vs 272px. |
| cargo +nightly fmt -p orchester-app | PASS, run twice during editing. |
| cargo test -p orchester-app --test secrets secrets_ | First attempt compile FAIL: Element has no opacity builder. Corrected using a Row around the shared action. Final PASS: 19 tests, including states/actions/privacy/person-only/retry/QA/theme and the new regression. |
| cargo clippy -p orchester-app --all-targets -- -D warnings | PASS for final source. |
| cargo build -p orchester-app --example secrets_qa | PASS for final source. |
| cargo +nightly fmt -p orchester-app -- --check | PASS. |
| git diff --check | PASS. |
| git diff --stat | PASS, scoped five source/test files before adding this report. |
| git diff -- crates/desktop-app/src/screens/secrets/list.rs crates/desktop-app/src/screens/secrets/form/fields.rs crates/desktop-app/src/theme/secrets.rs crates/desktop-app/src/theme/mod.rs crates/desktop-app/tests/secrets/states.rs | PASS, self-reviewed scope. |
| powershell -File .tmp/orc7-attempt6/launch.ps1 list | Launched owned fixture PID 29820. |
| powershell -File .tmp/orc7-attempt6/launch.ps1 writes | Launched owned fixture PID 37896. |
| Get-Process -Id 29820,37896 &#124; Select-Object Id,Path | PASS: both copied executables under .tmp/orc7-attempt6/bin/. |
| python .tmp/orc7-attempt6/capture.py 29820 .tmp/qa/secrets-native/app-nYrg6E/discovery list | Sandboxed attempt FAIL: protected discovery PermissionError. |
| python .tmp/orc7-attempt6/capture.py 37896 .tmp/qa/secrets-native/app-3dc24l/discovery writes | Sandboxed attempt FAIL: protected discovery PermissionError. |
| python tools/lurq-mcp-client.py 29820 app_state --app orchester-app --discovery-dir .tmp/qa/secrets-native/app-nYrg6E/discovery | FAIL: confirmed protected discovery PermissionError. |
| python .tmp/orc7-attempt6/capture.py 29820 .tmp/qa/secrets-native/app-nYrg6E/discovery list (approved sandbox escalation) | PASS: fresh five-row list, Saving and failed-add native captures. |
| python .tmp/orc7-attempt6/capture.py 37896 .tmp/qa/secrets-native/app-3dc24l/discovery writes (approved sandbox escalation) | PASS: fresh Replacing and failed-replacement native captures. |
| python .tmp/orc7-attempt6/capture.py 29820 .tmp/qa/secrets-native/app-nYrg6E/discovery close (approved sandbox escalation) | PASS: sampled qa_is_foreground=false; closed only owned fixture 29820 through window-close. |
| python .tmp/orc7-attempt6/capture.py 37896 .tmp/qa/secrets-native/app-3dc24l/discovery close (approved sandbox escalation) | PASS: sampled qa_is_foreground=false; closed only owned fixture 37896 through window-close. |
| Get-Process -Id 29820,37896 -ErrorAction SilentlyContinue &#124; Select-Object Id,Path | Expected empty result (exit 1): both owned fixtures absent after MCP close. |

Final git status --short: exactly five changed Rust files and this new evidence document. Final git diff --check: PASS; git diff --stat: PASS for the tracked source/test changes. Launch helper command sessions ended with exit 0 after both fixtures closed.

Fresh native smoke images for this saved source are under .tmp/orc7-attempt6/app/: five-row-list.png (2160 × 1350, 1440 × 900 logical), add-pending.png, add-failed.png, replace-pending.png and replace-failed.png (2160 × 1368, 1440 × 912 logical; their inspector regions are 360 × 656). Inspected these five app captures and read-only Pencil screenshots/Get for DrLE6, ZO3gD and fEyY0. All five rows are visible, pending groups/actions are dim, and failure restores ordinary contrast. This is implementer smoke evidence, not an independent verification verdict. The writes fixture has no references and does not prove a person-authorized mixed-use replacement. No installed-app data/process, OS vault, clipboard or global input was touched.

Not run: tools/check.ps1 (the pipeline Check step is authoritative), whole/unfiltered tests, workspace tests (domain source unchanged), macOS cross-build/native checks, packaging/install, clipboard/OS keychain checks, or independent design Verify. Earlier Check success is not a verdict on these changed inputs.

## Independent Verify

Use fresh captures of the new candidate. Read only Pencil layer Secrets ccz8n in data/orchester2.pen. Priority comparisons:

- Five-row list at 1440 × 900: DrLE6, complete 728 × 272 table and following notice.
- Saving at matching 360 × 656 inspector region: nykwz / ZO3gD. Verify labels, values, hints, project and loading action dim together; actual locks remain.
- Replacing at matching inspector size: nykwz / fEyY0. Verify the New value group and loading primary action dim; surrounding explanation, notice and Used by keep their normal contrast.
- Failure/retry returns to ordinary field/action appearance; preserve the existing privacy and person-only behavior.

Full state/node inventory and fixture instructions remain in docs/evidence/orc-7-verify-return.md: DrLE6, LDP7i/zAzUD, YGy5i, Gam8g, j0ffR/SJwDy, yTCmk, K2epbr, XoPMi/esH3M/l1edUs, aKeYe, L0uiU/TplaJ/GE9ao, Ds3OR/RlvMO, z9erZk/QOLZu, nykwz and SsCvf. Person-only native question/confirmation/decline/retry/input-lock states remain Unverified, as do macOS native checks. 960/light are excluded. The new implementer captures, if supplied, are smoke evidence only and do not replace independent Verify.
