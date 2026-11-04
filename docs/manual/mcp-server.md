# Orchester's MCP server

Orchester exposes everything it does to automation through **MCP** (Model Context Protocol). This page covers the
app's own MCP server: how to connect, what it permits, the conventions every tool follows, the error format, and
the window tools (`app_state`, `navigate`, `set_theme`, `hold_reads`, `set_update_state`, `sign_in_state` and lurq's
`lurq_*` tools).

The domain tools are documented in [mcp-tools-setup.md](mcp-tools-setup.md) and [mcp-tools-work.md](mcp-tools-work.md).
Two other MCP surfaces exist and are documented separately: the tools agents get inside runs
([agent-run-tools.md](agent-run-tools.md)) and the Orchestrator chat's toolset ([orchestrator-chat.md](orchestrator-chat.md)).

## 1. Three MCP servers at a glance

| Server | Who connects | Name the client registers | Tools |
|---|---|---|---|
| **The app's MCP server** (this page) | Your own agent CLIs (Claude Code, Codex), QA and design-verification agents | `orchester` → `mcp__orchester__*` | `lurq_*` window tools, `app_state`, `navigate`, `set_theme`, `sign_in_state`, `hold_reads` and `set_update_state` (debug) and 80 domain tools |
| **The runner's relay**, one per agent invocation | Agents inside runs | `Orchester` (capital O) → `mcp__Orchester__*` in Claude Code, bare names in Codex | Run tools (`read_file`, `edit_file`, `run_check`, `start_qa_app`, …), `approve`, `ask_question` |
| **The Orchestrator chat's server**, one per chat turn | The project's Orchestrator chat agent | `orchester` | 60 domain tools + 23 chat-only tools (git, MCP servers, `init_repository`, `apply_proposal`) |

Agents inside runs **never** reach the app's server: Claude Code runs use `--strict-mcp-config`, Codex runs turn off
the user's `orchester` server, and neither inherits the token. Runs drive only QA copies of the app they start
themselves (`start_qa_app`, `call_run_app`).

## 2. Connecting

### Transport

- MCP **streamable HTTP** at `http://127.0.0.1:<port>/mcp` — IPv4 loopback only, HTTP/1.1.
- Supported protocol versions up to **2025-11-25**. (A 2026-07-28 `server/discover` probe is declined with HTTP 400 /
  JSON-RPC `-32022`; Claude Code then falls back to 2025-11-25.)
- At most 32 simultaneous connections; a 10-second header-read timeout (which also bounds keep-alive idling).

### Admission (checked on every request, before MCP reads it)

1. `Host` must be `127.0.0.1:<port>` or `localhost:<port>` with the server's own port (DNS-rebinding defence) →
   otherwise **HTTP 403**.
2. `Origin`, if sent, must be an `http(s)://` origin on `127.0.0.1`, `localhost` or `[::1]` → otherwise **HTTP 403**.
3. `Authorization: Bearer <token>` (compared in constant time) → otherwise **HTTP 401**.

The token is 64 hex digits (244 random bits). Rotating it ends every open session but keeps the port.

### Address and token by build

| Build | Address | Token | Discovery file |
|---|---|---|---|
| **Installed app** (release installer / `tools/install.ps1` / `tools/package-macos.sh`) | Fixed port from Settings › Automation, default **47821** (valid 1024–65535) | In the OS secret store: Windows Credential Manager target `orchester-app-mcp/<installation id>`; macOS login keychain service `orchester-app-mcp` | none |
| QA copy / test build with `ORCHESTER_APP_MCP_DISCOVERY=<folder>` (`tools/qa-app.ps1` sets `.tmp/qa/<task>/mcp/`) | Free port each start | New each start, memory only | `<folder>/<pid>.json` |
| Any other build (debug `tools/run.ps1`, `cargo run`) | Free port each start | New each start | Windows `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json`; macOS `~/Library/Application Support/orchester-app-mcp/<pid>.json` |

- The installed app started **with** `ORCHESTER_APP_MCP_DISCOVERY` set runs without its MCP server, so the token can
  never land in a readable file.
- If the fixed port is taken, the server does not listen and never falls back to another port: Settings shows
  *Port … taken* and the status bar *MCP port … taken*.
- The discovery file is JSON `{"version":1,"pid","app":"orchester-app","transport":"streamable-http","port","url","token"}`,
  readable only by the user, removed when the server stops (stale files of killed processes are cleaned up by the next
  server).

### Settings › Automation (person-only controls)

- **Let agents connect to Orchester** — turns listening on/off (on by default).
- **Use port** — moves the server to another port in place.
- **Show** (at most 30 s), **Copy** (clipboard cleared after 60 s), **Rotate token**.
- **Agents connected** — the client sessions since start (client name/version, last request), memory only.

These controls refuse synthetic input; only a person can use them.

### Registering a client

Claude Code (user scope — never `--scope project`, which would write the token into `.mcp.json`):

```bash
claude mcp add --transport http --scope user orchester http://127.0.0.1:47821/mcp --header "Authorization: Bearer <token>"
```

Codex (token from an environment variable):

```bash
codex mcp add orchester --url http://127.0.0.1:47821/mcp --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN
```

…then set `ORCHESTER_APP_MCP_TOKEN` persistently (`setx` on Windows, `launchctl setenv` on macOS), or put
`[mcp_servers.orchester]` with `url` and `http_headers = { Authorization = "Bearer <token>" }` in
`~/.codex/config.toml`. Settings › Automation shows the exact command. The server name must be lower-case
`orchester`: `Orchester` is the run relay's name.

## 3. What the server permits

### Scopes

Tools outside the granted scopes are **not listed**, and calling one answers `unknown tool: <name>`.

| Scope | Granted | Tools |
|---|---|---|
| Observe | always | lurq looking tools, `app_state`, `sign_in_state`, read-only domain tools |
| `data` | always | domain tools that change data (each still applies its own gates) |
| Interact | only with **window control** | lurq input tools, `navigate`, `set_theme`, `hold_reads`, `set_update_state` |
| Navigate | never | `lurq_navigate` (use `navigate`, which keeps the tabs' histories) |

### Window control

- Default: **on** in debug builds, **off** in release builds (including the installed app).
- Settings › Automation › *The window* › **Agents can control this window** stores a value that overrides the build
  default; it applies from the next start.
- `ORCHESTER_APP_MCP_WINDOW_CONTROL=on|1|true|off|0|false` overrides both for one run.
- `app_state` reports `mcp.window_control` and `mcp.source` (`build`, `stored`, `environment`).
- *Look at the window* is always on.

### Permission gates

Some actions belong to a person. Two **person-only switches** in Settings › Automation › *Approvals and permissions* widen what
automation may do; nothing over MCP or synthetic input can flip them:

| Switch | Off by default; when off… |
|---|---|
| **Let automation answer approvals** (AA) | `decide_approval`, `decide_plan`, `answer_question`, `set_reviewer`, `continue_run`'s retry/return/accept/resolve and `raise_limits`/`raise_step_budget`, `update_project_limits` raising or removing the daily budget, `complete_run` with `anyway`, and profiles that approve automatically or are set to Autopilot are refused as `automation_not_allowed`. Neither Autopilot nor the reviewer stands in for it. |
| **Let automation change agent permissions** (AP) | Every *widening* of what agents may do is refused as `person_only`: broader profile permissions (a profile set to Autopilot included), another reviewer, MCP servers added/widened/turned on, a CLI program not found on the search path, non-harmless environment variables, running a CLI on an API provider, another account, new command tools or tool program changes, publishing pipelines that are weaker or new, publishing instructions/skills/re-worded profiles, turning repository agent files on, moving a task to another pipeline. **Narrowing is always allowed.** |

Allowing Autopilot in a project (`set_allow_autopilot` true) is a person's alone, whatever these switches say: it is
always `person_only` over MCP. Turning it off and switching a run to Manual (`set_run_manual`) are narrowings, always
taken. Further person-only switches concern the Orchestrator chat (see [orchestrator-chat.md](orchestrator-chat.md)).

### The input gate (applied to every call, in order)

1. **Token shown** — while a secret is visible in the window (the MCP token in Settings › Automation, or a pairing
   code), `lurq_find*`, `lurq_inspect`, `lurq_read_tree`, `lurq_screenshot` and `lurq_wait` answer
   `{"code":"token_shown"}`.
2. **Person-only controls** — any lurq input tool is refused `{"code":"person_only"}` while a protected control is
   on screen: the person-only switches, the MCP server section, Accounts, or a *widening confirmation* (Connections,
   Agents, Tools, Secrets, Skills, Project settings, Pipelines, task form, Local integration). Synthetic input that
   decides on a person's behalf (approvals, questions, plans, retry/return/stop, *Accept … as it is*, raising or
   removing a run's limit or the project's daily budget) needs AA. Approvals, questions, plans and step decisions are
   then recorded as automation's; a raised limit records no answerer.

## 4. Conventions of the domain tools

- **Naming things.** A project by its id or its name (an ambiguous name → `project_ambiguous`; use the id). A task by
  its key (`KON-12`) or id. A run by its id or its number (`7` or `#7`). Profiles by id or role; pipelines,
  connections, instructions, tools, MCP servers and modules by id or name; skills by id, name or slug.
- **Strict schemas.** An unknown or mistyped argument is refused (`invalid_arguments`).
- **Annotations.** Each tool carries `readOnlyHint` (true for reads) and `openWorldHint: false`.
- **Same rules as the screens.** Every tool goes through the services the screens use: it follows the same rules,
  writes the same records and raises the same events. None holds a rule of its own.
- **Links.** Results carry `link` fields: the app's route paths. A person opens them with a click (the chat makes
  them clickable). `navigate` can open them too.
- **Cross-project reads.** List tools with an optional `project` read every project when it is left out; each item
  then names its project and `unreadable_projects` lists those that could not be read (`project_not_opened_yet` for a
  project this app version has not opened: list tools never upgrade a project's data).
- **No secrets.** No tool takes or returns a secret's value. Secrets are referred to as `NAME@environment`.
- **Optimistic locking.** `save_profile_draft`, `save_tool_draft` and `save_skill_draft` take `expected_revision`;
  a draft changed meanwhile is refused as `draft_changed`.
- **Automation is recorded.** Decisions, answers, task organization and completions made over MCP are recorded as
  *automation's* and shown so in the app.
- **Deleting** tasks, secrets and accounts is a person's alone; no tool deletes them.

## 5. Errors

A refusal is a tool error (`isError: true`) whose text is JSON:

```json
{ "error": { "code": "person_only", "message": "…", "reasons": ["…", "…"] } }
```

`code` is stable and safe to match on; `reasons` are the app's own explanations, in the words the app shows a person
(`message` is them joined by newlines).

| Origin | Error text |
|---|---|
| Domain tools | `{"code","message","reasons"}` |
| App tools (`app_state`, `navigate`, …) | `{"code","message"}`, plus `known` (valid names) for `unknown_route`, `unknown_tab`, `unknown_theme` |
| Input gate | `{"code","message"}` — `token_shown`, `person_only` |
| lurq tools | Plain text |
| HTTP guard | HTTP 401 / 403, plain text |

### Common codes

| Code | Meaning |
|---|---|
| `invalid_arguments` | Arguments do not match the schema (or the tool name is unknown). |
| `*_not_found` | `project_not_found`, `task_not_found`, `run_not_found`, `approval_not_found`, `question_not_found`, `plan_not_found`, `pipeline_not_found`, `profile_not_found`, `step_not_found`, `connection_not_found`, `instruction_not_found`, `skill_not_found`, `tool_not_found`, `mcp_server_not_found`, `account_not_found`, `pull_request_not_found`, `module_not_found`. |
| `project_ambiguous` | Several projects have that name; use the id. |
| `person_only` | Only a person may do this (see gates). |
| `automation_not_allowed` | *Let automation answer approvals* is off. |
| `sign_in_required` | The named project belongs to an Orchester Cloud account and nobody is signed in; a person signs in, in the window. |
| `interrupted` | Background work did not finish (e.g. the app is closing). |
| `runner_unavailable`, `runner_reply_lost` | The project's runner is not connected / its reply was lost. |
| `stop_run_first` | Closing a task while its run has not ended. |
| `invalid_organization` | Blank Blocked reason, a cycle of blocking tasks, … |
| `shared_task` | Not possible in a shared project. |
| `draft_changed`, `stale_draft`, `no_draft` | Draft conflicts. |
| `pipeline_not_published` | The pipeline has no published version. |
| `task_not_editable`, `module_changed`, `invalid_module`, `project_data` | Project data errors. |

Runner rejections arrive with their own name, e.g. `task_not_queued`, `task_group_waiting`, `preflight`,
`run_not_running`, `run_not_paused`, `run_not_interrupted`, `run_not_waiting`, `run_ended`, `run_not_ended`,
`run_merged`, `worktree_in_merge`, `worktree_removed`, `limit_not_raised`, `limit_not_above_used`,
`daily_budget_used`, `no_agent_slot`, `plan_not_awaited`, `tool_approval_not_awaited`, `tool_approval_answered`,
`question_not_awaited`, `question_answered`, `invalid_answer`, `shutting_down`, `busy`. Tool-specific codes are listed
with each tool.

## 6. App tools

### `app_state` — read-only, always listed, no arguments

Where the window is:

- `theme`: `{setting: dark|light|system, mode}`.
- `mcp`: `{window_control: bool, source: build|stored|environment}`.
- `window`: `{width, height}` in logical pixels and `scale_factor` (`null` before the first layout).
- `route` of the active tab (`{name, path, params}` — exactly what `navigate` takes back), `history`
  (`{can_back, can_forward}`), and `tabs`: `[{id, active, route, project?}]`. For a project screen, `project` is
  `{name, path, state, runner}` where `state` is `open`, `unknown`, `folder_missing`, `unreadable`, `failed` (or
  `null` while it opens) and `runner` is `{state: starting|online|reconnecting|stopping|stopped|failed, cause:
  missing_executable|already_served|start_failed|lost|null}`.
- A window that could not open its data has no tabs and reports `startup`: `already_open`, `newer_data`,
  `update_failed`, `unreadable`, `not_writable`, `program_folder_unknown`.

### `navigate` — needs window control

Takes the **active tab** to a route as a new history entry (mouse back, Alt+← / Cmd+[ return). Answers
`{"route": {name, path, params}}`. All parameters are strings; `route` is required; `project` is the project's id
(as `app_state` reports it) and is required for every project screen. A parameter the screen does not take is
refused (`unexpected_param`). A `tab` left out is the one the screen opens on. Opening a page or an in-place question
is navigation; answering it stays a person's.

| `route` | Parameters | Allowed values |
|---|---|---|
| `all_projects` | — | |
| `add_project` | — | |
| `settings` | `tab`, and per tab the parameters below | `automation` (default), `general`, `notifications`, `accounts`, `orchester` (Orchester Cloud), `connect` (Connect a server), `invite` (an invite's landing) |
| `overview` | `project`, `tab` | `running` (default), `needs_you`, `queued`, `failed` |
| `orchestrator` | `project`, `chat` | a conversation id, or `new`; omitted = latest |
| `orchestrator_settings` | `project` | |
| `tasks` | `project`, `tab`, `q`, `highlight` | tab `active` (default), `needs_you`, `backlog`, `done`, `all`; `q` search; `highlight` a task key (as after Create). Shows the list or the board as last chosen for the project |
| `new_task` | `project` | |
| `edit_task` | `project`, `key` (req.) | |
| `task` | `project`, `key` (req.), `tab`, `view`, `file`, `from` | tab `summary` (default), `conversation`, `changes`; `view` `memory` (the task's memory, from Summary) or `local-integration` (Local integration, from Changes); `file` a changed file on Changes; `from` `checkpoint` (default) or `worktree`, only with `file` |
| `modules` | `project`, `tab`, `module`, `view` | without `module`: the list on tab `open` (default) or `closed`; `module` `new` (New module) or a module's id (its page on tab `summary`, the default, or `burndown`); `view` `edit` with a module's id opens Edit module |
| `merge_queue` | `project` | |
| `pull_requests` | `project`, `tab`, `number` | tab `open` (default), `needs_review`, `agent_working`, `merged`, `closed`; with `number` (`142` or `#142`), one PR on *Files changed*, or with `tab: feedback` its *Agent feedback* |
| `usage` | `project`, `query`, `view`, `period` | without `project`: all projects (`query` only); `query` is the page's link query (e.g. `from=2026-10-01&to=2026-10-15&group=task`, as `get_usage`'s `link`); `view` `usage` (default) or `delivery`; `period` (delivery) `30d` (default), `90d`, `365d` |
| `pipelines` | `project` | The list; a pipeline's page opens from its row on its Steps, Settings and Versions tabs |
| `agents` | `project` | |
| `instructions` | `project` | |
| `memory` | `project`, `tab` | `waiting`, `active`, `deprecated`, `superseded`, `rejected`; omitted = the page chooses; `settings` opens Project settings › Context, where the memory settings are |
| `skills` | `project`, `skill` | a skill id, `new` or `import`; omitted = library |
| `tools` | `project`, `tool`, `tab` | `tool`: id or `new` (a command draft); omitted = library. With a tool id, tab `configuration` (default), `inputs_outputs`, `access`, `usage_log` |
| `project_settings` | `project`, `tab`, `view` | tab `general` (default), `runs`, `orchestrator`, `context`, `connections`, `environment` (`secrets` too), `sharing`; with tab `runs`, `view: rules` opens *Approved for every run* |
| `connections` | `project`, `selected` | Project settings › Connections; `selected` the id of the connection whose page opens |
| `secrets` | `project` | Project settings › Environment |
| `new_connection` | `project`, `kind` | `agent_provider` (default, Add provider) or `mcp_server` (Add MCP server), as a page of Project settings › Connections |
| `edit_connection` | `project`, `connection` (req.) | An MCP server's id (its form), or an agent CLI connection's id (its account) |
| `files` | `project`, `file`, `at`, `line` | `file` a path in the project (the tree alone when omitted); `at` `checkout` (default), `base`, `run:<run id>`, `rev:<commit>`; `line` `12` or `12-20` |
| `import_resources` | `project`, `tab` | `all` (default), `pipelines`, `agents`, `instructions`, `tools`, `secrets` |

Settings' extra parameters, by tab (any other is `unexpected_param`):

| `tab` | Parameters | Allowed values |
|---|---|---|
| `automation` | `ask` | `rotate-token` (asks *Rotate the token?* in place) |
| `accounts` | `account`, `panel` | `account`: an account's id or name, or `claude-default` / `codex-default` (its page); `panel`: `add` (Add account, without `account`), or with `account` `sign_in`, `rename` or `remove` |
| `orchester` | `server`, `ask`, `invite` | `server`: a team server's id, name or host (its page); `ask`: `sign-out` (the cloud account, or with `server` that server) or `decline` with `invite` (an invite's id) |
| `connect` | `server`, `step` | `server`: the server whose address Connect a server starts at; `step`: `address`, `compare`, `join` or `username` (a server with its own certificate stays at `compare`, which only a person answers) |
| `invite` | `server`, `project` | `server` and the invite's project slug there |

Errors: `unknown_route` (with the known names), `missing_param`, `unexpected_param`, `invalid_param`, `unknown_tab`
(with the screen's tabs), `not_a_string`, `not_an_object`, `not_ready`, `no_session`.

### `set_theme` — needs window control

`theme` (req.): `dark`, `light` or `system` (the OS appearance when the app started). Stores the choice and redraws.
Answers `{setting, mode}`. Errors: `unknown_theme` (with `known`), `not_saved` (nothing changed), argument errors.

### `hold_reads` — debug builds only, needs window control

`held` (boolean, req.). `true` holds every read of a project's tasks, limits and agent profiles that starts from then
on, so a screen reading them for the first time stays in its loading state (for capturing loading states in design
verification); data already on screen stays. `false` releases them. Answers `{held}`.

### `set_update_state` — debug builds only, needs window control

Shows an update state in place of this copy's real one, for seeing every state of Settings › General's Updates, the status
bar's update item and the ready notification in a QA copy (which itself always reports `not_this_copy`). Takes the
JSON `update_state` answers: `state` with that state's fields, and `updated` (`{version, notes}` of the update this
start finished, for *Updated to* and *What's new*, or `null`); `running` and `percent` are ignored. `{"real": true}`
shows the real state again. The state lives in memory only and is gone at the next start; nothing is checked,
downloaded, written to `orchester.db` or installed for it, and *Restart to update* / *Install now* pressed in it answer
*Nothing installs: this update state was set for testing.* Answers what `update_state` answers, with
`set_for_testing`. Errors: `invalid_param` (an unknown state, step or reason, or a version that is not one),
`missing_param`, `not_a_string`, `not_a_bool`.

### `sign_in_state` — read-only, always answers

`{"signed_in": true}`, or `{"signed_in": false, "reason": …}` with why the last sign-in on this computer ended:
`revoked`, `password_reset`, `account_deleted`, `signed_out`, `two_factor` (sign in again with a code), `other`, or
`null` on a computer never signed in. Never returns an email or a token.

## 7. lurq window tools

The window tools come from the lurq UI toolkit (0.51.2), hosted in-process by the app's server.

**Conventions**

- Coordinates and sizes are pixels of the **last `lurq_screenshot` image** (physical pixels).
- Prefer `lurq_inspect` (optionally with `query`/`role`) → `lurq_act` by ref → `lurq_inspect` to verify; use
  screenshots only when semantic inspection is not enough.
- Refs (`ref_N`) are replaced by each `lurq_inspect` or `lurq_read_tree` of the same window; stale refs fail.
- Every window-touching tool takes `window` (an id or name from `lurq_windows`; default `"main"`; `"focused"` for the
  focused window). Ref-based calls need no `window`.
- Sensitive text is covered by a grey bar in screenshots; masked inputs expose only their mask (`masked=true`).
- Input arrives as **synthetic** input — the app treats it differently from a person's (see the input gate).
- Reply deadlines: `lurq_wait` = min(`timeout_ms`, 115 000) + 5 000 ms; screenshots 30 s; others 15 s. A call that
  timed out before it started is dropped.

### Looking tools (always available)

| Tool | Parameters | Result |
|---|---|---|
| `lurq_screenshot` | `window`?; `region`? `{x, y, width, height}`; `ref`? (crop to an element) | PNG image. |
| `lurq_read_tree` | `window`?; `filter` `interactive` (default) \| `all`; `max_depth`?; `max_chars` (default 30 000); `max_items` (canvas items per canvas, default 200) | Indented outline with bounds `@x,y WxH` and refs `[ref_N]`. |
| `lurq_inspect` | `window`?; `query`? (substring of name, id or class); `role`? (exact, e.g. `button`, `textbox`); `max_depth` (default 12); `max_nodes` (default 500) | Structured nodes: role, name, state, bounds, ref, allowed `actions`. Replaces the window's refs. |
| `lurq_find` | `query` (req.) | Searches the refs from the last inspect/read_tree of each window. |
| `lurq_find_by_id` | `id` (req.); `window`? | A fresh ref for the element with that id. |
| `lurq_find_by_class` | `class` (req.); `window`? | Every element with that class, fresh refs. |
| `lurq_windows` | — | Windows: id, name, title, kind, focus, minimized/maximized/full screen, size, scale factor, held input. |
| `lurq_menu` | — | The application menu: item ids, enabled state, accelerators. |
| `lurq_wait` | `window`?; `frames`? (wait that many presented frames, else until render idle); `timeout_ms` (default 10 000) | `done`. |
| `lurq_logs` | `lines` (default 200); `filter`? | The app's log lines, newest last. |
| `lurq_profile_start` | `max_samples` 1–240 | Starts a CPU profiling session, returns `profile_<n>` (needs the `perf_profile` build feature; at most 8 sessions). |
| `lurq_profile_read` | `id`? | A session's snapshot, or availability and limits without `id`. |
| `lurq_profile_end` | `id` (req.) | Ends and exports that session. |

### Input tools (need window control)

| Tool | Parameters | Notes |
|---|---|---|
| `lurq_act` | `ref` (req.); `action` (req.) `invoke` (click its centre) \| `hover` | Re-checks the element's live role, name and hittability. |
| `lurq_set_value` | `ref` (req.); `value` (req.): string for text inputs, boolean for checkboxes, number for sliders, label or index for selects | No keystroke simulation. |
| `lurq_interact` | `action` (req.): `click`, `double_click`, `move`, `drag`, `wheel`, `key`, `type`, `scroll_to`, `request_close`, `menu_activate`, `press`, `release`, `key_down`, `key_up`, `paste`, `drag_files`, `drop_files`, `file_drag_leave`. Target: `ref` or `x`/`y`. Plus `id` (menu item), `to_x`/`to_y`/`to_ref` (drag end), `button` (`left`, `right`, `middle`), `delta_x`/`delta_y` (wheel), `key` (e.g. `Enter`, `Tab`, `Escape`, `ArrowDown`), `text`, `files`, `paths`, `modifiers` (`shift`, `ctrl`, `alt`, `meta`), `window` | Paste never reads the OS clipboard. Held input (`press`, `key_down`, …) needs a session-based connection and is released when the session ends or the window loses focus. |
| `lurq_resize` | `window`?; `width`, `height` (req.) | Restores a minimized/maximized window first; answers the size taken. |

`lurq_navigate`, `lurq_file_dialogs` and `lurq_file_dialog_respond` exist in lurq but are never available in
Orchester.

## 8. QA copies

To drive a throwaway copy of the app instead of the installed one:

```bash
powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario>
```

It prints the pid and the discovery folder (`.tmp/qa/<task>/mcp/`). Inside runs, agents use `start_qa_app` and
`call_run_app` instead ([agent-run-tools.md](agent-run-tools.md)).
