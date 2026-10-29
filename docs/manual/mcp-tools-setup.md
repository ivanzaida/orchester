# MCP reference — project setup tools

These tools set a project up the way its configuration screens do: the project itself, agent connections and
accounts, MCP servers, agent profiles, pipelines, instructions, skills, command tools and secrets, and the end of a
run's work (its changes, merging, completing). They are served by the app's MCP server only (not by a shared
project's server). Conventions that apply to every tool — naming projects/tasks/runs, strict schemas, links, the
error shape and the person-only gates — are in [mcp-server.md](mcp-server.md). Work tools (tasks, runs,
approvals, usage) are in [mcp-tools-work.md](mcp-tools-work.md).

Notation used in the parameter tables:

- **Req.** — `yes` if the parameter is required. Everything else may be left out.
- Types follow JSON Schema: `string`, `integer`, `boolean`, `array<…>`, `object`, `T | null`.
- *Gate* — which person-only switch, if any, a call may need:
  - **AP** = *Let automation change agent permissions* (Settings › Automation › *Approvals and permissions*). Without it, a widening
    call is refused with `person_only`.
  - **AA** = *Let automation answer approvals*. Without it, the call is refused with `automation_not_allowed`.
- No tool takes or returns a secret's value.

---

## 1. Project

### `add_project` — changes data

Adds a project in a folder that is the **top of a git repository**, as the app's project setup does. Orchester keeps
the project's data in the folder's `.orchester/` (excluded from git) and suggests the project's key (e.g. `KON`) from
its name. If the repository is on GitHub and a signed-in `gh` can read it, the project is linked to that repository;
a failed link does not undo the project and is reported under `github` in the result.

| Parameter | Type   | Req. | Description                                                      |
|-----------|--------|------|------------------------------------------------------------------|
| `name`    | string | yes  | The project's name; the key is suggested from it.                |
| `folder`  | string | yes  | Absolute path of the folder at the top of the git repository.   |

Refusals: `not_a_git_repository` (not a repository's top), `no_commits` (runs need a commit to branch from),
`git_not_found` (git is not installed), and a refusal for a folder that already holds an Orchester project (it is in
`list_projects` then).

### `get_project` — read-only

A project's setup: id, key, name, folder, limits, code index, the environment variables its agents and command tools
get (plain text by design — they never hold secrets), whether agents load the repository's own agent files
(`AGENTS.md`, `CLAUDE.md`), its **base branch**, its GitHub link, how many connections, profiles, pipelines and
instructions it has, and its app link (its Overview).

`base_branch.branch` is the branch new runs start from and deliver into (`null`: whatever branch the checkout has
checked out). `base_branch.missing` is `true` when the chosen branch is gone — new runs are then refused — and `null`
when git cannot tell. The base branch is kept on this computer only; each run's own base is in its reads.

| Parameter | Type   | Req. | Description                    |
|-----------|--------|------|--------------------------------|
| `project` | string | yes  | The project's id, or its name. |

### `set_project_environment` — changes data · gate AP (except harmless variables)

Replaces the environment variables the project adds to every agent CLI and command tool its runs start (Project
settings › Environment). Names are letters, digits and `_`, not starting with a digit, each name once ignoring case.
Values are stored and shown in **plain text — never put a secret in one**. An empty list removes them all. Returns
the variables as saved.

Adding, changing or removing any variable other than the harmless set — `RUST_LOG`, `RUST_BACKTRACE`, `CI`,
`NO_COLOR`, `FORCE_COLOR`, `TZ`, `LANG`, `LC_ALL`, `LC_CTYPE`, `LC_COLLATE`, `LC_MESSAGES`, `LC_MONETARY`,
`LC_NUMERIC`, `LC_TIME` — is `person_only` unless AP is on: a variable can change what programs load, where they
connect, what they trust or where tools keep their configuration.

| Parameter   | Type                              | Req. | Description                                         |
|-------------|-----------------------------------|------|-----------------------------------------------------|
| `project`   | string                            | yes  | The project's id, or its name.                      |
| `variables` | array<{`name`: string, `value`: string}> | yes  | Every variable the project adds, replacing the current list. |

### `set_repository_agent_files` — changes data · gate AP when turning on

Turns on or off whether the project's agents load the repository's own agent files (`AGENTS.md`, `CLAUDE.md` and
the like), as Project settings › General › *Repository agent files* does. Applies to runs that start from now on. Turning off is always
accepted; turning on puts the repository's text into every agent's prompt and is `person_only` unless AP is on.

| Parameter | Type    | Req. | Description                                       |
|-----------|---------|------|---------------------------------------------------|
| `project` | string  | yes  | The project's id, or its name.                    |
| `enabled` | boolean | yes  | Whether agents load the repository's agent files. |

Related project tools in the work reference: `list_projects`, `set_code_index`, `update_project_limits`,
`get_auto_mode`, `set_auto_mode`.

---

## 2. Connections and accounts

### `list_connections` — read-only · cross-project

A project's connections, each with its app link (its inspector in Connections):

- **Agent connections** — id, name, provider (`codex_cli`, `claude_code_cli`, `anthropic_api`, `openai_api`),
  program, status from the last check, CLI version, number of models, and the API provider a CLI runs on
  (`base_url`, host, `key_secret`'s name, models).
- **GitHub connections** — id, name, host, status.
- **MCP servers** — id, name, enabled, transport (`stdio` with program, args and working directory, or `http` with
  url and authorization: scheme `none`, `bearer`, `basic` with username, `basic_credential` or `header` with name,
  and its secret as `NAME@environment`; the url's query values are left out), environment (each variable's name and
  whether it is plain or a secret — values are never listed), the tools that run without asking, and the roles of
  the profiles whose draft or latest published version chooses the server.

| Parameter | Type           | Req. | Description                                                        |
|-----------|----------------|------|--------------------------------------------------------------------|
| `project` | string \| null | no   | The project's id or name; left out, every project the app knows. |

### `add_cli_connection` — changes data · gate AP for an unusual program or an API provider

Adds a connection to an agent CLI, as Connections › Add connection does. The CLI is first checked with read-only
commands (version, whether it is signed in, its models). It is saved only when it answers as the expected CLI; the
check result comes back either way. A CLI that is not signed in is saved and reads `sign_in_required`.

With `api_provider` the CLI runs on that model API with an API key instead of its own sign-in: Claude Code needs an
Anthropic Messages API, Codex an OpenAI Responses API. The key is an existing **project secret** named by id or name;
the models typed are the ones profiles on the connection choose from.

Gates: a `program` that is not a copy found on the search path is `person_only` (checked before it runs) unless AP is
on; any `api_provider` is `person_only` unless AP is on, because agents would send the project's prompts, code and
command output to that host.

A given `program` is examined before any gate, without running it, and saved as given (a link stays a link):
`program_not_absolute` for a relative path, `program_not_found` when no file is there, `program_not_a_file` for a
folder, `program_not_executable` on macOS for a file without an execute permission (`chmod +x`), and
`program_unreadable` when it cannot be examined. Without `program`, when no copy is found, `program_not_found` names
the folders searched, in search order (on macOS: the inherited `PATH`, the login shell's `PATH`, then the usual
install folders).

| Parameter      | Type                    | Req. | Description                                                                  |
|----------------|-------------------------|------|------------------------------------------------------------------------------|
| `project`      | string                  | yes  | The project's id, or its name.                                               |
| `kind`         | `codex` \| `claude`     | yes  | Which CLI.                                                                   |
| `name`         | string                  | yes  | Unique among the project's connections, e.g. `Codex · Kontur`.               |
| `program`      | string \| null          | no   | Absolute path of the CLI; left out, the newest copy on the search path.      |
| `api_provider` | ApiProvider \| null     | no   | Run the CLI on a model API with an API key (see below).                      |

**ApiProvider** object:

| Field        | Type          | Req. | Description                                                                                          |
|--------------|---------------|------|------------------------------------------------------------------------------------------------------|
| `base_url`   | string        | yes  | `https://…`, or `http://` only for `localhost`, `127.0.0.1`, `[::1]`; no user, password, query or fragment. Example (DeepSeek): Claude Code `https://api.deepseek.com/anthropic`, Codex `https://api.deepseek.com`. |
| `key_secret` | string        | yes  | The project secret holding the key: its id, or its name when exactly one non-archived secret has it. |
| `models`     | array<string> | yes  | At least one model id the provider serves.                                                           |

### `set_api_provider` — changes data · gate AP when moving to a new host or key

Runs a Codex or Claude Code connection on an API provider, or back on its CLI's own sign-in (`null`). Moving to a
host or key the connection does not use now is `person_only` unless AP is on; going back to the sign-in, or changing
only the models, is always accepted. A connection on a provider runs on its CLI's *Default* account and reads no plan
limits. Returns the connection as `list_connections` lists it.

| Parameter      | Type                | Req. | Description                                                  |
|----------------|---------------------|------|--------------------------------------------------------------|
| `project`      | string              | yes  | The project's id, or its name.                               |
| `connection`   | string              | yes  | A Codex or Claude Code connection, by id or name.            |
| `api_provider` | ApiProvider \| null | no   | The provider (as in `add_cli_connection`); `null` = sign-in. |

### `check_connection` — changes data

Checks a connection's program again (its *Check again*) and stores what it found: status, version and models. A
native (Anthropic API) connection is tested as its *Test connection* tests it: the project's runner reads its key and
lists the endpoint's models, which are merged into its models; refused as `not_checked` with why when the runner is
not running or the test did not happen.

| Parameter    | Type   | Req. | Description                      |
|--------------|--------|------|----------------------------------|
| `project`    | string | yes  | The project's id, or its name.   |
| `connection` | string | yes  | The connection's id, or its name.|

### `list_models` — read-only

The models an agent connection offers, each with the reasoning efforts it accepts, as the profile editor offers them:
the ones its last check listed, an API provider's typed ones (`source` `api_provider`), or a native connection's
fetched and added ones (`source` `native`). A model not listed can still be given to `save_profile_draft` (it then
runs as *not verified*); an effort is kept only where the model lists it. `link` is the connection's app link.

Parameters: `project` (string, req.), `connection` (string, req.).

### `list_accounts` — read-only · no arguments

The person's agent CLI accounts on this computer (Settings › Accounts), which every project's connections can run
on: id, `cli` (`claude_code` | `codex`), name (`null` for an unrenamed Default), whether it is the CLI's Default (its
own folder), state (`signed_in`, `not_signed_in`, `sign_in_expired`, `sign_in_unknown`, `cli_not_found`,
`folder_missing`, `not_checked`), whether Orchester created its folder or the person brought it, the
account it shares a sign-in with, and its link. No email and no folder are listed.

`plan_limits` is the latest plan-limits reading (or `null`): `source` (`turn` — reported by an agent turn; `read` —
read by Orchester outside a run), `observed_at` (ms since epoch), `age_seconds`, `state` (`normal`, `near`,
`reached`), `plan`, `blocked` (why the CLI refuses turns: `window`, `credits`, `workspace`, `other`, or `null`), and
each window by length in minutes (`300` = five-hour, `10080` = weekly) with `used_percent`, `resets_at` and
`reset_since_reading` (true when the reset has passed since the reading — current use is then unknown, never
estimated). Codex accounts are read on their own; Claude Code accounts only by a turn or the person's *Check now*.

Adding, renaming, removing and signing in accounts are done by a person in the window.

### `set_connection_account` — changes data · gate AP for another account

Chooses which account an agent CLI connection runs on (the connection's *Account* field). Runs that start from now
on use it; a running agent keeps its account. The connection is checked again on that account. Choosing a different
account than the current one is `person_only` unless AP is on (it can send the project's code to another
organisation's provider).

| Parameter    | Type   | Req. | Description                                                                  |
|--------------|--------|------|------------------------------------------------------------------------------|
| `project`    | string | yes  | The project's id, or its name.                                               |
| `connection` | string | yes  | The agent connection's id, or its name.                                      |
| `account`    | string | yes  | An account of the connection's CLI by id or name; `default` = its Default.   |

---

## 3. MCP servers (for agents)

MCP servers configured here are given to **agents inside runs** — only to the agents of profiles that choose the
server (`save_profile_draft`'s `mcp_servers`).

### `add_mcp_server` — changes data · gate AP (always)

Adds an MCP server, as Connections › Add MCP server does. Either a local **`program`** that the agent's CLI starts
with each agent (a name on the search path or an absolute path; stored as the fixed file it resolves to, links
followed; must be outside the project's repository and on this computer), or a remote **`url`** reached over HTTP.
Always `person_only` unless AP is on. Returns the server as `list_connections` lists it.

| Parameter           | Type                           | Req. | Description                                                                                         |
|---------------------|--------------------------------|------|-----------------------------------------------------------------------------------------------------|
| `project`           | string                         | yes  | The project's id, or its name.                                                                      |
| `name`              | string \| null                 | —    | Small letters, digits, `-` and `_`, starting with a letter; unique. Agents know the server and its tools by it. |
| `program`           | string \| null                 | —    | Local server program. Not with `url`.                                                               |
| `args`              | array<string> \| null          | no   | Program arguments, passed as written; none may be empty or contain a line break.                   |
| `working_directory` | string \| null                 | no   | A folder inside the worktree or an absolute path; empty = the worktree.                            |
| `environment`       | array<{name, value}> \| null   | no   | Program variables. `${secret:NAME@environment}` in a value takes the value of an active project secret when the agent starts. |
| `url`               | string \| null                 | —    | Remote server, `http://` or `https://`. Not with `program`.                                         |
| `authorization`     | Authorization \| null          | no   | What a remote server is sent (see below).                                                           |
| `exempt_tools`      | array<string> \| null          | no   | The server's tools that run without asking, for profiles that ask before MCP actions.              |

**Authorization** (`scheme` selects the variant; each `secret` is an active project secret `NAME@environment`, never
a value):

| `scheme`           | Other fields          | Header sent                                             |
|--------------------|-----------------------|---------------------------------------------------------|
| `none`             | —                     | nothing                                                 |
| `bearer`           | `secret`              | `Authorization: Bearer <secret>`                        |
| `basic`            | `username`, `secret`  | `Authorization: Basic base64(username:secret)`          |
| `basic_credential` | `secret`              | `Authorization: Basic <secret>` (a pasted credential, its base64 part or a whole `Basic …` value) |
| `header`           | `name`, `secret`      | `<name>: <secret>` (e.g. `X-API-Key`)                    |

### `update_mcp_server` — changes data · gate AP for widening changes

Changes a server's fields, as Edit MCP server does. Takes the same fields as `add_mcp_server` plus **`server`**
(req.: id or name). A field left out keeps its value; giving `program` or `url` switches the transport; a field the
transport does not have is refused (`field_not_applicable`). Whether the server is on stays as it is.

Always accepted: dropping the authorization (`scheme: none`), removing tools from `exempt_tools`. `person_only`
unless AP is on: a new name (runs approve a server's tools by its name), another program, arguments, folder or
endpoint, an authorization that sends a different secret (another secret, scheme, username or header), any variable
other than the harmless ones (removals included), a variable that takes a secret, and a tool added to
`exempt_tools`. The judgement is made on the fields as given, before anything reads the disk.

### `remove_mcp_server` — changes data

Removes a server from a project. Refused as `in_use` while a profile's draft or latest published version chooses it
(the reasons name the roles) — switch them first, or turn it off with `set_mcp_server_enabled`, which keeps its
settings.

Parameters: `project` (string, req.), `server` (string, req.: id or name).

### `set_mcp_server_enabled` — changes data · gate AP when turning on

Turns a server on or off for every profile that chooses it (inspector's *Turn on* / *Turn off*). Agents that start
from now on get it or work without it; agents at work keep what they started with. Off is always accepted; on is
`person_only` unless AP is on.

Parameters: `project` (req.), `server` (req.), `enabled` (boolean, req.).

---

## 4. Agent profiles

A profile is an agent's role (e.g. *Builder*), the connection and model it runs on, its effort, its permissions and
its MCP servers. Pipeline steps run **published** profiles. Profiles are versioned: you save a **draft**, then
**publish** it as the next version. A draft started from an older version than the latest is a *stale draft*.

### `list_profiles` — read-only · cross-project

Each profile: id, role, connection, model, effort, where it stands (`draft`, `stale_draft`, `published` with its
version), and its link. Parameter: `project` (string | null).

### `get_profile` — read-only

One profile as its editor shows it (its draft if it has one, else its latest version): role, responsibilities, role
instructions, connection, model, effort, permissions (repository access, extra writable folders, commands, network,
approvals), MCP servers, where it stands, which published pipeline steps run it, its **`revision`** (for
`save_profile_draft`'s `expected_revision`) and its link.

| Parameter | Type   | Req. | Description                    |
|-----------|--------|------|--------------------------------|
| `project` | string | yes  | The project's id, or its name. |
| `profile` | string | yes  | The profile's id, or its role. |

### `save_profile_draft` — changes data · gate AP for widening

Saves a profile's draft with every field of its editor; without `profile` a new profile is made. Checked as the
editor checks it: role, an agent connection of the project and a model are required; role instructions may use
`{{variables}}`; permissions must be ones the connection's CLI enforces on this computer; MCP servers must be the
project's.

Narrowing is always accepted. Anything **broader than the latest published version** (for a new profile: broader
than the most restrictive permissions its CLI enforces, with no MCP server) is `person_only` unless AP is on: write
access, writable folders, broader commands, network on, MCP servers, no longer asking before tool calls, approving
automatically and Autopilot (both answer requests in a person's place, so they additionally need AA, as the
window's question does).

| Parameter           | Type                   | Req. | Description                                                                                   |
|---------------------|------------------------|------|-----------------------------------------------------------------------------------------------|
| `project`           | string                 | yes  | The project's id, or its name.                                                                |
| `profile`           | string \| null         | no   | Profile to change, by id or role; left out = new profile.                                     |
| `role`              | string                 | yes  | The agent's role, e.g. `Builder`; steps and instructions name it.                             |
| `connection`        | string                 | yes  | Agent connection, by id or name.                                                              |
| `model`             | string                 | yes  | A model id `list_models` gives, or another the CLI runs (then *not verified*).                |
| `effort`            | string \| null         | no   | A reasoning effort the model lists; left out = the CLI's default.                             |
| `responsibilities`  | string                 | no   | What the agent is responsible for, a sentence or two. Default `""`.                           |
| `instructions`      | string                 | no   | Role instructions carried by every prompt; may use `{{variables}}`. Default `""`.            |
| `permissions`       | Permissions \| null    | no   | Left out: new profile = most restrictive its CLI enforces; existing = keeps its own.          |
| `mcp_servers`       | array<string>          | no   | Project MCP servers its agents get, by id or name. Default `[]`.                              |
| `expected_revision` | string \| null         | no   | Optimistic lock: save only if the draft is still at this revision, else `draft_changed`.      |

**Permissions** object:

| Field                   | Type                                          | Req. | Description                                                                |
|-------------------------|-----------------------------------------------|------|----------------------------------------------------------------------------|
| `repository`            | `read_only` \| `write`                        | yes  | Repository access.                                                         |
| `commands`              | `ask_before` \| `allowed` \| `any_in_worktree`| yes  | How shell commands are handled.                                            |
| `allowed_commands`      | array<string>                                 | no   | With `allowed`: command prefixes that run, e.g. `cargo test`.              |
| `network`               | `off` \| `on`                                 | yes  | Network access.                                                            |
| `writable_folders`      | array<string>                                 | no   | Folders outside the worktree the agent may write (write access only).     |
| `ask_before_tool_calls` | boolean                                       | no   | Ask a person before each MCP or app tool call.                             |
| `approve_automatically` | boolean                                       | no   | *Approve automatically (don't ask me)*: Orchester answers what would ask a person. |
| `approval_mode`         | `autopilot` \| `manual`                       | no   | Who answers tool requests. Default `manual`; `manual` → `autopilot` widens (gate AP, and AA as approving automatically), the reverse never asks. |

### `publish_profile` — changes data · gate AP for broader or re-worded profiles

Publishes the draft as the next version (*Publish*), so pipeline steps can run it. Refused for a stale draft, a
connection that is gone or does not run agents, permissions its CLI cannot enforce, `draft_changed` if the draft
changed while it was checked, and `person_only` for a draft broader than the latest version while AP is off.

A draft whose role, responsibilities or role instructions differ from the latest version (for a first version: any
responsibilities or role instructions) changes what agents are told, so it is also `person_only` unless AP is on,
the person applies the Orchestrator's proposal of it, or the person lets the Orchestrator publish drafts.

Parameters: `project` (req.), `profile` (req.: id or role).

---

## 5. Pipelines

A pipeline is the ordered graph of **steps** a task runs: agent steps (run a published profile) and tool steps
(run a published command tool). Steps can wait at an **approval gate**, return to an earlier step on failure
(**failure route** with retries), and have a **token budget**. `delivers` says what a run hands over.

**`delivers`** values (used by pipelines and tasks):

| Value               | Meaning                                                                                              |
|---------------------|------------------------------------------------------------------------------------------------------|
| `changes`           | Repository changes, which are merged. Default.                                                       |
| `report`            | The last step's report; a run that committed nothing is *completed* instead of merged.               |
| `outside`           | Work done through MCP tools outside the repository: the report plus the tool calls that changed something. |
| `changes_or_report` | Changes when the run made any, else its report.                                                      |

### `list_pipelines` — read-only · cross-project

Pipelines that have a published version (which tasks run with): name, version, steps, link. Parameter: `project`
(string | null).

### `get_pipeline` — read-only

One pipeline as its editor shows it (draft if any, else latest version): description and steps (key, name, agent
profile or tool, runs after, approval gate, failure route with retries, step budget, repository access, model
override and narrowed permissions, instructions), `delivers`, where it stands, the **problems that keep it from
being published**, the published profiles and tools its steps can use, and its link.

Parameters: `project` (req.), `pipeline` (req.: id or name).

### `create_pipeline` — changes data

Makes a new pipeline as draft v1 (*New pipeline*). The name must give a slug no other pipeline has.

| Parameter  | Type                              | Req. | Description                                                         |
|------------|-----------------------------------|------|---------------------------------------------------------------------|
| `project`  | string                            | yes  | The project's id, or its name.                                      |
| `name`     | string                            | yes  | The new pipeline's name.                                            |
| `from`     | `starter` \| `empty` \| `copy`    | yes  | What it starts with.                                                |
| `copy_of`  | string \| null                    | with `copy` | Pipeline to copy (its draft or latest version), by id or name. |
| `delivers` | Delivers \| null                  | no   | Left out: `changes` (a copy: as its source).                        |

The **starter** is *Feature delivery*: Research → Plan (approval gate) → Implement → Run checks → Review (returns to
Implement) → Create pull request. Steps get the published profiles and tools named *Explorer*, *Planner*, *Builder*,
*Reviewer*, *Run test suite* and *Create pull request* where exactly one has that name.

### `save_pipeline_draft` — changes data · gate AP for weakening changes

Replaces the draft's steps, as the editor saves. Refused with the editor's reasons when the steps do not form a
pipeline (nothing is saved then).

| Parameter     | Type              | Req. | Description                                              |
|---------------|-------------------|------|----------------------------------------------------------|
| `project`     | string            | yes  | The project's id, or its name.                           |
| `pipeline`    | string            | yes  | The pipeline's id, or its name.                          |
| `steps`       | array<Step>       | yes  | Every step, in the order the editor shows them.          |
| `description` | string \| null    | no   | What the pipeline is for; left out = unchanged.          |
| `delivers`    | Delivers \| null  | no   | Left out = unchanged.                                    |

**Step** object:

| Field              | Type                     | Req. | Description                                                                                         |
|--------------------|--------------------------|------|-----------------------------------------------------------------------------------------------------|
| `key`              | string                   | yes  | Small letters, digits and `-`; other steps and instructions name it.                               |
| `name`             | string                   | yes  | Display name.                                                                                       |
| `profile`          | string \| null           | no   | Published agent profile that runs the step (id or name). Leave both `profile` and `tool` out = unassigned (blocks publishing only). |
| `tool`             | string \| null           | no   | Published command tool the step calls instead (id or name).                                         |
| `runs_after`       | array<string>            | no   | Keys of the steps it runs after (no cycles).                                                        |
| `approval_gate`    | boolean                  | no   | The step's result waits for a person's approval (e.g. a plan).                                      |
| `on_failure`       | {`return_to`: string, `retries`: integer ≥ 1} \| null | no | Return to an earlier step it runs after; after `retries` returns the run waits for a person. |
| `budget`           | integer \| null          | no   | Tokens the step may use; left out = no step budget.                                                 |
| `instructions`     | string                   | no   | What the step's agent/tool is asked to do; may use `{{variables}}`.                                 |
| `repository`       | `read_only` \| `write` \| null | no | Agent step's repository access (write only where its profile writes); default `read_only`.        |
| `model`            | string \| null           | no   | Agent step's model instead of its profile's.                                                        |
| `narrow`           | {`only_commands`?: array<string>, `network_off`?: bool, `ask_before_tool_calls`?: bool} \| null | no | Narrow the profile's permissions — never widen. |
| `takes_base_moves` | boolean \| null          | no   | Whether the agent is told to merge the base branch when it moves. Default rule: a step with write access that does not review or verify (no `review`/`verify` in its key, no failure route back to a writing step). |

Example command step: `{"key": "check", "name": "Check", "tool": "Check", "runs_after": ["implement"],
"on_failure": {"return_to": "implement", "retries": 2}}`.

`person_only` unless AP is on: dropping an approval gate or *ask about tool calls* that the latest published version
(before the first: the draft) has — removing the step that has it included — and adding a step that calls a tool
neither the draft nor the latest version calls under that key (it runs a program on this computer).

### `publish_pipeline` — changes data · gate AP unless provably no weaker

Publishes the draft as the next version, so tasks can run it. Refused while it has no steps or a step has no profile
or tool, or for a stale draft. Without AP only a draft **provably no weaker** than the latest published version is
published (same steps, wiring, failure routes, profiles and tools; each step's approval, asking about tool calls,
write access, commands and network the same or stricter). A pipeline's **first version** is always `person_only`
without AP. So every version a task can run is one a person published, allowed, or could not object to.

Parameters: `project` (req.), `pipeline` (req.).

`duplicate_pipeline` (copy and publish) is in [mcp-tools-work.md](mcp-tools-work.md#duplicate_pipeline--changes-data--gate-ap-unless-identical).

---

## 6. Instructions

Instructions are text Orchester adds to agents' prompts, scoped to the project, a role, a pipeline or a step, and
delivered at start and/or resume. Versioned like profiles (draft → publish).

### `list_instructions` — read-only · cross-project

Each: id, name, where it applies (`project`, `agent_role`, `pipeline`, `step`), which agents it reaches (`all`,
roles, profiles), when (`start`, `resume`), its text, where it stands (`draft`, `stale_draft`, `published`),
`draft_by_automation`, and its link. Parameter: `project` (string | null).

### `get_instruction` — read-only

One instruction (draft if any, else latest): text, scope, agents, events, where it stands, every published version
with its time and text, link. Parameters: `project` (req.), `instruction` (req.: id or name).

### `save_instruction_draft` — changes data · always allowed

Saves a draft; without `instruction` a new one is made under `name`. A draft reaches no agent, so saving is always
open; the Instructions page marks it *Draft written by automation* until a person saves it or it is published.

| Parameter        | Type                                                  | Req. | Description                                             |
|------------------|-------------------------------------------------------|------|---------------------------------------------------------|
| `project`        | string                                                | yes  | The project's id, or its name.                          |
| `instruction`    | string \| null                                        | no   | Instruction to change (id or name); left out = new.     |
| `name`           | string \| null                                        | new  | New instruction's name (names don't change later).      |
| `applies_in`     | `project` \| `agent_role` \| `pipeline` \| `step`     | yes  | Scope.                                                  |
| `role`           | string \| null                                        | with `agent_role` | A role of the project's profiles (reaches all agents of the role). |
| `pipeline`       | string \| null                                        | with `pipeline`/`step` | Pipeline id or name.                    |
| `step`           | string \| null                                        | with `step` | Step key or name.                                |
| `agents`         | `all` \| `roles` \| `profiles`                        | yes  | Which agents it reaches.                                |
| `agent_roles`    | array<string>                                         | with `roles` | Roles.                                          |
| `agent_profiles` | array<string>                                         | with `profiles` | Profiles by id or role.                      |
| `events`         | `start` \| `resume` \| `start_and_resume`             | yes  | When agents get it.                                     |
| `text`           | string                                                | no   | May use `{{variables}}`; may be blank in a draft.       |

### `publish_instruction` — changes data · gate AP (or Orchestrator proposal/switch)

Publishes the draft; agents get it from then on. Refused for blank text or a stale draft. Because it reaches every
targeted agent's prompt, it is `person_only` unless AP is on, the person applies the Orchestrator's proposal, or the
person lets the Orchestrator publish drafts.

Parameters: `project` (req.), `instruction` (req.).

---

## 7. Skills

Skills are packages (a `SKILL.md` body plus reference files) that Orchester gives agents as files they read when a
task calls for them. Scoped like instructions; versioned (draft → publish).

### `list_skills` — read-only · cross-project

Each: id, name, slug, description, scope, where it stands, `draft_by_automation`, the repository file it was
imported from (if any), link. Parameter: `project` (string | null).

### `get_skill` — read-only

One skill (draft if any, else latest): description, body, reference files (path and size, not content), scope, where
it stands, `draft_by_automation`, published versions (number and time), the tasks whose unfinished runs still use a
version (task, version, step, link), its `revision`, link.

Parameters: `project` (req.), `skill` (req.: id, name or slug).

### `save_skill_draft` — changes data · always allowed

Saves a draft; without `skill` a new skill is made under `name` (its slug is made from the name, unique in the
project). On an existing skill, a field left out keeps its value and `name` renames it (the slug stays). A new skill
starts with no description, an empty body, project scope and no files.

| Parameter           | Type                                   | Req. | Description                                                                 |
|---------------------|----------------------------------------|------|-----------------------------------------------------------------------------|
| `project`           | string                                 | yes  | The project's id, or its name.                                              |
| `skill`             | string \| null                         | no   | Skill to change (id, name, slug); left out = new.                           |
| `name`              | string \| null                         | new  | Name (unique ignoring case: `invalid_name`, `name_taken`).                  |
| `description`       | string \| null                         | no   | What an agent reads to decide whether to load it.                           |
| `body`              | string \| null                         | no   | Markdown of its `SKILL.md`.                                                 |
| `applies_in`        | `project`\|`agent_role`\|`pipeline`\|`step` \| null | no | Default `project`. A `step` must be an agent step (`scope_unavailable`). |
| `role` / `pipeline` / `step` | string \| null                | per scope | As for instructions.                                                 |
| `files`             | array<{`path`: string, `content`?: string \| null}> \| null | no | Replaces all reference files. `path` relative with `/`, no `.`/`..`, not absolute, not `SKILL.md`; omit `content` to keep the existing file at that path. At most 100 files of ≤ 1024 KiB each (`invalid_files`). |
| `expected_revision` | string \| null                         | no   | Optimistic lock (`draft_changed`).                                          |

### `publish_skill` — changes data · gate AP (or Orchestrator proposal/switch)

Publishes the draft; runs that start from then on load it. Refused: `blank_description`, `blank_body`,
`stale_draft`, `scope_unavailable`, `no_draft`, `draft_changed`, and `person_only` under the same rule as
`publish_instruction`.

Parameters: `project` (req.), `skill` (req.).

---

## 8. Command tools

Tools are what **tool steps** of a pipeline call. Kinds are `command`, `http` and `database`; the MCP server edits
command tools. A command tool runs **in the worktree of the step that calls it, without a shell, with the person's
rights, network access and no sandbox**, on files the agents before it wrote.

### `list_tools` — read-only · cross-project

Each: id, name, kind, description, where it stands, whether a step can run it, a command tool's command line, link.
Nothing about the secrets an HTTP or database tool uses. Parameter: `project` (string | null).

### `get_tool` — read-only

One tool (draft if any, else latest): program, arguments, timeout, passing exit codes, environment variables, output
limit; where it stands; which published steps call it; `revision`; link. Parameters: `project`, `tool` (both req.).

### `save_tool_draft` — changes data · gate AP for new tools and widening

Saves a command tool's draft; without `tool` a new one is made under `name`. The program is a name found on the
search path (the tool's `PATH`, else the project's, else the app's; relative folders skipped) or an absolute path to
an existing file **outside the project's repository**; it is stored as the file found. A path inside the repository
is refused — run a script through its interpreter instead (`program: powershell`, `arguments: ["-File",
"tools/check.ps1"]`).

| Parameter           | Type                         | Req. | Default (new tool) | Description                                         |
|---------------------|------------------------------|------|--------------------|-----------------------------------------------------|
| `project`           | string                       | yes  |                    | The project's id, or its name.                      |
| `tool`              | string \| null               | no   |                    | Tool to change (id or name); left out = new.        |
| `name`              | string \| null               | new  |                    | Unique; a tool keeps its name.                      |
| `program`           | string \| null               | new  |                    | Required for a new tool.                            |
| `arguments`         | array<string> \| null        | no   | none               | Passed as-is; no shell.                             |
| `description`       | string \| null               | no   | none               |                                                     |
| `timeout_secs`      | integer \| null              | no   | 600                | 1 s … 24 h.                                         |
| `exit_codes`        | array<integer> \| null       | no   | `[0]`              | Exit codes that pass.                               |
| `environment`       | array<{name, value}> \| null | no   | none               | Plain text; never a secret. Replaces the list.      |
| `output_limit_kib`  | integer \| null              | no   | 256                | Output the run keeps, 1 … 16384 KiB.                |
| `expected_revision` | string \| null               | no   |                    | Optimistic lock (`draft_changed`).                  |

Without AP, `person_only`: making a tool, changing its program or arguments, letting more exit codes pass, changing
any non-harmless variable. Always accepted: description, timeout, output limit, fewer exit codes.

### `publish_tool` — changes data · gate AP for first versions and widening

Publishes the draft so steps can call it. Refused for a stale draft and `draft_changed`. Without AP a tool's first
version is `person_only`, as is a draft that runs another program or arguments, lets more exit codes pass or changes
a non-harmless variable. Parameters: `project`, `tool` (both req.).

---

## 9. Secrets

### `list_secrets` — read-only · cross-project

A project's active secrets **by name and use only — never a value** (values stay in the OS keychain and are not
read). Each: id, name, kind, environment (a name is unique per environment, written `NAME@environment` where tools
and MCP servers refer to it), `added_at`, `replaced_at`, and `used_by` (tools, MCP servers, agent connections whose
API key it is, and the profiles whose draft or latest version chooses an MCP server using it, with that server),
each with a link, plus the link to Secrets.

Adding, replacing and archiving secrets are done by a person in the app. Parameter: `project` (string | null).

---

## 10. Ending a run's work

### `get_changes` — read-only

What a run changed, as the task's *Changes* tab reads it: the files of its last checkpoint against the base it
started from (path, change, additions, deletions), and what its worktree holds that no checkpoint has yet. Diffs too
large to read give their size only. `link` is the task's Changes tab.

Parameters: `project` (req.), `run` (req.: id or number, `7` or `#7`).

### `merge_run` — changes data

Merges a run — its task's latest successful run — into its base branch and waits for the merge, **when the main
checkout needs no local integration** (nothing local in the way, no local integration of the run under way).
Otherwise it refuses with `needs_decision` or `merge_failed`: preparing the checkout, conflict choices and approving
the exact merged result are a person's, on the app's *Local integration* page. A run already merged (by Orchester,
or outside it as git proves) is refused as `already_merged`. A run delivered by the **merge queue** is not merged by
`merge_run`. After `merge_failed` at conflicts, `continue_run` with `resolve_conflicts` asks an agent to resolve them.

Parameters: `project` (req.), `run` (req.).

### `complete_run` — changes data · gate AA with `anyway`

Completes a run that passed every step and **committed nothing** (its task's latest), as the task Summary's
*Complete task* does: the last step's report is kept as the run's Report and the task becomes Done.

| Parameter | Type    | Req. | Description                                                                  |
|-----------|---------|------|------------------------------------------------------------------------------|
| `project` | string  | yes  | The project's id, or its name.                                               |
| `run`     | string  | yes  | Run id or number.                                                            |
| `anyway`  | boolean | no   | Complete although the pipeline expects changes (*Complete anyway*). Needs AA. |

Refusals: `expects_changes` (pipeline expects changes and no `anyway`), `automation_not_allowed` (`anyway` without
AA), `has_commits` (use `merge_run`), `already_completed`, `run_not_completable` (did not succeed, not the latest,
merged, or waiting in a merge). The completion is recorded as automation's.
