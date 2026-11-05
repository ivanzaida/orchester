# ORC-710: the Orchestrator's server named apart from a user's `orchester` in Codex (2026-11-04)

**Question.** When the person's Codex configuration has `[mcp_servers.orchester]` (url form, as Settings › Automation ›
Connect an agent writes it), does Codex App Server keep the Orchestrator chat's server, given as
`-c mcp_servers.orchester-orchester.url=…` (`CodexMcp::named_apart`), apart from it, start the thread with the user's
`orchester` turned off by its dotted key, and run the chat's server?

**Setup.** Codex CLI 0.166.0 (`@openai/codex`, run as `node …/codex.js app-server --stdio`), Windows 11. A scratch
`CODEX_HOME` (`.tmp/orc-710/codex-home`), never the person's `~/.codex`, whose `config.toml` is only:

```toml
[mcp_servers.orchester]
url = "http://127.0.0.1:9/mcp"
```

The chat's server: a stand-in streamable-HTTP MCP server on loopback (`initialize`, `tools/list` with one tool),
given as `-c mcp_servers.orchester-orchester.url="http://127.0.0.1:<port>/mcp"`. A mock model provider
(`model_providers.mock`, `base_url` on port 9) so that no sign-in is needed; **no model turn was run**. Probe:
`.tmp/orc-710/probe.py` (in the run's worktree): `initialize`, `initialized`, `config/read` with layers, `thread/start`
with `config = {"mcp_servers.orchester.enabled": false}` (what the runner's `thread_config` sends), then the thread's
startup notifications.

**Seen.**

- `config/read`: `mcp_servers` has both, unmerged: `orchester` with the user's `url` (port 9) only, and
  `orchester-orchester` with the arguments' `url` only. Layers: `sessionFlags` lists `orchester-orchester`, `user`
  lists `orchester`.
- `thread/start` succeeded.
- `mcpServer/startupStatus/updated`: `orchester-orchester` went `starting`, then `ready`. No status at all for the
  user's `orchester`: Codex never started it. The stand-in got `initialize`, `notifications/initialized`,
  `tools/list`, `resources/list` and `resources/templates/list`.
- The scratch `config.toml` was byte for byte the same afterwards.
- `mcpServerStatus/list` (targeted, `serverName = "orchester-orchester"`) got no answer within 60 seconds. The likely
  cause is the stand-in: it answers `resources/list` with an empty `202` instead of a JSON-RPC result. The runner's
  readiness check against the real chat server is covered by `crates/runner/tests/chat_codex/readiness.rs` against
  the fake App Server, not by this probe.

**Not checked live.** A model turn that calls a tool under `orchester-orchester` and Codex's approval request for it
(`mcpServer/elicitation/request` naming `orchester-orchester`, which the runner's session names `orchester` again).
These are covered against `orchester-fake-codex` by
`chat_codex_names_its_server_apart_from_the_users_orchester_and_still_approves_its_tools`
(`cargo test -p orchester-runner --features test-agent --test chat_codex chat_codex`).
