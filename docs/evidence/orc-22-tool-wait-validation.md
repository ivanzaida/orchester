# ORC-22 implementation validation

2026-10-11. The current review-correction candidate is
`d2bb69a09f31e1bd8c61377faf178dc41fe78c8a` plus the saved changes documented in
**Review correction — current validation** below. No implementing agent commit
was made. That section is the verdict on the current inputs.

The initial validation below describes the earlier candidate. Orchester
checkpointed its implementation, fake-cell harness, tests and initial evidence
in `febfd82989e4abbe62ea3cd61fde9ee947fed778`, originally based on
`c263258d145eb22c167bda6a702e6e2cf35a6615`. The following candidate added the
person-approved `tools/check.ps1` change and reached `d2bb69a`. Its 41 passing
filtered tests and gate result are retained as historical evidence. Review
found that its fake delayed MCP item completion until cell collection, masking
the result-before-turn-completion race; those results are not validation of the
corrected fake or changed Rust below.

## Reproduction and behavior

Before the production fix, the fake started a real outside search on its relay,
held the approval, then emitted a completed model turn with a pending-cell final
message. The approval vanished when the runner ended the process tree.
`cargo test -p orchester-runner --features test-agent --test codex_tool_wait codex_tool_wait_approval`
failed at that assertion. This is pre-fix evidence, not the current verdict.

The eight initial runner tests cover approval and denial three seconds after
the model turn completes, the result returning before turn completion, a
delayed previous turn-start reply, pause, cancel, runner loss, and graceful
shutdown. Approved and denied calls finish in one invocation and one attempt.
Stopped decisions never produce a failed attempt, never execute the outside
read and refuse stale approval answers. The continuation retains the original
model, effort, environment and output schema.

## Initial filtered tests (before review correction)

All commands exited 0. There are 41 passing tests in these filtered suites.

| Command | Result |
| --- | --- |
| `cargo test -p orchester-runner --features test-agent --test codex_tool_wait codex_tool_wait` | 8 passed |
| `cargo test -p orchester-core --test app_server app_server` | 2 passed, 10 filtered out |
| `cargo test -p orchester-runner --features test-agent --test claude_approvals claude_approvals` | 11 passed |
| `cargo test -p orchester-runner --features test-agent --test claude_prompt_relay claude_prompt_relay` | 6 passed |
| `cargo test -p orchester-runner --features test-agent --test codex_relay codex_relay` | 5 passed |
| `cargo test -p orchester-runner --features test-agent --test approval_shutdown approval_shutdown` | 3 passed |
| `cargo test -p orchester-runner --features test-agent --test orchester_tools orchester_tools` | 6 passed with the scratch ripgrep directory prepended to PATH |

The file-tool test was initially 5 passed / 1 failed because this shell lacked
ripgrep. A real ripgrep was installed only in `.tmp/orc-22/tools` and that suite
was rerun successfully. No source dependency or user's installation changed.
Earlier intermediate runs of the new suite passed 2, then 7, then 8 tests;
the current 8-test result above supersedes those intermediate results.

## Initial quality checks and environment limits

| Command | Initial candidate result |
| --- | --- |
| `powershell -File tools/check.ps1` | Fresh exit 0, `check: OK`: formatting of all 13 workspace packages, full-workspace clippy, source size and UI literal checks pass with the approved per-package fix. |
| `cargo +nightly fmt --all -- --check` | Earlier diagnostic, before the gate change: Windows error 206, command line too long. A short drive alias canonicalized back to the long path and did not repair it. |
| `cargo +nightly fmt -p orchester-core -p orchester-runner -- --check` | Exit 0 |
| Per-workspace-package `cargo +nightly fmt --package <name> -- --check`, using `cargo metadata --no-deps --format-version 1` | Exit 0 for all 13 workspace packages |
| `cargo clippy -p orchester-core -p orchester-runner --all-targets --all-features -- -D warnings` | Exit 0; the later full-workspace clippy within check.ps1 also passed on the current Rust changes |
| `cargo clippy -p orchester-core -p orchester-runner --all-targets --all-features --target aarch64-apple-darwin -- -D warnings` | Blocked: native dependency build scripts cannot find `cc`; no runner cross-check verdict |
| `cargo clippy -p orchester-core --all-targets --all-features --target aarch64-apple-darwin -- -D warnings` | Exit 0 |
| `git diff --check` | Exit 0 |

The first check.ps1 run additionally caught an avoidable clone in the new test.
It was fixed using `std::slice::from_ref`; the fresh check has no clippy failure.
That earlier run is retained as context and is not the verdict on this candidate.

The per-package formatter check was run read-only with:

```powershell
$fmtMetadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$fmtFailed = $false
foreach ($fmtPackage in $fmtMetadata.packages) {
    if ($fmtMetadata.workspace_members -contains $fmtPackage.id) {
        Write-Output ('rustfmt: ' + $fmtPackage.name)
        cargo +nightly fmt --package $fmtPackage.name -- --check
        if ($LASTEXITCODE -ne 0) { $fmtFailed = $true }
    }
}
if ($fmtFailed) { exit 1 }
```

The person explicitly approved the per-package gate fix as its own small change
within this task. `tools/check.ps1` now obtains workspace package names through
Cargo metadata and checks formatting separately for each package, retaining
nightly-only formatting options and failure handling. Its fresh run exits 0.
Earlier gate runs failed with Windows error 206; these are retained as pre-change
evidence and are not the verdict on the final script. ORC-22 is now recorded in
the plan's Done list.

Scratch prerequisite commands: `cargo install ripgrep --locked --root .tmp/orc-22/tools --target-dir .tmp/orc-22/tools-target`
initially failed with schannel NoCredentials; setting
`$env:CARGO_HTTP_SSL_BACKEND = 'openssl'` and rerunning that command succeeded.
The successful file-tool test prepended
`(Join-Path $PWD '.tmp/orc-22/tools/bin') + ';' + $env:PATH` to PATH.

## Initial validation limits

No unfiltered or full-workspace test suite, signed-in Codex/Claude live run,
macOS runtime test, desktop build/start or Pencil verification was performed.
The historical project database was not read: the historical sequence in the
cause documentation is the one supplied in ORC-22. No visual surface changed,
so there are no new design nodes or states for a Pencil verifier to compare.

Review should check the provisional model final message, persistent pending
approval, same-thread result collection, request-ID handling and interrupted
decision paths. See `docs/interactive-tool-approvals.md`, ORC-22 section.

## Review correction — current validation

2026-10-11. Inputs: HEAD `d2bb69a09f31e1bd8c61377faf178dc41fe78c8a`
plus the saved runner source/test changes. The runner patch digest from
`git diff -- crates/runner | git hash-object --stdin` is
`455ea8d46d97aebbb07e8a4f3d4573fdadd59c10`. Rust and test inputs stayed
unchanged after the successful commands below; subsequent edits only update
the cause documentation, plan and this evidence. `tools/check.ps1` retains the
previously person-approved per-package formatting fix, unchanged in this correction.

The corrected fake completes its MCP item on the background thread as soon as
the relay returns the result, before collection. The fast-result test waits for
both that stored item-completion event and the returned-result marker before
releasing the model turn. It also proves no turn completion or cell collection
has happened yet. Fast approvals and fast denials then collect the existing
result in the same invocation and attempt. Delayed approval/denial and the four
interruption cases remain covered.

The App Server session retains an approval-result collection flag through MCP
item completion and resets it when starting the continuation. Because the
protocol does not certify whether the model collected the cell, a synchronous
approval also receives one conservative continuation. The fake now remains
available after a successful model turn as App Server does, and can return its
already-collected final report without rerunning tools or editing files again.
The existing synchronous relay regression asserts the two turn request IDs,
one attempt and unchanged counts of actual tool calls.

### Fresh successful filtered tests

All exited 0: **55 tests passed**. For runner suites, the scratch ripgrep
directory `.tmp/orc-22/tools/bin` was prepended to PATH.

| Command | Result |
| --- | --- |
| `cargo test -p orchester-runner --features test-agent --test codex_tool_wait codex_tool_wait` | 9 passed |
| `cargo test -p orchester-runner --features test-agent --test orchester_tools orchester_tools` | 6 passed |
| `cargo test -p orchester-runner --features test-agent --test claude_approvals claude_approvals` | 11 passed |
| `cargo test -p orchester-runner --features test-agent --test claude_prompt_relay claude_prompt_relay` | 6 passed |
| `cargo test -p orchester-runner --features test-agent --test codex_relay codex_relay` | 5 passed |
| `cargo test -p orchester-runner --features test-agent --test approval_shutdown approval_shutdown` | 3 passed |
| `cargo test -p orchester-core --test app_server app_server` | 2 passed, 10 filtered out |
| `cargo test -p orchester-runner --features test-agent --test execution execution` | 5 passed |
| `cargo test -p orchester-runner --features test-agent --test questions questions` | 2 passed |
| `cargo test -p orchester-runner --features test-agent --test codex_escalations codex_escalations` | 4 passed |
| `cargo test -p orchester-runner --features test-agent --test codex_resume codex_resume` | 2 passed |

### Gate, diagnostics and prerequisites in this correction

| Command | Result |
| --- | --- |
| `powershell -File tools/check.ps1` | Exit 0, `check: OK`; per-package formatting, full-workspace clippy, file sizes and UI literals pass on the current Rust/test inputs |
| `cargo +nightly fmt -p orchester-runner -- --check` | Initially exit 1 on new test formatting; corrected through file tools and rerun with exit 0, also covered by the gate |
| `cargo clippy -p orchester-runner --all-targets --all-features --target aarch64-apple-darwin -- -D warnings` | Exit 1: native dependency build scripts cannot find `cc`; no runner macOS cross-clippy verdict |
| `Get-Command rg -ErrorAction SilentlyContinue \| Select-Object Source` | Exit 1: no ripgrep on the shell's original PATH |
| `cargo install ripgrep --locked --root .tmp/orc-22/tools --target-dir .tmp/orc-22/tools-target` with `CARGO_HTTP_SSL_BACKEND=openssl` | Exit 0: real ripgrep 15.2.0 already installed in this worktree's scratch folder; reused it |
| `git status --short` | Exit 0: clean starting worktree; final changed paths are this correction |
| `git rev-parse HEAD` | Exit 0: starting revision above |
| `git diff --stat` | Exit 0: correction paths inspected |
| `git diff --check` | Exit 0 |
| `git diff -- crates/runner \| git hash-object --stdin` | Exit 0: patch digest above |

### Reproduction and intermediate failures retained as context

`cargo test -p orchester-runner --features test-agent --test codex_tool_wait codex_tool_wait_result_returned_before_turn_completion_is_collected`
was run twice before the production correction. The first run exited 1 on an
ownership error in the new assertion (the approval answer had been moved).
That was repaired by recording the expected outcome before sending the answer.
The second run compiled and exited 1 after the test timed out waiting for a
successful run: it had received the provisional question instead of collecting
the cell. This reproduces the review finding with the corrected event boundary.

An intermediate `codex_tool_wait codex_tool_wait` run passed all nine tests
after the production fix but before the later shared-fake correction; the fresh
nine-test run above supersedes it. An intermediate
`cargo test -p orchester-runner --features test-agent --test orchester_tools orchester_tools`
exited 1 (5 passed / 1 failed) because the old synchronous fake exited rather
than receiving the conservative continuation. The corrected fake's fresh
six-test pass above supersedes it.

### Not run and verification handoff

No full/unfiltered test suite, signed-in Codex/Claude live run, historical
project database read, macOS runtime test, desktop build/start or Pencil
verification was performed. No visual surface changed; there are no Pencil
nodes or UI states for the verifier to compare. Runner macOS cross-clippy
remains blocked by the environment's missing C compiler.

Review should examine the independent collection flag, its reset at
continuation start, the MCP completion-before-turn-completion synchronization,
the single invocation/attempt, and the synchronous case's lack of repeated
tool calls. Earlier test/gate results above are historical; this section covers
the changed inputs.

## Second review correction

2026-10-11. A further review found that continuations were unbounded (an MCP item Codex never completes, or a
failed turn, started turn after turn) and that a decision a yielded cell asks after its turn ended failed the step as
outside the active turn. The correction continues a completed turn only while a relay call runs or waits, or when one
returned since the last continuation; never continues a failed or interrupted turn; and accepts requests for turns
that ended with Orchester's tools at work (`docs/interactive-tool-approvals.md`, ORC-22 section). Four new
`codex_tool_wait` tests (a call never completed, an abandoned call, a failed turn, a command the yielded cell asks)
failed with the production changes reverted and pass with them. Fresh filtered runs: `codex_tool_wait` (13),
`orchester_tools` (6), `codex_relay` (5), `claude_approvals` (11), `approval_shutdown` (3), `questions` (2),
`codex_escalations` (4), `command_approvals` (4), `send_now` (7), `send_now_delivery` (4), `retries` (4),
`approval_choices` (11), `auto_mode` (10), `orchester_file_tools` (4), `codex_resume` (2), and
`orchester-core --test app_server` (13). Earlier sections are historical for these inputs.

## Third review correction

2026-10-11. A re-review found that a decision a yielded cell asked could still be lost: a continuation started while
it waited, so the model could end that turn (the decision ended interrupted) or ask a second decision (the turn
failed). It also asked for a hard cap on continuations and for a requested pause to checkpoint before a continuation.
Now a waiting decision keeps the turn alive and holds the continuation until it is settled; at most three
continuations follow in a row without a decision or a call the runner waited for; a pause stops before the
continuation. The tool-wait tests are split into `codex_tool_wait` (13) and `codex_tool_wait_bounds` (4). The four
new tests (a waiting decision with a continuation that would give up, one that would ask another decision, the cap,
the pause) failed with the previous production code and pass now.
