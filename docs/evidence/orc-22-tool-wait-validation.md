# ORC-22 implementation validation

2026-10-11. Saved candidate: branch `orchester/orc-22-run-1`, based on
`c263258d145eb22c167bda6a702e6e2cf35a6615`, with the uncommitted ORC-22
implementation, fake-cell harness and regression tests. No commit was made.
The latest results below cover the current Rust changes, including the
deterministic result-before-turn-completion synchronization, distinct
continuation request IDs, and the added core protocol regression. Subsequent
Rust changes invalidate these results.

## Reproduction and behavior

Before the production fix, the fake started a real outside search on its relay,
held the approval, then emitted a completed model turn with a pending-cell final
message. The approval vanished when the runner ended the process tree.
`cargo test -p orchester-runner --features test-agent --test codex_tool_wait codex_tool_wait_approval`
failed at that assertion. This is pre-fix evidence, not the current verdict.

The eight current runner tests cover approval and denial three seconds after
the model turn completes, the result returning before turn completion, a
delayed previous turn-start reply, pause, cancel, runner loss, and graceful
shutdown. Approved and denied calls finish in one invocation and one attempt.
Stopped decisions never produce a failed attempt, never execute the outside
read and refuse stale approval answers. The continuation retains the original
model, effort, environment and output schema.

## Current filtered tests

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

## Quality checks and environment limits

| Command | Current result |
| --- | --- |
| `powershell -File tools/check.ps1` | Exit 1: Windows error 206 from `cargo +nightly fmt --all -- --check`. Workspace clippy, source size and UI literal checks pass. |
| `cargo +nightly fmt --all -- --check` | Windows error 206: command line too long in this worktree. A short drive alias canonicalized back to the long path and did not repair it. |
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

The gate remains unchanged. Approval was requested to make it perform those
same checks per package, because AGENTS.md forbids changes outside the requested
scope without approval. ORC-22 is not marked done while check.ps1 fails.

Scratch prerequisite commands: `cargo install ripgrep --locked --root .tmp/orc-22/tools --target-dir .tmp/orc-22/tools-target`
initially failed with schannel NoCredentials; setting
`$env:CARGO_HTTP_SSL_BACKEND = 'openssl'` and rerunning that command succeeded.
The successful file-tool test prepended
`(Join-Path $PWD '.tmp/orc-22/tools/bin') + ';' + $env:PATH` to PATH.

## Not run

No unfiltered or full-workspace test suite, signed-in Codex/Claude live run,
macOS runtime test, desktop build/start or Pencil verification was performed.
The historical project database was not read: the historical sequence in the
cause documentation is the one supplied in ORC-22. No visual surface changed,
so there are no new design nodes or states for a Pencil verifier to compare.

Review should check the provisional model final message, persistent pending
approval, same-thread result collection, request-ID handling and interrupted
decision paths. See `docs/interactive-tool-approvals.md`, ORC-22 section.
