# ORC-7: post-vault COMMIT error contract — Implement9

Candidate: the scoped commit accompanying this report on `orchester/orc-7-run-1`, based on `c335d3901da395ae3c4e3230ec8c13e69e890848`. The final Implement response identifies the exact commit. These results cover the saved sources in that commit; the only subsequent edits are this evidence report and checkpointing.

## Correction and regression

The independent finding in .tmp/orchestrator/orc7/re-review-report.md is reproduced by the prior control flow: BEGIN and COMMIT used the same Store error mapper. ProjectDb::with_write_transaction now takes separate BEGIN and COMMIT error mappers; the latter receives the successful callback's outcome. replace_allowed_in maps a COMMIT failure after Replaced to RotationNotRecorded with the correct SecretId and original storage source. A COMMIT failure after NeedsPerson remains Store. Callback errors propagate unchanged; the existing rollback guard also remains active during COMMIT error mapping and unwinding.

The workspace regression uses a real SQLite deferred foreign-key constraint, inserted by an UPDATE trigger. UPDATE succeeds and outer COMMIT fails with SQLITE_CONSTRAINT_FOREIGNKEY; no production fault-injection API or OS keychain is involved. It asserts exactly one in-memory vault change, the replacement credential retained without exposing it in assertion output, unchanged durable secret metadata (including no rotated_at), RotationNotRecorded with its source, the existing errors.secretRotationNotRecorded key and partial-success English message, and a successful second-handle write after failure.

Two negative workspace regressions check competing-writer BEGIN failure and typed vault refusal: no credential is stored and neither error claims partial success. A store regression explicitly observes the successful UPDATE inside the transaction, passes an external-value-unchanged outcome to the COMMIT mapper, and checks rollback plus reuse of the same and a second database handle. Existing callback-error/unwind and authorization/barrier tests still pass. The NeedsPerson COMMIT mapping is explicit in production; no native NeedsPerson COMMIT failure was injected.

No UI source, locale, theme, navigation, authorization gate, deprecated app, design file or install was changed. No platform-specific code or dependency was added. Source sizes remain below 600 lines.

## Commands and results

Every shell command ran from this worktree. All completed with exit 0. Repeated commands are grouped below; no failed build/test command occurred.

| Command | Outcome |
| --- | --- |
| `git status --short` | PASS: initially clean; later only scoped source/test files and this report. Final response records clean committed state. |
| `git log -3 --oneline` | PASS: confirmed c335d390 base. |
| `cargo +nightly fmt -p orchester-store -p orchester-workspace` | PASS. |
| `cargo test -p orchester-workspace --test secrets transaction_failures` | PASS twice: 3 tests each; final run after making the credential assertion's failure output value-free. |
| `cargo test -p orchester-store --test secret_transaction secret_transaction_` | PASS: 3 tests. |
| `cargo test -p orchester-workspace --test secrets secrets_` | PASS: 11 tests; only the new credential assertion's failure wording changed afterward, covered by the final transaction_failures run. |
| `cargo test -p orchester-workspace --test secrets mcp_variable_` | PASS: 1 test. |
| `cargo test -p orchester-workspace --test secrets tool_and_mcp_` | PASS: 1 test. |
| `cargo test -p orchester-app --test secrets secrets_` | PASS: 19 headless state/action/privacy/person-only/layout/QA tests with corrected production dependencies. |
| `cargo clippy -p orchester-store -p orchester-workspace --all-targets -- -D warnings` | PASS twice, including the final test sources. |
| `cargo +nightly fmt -p orchester-store -p orchester-workspace -- --check` | PASS. |
| `git diff --check` | PASS; repeated before staging. |
| `git diff --stat` | PASS: inspected tracked scope before adding the new test file and report. |
| `git diff -- crates/store/src/project_db/write_transaction.rs crates/workspace/src/secrets/data.rs crates/store/tests/secret_transaction.rs crates/workspace/tests/secrets.rs` | PASS: self-reviewed transaction boundary and existing-test updates; new test file read through Orchester. |
| `git add crates/store/src/project_db/write_transaction.rs crates/store/tests/secret_transaction.rs crates/workspace/src/secrets/data.rs crates/workspace/tests/secrets.rs crates/workspace/tests/secrets/transaction_failures.rs docs/evidence/orc-7-commit-error-return9.md` | Authorized scoped staging; final response records completion. |
| `git diff --cached --check` | Final response records completion. |
| `git diff --cached --stat` | Final response records completion. |
| `git commit -m "fix(workspace): preserve secret rotation commit failures"` | Authorized scoped commit; final response records outcome/revision. |
| `git log -1 --oneline` | Final response records committed candidate. |

Not run: tools/check.ps1 (the pipeline's Check9 is authoritative, per step instructions); broad/unfiltered tests; macOS cross-build/native checks; native QA app launches/captures; real keychain/provider/disk-full injection or clipboard integration; packaging/install; master integration/navigation tests; independent code re-review/design verification. No QA processes were started or stopped.

Repository and Rust skill were read through Orchester. UI skills and lurq references were read for the task handoff. Pencil was accessed read-only with get_app_state, read_skill and execute Get/Print calls for Secrets ccz8n and the list/replacement/pending/error nodes; no design mutations or implementer parity claim.

## Handoff

Run Check9 and independent code re-review on the newly committed candidate. Check8 and the c335 visual reports retain their original revision identity.

Verify the existing Windows wide dark states under Secrets ccz8n, at their matching design sizes; no confirmation bypass is authorized:
- Five-row mixed list/inspector: DrLE6; MCP/tool/unused panels LDP7i/zAzUD, AZJK1, L7UxtU.
- Add, duplicate and validation: YGy5i, Gam8g, j0ffR/SJwDy.
- Replacement and protected inline question/decline/input lock: yTCmk, with protected input supplied by a person.
- Saving/Replacing: nykwz/ZO3gD and nykwz/fEyY0; failure/retry: CeQod, UGNxm. The corrected COMMIT failure must retain the existing partial-success message.
- Archive/impact/pending/error: K2epbr, XoPMi/esH3M/l1edUs, oa4Eb, xA05w, lPksq, SsCvf.
- Usage: aKeYe. Project/production empty: L0uiU/TplaJ/GE9ao, Ds3OR/RlvMO. Read failure/recovery: z9erZk/QOLZu.

The historical nine minor findings and every Unverified/person-only/macOS limitation remain; do not relabel earlier screenshots as evidence for this candidate. The bounded c335 report is .tmp/verify/orc7-c335/report.md. Full inventory remains in docs/evidence/orc-7-verify-return.md. 960/light remain excluded. Preserve active MCP links and functional ORC-6 Tools during final master integration; that integration is outside this correction.
