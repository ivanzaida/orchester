# Team configuration is read live from the team server

Status: decided 2026-11-10 (the person). It changes how a
team project's configuration reaches the desktop; the model itself stays as `team-configuration.md` describes it.

## Why

Today the desktop keeps a copy of the team's configuration in the project database: every team resource and its
versions (`resources`, `resource_versions`, `shared_version_bodies`, `shared_version_references`, the standings) are
cached by the event stream (`crates/workspace/src/sharing/sync/`). The copy drifts from the server:

- a version is fetched only when it is newer than the cached one (`sync/config/mod.rs`, `resource.latest > cached`),
  so a version the server rewrote is never read again here;
- the server's schema-32 migration rewrote the references of old versions but not their bodies; a profile published
  before the model still reads as the old local form and stays *Can't tell yet whether it is ready here*;
- on 2026-11-10 the server listed 12 profiles and this computer's hub 3, with 12 in the cache.

Two copies of the truth cost more than they save.

## The rule

1. **The team server is the only source of the team's configuration.** Profiles, MCP servers, instructions, skills,
   tools, pipelines and reviewers of a team project, with their latest versions and who may change them, are read
   from the server and held **in memory only**, per open project, for the session.
2. **When it is read.** When a team project opens (at the app's start for the projects open then), and again after
   the event stream reconnects. Nothing polls.
3. **How it stays current.** The event stream's `resource.*`, `member.*` and `project.changed` events update what is
   in memory (a published version: that resource's latest version is read; archived/restored: its mark; a member's
   role: who may change). The Agents hub and every reader follow the in-memory state; nothing is written to the
   project database for it.
4. **What this computer keeps** in its databases is only what is local by nature (`team-configuration.md`, "What a
   computer fills in"): the connection that runs a profile, the program path of a local MCP server, secret values,
   whether a team MCP server is on here. These are overrides keyed by the team resource's id; they never hold a copy
   of a team body.
5. **Runs.** A team run already starts from the execution bundle the server pins (`team_work/bundle.rs`), checked
   against what this computer filled in; that does not change. Readiness before a start is computed from the
   in-memory configuration plus the overrides.
6. **Editing.** An admin's Save sends the next version to the server at once (the panel's Save/Cancel, no drafts,
   as the local panels do since 2026-11-09). Without the server there is nothing to save to: there is no outbox of
   versions waiting to send. A refusal is said in place.
7. **When the server cannot be reached** and nothing was read this session, the hub says so (design `A25 · Agents ·
   team server unreachable` in `Agents · v2` `acFkz`): *Can't reach the team* in the head, no *New*, tabs without
   counts, and in the body the reason and *Try again*. Nothing older is shown. If it was read this session and the
   stream drops, what is in memory stays shown with the head saying the team cannot be reached, and Save is refused
   until it comes back.

## What changes

- **Workspace**: a per-project in-memory team configuration (a service the app owns), filled by a read of the server
  and updated by the stream; `team_hub`, readiness, `used_by`, the team MCP tools and the start's readiness read it
  instead of the project database. The stream stops writing resources and versions to the database
  (`take_shared_snapshot` / `take_shared_event` keep only what is not configuration, e.g. tasks).
- **Store**: a migration drops the cached team versions of shared projects (`shared_version_bodies`,
  `shared_version_references`, `shared_publishes`, `waiting_versions` and the team resources' `resources` /
  `resource_versions` rows of a shared project) and keeps the overrides (`team_fill_ins`, `mcp_server_programs`,
  connection picks, MCP switches).
- **Desktop app**: the Agents hub's team read (`queries/hub.rs::team_hub`) reads the in-memory configuration; its
  state is *reading*, *read* or *unreachable*; events invalidate it as they do today.
- **Removed**: the outbox of versions waiting to send (A16 *Waiting to send*), conflict/rebase over a cached base
  (A18–A21 are rethought: the server refuses a publish on an older version and the panel reloads), drafts of team
  resources kept on this computer.

## Not changed

Tasks and runs of a team project, the claim authority and the execution bundle; a local project, which keeps its
configuration in its database as today.

## Where it stands (2026-11-11)

- **Done**: the team's configuration is read into memory when the stream opens and kept current by its events
  (`crates/workspace/src/team_config/`, `sharing/sync/live.rs`); the hub, readiness, fill-ins and team MCP tools read
  it. An admin's change goes to the server at once (`sharing/team_edit/`: `Servers::publish_team`, made on the version
  the edit was opened on, and `Servers::create_team`); a publish on an older version is refused (`Changed`) and memory
  takes the server's latest. The team MCP server panel edits under the Save/Cancel bar with no draft; the hub's Create,
  `add_mcp_server`, `update_mcp_server` and `share_pipeline` create or publish on the server. The outbox is gone:
  waiting versions, *Use v<N> here*, conflict/rebase/merge, taken-back resources, drafts of team resources, A16–A21. The store migration
  `team_outbox` drops `shared_publishes`, `waiting_versions`, the unsent versions no run or task uses and the drafts of
  team resources, and keeps the overrides. A team project's profile, instruction, skill and tool pages are the hub's.
  While the team server cannot be reached the hub shows what it read, offers no *Edit* or *New*, and an open form
  keeps what was typed with *Save* unavailable and saying why.
- **QA copies**: a QA copy joined to the stand-in team server (`orchester-api`'s `test_support::team_server`) reads
  the team's configuration from it as from a real server: the stand-in answers a project's event stream once asked to
  (`open_event_streams`; open, no events), so the read at the stream's open fills memory. Every joined state holds on
  the stand-in, before the join, each team resource the seed cached; `team-hub` and `team-hub-member` wait for that
  read; the `team-edit*` states set their switch (down, demoted, rejecting) after it, and `team-archived` archives
  `pencil-design` on the stand-in. `team-hub-down` joins with the stream not answering and then stops the stand-in, so
  nothing was read this session and the hub says it cannot reach the team server (A25 `Zhcif`).
- **Part 3, the copy goes**: every reader of a team project's configuration reads memory, never the project
  database. The task form's and the tools' pipelines (`Workspace::project_pipelines`, `ProjectTasks::with_team_pipelines`,
  `team_published_pipelines`, with `Assignables::of_team` for what their steps run), the team pipelines a team task may
  name (`TeamPipelines::of(&TeamConfig)`, for actions, the team tools, Join and *Share task*), the Pipelines page and
  editor of a team project (`Workspace::pipeline_library`, `Workspace::open_pipeline`), and its *Versions* tab, read
  from the server (`Servers::team_pipeline_history`, `GET …/resources/{id}/versions`). A shared task names its pipeline
  by id whether or not memory has it; which are missing is told from memory. The runs an archive stops are found
  through memory and the runs' own inputs (`sync::archive::pins`); when the stream opens, the runs that pin a resource
  archived while this computer was not following are stopped. The stream writes no configuration to the database
  (no `config::sync`, no cached versions or profiles, `apply` only moves the cursor for a resource's events); Share
  keeps nothing of what it uploaded, so the project's own resources stay as they were, hidden while it is shared and
  its own again when it is made local. The store migration `team_cache` drops the team's resources, versions, bodies
  and references of a shared project except a version a run or a task here pins, and keeps the overrides.
  `sharing::compare` keeps only a local project's history; its conflict types are gone. While the configuration is
  being read or cannot be read, these readers say so (`TeamNotRead`) instead of reading an older copy.
- **Team pipelines are edited here**: a team pipeline is changed by an admin on
  the pipeline page (D-043 in Pipelines v2): Save, *Create v1* and Duplicate send it to the team server at once
  (`sharing/project/team_pipeline.rs`, `pipelines/team_write.rs`) and the page shows it saving until the answer; memory
  takes what the server answered and nothing is written to the project database, and a refusal or a server that cannot
  be reached keeps the edit as typed with why. A rename goes first, as the resource's PATCH on the revision memory
  has, then the steps; when only the name was taken the page says the steps were not saved. A member reads it only.
  The MCP tools `save_pipeline`, `create_pipeline`, `duplicate_pipeline` and `share_pipeline` wait for the server the
  same way. The tools that draft or publish a profile, instruction, skill or tool refuse a team project
  (`team_no_drafts`). An admin edits a team profile, instruction, skill and command tool in place in the
  hub's panel (`A26`–`A31` in `acFkz`, `screens/team_hub/edit/`, `agents::team_body_edit`): Save publishes the next
  version at once, a newer version refuses it and reloads the panel, and while the server cannot be reached what was
  typed stays with Save unavailable. A profile is made and edited with one form (`screens/hub/profile_form/`):
  the create form and the admin's edit have the same *Role*, Agent CLI, model and effort selects and lists with *Add*.
  An instruction, a skill and a command tool are made and edited with one form each too (`screens/hub/forms/`): the
  new one's form, the admin's edit of a team one and a local one's panel draw the same fields (an instruction's format
  and *Text*, a skill's description, files and *SKILL.md*, a tool's kind, description, program, arguments and time
  limit) with the same checks; a local instruction's panel no longer chooses *When* (it keeps its version's), and a new
  skill's SKILL.md is typed rather than made from its name.
  Not edited there yet: a skill's files, an HTTP or database tool, and the leave guard of an unsaved team edit.
- A team tool names the secret it sends or connects with by **name** (core's `TeamTool`, `docs/cloud-api.md` "Team
  tool body"; 2026-11-11): publishing a local tool to the team (Share, an admin's Save or *Create*) turns the
  publisher's secret id into that secret's name, archived or not, and a tool whose secret the project has no row for
  is refused before anything is sent (`ToolSecretGone`). The id stays a row key of each computer: it never goes into a
  team body and is never shown. The tool's panel (A6 `u9Hcc`, M6 `yTVeE`, `screens/team_hub/tool/`) reads *Headers*
  `Authorization: Bearer ← secret NAME`, *Secrets it needs* by name and, *On this computer*, that name's value set or
  not, with *Add value* pinned while it has none: the same fill-in and person-only rule as a team MCP server's secret
  (`set_team_secret_value`). Readiness needs that value by name, and the runner that accepts a team run checks it
  under the same name (`PreflightProblem::SecretMissing`). A version published before this form still holds the
  publisher's id: the panel says so in words without the id (`team_tool_secret_by_id`, *undecided*), and an admin
  whose project has that secret publishes it again by name (*Publish again by name*). Every tool of the team is read
  from the live configuration; the stream keeps no copy.

## Open questions

- Members and the project's owners/admins: read with the configuration (in memory) or kept as today.
- Whether the MCP tools of a team project answer `team_unreachable` when the server is down (proposed: yes, with the
  reason).
- A16–A21 in `acFkz` need redrawing once the outbox and cached-base conflicts go.
