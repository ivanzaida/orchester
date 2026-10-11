# ORC-7: authorization corrections after independent review

Candidate: the scoped fix accompanying this report on `orchester/orc-7-run-1`, based on `8ebe504c513c6788809a33b43de6be7c8e36106c`. The final Implement response identifies its commit. Tests below cover these saved sources; the only subsequent changes are this report and checkpointing.

## Changes

P1: replacement authorization reads native connection API key SecretId references, MCP references, tool drafts and every retained published tool version. It no longer treats the draft-first display snapshot as an authorization index. Native-only references return NeedsPerson for UnusedOnly, with no vault write or rotation timestamp change. PersonConfirmed remains valid through the existing person-only flow. Its domain regression uses an in-memory vault, without driving or bypassing any protected window confirmation.

P2: an immediate SQLite transaction begins before reading references and remains open through the external vault write and rotation metadata commit. All SQLite binding writers, including other ProjectDb handles/processes, share that writer lock. The transaction guard rolls back on callback errors, commit failures and unwinding. No post-write reference recheck is used as authorization.

Three deterministic channel-barrier regressions pause vault.store while a second, already-open ProjectDb attempts an MCP, native or tool binding. The write must return SQLite DatabaseBusy before the vault is released. After rotation commits, that same writer can bind, and the next UnusedOnly call must return NeedsPerson without changing the vault or timestamp. Separate storage regressions check rollback and writer release on callback failure and unwind.

The tool/MCP display and archive counts, screen layouts, strings, theme, protected input mechanisms, active MCP/tool link callbacks and deprecated app are unchanged. Native keys and retained versions participate in authorization independently of that display. No new runner execution is introduced.

Finding 3 was withdrawn as an independent blocker by the reviewer's integration clarification. This return does not disable tool links or merge master. Final integration must preserve master b5b6c031's functional ORC-6 Tools route. The optional concrete Tool(id, ToolTab::Configuration) navigation change is deferred.

## Commands and results

All commands ran from this run's worktree. No tools/check.ps1 invocation: the pipeline Check step is authoritative for this return.

| Command | Outcome |
| --- | --- |
| git status --short | PASS: initially clean; subsequent checks show only the scoped files. |
| git log -3 --oneline | PASS: identified base 8ebe504c. |
| cargo +nightly fmt -p orchester-store -p orchester-workspace | PASS, three invocations while editing. |
| cargo test -p orchester-workspace --test secrets secrets_ | PASS: 8 tests, including five new authorization/barrier regressions. |
| cargo test -p orchester-workspace --test secrets mcp_variable_ | PASS: 1 test. |
| cargo test -p orchester-workspace --test secrets tool_and_mcp_ | PASS: 1 test. |
| cargo test -p orchester-store --test secret_transaction secret_transaction_ | Initial compile FAIL: test called private record_rotation; corrected to public from_stored construction. Two subsequent PASS runs: 2 tests each, latest after removing redundant clone. |
| cargo test -p orchester-app --test secrets secrets_ | PASS: 19 headless state/action/privacy/person-only/layout/QA tests against the corrected workspace/storage sources. |
| cargo clippy -p orchester-store -p orchester-workspace --all-targets -- -D warnings | First PASS before adding storage cleanup tests; next FAIL for redundant clone in the new storage test; corrected final PASS. |
| cargo +nightly fmt -p orchester-store -p orchester-workspace -- --check | PASS; final check repeated after the test-only clone correction. |
| git diff --check | PASS throughout editing and at handoff. |
| git diff --stat | PASS: inspected tracked scope before adding new files to the index. |
| git diff --numstat | PASS: inspected tracked sizes. |
| git diff -- crates/workspace/src/secrets/data.rs | PASS: self-reviewed authorization and transaction boundary. |
| git add crates/store/src/project_db/mod.rs crates/store/src/project_db/write_transaction.rs crates/store/tests/secret_transaction.rs crates/workspace/src/secrets/data.rs crates/workspace/src/secrets/mod.rs crates/workspace/src/secrets/replacement.rs crates/workspace/tests/secrets.rs crates/workspace/tests/secrets/replacement.rs docs/evidence/orc-7-authorization-return8.md | Scoped staging for the authorized commit; see final response for outcome. |
| git diff --cached --check | See final response for final staging outcome. |
| git diff --cached --stat | See final response for final staged scope. |
| git commit -m "fix(workspace): serialize secret replacement authorization" | Requested scoped commit; final response records outcome/revision. |
| git log -1 --oneline | Final response records the committed candidate. |

Not run: tools/check.ps1; broad/unfiltered/workspace-wide tests; macOS cross-build or native checks; native app/QA fixture launches; OS keychain/provider or clipboard integration; packaging/install; independent code re-review/design Verify; master integration/navigation tests. No installed process/data, global input, protected confirmation or design file was touched.

## Handoff to Check, Review and Verify

Run Check8 on this candidate; Check7 is evidence only for unchanged 8ebe inputs. Independently re-review native/reference coverage and the transaction interval, including the second-handle barrier tests.

No visible source changed in this return. The scoped visual PASS in .tmp/verify/orc7-independent8ebe/report.md remains evidence for exact 8ebe, not a new-build verification verdict. Its nine minor differences and all person-only/macOS/clipboard/keychain/navigation limits remain explicit.

Verify the Secrets layer ccz8n in data/orchester2.pen at the matching Windows dark sizes, respecting the protected-input constraints:
- Five-row mixed list and inspector: DrLE6; MCP/tool/unused panels LDP7i/zAzUD, AZJK1, L7UxtU.
- Add, duplicate, validation: YGy5i, Gam8g, j0ffR/SJwDy, K2epbr.
- Replacement and protected inline question/decline/input locks where a person can supply the required input: yTCmk; no automation of person-only confirmation.
- Saving/Replacing and failure/retry: nykwz/ZO3gD, nykwz/fEyY0, CeQod, UGNxm. Reference-free captures do not verify mixed-use person-only states.
- Archive, impact and pending/error: XoPMi/esH3M/l1edUs, oa4Eb, xA05w, lPksq.
- Usage: aKeYe. Project/production empty: L0uiU/TplaJ/GE9ao, Ds3OR/RlvMO. Read failure/recovery: z9erZk/QOLZu. See docs/evidence/orc-7-verify-return.md for the complete inventory, including SsCvf.

960/light remain excluded; macOS and native person-only states remain Unverified. Final integrated navigation must preserve functional ORC-6 Tools and active named MCP navigation; this branch's old Tools placeholder is not grounds for disabling the links in the integrated product.
