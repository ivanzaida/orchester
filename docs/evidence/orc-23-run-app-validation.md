# ORC-23 implementation validation · 2026-10-11

Candidate: the saved, uncommitted implementation in run worktree
`01a12acf-2d86-700e-8094-befc53b4e0b5`, based on
`eae8b2668382e98cbee8318d0fc3c9269df9d7a2`. No commit or installed-app change was made.
The final results below cover the current Rust candidate; subsequent edits are documentation only.

## Implemented

The invocation relay offers `call_run_app(pid, tool, arguments)` without approval to Codex and Claude Code.
It calls `RunProcesses::app`, which reuses ORC-21's validation, before discovery or HTTP. A Windows process
handle or macOS audit identity stays alive across the requests. Discovery identifies the exact pid and app,
and requires the exact loopback MCP URL. The supported executable names are `orchester-app` and `orchester`.
For the deprecated desktop, `lurq_*` calls use its window server and other tools use its domain server.

The relay discovers the per-user file or a QA profile under this worktree's `.tmp/`. QA traversal is bounded
and cannot follow an escaping folder. The HTTP client disables proxies and redirects, uses sensitive
authorization/session headers, bounds response size and time, supports JSON and SSE, and returns when the
matching SSE event arrives even if the stream stays open. Errors never include discovery or HTTP bodies.
Text and structured result values/keys are redacted before leaving the relay. The answer cap is reapplied
after redaction. PNGs are decoded within image allocation/dimension limits and re-encoded without metadata
under `.tmp/run-app/`; image content becomes the saved relative path.

The agents' start/resume steering, approval documentation and design-verification skill use the relay
instead of asking to run a worktree Python script outside the sandbox. The manual Python helper remains.

Dependencies: reqwest, rustls, image and tempfile already exist in the workspace. The relay needs HTTP
framing/timeouts/redirect control, the same crypto-provider setup as existing HTTP clients, and bounded PNG
decoding/re-encoding. base64 0.22 was already transitive; it is now declared at workspace level for validated
MCP image decoding, rather than introducing a custom decoder for untrusted input.

## Final filtered tests

| Command | Outcome |
| --- | --- |
| `cargo test -p orchester-core --test orchester_tools call_run_app` | Exit 0: 5 passed |
| `cargo test -p orchester-core --test orchester_tools orchester_tools` | Exit 0: 41 passed |
| `cargo test -p orchester-core --test orchester_tools run_processes` | Exit 0: 6 passed |
| `cargo test -p orchester-core --test run working_prompt` | Exit 0: 1 passed |
| `cargo test -p orchester-runner --features test-agent --test orchester_app_tools call_run_app` | Exit 0: 2 passed |
| `cargo test -p orchester-runner --features test-agent --test orchester_process_tools run_processes` | Exit 0: 2 passed |
| `cargo test -p orchester-exec --test run_processes run_processes` | Exit 0: 9 passed, 1 intentionally ignored stand-in entry point |

The new runner tests start copied stand-ins and drive them through the actual per-invocation relays
with read-only access, network off and MCP approvals enabled. They cover navigation/structured content,
an SSE PNG with the connection left open, token echo in successful/error results and structured keys,
invalid PNGs, refused redirects, outside/protected processes, an installed-app stand-in, another run's
stand-in, and mismatched pid/app/non-loopback discovery. Refused targets receive no tool call.
All persisted activity event variants are checked for the stand-in credential. Every started stand-in
is cleaned up using its own child handle. The new test project and QA artifacts live under worktree `.tmp/`.

## Build and quality commands

| Command | Outcome |
| --- | --- |
| `cargo check -p orchester-core -p orchester-exec -p orchester-runner` | Exit 0 during initial implementation; final Windows compilation is covered by clippy below |
| `cargo clippy -p orchester-core -p orchester-exec -p orchester-runner --all-targets --all-features -- -D warnings` | Final exit 0 |
| `cargo check -p orchester-core -p orchester-exec --target aarch64-apple-darwin` | Final exit 0 |
| `cargo clippy -p orchester-core -p orchester-exec --all-targets --all-features --target aarch64-apple-darwin -- -D warnings` | Final exit 0 |
| `cargo check -p orchester-core -p orchester-exec -p orchester-runner --target aarch64-apple-darwin` | Final exit 1: existing native dependency build scripts cannot find target C compiler `cc` (ring, SQLite, zstd and tree-sitter); no runner macOS compilation verdict |
| `cargo fmt --all` | Exit 1: Windows command length limit (OS error 206); replaced with scoped nightly formatting |
| `cargo +nightly fmt --manifest-path crates/core/Cargo.toml` | Exit 0 |
| `cargo +nightly fmt --manifest-path crates/exec/Cargo.toml` | Exit 0 |
| `cargo +nightly fmt --manifest-path crates/runner/Cargo.toml` | Exit 0 |
| `cargo +nightly fmt --manifest-path crates/core/Cargo.toml -- --check` | Exit 0 |
| `cargo +nightly fmt --manifest-path crates/exec/Cargo.toml -- --check` | Exit 0 |
| `cargo +nightly fmt --manifest-path crates/runner/Cargo.toml -- --check` | Exit 0 |
| `git diff --check` | Exit 0 |
| `rustup target list --installed` | Exit 0: aarch64-apple-darwin is installed |
| `git status --short` | Exit 0: changes are uncommitted |
| `git diff --stat` | Exit 0 |
| `git rev-parse HEAD` | Exit 0: base revision named above |
| `git diff -- crates/core/src/orchester_tools/mod.rs crates/core/src/orchester_tools/policy.rs .claude/skills/design-verification/SKILL.md` | Exit 0: inspected |

Earlier development failures are preserved as history, not verdicts on the current candidate:
the first runner relay test attempt exposed missing rustls-provider initialization (fixed using the
existing HTTP-client pattern); a subsequent SSE fixture attempt had escaped HTTP separators (fixed).
The initial clippy follow-up reported a needless borrow and an oversized test helper (fixed/split).
The same commands were rerun and their final passing results are listed above. The macOS combined
check was retried on the final candidate and still stops in native dependencies.

## Not run and handoff

- `powershell -File tools/check.ps1` was deliberately not run: the Implement instructions assign the
  authoritative full gate to the following Check step. Formatting and clippy above cover only changed crates.
- No unfiltered cargo test, complete workspace test suite, live signed-in CLI run, real GUI app QA,
  macOS runtime QA, install, commit, global input or whole-screen capture was performed.
- No visual implementation changed and this task names no Pencil nodes. There are no design frames
  for the verifier to compare for this implementation.
- Functional QA should start its own copied app, then call `app_state`/`lurq_windows`, `navigate`,
  `lurq_screenshot` and an interaction through a relay built with this candidate; inspect the returned PNG
  and verify that no approval or discovery credential reaches the agent/activity. Refuse an outside pid
  and mismatched discovery using stand-ins, never by driving the person's install.
- The running invocation's relay is not hot-reloaded by source edits. A fresh relay built from this
  candidate is needed to expose the new tool in a live QA invocation. No installed runner was modified.
