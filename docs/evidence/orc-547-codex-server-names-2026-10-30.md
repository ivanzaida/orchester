# ORC-547 · A chosen MCP server named like one of the user's own Codex servers (2026-10-30)

What Codex does when the user's own Codex configuration has an MCP server named like a server a step chooses, and
how Orchester gives the step's server to Codex instead of refusing the step. Checked on Windows 11 with **Codex CLI
0.159.2** (npm `C:/nvm4w/nodejs/codex.cmd`), a scratch `CODEX_HOME` and a stand-in for the Responses API on
`127.0.0.1` (Codex's `model_providers.mock`, `wire_api = "responses"`), so no account and no token were used.

The probe scripts lived in the run's worktree under `.tmp/codex-collision/` (`probe.py`, `server.py`, a minimal MCP
stdio server whose tool `whoami` answers with its role and the variables it got and which writes a marker file when it
starts, and `mock_model.py`, which answers the first request with a call of that tool and the next with a message).

## Under the same name (before)

User configuration `[mcp_servers.pencil]` with `command`, `args = [server.py, "user", "user-arg"]` and
`env = { USER_ONLY = "from-user" }`; the step's server given as `-c mcp_servers.pencil.command=… .args=[server.py,
"project"] .env.PROJECT_ONLY="from-project"`.

- `config/read`: the effective `mcp_servers.pencil` has the arguments' `command` and `args`, and **both** `env`
  entries: `{"PROJECT_ONLY": "from-project", "USER_ONLY": "from-user"}`. The server started with `USER_ONLY=from-user`
  (marker file).
- With the user's `pencil` an HTTP server (`url = "http://127.0.0.1:9/mcp"`), `config/read` itself fails:
  `invalid configuration: url is not supported for stdio in mcp_servers.pencil` (code -32603). The arguments are fixed
  when App Server starts, so a name must be chosen before then.

## Under another name (after)

The step's server given as `-c mcp_servers.orchester-pencil.command=… .args=… .env.PROJECT_ONLY=…
.tools={"whoami"={approval_mode="approve"}}`, the thread started with `config = {"mcp_servers.pencil.enabled": false}`.
Both user transports (`command` and `url`) behaved the same:

- `config/read` lists `orchester-pencil` with the project's settings only and `pencil` with the user's, unmerged.
- `mcpServerStatus/list`: `orchester-pencil connected` (tool `whoami`), `pencil disabled`. No marker of the user's
  server: it never started.
- The model got the tool namespace `mcp__orchester_pencil`; its call ran without an approval request (the approval
  override under the new name works) and the `mcpToolCall` item names `server: "orchester-pencil"`. The tool answered
  `{"role": "project", "argv": ["project"], "USER_ONLY": null, "PROJECT_ONLY": "from-project"}`.
- The user's `config.toml` was not written to.

## The names read before Codex starts

App Server answers `initialize`, `initialized` and `config/read` in about 0.15 s from start; it stops at the end of its
input without answering, so the input stays open until the answer is in (as for the plan limits read). The runner
starts it on the step's account, without the step's servers and without its API provider, reads the keys of
`config.mcp_servers` (every loaded layer: the user's, the system's) and gives each chosen server whose name is among
them the name `orchester-<name>` (`-2`, `-3`, … while taken). The step's own `config/read` still refuses a name that is
taken by then (the configuration changed in between) and Orchester's own `Orchester`, which keeps its name.

Live test of that read against the real CLI:

```text
ORCHESTER_LIVE_CODEX=C:/nvm4w/nodejs/codex.cmd cargo test -p orchester-runner --features test-agent --test mcp mcp_live_codex -- --ignored
```

Fake-CLI tests: `cargo test -p orchester-runner --features test-agent --test mcp mcp` (the step runs with
`orchester-pencil`, the user's `pencil` off, the exemption, the approval and the activity by `pencil`, the prompt
naming it) and `cargo test -p orchester-core --test mcp codex_names`.

## Not changed

- A model provider of the user's own named `orchester` (`refuse_taken_provider`) and a user server whose name has a
  dot (`McpInventoryError::DottedName`) still refuse the step and say to rename it.
- The Orchestrator chat's Codex keeps its one server, `orchester`, under its own name.
