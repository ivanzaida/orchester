# Orchester — plugins

Date: 2026-10-28. Status: **proposal**: the questions in [Open questions](#open-questions) still need the person's
answers. Agreed with the person in chat so far: the model follows FiveM's resources (client, server and shared scripts
in one folder, installed on a server that hands the client part to its clients); plugins are written in Luau; the
feature is called *Plugins*; plugins can extend the UI; the solution is generic, so client scripts run on every
client, the browser included. The team server's admin pages for plugins are drawn in `orchester-web.pen` (layers
"Plugins" and "Plugins · components"); the desktop's are not drawn yet.

## Summary

A **plugin** is a folder with a `manifest.lua` and Luau scripts. **Server scripts** run on a team server; **client
scripts** run on every client of a project: the desktop app, and the browser in the team server's web admin;
**shared scripts** run on both. An admin installs a plugin on a team server and turns it on for shared projects; the
server runs the server part, signs the client part and hands it to every member's desktop and to the admin's
browser, which run it in a sandboxed Luau runtime. A person can also install a plugin on their own desktop for local
projects. Plugins are isolated from each other: a plugin never sees, calls or depends on another. Plugins react to events, add MCP tools, keep their own state, reach the network only on
the hosts their manifest lists, use secrets only through handles, and extend the UI only by describing it from
Orchester's own building blocks in fixed places.

```
               team server (Linux)                      clients
           ┌──────────────────────────┐      ┌──────────────────────────────┐
 install → │ server_scripts (Luau)    │      │ desktop: client_scripts      │
           │ secrets, http, audit     │◄────►│   Luau native, rendered by   │
           │ signs client package ────┼─────►│   lurq                       │
           └──────────────────────────┘      ├──────────────────────────────┤
                         ▲   net events      │ web admin: client_scripts    │
                         └──────────────────►│   Luau → WASM, rendered by   │
                                             │   React (web/ui)             │
                                             └──────────────────────────────┘
```

## Goals and non-goals

Goals:
- Integrations and automations that teams write once and share: Slack, Notion, Linear, GitHub Issues, Sentry,
  secret stores, SIEM export.
- The same plugin works on Windows, macOS, the Linux team server and the browser, with one API.
- Plugins can add UI: pages, panels, columns, actions and cards, which look like the rest of Orchester.
- Safe enough that a server may hand plugin code to members automatically.

Non-goals:
- Plugins never ship their own HTML, CSS or native UI code, and never draw outside their slots.
- Plugins never start programs, read files or reach hosts outside their manifest. Work of that kind (agent adapters
  for other CLIs, pipeline steps that run tools) stays a separate kind, see [Other kinds](#other-kinds-of-plugins).
- Plugins never widen what agents may do. That stays a person's decision in the window (`mcp-person-gate`).
- No plugin store, paid plugins or revenue share in the first versions.

## The plugin package

```
github-issues/
  manifest.lua
  shared/*.luau
  client/*.luau
  server/*.luau
  locales/en.json, uk.json
```

```lua
api 'v1'
name 'github-issues'
version '1.0.0'
description 'description'            -- an i18n key in the plugin's locales

shared_scripts { 'shared/*.luau' }
client_scripts { 'client/*.luau' }
server_scripts { 'server/*.luau' }

permissions {
  tasks = { 'read', 'write' },  -- data grants: each one is listed, write does not imply read
  runs = { 'read' },
  http = { 'api.github.com' },
}

settings {
  repository = { type = 'string', label = 'settings.repository' },
  token = { type = 'secret', label = 'settings.token', hosts = { 'api.github.com' } },
}

ui {
  page = { id = 'board', label = 'ui.board', icon = 'kanban' },
  task_panel = { id = 'issue' },
  task_column = { id = 'issue_number' },
  task_action = { id = 'create_from_issue' },
}
```

- `api` is the version of the plugin API. A host refuses a plugin whose API it does not support, with the reason.
- The manifest is Luau, run once in an empty sandbox that only has the declaration functions. It cannot reach
  anything.
- The client package is the manifest, `shared_scripts`, `client_scripts` and `locales`. `server_scripts` never
  leave the server.
- `permissions` has two parts:
  - **Data grants** by domain, each an array of `read` and `write`. A `read` grant opens both the domain's reads
    through the API and its events; there is no separate list of events.
  - **Reach**: the `http` hosts.

  A plugin's network between its own parts needs no permission and is not listed: it opens nothing.

  | Domain | `read` gives | `write` gives |
  | --- | --- | --- |
  | `tasks` | tasks, modules, tags: `tasks.find`, `task.*`, `module.*`, `tag.*` | `tasks.create_draft`, `tasks.update`, `tasks.link`, `modules.*` writes, `task.state` |
  | `runs` | runs, steps, decisions, delivery, the Orchestrator's proposals and daily report: `runs.get`, `run.*`, `approval.*`, `question.*`, `plan.*`, `decision.*`, `needs_you.*`, `merge_queue.*`, `pull_request.*`, `orchestrator.*` | `runs.message`: a message to a run's agent |
  | `agents` (sensitive) | `agent.*`: turns, command programs and exit codes, changed paths, MCP tool calls | — |
  | `usage` | `limits.*`, `run.usage_recorded`, `runner.*`, `connection.*` | — |
  | `config` | `config.*`: published pipelines, profiles, instructions, tools, skills | — |
  | `memory` | `memory.*` | — |
  | `team` (sensitive, server only) | `member.*`, `invite.*`, `device.*`, `secret.used` | — |

  Without a grant, `orchester.on` for that domain's event fails at registration, and the plugin does not start, with
  the reason. The events about the plugin itself (`plugin.*`), `project.opened`/`closed`, `server.connected`/
  `disconnected`, `slot.*` and `project.changed` need no grant.
- **Secrets are settings** of type `secret`, declared in the manifest like any setting:
  - The person who sets the plugin up binds each one to a secret from their store: the admin on the server, or the
    member on the desktop.
  - `settings.token` is a **handle**, never the value.
  - `hosts` lists where the secret may be sent, and must be a subset of `permissions.http`. The host puts the value
    only into a request to one of those hosts, so a plugin allowed two hosts cannot send one host's token to the
    other.
  - A secret setting without `hosts` is refused at install.
- `permissions`, the secret settings and `ui` are what a person reviews before turning a plugin on. A new version whose permissions or
  UI contributions are wider than the running version's waits for the person's approval (see
  [Trust and permissions](#trust-and-permissions)).

## Where a plugin runs

| Project | Server scripts | Client scripts |
| --- | --- | --- |
| Shared project, plugin installed on its team server | the team server | every member's desktop with the project open, and the web admin's pages of that project |
| Local project, plugin installed on the desktop | the desktop runs them as well, in a separate state | the desktop |

- A local plugin is installed by the person on their own computer, from a folder, an archive or a git URL, and turned
  on per project. With no team server, the desktop runs both parts; `orchester.host()` tells a script where it runs.
- Plugins kept in a repository (`orchester/plugins/` committed with the code) are offered, never started on their
  own. The first start in a checkout is the person's decision in the window, like a workspace trust prompt, so cloning
  a repository never runs its code.
- A plugin installed on a team server works only in that server's shared projects. It never sees local projects or
  another server's.

## Distribution from the team server

1. The admin installs a plugin (archive, folder or git URL and ref) and turns it on for shared projects.
2. The server checks the manifest, starts the server part, and builds the client package: a
   deterministic archive, its SHA-256, and a signature by the server's key (the certificate the server already
   has).
3. The project's sync tells clients the plugin's id, version, hash and permissions. A desktop downloads the package,
   checks the hash and the signature against the server it joined, caches it under the project's data, and starts
   it.
4. Updating a plugin restarts it everywhere: the server part at once, clients when they next sync. Turning a plugin
   off stops it everywhere and removes it from the cache.
5. A desktop without the server keeps running the cached version. Nothing new is fetched until the server is back.

## The runtime

- **Luau**, the sandboxed Lua dialect, through the `mlua` crate (feature `luau`, vendored, async) on the desktop and
  the team server. In the browser, Luau compiled to WebAssembly with a small JavaScript host. One VM build, one set of
  semantics.
- One Luau state per plugin, per project, per host; the scripts of one plugin on one host share it. **Plugins are
  isolated**: there are no exports, no dependencies between plugins, and no API that names, lists or reaches another
  plugin. Two plugins only meet through Orchester's own data (a task one changed, the other sees as `task.changed`).
- Limits per state: memory (the allocator's limit), CPU through Luau's interrupt (a callback that runs too long is
  stopped with an error), and a wall-clock limit per host call.
- Async: every host call that waits (HTTP, events across the network, task changes) yields the coroutine. Scripts
  read like straight-line code. Many plugins share the host's runtime: tokio on the desktop and the server, the event
  loop in the browser.
- Lifecycle like FiveM: `starting → started → stopping → stopped`, or `failed` with the error and the script line.
  `restart` reloads the scripts; a plugin's `store` survives. A failed plugin never
  takes down the host or another plugin; its UI slots show the failure state.
- Plugins run in the app's process (workspace layer), not in the runner. Run events reach them through the
  workspace's existing event stream.

## The API

All host functions live under `orchester`. Every operation that changes project data follows the same rules and
refusals as the domain MCP tools (stable `code`, `reasons`); plugins get no back door.

| Area | Functions |
| --- | --- |
| Events | `orchester.on(name, fn(event))` for Orchester's events (see [Events](#events)). There is no local `emit`: the scripts of one plugin on one host share a state and call each other's functions directly |
| Network | `server.*` in client scripts, `client.*` in server scripts, each naming the other side: `emit`, `request`, `on`, `on_request` between the parts of the same plugin (see [Network](#network-between-a-plugins-parts)) |
| Project data | `tasks.find`, `tasks.create_draft`, `tasks.update`, `tasks.link(external_url)`, `runs.get`, `runs.message`, `modules.*`, each within the manifest's data grants. Plugins create drafts and never start runs |
| Per-task state | `task.state[key]`, synced through the team server in shared projects (FiveM's state bags) |
| Network | `http.request{method, url, headers, json, body}`, `ws.connect(url)`, only to the manifest's hosts. In the browser, both go through the team server's proxy |
| Secrets | a `secret` setting (`settings.token`) is a handle. The host substitutes the value into a header or body only when a request leaves for one of that setting's `hosts`; a script can never read it, log it, compare it or send it elsewhere |
| Storage | `store.get/set/delete`: per plugin and project, size-limited |
| Settings | `settings.repository`, read-only, from the manifest's schema |
| Time | `every(interval, fn)`, `after(delay, fn)` |
| UI | `ui.render(slot, tree)`, `ui.on_action(id, fn)` (see [UI](#ui)) |
| MCP tools | `tool(name, description, schema, fn)` (see [MCP](#mcp)) |
| Logs | `log.info/warn/error`, to the plugin's log, which the admin and the person can read |
| Host | `host()`: `"server"`, `"desktop"` or `"web"` |

Luau type definitions for the whole API are published with each API version, so authors get completion and type
checks.

## Network between a plugin's parts

Each side names the **other** side: client scripts have a `server` object, and server scripts have a `client` object,
which stands for the plugin's clients. The four functions are the same on both.

| In client scripts | In server scripts | Meaning |
| --- | --- | --- |
| `server.emit(name, ...)` | `client.emit(target, name, ...)` | send a notification to the other side; returns at once, no answer |
| `server.request(name, ...)` | `client.request(target, name, ...)` | send a request and wait for the answer; yields until the other side's handler returns |
| `server.on(name, fn(...))` | `client.on(name, fn(caller, ...))` | handle a notification the other side emitted |
| `server.on_request(name, fn(...) return ... end)` | `client.on_request(name, fn(caller, ...) return ... end)` | answer a request; the handler's return values go back |

```lua
-- client/board.luau: ask the server part
local ok, issues = pcall(server.request, 'issues', { repo = 'kontur/app', state = 'open' })

-- server/issues.luau: answer the clients
client.on_request('issues', function(caller, filter)
  if caller.person.role == 'viewer' then error('not allowed') end
  return fetch_issues(filter)  -- may itself await HTTP
end)
```

- **Scope.** A plugin talks only to itself. Plugin A's server part reaches only plugin A's clients, and plugin A's
  clients reach only plugin A's server part. Plugin B can never be reached, on either side. This is enforced by the
  hosts, not left to names:
  - every message is routed by the sender's plugin id and project, which the host attaches;
  - a script cannot set or see these, and there is no way to name another plugin;
  - names are plain strings without dots, so two plugins can use the same name without meeting.
- **Versions during an update.** While clients move to a new version, the server part may hear from clients still on
  the previous one. `caller.version` tells which, so a handler can answer both or refuse with a clear error.
- **Caller.**
  - On the server, `caller` is the client that sent the message: `{person = {id, name, role}, device = {id, name}, version,
    host = "desktop" | "web", id}`. The person and device come from the team server's own sign-in, so a handler may
    trust them for its own checks.
  - On a client there is no `caller`: a message can only come from the plugin's own server part, so `server.on`
    and `server.on_request` handlers get just the arguments.
- **Target** (in server scripts only, on `client.emit` and `client.request`), since a server has many clients:
  - a `caller` it received;
  - a person, meaning all their connected clients;
  - `"all"`, meaning every connected client of the project.

  `client.request` takes a single client; to ask several, it loops.
- **Values.**
  - Arguments and return values must be serializable: `nil`, booleans, numbers, strings, and tables of these, within a
    size limit.
  - Functions, coroutines, secret handles and UI trees cannot cross. Trying is an error on the sending side.
  - A handler may return several values.
- **Handlers.**
  - `on` may have several handlers per name.
  - `on_request` has exactly one; registering a second for the same name is an error.
  - Handlers run asynchronously and may await other API calls.
- **Errors.** A request raises a typed error the caller can catch with `pcall`:

  | Error | When |
  | --- | --- |
  | `no_handler` | nothing is registered for the name |
  | `handler_error` | the handler raised; its message, not its stack |
  | `timeout` | the default is 30 s; `request{timeout = 5}` changes it |
  | `offline` | the other side is not connected |
  | `too_large` | a value is over the size limit |

- **Offline.**
  - `request` fails at once with `offline`; it is never queued.
  - `server.emit` from a client while the server cannot be reached is queued, bounded, and sent in order when the
    connection returns. It keeps its original time, and an overflow is reported, as with events.
  - `client.emit` from the server to a client that is not connected is dropped.
- **Local projects.** With no team server, the desktop runs both parts, and `server.request` reaches the server part
  in the same process with the same semantics. A plugin needs no code for the difference.

## Events

The catalogue below is drafted from the app's own domain events: `RunEventKind`, `QueueEvent`, `AgentEvent`,
notifications `Kind`, `NeedKind`, `WorkflowStatus`, the team server's `EventKind`, the merge kinds and the memory
statuses. Each plugin event is a stable, versioned projection of those. A plugin never sees the internal enums, so the
app can change them freely.

### Conventions

- **Names** are `domain.past_tense` in snake case, for example `run.step_finished`. A plugin's own network
  messages are plain names without a dot, for example `approve`; the host scopes them to the plugin.
- **Envelope.** Every event carries:

  | Field | Meaning |
  | --- | --- |
  | `name` | the event's name |
  | `id` | a unique id; handlers use it to stay idempotent |
  | `at` | when it happened; an event made offline keeps its original time |
  | `project` | `{id, key, name}` |
  | `host` | where the handler runs: `server`, `desktop` or `web` |
  | `actor` | `{kind = person \| automation \| agent \| plugin, id, name}`; for a change made by another plugin, only `kind = plugin` and `self = false`, never which plugin |
  | `origin` | `local`, or `server` when another member's change arrived through sync |

- **References**, not content. Payloads name things: task `{id, key, title}`, run `{id, number}`, step
  `{index, name, kind}`, person `{id, name}`. Payloads **never** carry prompts, agent messages or reasoning,
  transcripts, file contents, diffs, command output or secret values. A plugin that needs more reads it through the
  API, within its grants.
- **Subscribing.** A plugin receives a domain's events when it has that domain's `read` grant (see
  [The plugin package](#the-plugin-package)). There is no list of events in the manifest. `orchester.on` with a name
  outside the plugin's grants is an error at registration. The `plugin.*` events are only about the plugin itself.
- **Delivery.**
  - In order per project and host.
  - Handlers run asynchronously and never hold up the app, a run or a merge.
  - Each plugin has its own bounded queue. If it overflows, events are dropped, and the plugin and its log get one
    `plugin.events_dropped` with the count. Nothing is lost silently.
  - A handler's error is logged and never affects anything else.
- **Notification only.** In v1 no event can veto or change what happens. Guard hooks that can refuse are listed in
  [Open questions](#open-questions).
- **Where an event appears:**
  - **C** — on the client where it happens: the desktop of the member whose run it is.
  - **S** — on the team server, for shared projects.
  - **W** — in the web admin's client scripts.
  - Server-side run events wait for the team server to learn run progress (phase S3 of the server plan). Until then,
    `run.*` and the delivery events are **C** only.

### Plugin and host

No grant needed.

| Event | Where | Payload |
| --- | --- | --- |
| `plugin.started` | C S W | version, reason (`start`, `restart`, `update`) |
| `plugin.stopping` | C S W | reason (`stop`, `restart`, `update`, `host_shutdown`) |
| `plugin.settings_changed` | C S W | the names of the changed settings |
| `plugin.events_dropped` | C S W | count, first and last `at` |
| `project.opened` / `project.closed` | C | — |
| `project.changed` | C S | the changed setting names (base branch, limits, environment names) |
| `server.connected` / `server.disconnected` | C | the team server's address |
| `slot.opened` / `slot.closed` | C W | slot, its context (task, page) |

### Tasks, modules and tags

Grant: `tasks` read.

| Event | Where | Payload |
| --- | --- | --- |
| `task.created` | C S W | task, workflow status, priority, module, tags, whether it is a draft |
| `task.changed` | C S W | task, the changed fields: title, brief, criteria, priority, story points, tags, module, pipeline, limits, blocked |
| `task.status_changed` | C S W | task, `from` and `to` workflow status (backlog, todo, in progress, in review, done, canceled) |
| `task.submitted` | C S W | task (draft → queued) |
| `task.claimed` / `task.released` | S W, and C for one's own | task, person, kind (`manual` or `run`) |
| `task.blocked` / `task.unblocked` | C S W | task, the reason or the blocking tasks |
| `task.deleted` | C S W | task |
| `task.link_added` / `task.link_removed` | C S W | task, the external URL, whether this plugin made the link |
| `module.created` / `module.changed` / `module.closed` / `module.reopened` | C S W | module `{id, name}`, the changed fields |
| `tag.created` / `tag.renamed` / `tag.deleted` | C S W | tag name, its old name when renamed |

### Runs and steps

Grant: `runs` read.

| Event | Where | Payload |
| --- | --- | --- |
| `run.created` | C | run, task, pipeline and its version, base branch |
| `run.started` | C | run |
| `run.step_started` | C | run, step, profile, connection kind, model |
| `run.step_waiting` | C | run, step, why it waits (person, limit, base moved) |
| `run.step_finished` | C | run, step, outcome (`passed`, `failed`, `skipped`, `accepted`), tokens, active time |
| `run.paused` / `run.resumed` | C | run, the reason |
| `run.interrupted` | C | run, the reason (app quit, runner stopped, crash) |
| `run.continued` | C | run, who continued it (person or automation) |
| `run.stopped_at_limit` | C | run, limit kind (tokens, time, daily budget, group budget, step budget, plan limit) |
| `run.limit_raised` | C | run, the new limits |
| `run.permissions_changed` | C | run, the names of what was widened or narrowed |
| `run.waiting_for_person` | C | run, what it waits for: approval, question, plan, decision, input |
| `run.succeeded` | C | run, whether it committed nothing, what it delivers (changes or report) |
| `run.failed` | C | run, the failure kind, the step |
| `run.cancelled` | C | run, who cancelled it |
| `run.reopened` | C | run |
| `run.usage_recorded` | C | run, step, tokens by model. Batched at most once a minute per run |

### Decisions

Grant: `runs` read.

| Event | Where | Payload |
| --- | --- | --- |
| `approval.requested` | C | run, step, approval id, what is asked: command program name, MCP server and tool, or a permission name. Never the full command line |
| `approval.decided` | C | approval id, `approve` or `deny`, decided by person or automation |
| `question.asked` / `question.answered` | C | run, question id, answered by person or automation. Never the question's or the answer's text |
| `plan.proposed` / `plan.decided` | C | run, plan id, decision (`approve`, `revise`, `reject`) |
| `decision.requested` / `decision.made` | C | run, kind (`stuck`, `return`, `recovery`), the choice |
| `needs_you.added` / `needs_you.removed` | C | the waiting kind (approval, choice, question, plan, decision, interrupted, stopped at limit, ready to merge, ready to complete, committed nothing, delivery left, delivery blocked, memory), task, run, person |

`needs_you.*` mirrors the Needs you panel and is the one event most notifier plugins need.

### Delivery

Grant: `runs` read.

| Event | Where | Payload |
| --- | --- | --- |
| `run.ready_to_merge` / `run.ready_to_complete` | C | run |
| `merge_queue.joined` / `merge_queue.left` / `merge_queue.requeued` | C | run, position |
| `merge_queue.check_started` / `merge_queue.check_passed` / `merge_queue.check_failed` | C | run, check name |
| `merge_queue.blocked` | C | run, the reason |
| `run.conflicts` | C | run, the number of conflicted files |
| `run.merged` | C, then S W when shared | run, task, merge kind (fast-forward, merge commit, squash, already merged), the commit |
| `pull_request.created` | C | run, the PR's URL and number |
| `pull_request.feedback_received` | C | PR, kind (`question` or `fix`), the reviewer's name |
| `pull_request.merged` / `pull_request.closed` | C | PR, task |

### Agents

Grant: `agents` read (sensitive).

| Event | Where | Payload |
| --- | --- | --- |
| `agent.turn_started` / `agent.turn_finished` | C | run, step, turn number, outcome (`completed`, `failed`, `stopped`) |
| `agent.command_finished` | C | run, step, the program's name, the exit code, the duration. Never the arguments or output |
| `agent.file_changed` | C | run, step, the path relative to the worktree, the change kind (added, modified, deleted, renamed) |
| `agent.mcp_tool_called` | C | run, step, MCP server name, tool name, outcome (finished, declined, denied) |
| `agent.permission_denied` | C | run, step, what was denied (by name) |
| `agent.error` | C | run, step, the error kind (not signed in, runner problem, provider error) |

### Usage and limits

Grant: `usage` read.

| Event | Where | Payload |
| --- | --- | --- |
| `limits.plan_limit_near` / `limits.plan_limit_reset` | C | provider, account, window, the share used |
| `limits.budget_near` / `limits.budget_reached` | C | kind (daily or group), the amount |
| `runner.offline` / `runner.online` | C | project |
| `connection.check_failed` / `connection.check_passed` | C | connection name and kind |

### Configuration, memory and the Orchestrator

Grants: `config` read for `config.*`, `memory` read for `memory.*`, `runs` read for `orchestrator.*`.

| Event | Where | Payload |
| --- | --- | --- |
| `config.published` | C S W | kind (pipeline, agent profile, instruction, tool, skill, module), name, version, publisher |
| `config.draft_saved` | C | kind, name |
| `memory.proposed` / `memory.accepted` / `memory.rejected` / `memory.deprecated` | C | memory id and title |
| `orchestrator.proposal_made` / `orchestrator.proposal_decided` | C | proposal id, kind, decision |
| `orchestrator.daily_report_ready` | C | date, counts (merged, failed, waiting), the report's link. The report's text through the API |

### Team (server)

Grant: `team` read (sensitive).

| Event | Where | Payload |
| --- | --- | --- |
| `member.joined` / `member.left` / `member.removed` | S W | person, role |
| `member.role_changed` | S W | person, `from` and `to` role |
| `invite.sent` / `invite.accepted` / `invite.revoked` | S W | invite id, email domain only, role |
| `device.joined` / `device.revoked` | S W | person, device name |
| `secret.used` | S | secret name, the run that used it, or `this plugin`. Never the value, never another plugin |

## UI

A plugin describes UI; the host draws it. The description is a tree of Orchester's own building blocks: the **plugin
UI kit**, drawn in `orchester-web.pen` ("Plugins · components").

- Blocks: page header with actions, board (columns and cards), table, list row, chips and labels, button, panel,
  key-value rows, empty state, filter bar, form fields, sync-status line, markdown text.
- **Slots**, the only places a plugin may draw:
  - a page in the sidebar, under a Plugins group;
  - a panel on the task page;
  - a column or badge in the task list;
  - an action on task rows and the task page;
  - a card on the Overview;
  - the plugin's own Settings, generated from its settings schema.
- Each contribution shows a quiet attribution ("from github-issues").
- **Rendering.** The tree is a versioned JSON schema in the shared plugin crate. The desktop draws it with lurq
  components and theme roles; the web admin draws it with `web/ui` components and design tokens. A tree that does
  not match the schema is refused and the slot shows the failure.
- **No styling from plugins.** Colors, fonts, sizes and spacing are never part of the tree; blocks take only semantic
  variants (`tone = "warning"`).
- **Text.** Every string in a tree is a key in the plugin's own locale files, or raw user data marked as such. Hosts
  check key and placeholder parity across the plugin's locales at install, as `tests/i18n.rs` does for the app.
- **Interaction.** Buttons and form submits send `{action, payload}` to the plugin's `ui.on_action` in the same
  client. Local interaction (drag on a board, filtering, expanding) is done by the block itself and reported as an
  action. The plugin answers with a new tree or a patch. A refused action restores the block and shows the reason.
- **Failure isolation.** A slot whose plugin failed, timed out or produced an invalid tree shows the shared failure
  state with the plugin's name; the rest of the page works.

## MCP

- A plugin's `tool(...)` appears in the app's MCP server as `<plugin>.<tool>`, with the same refusal codes,
  `person_only` rules and window-control rules as built-in tools. In a shared project, a tool of a server-side plugin
  is forwarded to the team server.
- An agent in a run gets a plugin's tools only when its profile grants them, and the team policy allows them. Calls
  with side effects follow the step's approval rules.
- Secret values never appear in a tool's arguments or results; tools take secret names.
- Plugin tools are not given to anything by default.

## Trust and permissions

- **Who installs.**
  - On a team server, an admin.
  - On a desktop, the person, in the window, never over MCP.
  - Plugins from a repository are started only after the person allows them in the window.
- **What a member sees.** Project settings › Plugins on the desktop lists every plugin the project runs, who installed
  it, its version and its permissions and UI contributions.
- **Widening.**
  - A new version with wider permissions or UI contributions waits for the admin's approval on the server.
  - Each member then confirms on their desktop before the wider client part runs there; until then the member keeps
    the previous version.
  - Narrowing applies at once.
- **Team policy** (ORC-137) can allow or forbid plugins and their tools per shared project.
- **Audit** (ORC-138) records:
  - installs, updates, approvals, turning on and off;
  - every plugin call that changes project data or leaves the host.
- **Signing.**
  - Client packages from a team server are signed by that server.
  - Local plugins are trusted by the person who installed them.
  - The hash of every running package is shown, so a person can compare it.
- **Secrets.**
  - Secret settings bound on the server stay on the server.
  - A desktop's client scripts get only the secret settings the member binds from their own store.
  - Every secret is usable only toward the hosts its setting lists.

## Other kinds of plugins

These complete Luau plugins; they are not replaced by them.

- **Configuration packs.**
  - Versioned bundles of pipelines, profiles, instructions, skills, tools and MCP server settings, with no code.
  - The Claude Code and Codex plugin formats are imported the way existing agent configuration is (ORC-20).
- **Process plugins.**
  - A program that speaks MCP over stdio with Orchester extensions.
  - Used for what needs the computer itself: adapters for other agent CLIs (Aider, OpenCode, Ollama), and pipeline
    steps that run tools.
  - Installed only by the person, on a desktop. Never handed out by a team server, because a process is not
    sandboxed.

## What each side has to support

### Shared crate (`orchester-plugins`, this repository)

A real boundary: the desktop and the team server both host plugins, and the server builds this repository's crates
at a pinned revision.

It contains:
- manifest parsing and validation;
- the package format, its hash, and signature checks;
- the Luau host (`mlua`) with the sandbox, limits and lifecycle;
- the `orchester.*` API as traits the host implements: events, project data, HTTP, secrets, storage, UI and tools;
- the UI tree types and their JSON schema;
- the Luau type definitions;
- the conformance plugins that every host must pass.

### Desktop (`crates/workspace`, `crates/desktop-app`)

- **Running plugins.** Run plugins per open project, local and shared. Sync, download, verify, cache and start a
  shared project's plugins, and keep them running offline.
- **Installing and managing.** Install local plugins from a folder, archive or git URL. Offer repository plugins, and
  ask before their first start.
- **Screens.**
  - Project settings › Plugins: list, state, version, permissions, UI contributions, logs, turning on and off, member
    confirmation of a widened update.
  - App settings › Plugins: local plugins.
  - Both need designs in `orchester2.pen`.
- **UI kit and slots.**
  - The plugin UI kit in lurq, with theme roles and i18n from the plugin's locales.
  - The slots: sidebar Plugins group, task page panel, task list column, task actions, Overview card, generated
    Settings.
- **Connecting plugins to the app.**
  - Feed workspace and run events to plugins.
  - Expose plugin tools in the app's MCP server and, through profiles, to agents.
- **Secrets.** A member binds each of a plugin's secret settings to a secret in the OS secret store; the setting keeps
  only the binding.
- **Platforms.** Windows and macOS both use the vendored Luau build, which compiles with MSVC and clang. Hosting adds
  no `cfg` code.

### Team server (`orchester-platform`)

- Run `server_scripts` with the same crate.
- Install plugins, check them, build signed client packages and serve them through sync.
- Carry `emit` and `request` between the server part and the clients, including browsers on the admin's live
  channel, with the caller's verified person and device.
- Proxy HTTP for browser clients: check the allow-list, substitute secrets, record the call in the audit log.
- The admin pages drawn in `orchester-web.pen`: the list, installing, the plugin page with its tabs, approving a
  widened update, failures.
- The policy and the audit integration.

### Web admin (`orchester-platform`, `web/server` and `web/ui`)

- Luau compiled to WebAssembly with a JavaScript host implementing the same API. HTTP goes through the server's
  proxy, and no secret ever reaches the browser.
- The plugin UI kit in React, from the same JSON schema, and the slots in the admin's pages.
- The conformance plugins run in a headless browser through Playwright.

## Sharing code between the repositories

The public repository (`ivanzaida/orchester`) owns the plugin contract; the private `orchester-platform` uses it.

- **Rust.** The platform depends on `orchester-plugins` the way it already depends on `orchester-api` and
  `orchester-core`: a git dependency pinned to one commit (`rev`) in its `Cargo.toml`. The order of changes is the
  existing one: change the public repository first, push, then move the pin and run `cargo update`.
- **Local development without pushing.**
  - While public `master` stays unpushed (until the redesign release), the pin cannot move: Cargo resolves only
    pushed commits.
  - The platform therefore reads the public crates from the local checkout. It uses a `[patch]` in its own
    git-ignored `.cargo/config.toml`:

    ```toml
    [patch."https://github.com/ivanzaida/orchester"]
    orchester-api     = { path = "H:/projects/orchester/crates/api" }
    orchester-core    = { path = "H:/projects/orchester/crates/core" }
    orchester-plugins = { path = "H:/projects/orchester/crates/plugins" }
    ```

  - `Cargo.toml` keeps the honest `rev`. A clean build and the platform's CI use the pushed commit.
  - The platform's `tools/check.ps1` says plainly when the override is active, so a passing check is never mistaken
    for a passing build against the pin.
- **Files that are not Rust**: the UI tree's JSON schema, the Luau type definitions and the conformance plugins.
  - They live in `orchester-plugins`, embedded with `include_str!`.
  - A small binary writes them out:

    ```
    cargo run -p orchester-plugins --bin plugin-contract -- --out <dir>
    ```

  - The platform runs it through the same git dependency, so its copy always matches the pin, or the local override.
    There is no submodule and no hand copy.
  - The web admin generates its TypeScript types from the schema at build time (a dev dependency in `web/`, justified
    when added).
- **Not now:** publishing to crates.io (a release cycle for an internal contract); copying Rust code between the
  repositories.

## Phases

1. **Spike.** One plugin (Slack notifications) end to end:
   - on the desktop with `mlua` (events, HTTP with a secret handle, limits);
   - in a browser page with Luau in WASM.
   This settles the async model, the limits and the WASM build.
2. **Team server host and admin pages.**
   - `orchester-plugins`, server scripts, installing, signing, the admin pages as drawn, audit records.
   - No client scripts yet.
3. **Clients.**
   - Desktop client scripts, sync and cache, net events.
   - Project settings › Plugins with member confirmation.
4. **UI.**
   - The UI kit and slots, on the web admin and on the desktop.
   - The GitHub Issues board as the first plugin with UI.
5. **Local plugins and repository plugins** on the desktop. MCP tools for plugins.
6. **Configuration packs and import of Claude Code and Codex plugins.**
7. **Process plugins** for agent adapters and pipeline steps, when one is needed.

## Dependencies

- `mlua` with the `luau`, `vendored`, `async` and `serde` features: the Luau VM and a safe Rust API around it. There is
  no pure-Rust Luau; writing an interpreter is out of the question. `mlua` itself uses `unsafe` internally; our code
  stays free of it.
- In the web admin, a Luau WebAssembly build. Build it from Luau's sources with Emscripten in `orchester-platform`
  (to verify: size and the async bridge, in the spike) rather than take an unmaintained npm package.

## Open questions

1. **Phase order.** Is the team server first (phases 2–4 as listed), or the desktop's local plugins first, since
   most use is on the desktop?
2. **Plugin data in shared projects.** May server plugins write tasks in a shared project as the admin, or only as a
   dedicated "plugin" actor shown as such in history?
3. **Member confirmation.** Should a member be able to refuse a server plugin entirely, or only delay a widened
   update? FiveM gives clients no choice.
4. **Repository plugins.** Keep `orchester/plugins/` in repositories, or only server-installed and locally installed
   plugins?
5. **Browser clients.** Do the admin's pages run every client script of the project's plugins, or only those whose
   manifest contributes UI there?
6. **Signing beyond the server.** Should plugin authors sign their packages too (author identity through Orchester
   Cloud), or is the server's signature enough for now?
7. **Guard hooks.** v1 events only notify. Should a later API version add hooks that may refuse, under a timeout and
   a policy grant? Candidates: `task.before_create`, `run.before_start`, `run.before_merge`,
   `approval.before_decide`. They are what policy plugins need, but each one puts plugin code in the path of work.
8. **Replay.** Is there an event history a plugin can read from a cursor after downtime? The alternative is that
   plugins catch up through the API (`tasks.find` changed since a time). v1 proposes no replay.
9. **Agent detail.** Should `agent.command_finished` ever carry the command line, behind a separate grant? It can hold
   secrets passed on the command line, so the proposal is never.
