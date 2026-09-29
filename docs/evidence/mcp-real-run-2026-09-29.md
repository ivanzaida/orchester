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
  zero API tokens and no model tool call. Redacted output was preserved as
  `.tmp/m1/claude-auth-failed.stdout.redacted.jsonl` before the signed-in replay.

The probes exercised the real CLIs with model turns where authorization allowed them. They did not start a full
Orchester pipeline run. The fixture files contain only observed CLI events with Boolean or inert marker results;
test secret values never enter prompts, fixture text or MCP output.

## Verification

- `cargo test -p orchester-core --test mcp mcp_codex_signed_in`: 2 passed.
- `powershell -File tools/check.ps1`: passed (rustfmt, clippy, file size and UI literal gate).
- No full test suite or macOS runtime was run.

At this point M1 remained open because Claude's `mcp__<name>` allow rule had not been exercised in a signed-in model
turn. The follow-up below closes that evidence gap.

## Claude authentication follow-up · 2026-09-29

In the isolated `codex/m1-claude-mcp` worktree, Claude Code 2.1.89 `claude auth status --json` returned exit code 1
and `loggedIn: false`. The raw authentication output was neither printed nor saved; only sanitized status fields were
surfaced. No model turn was attempted without a signed-in session. The `mcp__<name>` allow rule was unverified then.

## Claude signed-in allow-rule follow-up · 2026-09-29

The user signed in on the same host. `claude auth status --json` then returned exit code 0 and `loggedIn: true`;
only those sanitized fields were printed. Claude Code 2.1.89 ran a model turn in the disposable `.tmp/m1/project`
with `--strict-mcp-config`, the one-server `m1probe` config, `--tools ""`, `--permission-mode dontAsk` and
`--allowedTools mcp__m1probe`. The `init` event listed `mcp__m1probe__ping` and a connected `m1probe` server.
The assistant issued a real `tool_use` for `mcp__m1probe__ping`; its `tool_result` contained
`M1_TOOL_OK; server_secret_present=false`. The final result reported `subtype: success`, `is_error: false` and zero
permission denials. The process exited 0 in 8.9 seconds. Thus the server-name allow rule permitted this MCP call
in a signed-in model turn. This probe did not test a full Orchester pipeline or a server with a real secret.

The raw CLI stream stayed in ignored `.tmp/m1/claude.stdout.redacted.jsonl`; the prior signed-out stream was
preserved separately. `crates/core/tests/mcp/fixtures/claude-signed-in-mcp.jsonl` is a trimmed observed fixture
with synthetic IDs and only the inert marker, tool name, connected status and successful result. The parser test
checks the session, call and completed MCP result. No authentication token or secret value was printed or saved.

The implementing worktree's named `mcp_claude_signed_in_allow_rule_permits_the_probe_call` test passed 1/1 and
`powershell -File tools/check.ps1` returned `check: OK`. A separate agent applied all three changed files to a
detached worktree and SHA-256 matched them byte-for-byte, then repeated the named test 1/1 and `check.ps1` OK in an
exclusive target directory. The independent agent also compared the trimmed fixture against the already redacted
actual stream, including correlated tool IDs, exact inert result, model answer, success flag and zero permission
denials. Its report is `.tmp/verify/m1-claude-allow-rule/report.md`. No full Orchester pipeline, full test suite or
macOS runtime was run in this follow-up.
