# ORC-561 · Recorded-request decision model tests

Workspace/runner implementation against base `20bc6537217727bf804e4b5b114615ab352ade91`, plus this run's saved changes.

## Invocation and progress

- The explicit workspace action is `Runners::test_decision_connection(root, submission_id, connection, model, Writer)`.
  It sends IPC `TestDecisionConnection` (protocol 40); the runner repeats the hosted-server authority check.
  No connection save, check, agent invocation or project startup invokes a test.
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
