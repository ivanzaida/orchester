# ORC-561 · Recorded-request decision model tests

Workspace/runner implementation against base `20bc6537217727bf804e4b5b114615ab352ade91`, plus this run's saved changes.

## Invocation and progress

- The explicit workspace action is `Runners::test_decision_connection(root, submission_id, &connection, model, Writer)`.
  It sends IPC `TestDecisionConnection` (protocol 41) with the displayed connection's ID, URL and key reference.
  The runner rejects changed URL/key settings before reading history or resolving the key, then repeats the
  hosted-server authority check. No connection save, check, agent invocation or project startup invokes a test.
- `connections::decision::test::progress_in` reads the submission's durable progress while its IPC reply is pending.
  `latest_in` reads the latest result for the connection, hiding results from an edited server URL or key reference.
- `stop_in` sets the durable stop flag. The runner polls it every 25 ms and before refilling the eight-request pool.
  Dropping the pool cancels in-flight HTTP futures; a request already received by the server may still spend tokens.
- Only one comparison runs per project. Submission IDs cannot be replayed to spend again. New runners mark unfinished
  comparisons interrupted and never resume them automatically.
- The persisted report contains the selected model, endpoint and key reference, request totals, completions, in-flight
  count, failures, too-long inputs, median whole-request milliseconds, reported input-token subtotal, unknown-usage
  count, and counts at 0.50, 0.80, 0.90 and 0.95. Failed answers count as asking the person at every threshold.
  These comparisons never answer live approvals or change their reviewer usage/budgets.

## Historical reconstruction

Each approval with reviewer usage is selected once, using its last review's start time and verdict. Its invocation's
immutable permissions provide step facts; the pinned run pipeline provides the step name; only `run_task_snapshots`
provides the task. The request, working folder, command note / tool arguments use the same recorded fields as live
review. The last three stored step messages at or before the historical review began are included, oldest first.
Later task edits, permissions and messages do not substitute for historical facts.

The existing bounded review input and ORC-555 System One question builder are reused, followed by the existing
secret-safe HTTP transport, model lookup, parser and combined two-second deadline. Unknown fields are removed from
the request and listed as JSON paths in `state.not_stored`; their union is retained in the report. These include
historical parser findings, build-variable names and roots facts that were not retained. Missing legacy permissions
or task snapshots are omitted rather than replaced by current settings. Too-long requests are counted separately
and never truncated into a different request.

## Scope and verification

This task adds the workspace/runner API and durable read model; it changes no desktop visual files. The brief's
`docs/design/decision-reviewer.md` is absent in this worktree. The connection page can use the action/read/stop API
above; no new desktop control or design state is claimed by this implementation. Frame 02 `GM4yE` is the brief's
named visual target for a later connection-page UI task.

The runner integration tests use a seeded project database and a concurrent loopback System One stand-in. They cover
historical task/messages/permissions, omissions, all four threshold rows, latency and tokens, concurrency eight,
progress and stop, hosted automation refusal, repeated and overlapping submissions, interruption, and empty history.
The existing decision-review transport tests also cover the shared HTTP path. No live inference, desktop QA or macOS
cross-build was run. No `cfg(target_os)` code was added or changed.

## Commands and outcomes

All commands ran in this worktree's sandbox. The scratch launcher sets only `RUSTC_WRAPPER=""` in its child
environment because the inherited sccache wrapper returned Windows "Access is denied"; no machine settings changed.

| Command | Outcome |
| --- | --- |
| `git status --short` | Initial tree clean; final changes limited to this feature and documentation. |
| `git rev-parse HEAD` | Base `20bc6537217727bf804e4b5b114615ab352ade91`. |
| `git diff --stat` | Inspected tracked change summary. |
| `git diff` | Self-reviewed tracked changes; new files read separately through file tools. |
| `git diff --check` | Passed. |
| `cargo test -p orchester-runner --test decision_test decision_test` | Initial attempt failed before project compilation: inherited sccache access denied. |
| `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-runner --test decision_test decision_test` | First two launcher attempts failed before Cargo; launcher corrected. Then fixed a fixture's unchecked TaskUpdate and incomplete task edit. Final candidate: **7 passed**, including interruption usage assertions. |
| `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-runner --features test-agent --test decision_review decision_review` | **9 passed, 1 ignored** (live inference). Shared HTTP path unchanged by subsequent reporting-only edits. |
| `cargo +nightly fmt -p orchester-core` | Passed, including final formatting pass. |
| `cargo +nightly fmt -p orchester-store` | Passed, including final formatting pass. |
| `cargo +nightly fmt -p orchester-ipc` | Passed, including final formatting pass. |
| `cargo +nightly fmt -p orchester-runner` | Passed, including final formatting pass. |
| `cargo +nightly fmt -p orchester-workspace` | Passed, including final formatting pass. |
| `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 powershell -ExecutionPolicy Bypass -File tools/check.ps1` | Read all output of all three runs. First failed dispatcher size (81/80), fixed by extracting the estimate reply handler; second failed one redundant test clone, removed. Final run passed rustfmt, all-workspace/all-target/all-feature Clippy with warnings denied, file sizes and UI literals. Last line: **`check: OK`**. |

The final gate covers all saved Rust changes; only the Done entry and this verification record were added afterward.
Not run: unfiltered/full-suite tests, live inference, desktop QA/Pencil comparison (no visual changes), and macOS
cross-check (no Apple SDK/C cross-compiler on this Windows machine). No platform-specific code changed.
No app copies were started. No commits were made.

## Review return: connection snapshot race

The correction covers saved changes on top of `e7cf7a00ba0e4368c9df2b5f5fede832bd67de3a`.
The workspace now requires the displayed `DecisionConnection` snapshot, and IPC protocol 41 carries its ID,
base URL and key reference through the scheduler. The runner compares URL/key with the loaded connection before
reading history, resolving credentials or sending requests. A mismatch returns typed `ConnectionChanged`;
the reviewer uses the validated, owned connection settings for the whole test.

The added `decision_test_stale_endpoint_or_key_is_refused_before_sending` regression serializes the submission,
edits the stored endpoint or key reference, then processes the submission. Both edits are refused without a report
or a request reaching either stand-in server. No desktop visual or platform-specific code changed.

Commands for this correction (earlier command evidence above applies to the earlier candidate only):

- `git status --short`: clean at start; final changes restricted to the correction and this evidence.
- `git rev-parse HEAD`: `e7cf7a00ba0e4368c9df2b5f5fede832bd67de3a`.
- `git diff`: self-reviewed; new IPC file read separately.
- `git diff --stat`: inspected.
- `git diff --check`: passed.
- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-runner --test decision_test decision_test`: **8 passed**.
- `cargo +nightly fmt -p orchester-ipc`: passed.
- `cargo +nightly fmt -p orchester-runner`: passed.
- `cargo +nightly fmt -p orchester-workspace`: passed. Formatting was repeated after the test completed.
- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 powershell -ExecutionPolicy Bypass -File tools/check.ps1`: passed; all output read. Last line: **`check: OK`**.

The gate covers all saved Rust changes for this correction. Only this evidence entry was appended afterward.
Not run: full-suite tests, shared transport regression (transport unchanged), live inference, desktop QA/Pencil
comparison (no visual change), or macOS checks (no Apple SDK/C cross-compiler); no `cfg(target_os)` code changed.
No app copies were started and no commits were made.
Unrelated observation left unchanged: `crates/ipc/tests/browser.rs:274` asserts protocol 38; the input revision
already used protocol 40. That test was not run in this correction.

## Verification after the moved-base merge

Verified merge `2e10eab162abeefdf0f4a418fff88f2c1a58b178`, with parents `ab8757ac0` (saved correction)
and `97956508a` (module/decision-models, ORC-562). The person performed the merge externally after Git metadata
writes were refused by the sandbox. Only `docs/plan.md` conflicted; the resolution retains both tasks' Done entries.
No production-code conflict resolution was needed. The runner, IPC and workspace Test code match the first parent.
Updated the ORC-561 Done entry to record protocol 41 and eight tests; no Rust changes were needed after merging.

Fresh commands and outcomes for the merged candidate:

- `git status --short`: clean initially.
- `git log -4 --oneline --parents`: confirmed the merge and both parents.
- `git show --stat HEAD`: inspected merged files and documented conflict.
- `git show --format=fuller --no-patch HEAD`: confirmed merge metadata.
- `git diff HEAD^1 HEAD -- crates/runner crates/ipc crates/workspace/src/runner`: empty.
- `git rev-parse HEAD`: `2e10eab162abeefdf0f4a418fff88f2c1a58b178`.
- `git diff --check`: passed.
- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-runner --test decision_test decision_test`: **8 passed**.
- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-workspace --test decision_connections mcp_decision_reviewer`: **3 passed** (another target had no matching tests).
- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 cargo test -p orchester-workspace --test decision_reviewer_setting mcp_decision_reviewer`: **3 passed**.
- `cargo +nightly fmt -p orchester-ipc`: passed.
- `cargo +nightly fmt -p orchester-runner`: passed.
- `cargo +nightly fmt -p orchester-workspace`: passed.

Earlier attempted `git add -A`, `git commit --allow-empty -m "checkpoint before merging module/decision-models"`
and `git merge --no-ff module/decision-models` each failed on sandbox Git metadata writes; no merge occurred
from those commands. The external merge above supersedes those attempts.

- `powershell -ExecutionPolicy Bypass -File .tmp/decision-test/run.ps1 powershell -ExecutionPolicy Bypass -File tools/check.ps1`: passed; whole output read. Final line: **`check: OK`**.
- Final `git diff --check`: passed; `git status --short`: only this evidence and the plan entry changed.

The fresh gate covers the merged Rust candidate at `2e10eab162abeefdf0f4a418fff88f2c1a58b178`.
Only this result was appended after the gate. Not run: full-suite tests, live inference, shared transport
regression (transport unchanged), desktop QA/Pencil comparison (no visual changes), and macOS checks (no Apple
SDK/C cross-compiler). No `cfg(target_os)` code changed. Ready for the pipeline's independent Review step.
