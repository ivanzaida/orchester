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

## Orchester Platform (H:/projects/orchester-cloud, branch `module/team-configuration`, nothing started)

The team server and web halves, in the Orchester Platform project's module "Team configuration". Each server task
waits for its contract in this repository to be merged into this branch, pushed, and the pin moved (OP-197).

| Task | What | Depends on |
| --- | --- | --- |
| OP-197 | Move the orchester pin (`orchester-api`, `orchester-core` git rev in Cargo.toml) to the contract commit | ORC-810 (done), ORC-811, ORC-817, ORC-820, ORC-821 merged and pushed |
| OP-198 | Server: MCP server as a team resource kind (migration widening `resources.kind`, body checks, admin-only writes, archive stops runs, bundles carry it) | OP-197 |
| OP-199 | Server: references by server id and secret name (`body.rs`, `rows.rs`, `version_references` migration, bundles without foreign ids) | OP-197 |
| OP-200 | Server: migrate existing versions (connection → CLI kind; MCP reference → team MCP server, deduplicated by name; secret → name; pinned runs keep their snapshot) | OP-198, OP-199 |
| OP-201 | Server: `GET …/resources/{id}/used-by` (Used by and runs that stop) | OP-197 (contract ORC-811) |
| OP-202 | Server: catalog keeps only CLI kinds and models per role | OP-199 |
| OP-203 | Web design in `web/design/orchester-web.pen`: team MCP servers, pickers by team resource, create with Create/Cancel, archive with Used by | — |
| OP-204 | Web: team MCP servers list, page and editor | OP-198, OP-203 |
| OP-205 | Web: editors pick team resources by id, CLI kind and secret names | OP-199, OP-202, OP-203 |
| OP-206 | Web: create publishes v1; archive confirmation with Used by | OP-201, OP-203 |

Never deploy or touch orc.lurq.dev; the person deploys.

## Pre-existing failures (not this module)

i18n tests on master: `claims.refused.fenced` has no source, `project_settings.memory` repeated, `stepCannotWrite`
duplicated in `crates/workspace/assets/locales/en.json`.
