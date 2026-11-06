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
| ORC-811 | `GET …/resources/{id}/used-by`: what names a resource and the runs that stop; client, stand-in, docs | merged here (59d9fcc25) |

## In progress

- **ORC-816** (create = v1, no draft): WIP on `origin/claude/orc-816-create-v1` (30e05f981), cut from this
  branch before the merges above. Its commit body lists what is done and left: tests not rerun, other locales than
  `en.json` not checked, `check.ps1` not run, UI does not call `Servers::send_created` yet.

## Next (unblocked once this branch is pushed)

- ORC-817 (references by server id / secret name), ORC-818 (sync team MCP servers).
- Then ORC-819…823, and the Orchester Platform pin (OP-197) once the contract tasks are pushed.

## Orchester Platform tasks (H:/projects/orchester-cloud, branch `module/team-configuration`)

The team server and web halves. Nothing is started; the branch equals master (7c35aa0) and is pushed. Each server
task waits for its contract task in this repository to be merged into `module/team-configuration`, pushed, and the
pin moved (OP-197). Never deploy or touch orc.lurq.dev; the person deploys. Model: `docs/design/team-configuration.md`
in this repository.

### OP-197 · Move the orchester pin to the team configuration contract (high)

Depends on: ORC-810 (done, merged), ORC-811, ORC-817, ORC-820, ORC-821 merged into this repository's
`module/team-configuration` and pushed.

Move the git rev of `orchester-api` and `orchester-core` in the platform's `Cargo.toml` to that commit and run
`cargo update -p orchester-api -p orchester-core` (platform `README.md` "The shared crates"). Fix compile errors only
as far as needed to build; behaviour changes are the other tasks. For local work before a push,
`.cargo/config.example.toml`'s `[patch]` to `../orchester/crates` may be used, never committed.

Done when: the pin points at a pushed commit containing the contract; the workspace builds; nothing else changed.

### OP-198 · Server: MCP server as a team resource kind (high)

Depends on: OP-197.

The team server stores MCP servers as team resources: a forward migration in `crates/server/src/db/migrate.rs` widens
the `resources.kind` CHECK (`resources.sql`) with `mcp_server`; `crates/server/src/sharing/resources/body.rs` checks the
body with core's rules (no path, no values, no local ids); create, publish, rename, archive and restore work as for
other kinds, admin-only (`sharing/access.rs`); deltas in `sharing/resources/stream.rs`; archive stops runs that pin a
version (server.md row 79) for this kind too; bundles (`sharing/bundles.rs`, `bundles/generate.rs`) carry the MCP
server versions a profile names. Tests in `crates/server/tests/sharing/` (split files over 600 lines first).

Done when: the `mcp_server` kind is created, published, archived and restored by admins only; bodies with paths,
values or local ids are refused; bundles include the named MCP server versions; server tests pass.

### OP-199 · Server: references by server id and secret name (high)

Depends on: OP-197.

A team resource's version names other team resources only by server id and secrets only by name; a profile names
its CLI kind, not a connection. Change `crates/server/src/sharing/resources/body.rs` (every named id must be an
existing, not archived team resource of the right kind in the same project), `rows.rs` and the `version_references`
table (forward migration: kind of target, server id or secret name), `wire.rs`, and the bundles (`generate.rs`: no
`LocalBinding` of foreign ids). Refuse a body that carries a local connection id, MCP server id or secret id. Tests in
`crates/server/tests/sharing/`.

Done when: references are stored as server ids and secret names; bodies naming unknown, archived or foreign ids are
refused; bundles carry no foreign ids; server tests pass.

### OP-200 · Server: migrate existing team versions to the new references (high)

Depends on: OP-198, OP-199.

A one-time forward migration rewrites each existing version's references: a connection reference becomes the
profile's CLI kind; an MCP server reference becomes a team MCP server created from the publisher's local definition
as the version recorded it (without its program path), deduplicated by name within the project; a secret reference
becomes its name. Versions are immutable: write a rewritten body as a new version only where the spec requires,
otherwise rewrite in the migration with a record of what changed; runs pinned to old versions keep their snapshot.
Test on a database built in the test (never orc.lurq.dev's). Report the migration's plan for orc.lurq.dev; deploying is
the person's.

Done when: the migration rewrites connection, MCP and secret references as specified; duplicate MCP servers by name
merge into one team MCP server; pinned runs keep their snapshot; tested on a fixture database.

### OP-201 · Server: used-by route for a team resource (medium)

Depends on: OP-197 (contract ORC-811).

Implement `GET …/resources/{id}/used-by` as the contract says: the latest published versions of other team resources
that name it, and the runs that have not ended and pin any version of it (task key, run, claim holder). Member read.
`crates/server/src/http/resources.rs`, `sharing/resources/*`, and the web route (`http/web_resources.rs`). Tests in
`crates/server/tests/sharing/`.

Done when: the route answers Used by and the running pins; members can read it; server tests pass.

### OP-202 · Server: catalog keeps only CLI kinds and models (low)

Depends on: OP-199.

Row 75's references catalog (`crates/server/src/sharing/catalog.rs`, `db/catalog.sql`, `http/catalog.rs`) collected
members' connections and MCP servers by kind, name and role so the web editor could name them. The web editor now
picks MCP servers, tools and skills from the team's own resources, and CLI kinds and models from the catalog. Keep
only CLI kinds and models per role; drop the members' connection and MCP server entries (forward migration). Tests in
`crates/server/tests/sharing/catalog.rs`.

Done when: the catalog reports only CLI kinds and models per role; the migration drops the members' connection and
MCP entries; server tests pass.

### OP-203 · Web design: team MCP servers, pickers by team resource, archive with Used by (medium)

Depends on: nothing; can start now.

Draw in `web/design/orchester-web.pen` (through the pencil MCP with the main checkout path
`H:/projects/orchester-cloud/web/design/orchester-web.pen`; the first execute lists the top-level frames and stops if
they are not the web boards; Insert-made nodes do not render, use Replace with the full tree or Copy): the MCP servers
list and page for admins and members; the create page with only Create and Cancel (v1 at once); the profile editor's
pickers for MCP servers, tools, skills and instructions from the team's resources, CLI kind and model from the
catalog, secrets by name; the archive confirmation with Used by and the runs that stop. Put the frames in the Team
server board's Agents area, next to the agents pages. The desktop design to follow is layer `acFkz` "Agents · v2" in
this repository's `crates/desktop-app/design/orchester.pen` (A23 `Pnpbe` is the archive confirmation). The person
commits the `.pen` file.

Done when: frames for the MCP servers list and page, create, pickers and the archive confirmation exist and render;
admin and member variants; nothing outside the Team server agents area changed.

### OP-204 · Web: team MCP servers list, page and editor (medium)

Depends on: OP-198, OP-203.

`web/server`: the MCP servers list and page in the project's Agents area (`app/[slug]/agents` and the resource
pages), with create (only Create and Cancel, v1 at once), edit → Publish vN, rename and archive for admins; members
read. `lib/team/library.ts` gets the `mcp_server` kind; split `lib/team/resource.ts` / `resource-edit.ts` if they grow
over 600 lines. Match the frames drawn in OP-203.

Done when: admins create, edit, publish and archive team MCP servers in the web app; members read only; it matches
the web design frames.

### OP-205 · Web: editors pick team resources by id, CLI kind and secret names (medium)

Depends on: OP-199, OP-202, OP-203.

`web/server` profile and pipeline editors: MCP servers, tools, skills and instructions are picked from the team's own
resources by server id; the CLI kind and model from the catalog; secrets by name (typed, or picked from names already
used). Remove the members' connection and MCP server pickers (`components/team/resource-pickers.tsx`,
`lib/team/resource-connections.ts`) and `NewReference`'s local-id shapes (`lib/team/resource-edit.ts`). Pipeline steps
pick profiles and tools by server id.

Done when: no editor names a member's connection, path or secret id; pickers list the team's resources and the
catalog's CLI kinds and models; it matches the web design frames.

### OP-206 · Web: create publishes v1; archive confirmation with Used by (medium)

Depends on: OP-201, OP-203.

`web/server`: the new-resource pages (`resource-new-page.tsx`, `components/team/resource-create.tsx`) have only Create
and Cancel and publish v1 at once (no drafts for a resource that has no version yet). Archive
(`app/[slug]/resource-actions.ts`) first reads used-by and shows a confirmation listing what uses the resource and
which runs will stop; archiving is never refused for being used.

Done when: new-resource pages create v1 with no draft; the archive confirmation lists Used by and the runs that stop;
it matches the web design frames.

## Pre-existing failures (not this module)

i18n tests on master: `claims.refused.fenced` has no source, `project_settings.memory` repeated, `stepCannotWrite`
duplicated in `crates/workspace/assets/locales/en.json`.
