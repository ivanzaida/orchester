# Team configuration: where the work stands (2026-11-07)

Temporary note for continuing on another computer; delete it before `module/team-configuration` goes to master.

Model and decisions: `docs/design/team-configuration.md`. Design: `crates/desktop-app/design/orchester.pen`, layer
`acFkz` "Agents · v2" (committed on master in 3f9f05f39). Tasks: Orchester module "Team configuration" (ORC-809…848)
and Orchester Platform module "Team configuration" (OP-197…206). The team server refuses status changes over MCP
(`decision_required`, D4), so every task still shows Backlog there; this file is the real state.

## Done

| Task | What | Where |
| --- | --- | --- |
| ORC-809 | Design A23 `Pnpbe` (archive confirmation with Used by) and L11 `dS97b` (local twin) | orchester.pen |
| ORC-810 | `SharedResourceKind::McpServer` and core `TeamMcpServer`; docs; fake server | merged here (ff87b95c4) |
| ORC-812 | Each profile gets only the MCP servers its version names | merged here (fe2a78a6f) |
| ORC-813 | `mcp_server_programs` in the application database | merged here (20dbd8c17) |
| ORC-814 | Leaving a team deletes the team's resources and cached versions | merged here (dae4066c9); **UI text changed: design verification not done** |
| ORC-815 | Per-project fill-ins: connection pick, MCP on here, secret values by name | merged here (d492b65a4) |

## In progress

- **ORC-816** (create = v1, no draft): WIP on `origin/claude/orc-816-create-v1` (30e05f981), cut from this
  branch before the merges above. Its commit body lists what is done and left: tests not rerun, other locales than
  `en.json` not checked, `check.ps1` not run, UI does not call `Servers::send_created` yet.

## Next (unblocked once this branch is pushed)

- ORC-811 (used-by contract), ORC-817 (references by server id / secret name), ORC-818 (sync team MCP servers).
- Then ORC-819…823, and the Orchester Platform pin (OP-197) once the contract tasks are pushed.

## Pre-existing failures (not this module)

i18n tests on master: `claims.refused.fenced` has no source, `project_settings.memory` repeated, `stepCannotWrite`
duplicated in `crates/workspace/assets/locales/en.json`.
