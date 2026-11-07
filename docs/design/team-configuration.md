# Team configuration: what the team server owns and what each computer fills in

Status: **model and open questions decided** (2026-11-06; two more decisions 2026-11-07), being built on the module
branch `module/team-configuration`. It replaces the patches made on 2026-11-05/06 (rows 74 and 80 of
`docs/architecture/server.md`, slug clashes, hidden local resources, reference bindings) with one model. The rows of
`docs/architecture/server.md` describe it since ORC-847 (rows 30, 52, 74, 75, 80 rewritten, 81–94 added);
`team-configuration-handover.md` says what is built. Decided in conversation on 2026-11-06 are marked *(decided)*; the
rest is proposed.

## Why

Today a team profile or pipeline is a copy of a resource from the publisher's computer. It carries that computer's
ids: its connection, its MCP server, its secrets. Every other computer has to bind those foreign ids by hand, two
references can share a name (`pencil-design` 01a19225… and 01a12726…), a run whose reference is not bound starts
silently without it, local copies clash with team ones by slug, and the desktop has to hide what it keeps. Each fix
added a case. The model below removes the cause: **the server never holds an id or a path from somebody's computer**.

## Who owns what *(decided)*

| Owner | What |
| --- | --- |
| **Team server** — the one source of truth | the project, members and roles; tasks, modules, releases, claims and run progress; the team's configuration: pipelines, agent profiles, MCP servers, instructions, skills, tools. Only admins change the configuration (row 72) |
| **Each computer** | connections and accounts (Claude Code, Codex, API providers), the program path of each MCP server, secret values and environment, which team MCP servers are switched on here; runs, worktrees, transcripts |

### Per resource kind

| Kind | On the server | On each computer |
| --- | --- | --- |
| Pipeline | everything: steps, the profile or tool of each step (server ids), gates, Deliver | nothing |
| Agent profile | role, instructions, CLI kind, model, effort, permissions; the MCP servers, tools, skills, instructions it uses (server ids); the secrets it needs (names) | which local connection and account runs it |
| MCP server *(decided)* | name, how it runs (local program or remote URL), arguments, folder, tools that run without asking, the variables it needs and its secret headers (each the name of the team secret it takes); a remote server's URL | the program's path; the secrets' values; on or off here: nobody has decided until a person switches it on or off (an automation may switch it off, never on) |
| Instruction, skill | everything | nothing |
| Tool (command, HTTP, database) | everything except secret values | secret values |
| Secret *(decided)* | only its name and what it is for; never a value | its value |

**Rule:** a team resource refers only to other team resources (by server id) and to names. A computer fills in only
what is local by nature: connections, program paths, secret values.

## What a computer fills in

- **Connection for a profile.** A profile names its CLI kind. With one local connection of that kind it is used
  without asking; with several, the person picks once per profile, kept on this computer; with none, the profile
  says what to add.
- **MCP server program** *(decided)*. The team server holds the whole MCP server; for a local one the person sets only
  the path to its program on this computer, once. Nothing is guessed or found by name for other computers; **Share** is the
  one exception, and only for the sharer's own computer: a local server started by a name on the search path (`npx`,
  `uvx`) is looked up once there when the project is shared, the file found is this computer's path, and a name that is
  found nowhere is reported in the Share result as needing its path set. A remote MCP server needs
  nothing unless it has secret headers. A program's variables and a remote server's secret headers name the team secrets
  they take (a variable the secret of its own name; a header its secret and an optional prefix such as `Bearer `), so
  the names an MCP server needs are read from its body.
- **Secrets.** Each needed name gets a value here, once.
- **Readiness.** Every profile, MCP server and tool shows *ready here* or exactly what is missing. A team MCP server
  nobody has decided on here is *Needs you here* (it is missing "switched on"); one a person switched off, or an
  automation did (off is the safe direction, so automation may), is *Off here*. Readiness also has an *undecided*
  state for what cannot be known yet: a version that cannot be turned into the team's form, a connection or secret the
  project no longer has, a keychain that cannot say. It is not assumed ready. In a local project the same read checks
  what a run would: the value of each secret a tool uses, and the program of each MCP server a profile chooses that
  is turned on, looked up as a run does (the project's `PATH`, else Orchester's search path, or the project; a fixed
  file outside the repository). A run whose pipeline is not ready here does not start: it refuses with the missing
  parts listed (`not_ready_here`). Nothing is ever dropped silently.

## Edge cases

| # | Case | Proposed decision |
| --- | --- | --- |
| 1 | Existing team projects (Orchester today) whose team resources carry foreign ids | A one-time migration on the server rewrites each version's references: a connection reference becomes the CLI kind; an MCP server reference becomes a team MCP server (created from the publisher's local definition, by name, deduplicated by name); a secret reference becomes its name. Runs pinned to old versions keep their snapshot |
| 2 | Local resources in a team project (made before sharing, or by a member) | Not shown and not usable in a team project; kept untouched in the database. An admin can **Publish for the team** one of them (it becomes a team resource with a new server id) |
| 3 | A person joins a team with resources of their own | Same as 2: they stay local and invisible in the team project |
| 4 | Two local MCP servers with the same name | The team MCP server is filled in by the person once; no name matching guesses |
| 5 | The CLI, MCP program or a secret is missing here | The resource shows what is missing; runs refuse `not_ready_here`; other computers are unaffected |
| 6 | Several connections of one CLI kind | Person picks once per profile; a new connection later does not change the pick |
| 7 | Publish, archive, restore, rename | Server-side, admins only. Rename changes the name, never the id. Archive hides it from lists and pickers; restore brings it back |
| 8 | A resource is archived while a run uses it | *(decided)* The run stops, as row 79 does today: each run that has not ended and pins a version of it is cancelled, and the person is told which resource |
| 9 | A new version widens what agents may do | Unchanged: this computer keeps the previous version until the person presses *Use vN here* (row 35) |
| 10 | Offline | Reads come from the local cache of team resources; publishing waits in the outbox (row 36); runs start from cached versions |
| 11 | Admin vs member | Members read and run; only admins create, edit, publish, archive (row 72). Filling in local parts is everyone's |
| 12 | Share (local project → team) | Uploads the project's resources as team resources with their ids; references are rewritten as in 1 before upload |
| 13 | Leave the team (team → local) | *(decided)* The project keeps no copy of the team's configuration: it leaves with none |
| 14 | Local projects | Unchanged: everything stays local as today |
| 15 | Slug clashes | Gone: team resources are identified by server id and only team resources are shown in a team project |
| 16 | The web app's editor | Edits the same resources; picks MCP servers, tools and skills from the team's list, CLI kinds and models from the catalog (row 75) |

## Decided 2026-11-06 (the person)

1. A resource archived during a run stops the run (row 79 stays).
2. The team server creates and holds MCP servers; for a local one a computer sets only the program's path.
3. Secrets on the server: names only.
4. Leaving the team keeps no copy of the team's configuration.
5. Local projects get the same Agents page as team projects, without the team / on-this-computer split.
6. No drafts for a new resource: the new-resource page has only Create (publishes v1) and Cancel; editing an existing
   resource ends in Publish vN. Confirmations are modals; new and import are pages.
7. A member who is not an admin sees the team part read-only, with no disabled buttons, and changes everything under
   *On this computer*.
8. Memory keeps its own page, per computer.
9. Pipelines keep their own page; they are not a tab of the Agents filter.
10. The program path of an MCP server is shared by the whole computer; secret values are per project.
11. Archiving a resource that something uses is not refused: a confirmation modal lists what uses it (*Used by*) and
    which runs will stop.
12. Only a person in the window turns a team MCP server on here; the MCP tool answers `person_only`.

## What it replaces

Rows 30, 52, 74, 75 and 80 of `docs/architecture/server.md` (resource bodies with foreign references, local
pipelines in a team project, the reference bindings of *On this computer*, slug-taking), which ORC-847 rewrote; the
decisions of 2026-11-06 are rows 81–92 there and those of 2026-11-07 rows 93 and 94. The code that binds references
and hides local resources by slug is removed by ORC-845.

## Decided 2026-11-07 (the person)

1. The old local profile format is dropped: no compatibility on the desktop or the team server and no special message;
   old-form profiles are republished. ORC-846, which would have read it, is closed as won't do.
2. A team MCP server nobody has decided on at this computer is *Needs you here*; only one a person or an automation
   switched off is *Off here*.
