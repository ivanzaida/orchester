# M1 · MCP behaviour in signed-in CLI runs

Date: 2026-09-29 (CLI log timestamps are UTC on 2026-09-28). Worktree: `codex/mcp-real-run`.
The throwaway project, MCP server and probe scripts are under the ignored repository directory `.tmp/m1/`.
No installed Orchester build, existing project database or stored user secret was used.

## Method and observed output

- Codex CLI 0.139.0 reported `Logged in using ChatGPT`. Each probe used `codex -a never exec --json --ephemeral
  --ignore-user-config --ignore-rules --skip-git-repo-check --sandbox read-only` in `.tmp/m1/project`, with only the
  named test MCP server configured through `-c`. A random test value existed only in the spawned Codex process
  environment. The server's `env_vars` named it, and `shell_environment_policy.exclude` excluded it from agent
  commands. Neither the value nor a hash of it was printed or saved.
- A model turn called `m1probe.ping`. Its completed `mcp_tool_call` result was
  `M1_TOOL_OK; server_secret_present=true`. The same turn's completed PowerShell environment existence check printed
  `False`. The CLI JSONL lines are in `crates/core/tests/mcp/fixtures/codex-signed-in-mcp.jsonl`; full redacted stdout
  is in `.tmp/m1/good.stdout.redacted.jsonl`. An earlier shell command form was declined by policy; the completed
  `Test-Path` command is the evidence for exclusion.
- The first call of an unannotated `ping` returned `user cancelled MCP tool call`. That initial raw capture was
  overwritten by a rerun; the exact event copied from the command result is saved in
  `.tmp/m1/initial-unannotated-cancel-observed.jsonl`. Two further unannotated model turns returned the same
  `M1_TOOL_OK; server_secret_present=true` marker successfully, including one under a fresh server name. Their
  redacted stdout files are `.tmp/m1/unannotated-repeat-1.stdout.redacted.jsonl` and
  `.tmp/m1/unannotated-repeat-2.stdout.redacted.jsonl`; the completed events are also in
  `crates/core/tests/mcp/fixtures/codex-signed-in-unannotated.jsonl`. The cancellation is not a reproducible rule
  from this evidence.
- A configured `m1broken` server pointed to a nonexistent program. The signed-in turn completed successfully and
  stdout contained no MCP error event. During shutdown, stderr reported `MCP startup failed: The system cannot find
  the file specified. (os error 2)` without naming the server. The exact observed warning and completion event are
  `crates/core/tests/mcp/fixtures/codex-signed-in-startup.stderr` and
  `crates/core/tests/mcp/fixtures/codex-signed-in-startup.jsonl`. The existing parser maps the warning to
  `McpServerProblem::FailedToStart` with `server: None`; the new fixture test checks that mapping.
- Claude Code 2.1.89 reported `loggedIn: false`. With `--strict-mcp-config --mcp-config`, `--allowedTools
  mcp__m1probe`, `--tools ""` and `--permission-mode dontAsk`, its `init` event listed `mcp__m1probe__ping` and
  `m1probe` as connected. It then returned `authentication_failed` and `Not logged in · Please run /login`, with
  zero API tokens and no model tool call. Redacted output is `.tmp/m1/claude.stdout.redacted.jsonl`. The allow rule's
  execution behaviour remains unverified until Claude is signed in.

The probes exercised the real CLIs with model turns where authorization allowed them. They did not start a full
Orchester pipeline run. The fixture files contain only observed CLI events with Boolean or inert marker results;
test secret values never enter prompts, fixture text or MCP output.

## Verification

- `cargo test -p orchester-core --test mcp mcp_codex_signed_in`: 2 passed.
- `powershell -File tools/check.ps1`: passed (rustfmt, clippy, file size and UI literal gate).
- No full test suite or macOS runtime was run.

M1 remains open because Claude's `mcp__<name>` allow rule has not been exercised in a signed-in model turn.

## Claude authentication follow-up · 2026-09-29

In the isolated `codex/m1-claude-mcp` worktree, Claude Code 2.1.89 `claude auth status --json` returned exit code 1
and `loggedIn: false`. The raw authentication output was neither printed nor saved; only sanitized status fields were
surfaced. No model turn was attempted without a signed-in session. The `mcp__<name>` allow rule remains unverified,
and M1 stays in progress.
