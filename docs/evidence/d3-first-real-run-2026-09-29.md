# D3: first Orchester runs on a disposable Orchester checkout (2026-09-29)

## Isolation and setup

- The project repository was a local, disposable clone at `.tmp/d3/project`, based on `master` commit `57cee83`. Orchester created its run worktrees and `orchester/*` branches inside this clone. The root checkout, user project database and stable installation were not used.
- The desktop and runner ran from `.tmp/d3/app`, with their database beside those binaries. They were built from the separate `codex/d3-first-run` worktree with `--features mcp` and a temporary local lurq no-focus patch. This build passed. MCP reported the app window at 2160×1530 with `focused:false`; the local runner was online.
- The Codex connection used `C:\nvm4w\nodejs\codex.cmd`, CLI 0.139.0, and verified a signed-in ChatGPT session. Published profiles were `D3 Implement` (repository write, one extra writable folder) and `D3 Review` (read-only). The extra folder and project `CARGO_TARGET_DIR` were both `H:/projects/orchester/.tmp/targets/d3`; `CARGO_INCREMENTAL=0` was added before run 2.
- The two-step `D3 first real run` pipeline was Implement → Review. Nothing was pushed, merged or installed; run 2's local commit is described below.

## Run 1: permission and command-policy probe

- The first pipeline version accidentally narrowed the Implement step to `read_only`, despite its published profile allowing `write`. The published pipeline body has `kind.agent.repository=read_only`; the run UI showed `D3 Implement · Read-only`. This was setup behavior, not a mismatch between the saved profile and runner.
- Run 1 created a real branch `orchester/dos-1-run-1` and a separate worktree under the disposable clone. Implement ran once with `gpt-5.5` for 88.5 seconds. The CLI reported 240,154 input tokens, 217,344 cached input tokens and 2,216 output tokens; Orchester billed 25,026 tokens. Review was skipped after Implement reported failure.
- The agent was instructed to check free space before any cargo command. Under the Codex CLI `-a never` policy, attempts using `Get-PSDrive`, `fsutil volume diskfree H:` and `Get-Volume H` were declined as “not an allowed command” (exit code -1). Simpler `Get-Location` and `Get-ChildItem` commands completed. The agent also reported that creating its evidence note was rejected by the read-only sandbox. It correctly did not claim to run cargo because the required disk gate could not be verified.
- A filtered metadata and outcome snapshot is preserved in ignored `.tmp/d3/run1-snapshot.json`. It contains the published permission bodies, pipeline step access, run status and usage, command statuses and the agent's final outcome. No raw prompt, secret or CLI auth material was copied.

## Run 2: writable Implement and cargo probe

Pipeline v2 explicitly set Implement step access to `write` and left Review read-only. The project environment gained `CARGO_INCREMENTAL=0`. An external host check found 24.05 GiB free on H: before the new run; the agent was asked to attempt named cargo checks without a CLI disk-space probe. The run UI confirmed `D3 Implement · Edit files + 1 folder · Any command · Network on` at start. The project target remained in the configured extra writable folder.

While Implement was working, the Codex CLI process spawned by this run's runner had `-a never`, `--sandbox workspace-write`, `windows.sandbox=elevated`, `sandbox_workspace_write.writable_roots`, `sandbox_workspace_write.network_access=true`, `--ignore-user-config` and `--ignore-rules` in its command line. These flags were checked against the exact child PID without printing the command line or any prompt.

| Command inside the Codex sandbox | Observed result | Elapsed |
| --- | --- | ---: |
| `cargo check -p orchester-core` | exit 0 | 8.96 s |
| `cargo test -p orchester-runner --features test-agent --test runner runner_answers_ping_and_stops_at_checkpoints` | exit 0; 1 passed, 9 filtered out | 39.41 s |
| `cargo test -p orchester-runner --features test-agent --test runner the_desktop_refuses_a_pipe_served_by_another_process` | exit 0; 1 passed, 9 filtered out | 0.30 s |
| `cargo build -p orchester-runner -p orchester-desktop --features mcp` | exit 0 | 50.46 s |
| `powershell -File tools/check.ps1` | exit 0; `check: OK` | 54.60 s |

The agent wrote `docs/evidence/d3-agent-probe-2.md` in its run worktree with these outcomes. Orchester then **automatically committed** that file on the disposable run branch as `5529d2a DOS-2: Implement (run #2)`. The agent did not request a commit, but its statement that no commit occurred was inaccurate about the final orchestrated state. Nothing was pushed, merged or installed.

Implement passed after one 290.1-second invocation (230,975 input, 200,832 cached input, 3,631 output; 33,774 billable tokens). The separate read-only Review passed after one 74.8-second invocation (106,283 input, 81,152 cached input, 2,657 output; 27,788 billable tokens). It checked the evidence file and clean run worktree, explicitly noted the automatic commit, and did not rerun cargo. The run finished `succeeded` / *Ready for your review* with 61,562 billable tokens. A filtered outcome snapshot is preserved in ignored `.tmp/d3/run2-snapshot.json`; H: had about 22.65 GiB free after the build.

## Coverage limit

The disposable project at `57cee83` uses lurq 0.30.2 and lacks the temporary L6 no-focus startup patch used by the isolated dev app. Agents were therefore told to build but not launch that desktop binary: launching it could focus the user's window. Reaching the embedded MCP server from a desktop started **inside** a run remains unverified.
