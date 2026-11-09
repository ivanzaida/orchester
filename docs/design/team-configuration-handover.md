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
| ORC-818 | Team MCP servers cached and archived like other kinds; archive stops runs whose profile chooses the server (typed `TeamResource` refs since ORC-817) | merged here (728925a87, row 79 fixed in 456af94d8) |
| ORC-825 | Fill-in writes: connection pick, MCP program path (refreshes every project of the server), secret value by name, MCP on here; person-only via `Answerer` | merged here (ec98cf814) |
| ORC-819 | `used_by` read: team via the route, local from the project database with the archive's `pins()` rule; used-by contract amended (no run number, listed from the first report, superseded runs ended, no MCP server runs on the server until the bundle carries them) | merged here (f52581fd1) |
| ORC-816 | Create = v1 with no draft (store create_published/discard_created, workspace, MCP save_*_draft, Servers::send_created) | merged here (c7af6227d) |
| ORC-835 | The hub's create forms (A7–A12): New opens the kind's form in the panel with only Create and Cancel; Create sends v1 to the team server at once and a refusal is said in place; the old profile, instruction, skill and tool windows say Create and send at once too. Not in the forms yet, because a profile of this computer cannot hold them: a profile's instructions, MCP servers, tools and skills; a skill's other files and CLIs; HTTP and database tools; "Profiles that pick it" for an instruction. An unreachable server keeps a created v1 waiting to send (the hub lists it as waiting, the seed scenario `team-hub-waiting` shows it); a later refusal takes it back whole only when it is the resource's lone version with no draft, and the hub's *was not created* notice for it is kept in memory only, so it is gone after a restart. **Design verification not done** | this branch |
| ORC-836 | Editing a team MCP server in the hub's panel (A13): *Edit* makes the team's part a form kept as the resource's draft, *Publish for the team* sends the next version and the team server's answer is said in place. A refused publish takes the version back as the draft and says why: no longer an admin turns the page read-only with *Discard my draft* and *Copy my changes* (A14, "Ask an Admin"), content the server rejects names its reason (A17; `422 invalid`'s reason is kept on the version as `invalid:<reason>`), a server that cannot be reached leaves *Waiting to send* with *Cancel sending* (A16). Create's refusals match (A15 name taken as a field error, A14 and A17 as notices; a created resource waiting to send can be cancelled from its form or from the hub's panel). The profile, instruction, skill and tool editors keep their own pages: their waiting bar offers *Cancel sending* and, for a refused version, the reason and *Take it back as a draft*. QA states `team-edit`, `team-edit-demoted`, `team-edit-offline`, `team-edit-rejects` (a person presses in them) and, made without a press for a design verifier, `team-edit-published` (the accepted v2), `team-edit-refused-admin` (A14: read-only, draft kept; says *You are no longer an admin of the team*, because the server's own "refused" title shows only right after a person's press), `team-edit-waiting` (A16), `team-edit-rejected` (A17 as the outbox holds it after the person left: the waiting bar with the reason and *Take it back as a draft*) and `team-create-name-taken` (A15 as it stands after the person left the form: the hub's *taken back* notice; the form's own field error needs a person's Create). A version in the outbox that the server refused or that met another is shown in the panel (waiting bar, no *Edit* on top); one send of a project's outbox at a time (`Servers::send_lane`), so *Cancel sending* cannot delete a version being sent. Declared deviations from A13: Arguments and Variables stay multi-line and *Runs as* stays segmented, as in the create form (ORC-835); the *Versions* and … buttons of the head are not there (no version history for these kinds, ORC-839). Not done: editing a profile, instruction, skill or tool in the panel; A17's *Remove sentry-issues* / *Open sentry-issues* (the server's reason does not name the resource); the time and the person in A14/A15's texts; "N tries" in A16; the version waiting here is used by this admin's own new runs until the server numbers it (A16 says nobody's runs use it). **Design verification not done** | this branch |
| ORC-836b | Follow-ups of ORC-835/836's reviews and design check. The edit form has *Variables it needs*; a banner stands between the head and the chips; a list row says the outbox first (*Waiting to send · v2* blue, *Your draft on v1 · publish refused*) and what the computer needs after it; a version the server refused as an edit is the refused form (red *The team's server refused v2: …*, the edit bar with *Publish for the team* and *Discard draft*, which take it back as the draft first) and not the waiting bar; the waiting bar of a new resource whose first version stays refused offers *Discard it*; a created resource whose first version the server refused later, with a later version or a draft over it, stays listed (`OFFERED` keeps a resource with a `created` publish); `team_created::settle` keeps a version queued after a failure the server may have answered; the pass of the outbox notes the new resources it took back even when it stopped early; the send lane is keyed by the folder's real path; a folder inside the worktree needs its path; a removed team MCP server refreshes Connections. QA states, made without a press: `team-edit-waiting`, `team-edit-refused-admin` and `team-edit-rejected` now have the program path of `pencil-design` set (their rows say only the outbox's words, as A14/A16/A17 do) and `team-edit-variables` (`pencil-design` takes `PENCIL_TOKEN`). **Design verification not done** | this branch |
| ORC-837 | Concurrent edits of a team MCP server in the hub's panel (rule 18, A18–A21): a draft keeps the version it was edited from (`save(…, base)`, `McpDraft::based_on`; a refused or conflicting version is taken back on its base, `reopen_draft_on_its_base`), so what the team published since is told apart from what the person changed. `McpEdit::newer` carries it with the field-by-field comparison; the three-way merge is `agents::team_mcp_merge` (pure: a field only one side changed goes to it, one both changed the same way is that value, one both changed differently asks, and without the base every difference asks). A18: one quiet line over the form with *View changes*; a publish on an older version is refused as a conflict (`EditRefused::Changed`, nothing sent), and a version in the outbox that met the team's is the same page (A19: the comparison with *Mine*/*Theirs* for a field both changed, *Rebase my draft on vN* (disabled until each is picked), *Open vN*, *Discard my draft* in a modal, A20). The pick is never preselected (the design shows *Mine* chosen). A21: an archived server whose draft is kept is listed under *Archived* (`TeamHub::archived`), publish is refused as archived, *Restore* (admins, in a modal; `Servers::restore_resource`, retries once on the revision the server names, applies the server's answer here) brings it back. Rebase, discard and restore are a person's (`Guarded::PermissionWidening`, as publish). QA states made without a press: `team-edit-newer` (A18), `team-edit-conflicting` (A19; *Discard my draft* opens A20), `team-edit-conflict-clean` (no field both changed), `team-edit-archived` (A21). Not done for the other kinds (profile, instruction, skill, tool): they keep *Open my draft* and *Publish mine as vN+1* and the JSON-level `compare::conflict`; the merge needs a per-kind field table like `team_mcp_merge`'s. Not in A21: who archived it (the stream says only that it was archived). **Design verification not done** | this branch |
| ORC-817 | Team references are `CliKind`, `TeamResource { kind, id }`, `SecretName` (no local ids on the wire; the legacy cached variants went with ORC-845); archive `pins()`, used-by and fill-ins read them. **Interim until ORC-820/830: a team profile/tool naming a local connection or secret is refused (`local_entities_named`), the desktop sends `references: []`, team start's MCP servers are a stopgap until ORC-826** | merged here (2f94bef8d) |
| ORC-829 | add/update/remove_mcp_server write team MCP servers (admin-only; widening-gated; `program` = team body name, the path is each computer's fill-in); remove archives after reading used-by and reports `runs_stopping_here`; a team MCP server URL has no query or fragment (core); chat MCP proposals refused in team projects until a team-aware preview (follow-up) | merged here (bb8dd77f4) |
| ORC-828 (part 1) | MCP tools: pick_profile_connection, team MCP on (person_only always) / off, program person_only, list_secrets names; **readiness in get_profile/list_profiles and a team MCP server listing are part 2 (after ORC-822/824)** | merged here (aa745f907) |
| OP-197 step 1, OP-198, OP-201 | Platform: pin on this branch (now e5d53c929), MCP server kind (migration 30; bundle MCP part waits for ORC-880), used-by route (current work only) | platform module/team-configuration 48d14c0 |
| ORC-820 | Team profile body: CLI kind, team ids, no writable folders (refused, not stripped); bundle inputs take the execution profile per version; unresolved connection said | merged here (2fd23d062) |
| ORC-821 | Team pipeline body: profiles and tools by server id; a version this computer cannot send is recorded refused | merged here (fe068697d) |
| ORC-881 | The catalog reports only CLI kinds and models | merged here (21581e877) |
| ORC-822 | Readiness rule in core: Ready here or the ordered missing parts | merged here (55cf66bac) |
| ORC-880 | Execution bundle schema 3 pins the team MCP server versions its profiles name | merged here (bfa3ae62d) |
| ORC-823 | Versions widen by the team references they name (new MCP server, tool, secret; header bound to another secret) | merged here (e1d0ab014) |
| ORC-882 | An MCP server's variables and secret headers name team secrets (`{header, secret, prefix}`) | merged here (d09a38eb9) |
| ORC-883 | A team start whose bundle schema differs fails with a reason | merged here (c4e8ceab8) |
| ORC-830 | Share uploads a local project's configuration in the team's form; the sharer stays ready; what could not be carried is said | merged here (4973a375e) |
| ORC-884 | `bundle_missing` carries why (`profile_unreadable`, `mcp_server_archived`, …) | merged here (5f0243edb) |
| ORC-824 | Readiness of each team resource here; groups; a server nobody decided on is Needs you here | merged here (5082c794f) |
| ORC-838 | Archive in a team panel (A22, A23): an admin's … menu in the head of each kind's panel holds *Archive*, which opens a confirmation that lists *Used by* and the runs that will stop (the team server's `used-by`, `Servers::used_by`) and asks the server to archive; never refused for being used, confirmed by a person only (automation may open it). The stream's `resource.archived` stops the runs on each computer as before and now records which resource (id, kind) and who archived it (`archive_stops.resource_id`, `resource_kind`, `archived_by`; none for one a snapshot reported), which the task page names in a stopped card and its status line (A22). Run again refuses while a profile still names the archived server, tool or instruction (`Undecided::ChoosesArchivedMcpServer`, `UsesArchivedTool`, `UsesArchivedInstruction`, tested per kind with a restore; a **skill cannot be told archived** here, since this computer keeps none of the team's, so a profile naming one stays ready and the modal's note and the card's *still uses…* show only for a server, tool or instruction: `readiness::archive_blocks_profiles`). A run an archive stopped says so in the header (red *Stopped at 03 Implement · pencil-design was archived*, *Run 8 · … · stopped 15:21*), on the Summary's step rail (chip in `danger`) and last-event line, and in the property rail (*Stopped*); Run again is the header's button for it, not a menu row (A22). `Servers::archive_resource` takes an `Answerer` and refuses automation (`NotArchived::PersonOnly`); the MCP tool `remove_mcp_server` on a team MCP server never archives: it reads what uses it and answers `person_only` with that preview (`error.preview`: `resource`, `used_by`, `runs_stopping_here`) so the client tells the person to confirm in the window (every MCP caller is automation; the hard-coded `Answerer::Person` it had bypassed the rule). The confirm re-reads *Used by* and keeps the modal open with the new list when the runs that stop changed. A22's pencil-design row has no *Open →*: an archived resource has left the hub's lists, so the hub would open another resource. QA states `team-archive` (A23: press `team-hub-more`, then `team-hub-archive`) and `team-archived` (A22: KON-12). The CLI mark on Builder's row and the run summary's last event line are built, and the hub's line for a profile that names an archived server, tool or instruction says *Names pencil-design, which the team archived*; the task page follows the shared cache while it shows the stopped card, so a restore drops *still uses…* (`team_qa_hub_archive`). **Not built:** the local wording (L11) and a local archive: a local project has no hub panel yet and no archive of its own (its removal has Unused and Reset modes), so the dialog is the team's only; A22's *Ask an admin to restore it*, the step and Builder's version in the card's body, and *Open →* on the archived row. **Follow-ups:** a restore's re-read is tested through the shared-cache wake-up, not through the real event stream (the stand-in team server sends none); a pipeline that runs an archived profile or tool as a member reports `RunsUnknown` (the archived resource has left the lists), not the archive. **Design verification not done** | this branch |
| ORC-844 | Project settings › Connections keeps the CLI connections and accounts (MCP servers live in the Agents hub; the MCP server page and form stay for its panels), Sharing › On this computer, the missing-here bars and the *Runs here use* picker are gone, with their texts and the shared-here seed scenarios. **Design verification not done** | this branch |
| ORC-843 | The local project's create forms (L3–L7): the hub's New under a kind opens that kind's form in the panel with only Create and Cancel (`HubView::adding`, the route `agents?filter=…&new=1`; under All a menu); the team hub's form is mode-aware (`team_hub::create::Mode`, the same parts) rather than forked. Create makes v1 at once with no draft; a local MCP server has no versions, so Create makes the server with its program path (*Choose…*), its variables, the value of each secret they take (the system's secret store, one create with the server: `connections::mcp::create_with_values` removes the server again when the store refuses a value) and whether runs may use it. A local profile names its connection (chosen for it when its CLI has one) and its MCP servers. The old New pages of a profile (`agents/new`) and of an MCP server (`connections/new?kind=mcp_server`) open the hub's form in a local project (`local_hub::form_or`); a team project keeps them. The instructions', skills' and tools' old New pages are **not** redirected yet: they create what the forms cannot (an instruction's scope, fixed at creation; a skill's files, scope and CLIs; a tool's timeout, exit codes, environment, approval questions and HTTP kind) and their tests cover that; redirect them (`form_or` in `screens/mod.rs`) once the forms take those, or when the person decides to lose them. `navigate` takes `view: new` with a kind's `filter`. Not in the forms, because a profile of this computer cannot hold them or the model has no such kind: a profile's instructions, tools and skills (L3's three *Choose…* rows are one note), an instruction's *Profiles that pick it* and its scopes (the old New page still makes role, pipeline and step scopes), a skill's other files, *Import a folder…* and *Loads on*, HTTP and database tools (L7 draws an HTTP request; only commands are made). A secret a new server takes shows *no value* until one is typed even if the secret store already holds one (the vault's presence check is ORC-886). **Design verification not done** | this branch |
| ORC-845 | The reference bindings are removed: `bind_new`/`bind_own`, `BindingsHere`, the `reference_bindings` table and the references of versions cached before the model (migration 163; the local body of such a version stays), the slug give-way and the legacy `local_body` fallback; run inputs schema 2 has no `references` and still reads schema 1 | this branch |
| ORC-846 | Closed, won't do (person, 2026-11-07): the old local profile format is dropped; old-form profiles are republished | — |

## Decisions (person, 2026-11-07)

- The old local profile format is dropped: no compatibility on the desktop or the team server; no special message.
- A team MCP server nobody decided on here is Needs you here; only one switched off is Off here.

## In progress

- ORC-826 (team start from fill-ins, `not_ready_here`), ORC-827 (local start `not_ready_here`), ORC-828 part 2
  (readiness in the MCP tools), ORC-886 (vault presence check without reading the value).

## Next

- UI: ORC-831…844 (each with design verification), ORC-885 (start failure reason on the task page, claim release; done on `claude/orc-885-start-failure`, 2026-11-08).
- ORC-847 docs, ORC-848 QA; then merge `master` into this branch.
- Platform: OP-203…206 (web), deploy blocker `FIRST_DESKTOP_WITH_BUNDLE_REASONS` / `MINIMUM_DESKTOP_VERSION`.

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
