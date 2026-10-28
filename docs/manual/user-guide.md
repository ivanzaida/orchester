# User guide

How to use the Orchester app: the window, every screen, and the workflows from adding a project to merging a run.
Terms are defined in [concepts.md](concepts.md); settings are in [settings.md](settings.md). Quoted labels are the
app's own English text.

## 1. The window

### Title bar

From left to right: the **app menu** (Orchester mark — *New tab*, *All projects*, *Settings*, *Quit Orchester*), the
**tab strip** with its *New tab* (+) button, a drag area (double-click maximizes), the **"N need you" pill**, and the
window controls (Windows: right; macOS: traffic lights on the left). The OS window title follows the active tab:
"<tab> — Orchester".

### Tabs

Each tab is an independent place with its own Back/Forward history (up to 50 entries). A new tab opens *All
projects*. A project tab reads "<project> · <screen>"; a task tab shows its key.

- An **attention mark** (amber ✋ and a count) shows that something in that project waits for you; a red alert marks
  a runner offline or tasks that couldn't be read.
- A **dot** marks a tab whose task form holds unsaved edits.
- Middle-click closes a tab. When tabs overflow, a **tab list** button appears (Ctrl+Shift+A / ⌘⇧A) with search once
  more than 8 tabs are open.

### Keyboard and mouse

| Windows | macOS | Action |
|---|---|---|
| Ctrl+T | ⌘T | New tab |
| Ctrl+W | ⌘W | Close the active tab |
| Ctrl+Tab / Ctrl+Shift+Tab | same | Next / previous tab |
| Alt+← / Alt+→ | ⌘[ / ⌘] | Back / Forward in the active tab |
| Ctrl+Shift+A | ⌘⇧A | Open the tab list (when tabs overflow) |
| Mouse back/forward buttons | same | Back / Forward |

**Links** (sidebar entries, breadcrumbs, rows that open a page, *Open …* buttons, links in chats):

- Plain click, or Enter/Space → opens in the active tab.
- **Ctrl+click (⌘-click) or middle-click** → a new tab to the right; the current tab stays active.
- **Ctrl+Shift+click (⌘⇧-click)** → a new tab that becomes active.

An item of *another* project (a Needs-you item, a notification, a chat link) never replaces your current tab: a tab
of that project is used or opened, and Back there returns you once to where you came from.

### Sidebar

At the top, the **project switcher**: recent projects (with *Open in new tab*), *All projects*, *Open folder…* and
*Add project*. Then the project's screens:

- **Overview, Orchestrator, Tasks, Modules, Merge queue, Pull requests, Usage, Files**
- *Configure*: **Pipelines, Agents, Instructions, Memory, Skills, Tools, Secrets, Connections**
- A **Running** section while runs work: "<key> · <step>" with the step's live agent time; a hand when the run waits
  for you.
- At the bottom: **All projects**, **Settings**, **Project settings**.

Counts show beside Tasks, Merge queue and Memory (records waiting for approval).

### Status bar

It belongs to the active tab's project. Left: project name, **runner** state (*Runner online*, *Reconnecting…*,
*Runner offline* with why; click for the runner log; *Restart runner* when it can't start), and an **activity slot**
(operations under way, the newest failure, the latest event). Right:

- **Agents** N/M (agents at work / slots), **Queued** N, **N need you**, tokens "used / budget today".
- **Plan limits** per CLI ("Claude 42% · Codex 65%"); click for a card per account with the 5-hour and weekly
  windows and their resets.
- **Approvals waiting** with *Review*; **Auto-approving** with *Stop*.
- **Team server** state, **Orchester Cloud · offline** when the cloud can't be reached.
- **MCP** address and state (click for a popover with connected agents and window control); *Agents can control this
  window* while that is on; the version.

### Needs you panel

The title-bar pill counts everything that waits for you in every project your tabs show. Pressing it opens the
**Needs you panel** (oldest first, grouped by project). Approvals and questions with suggested answers **expand in
place** — answer them right there. Other items open their place (plan or decision → the task's Summary; question →
Conversation; memory → Memory › Waiting for approval; merge → Merge queue). Esc closes.

### Notifications

The installed app shows OS notifications (Windows toasts, macOS UserNotifications) in three groups — *Needs you*,
*Stopped*, *Finished and info* — only while the window isn't focused or the place isn't on screen. Events of a group
within 30 s coalesce; a click opens the exact place. The taskbar overlay / Dock badge shows the Needs-you count.
Configure them in Settings › Automation › Notifications.

### Closing the app

Orchester has **no background mode**: closing the window stops every runner and agent. With agents at work, a
closing panel replaces the page:

- **Wait for agents to save** (Enter) — no new steps start; each agent continues to its next checkpoint, runs are
  marked Paused, then Orchester closes.
- **Quit now** — stops everything at once; work after each run's last checkpoint is not recorded; uncommitted changes
  stay in the worktree.
- **Cancel** (Esc).

After *Quit now* or a crash, the project's Overview and the status bar show "N runs were interrupted when Orchester
closed" → *Review runs* → **Continue all N from checkpoints** or **Don't continue**.

### Startup screens

- **Orchester is already open** — one Orchester opens a data folder at a time: *Switch to that window* (Windows) or
  *Close this window*.
- **Orchester can't open / read / update / write its data** — with *Open log* and *Show data folder*.

## 2. Signing in

Until this computer is signed in to **Orchester Cloud**, the window shows only the sign-in gate ("Sign in to start").

1. Enter *Email* and *Password*, press **Sign in**.
2. With two-factor on: enter the 6-digit code and **Verify**, or *Use a recovery code instead*.
3. *Create an account* and *Forgot password?* open the cloud's pages in your browser.

Signing in needs the internet once; after that Orchester works offline and re-checks when it can. While signed out,
projects and running work stay as they are, and MCP clients get "sign in required". *Advanced · cloud address* sets a
test cloud. Only a person can sign in, never over MCP.

## 3. Projects

### All projects (`/`)

The projects Orchester knows on this computer: status (runner state, *Folder missing*, *Can't be opened*), active
runs, needs-you count, last opened. Actions:

- **Add project**, **Open project folder…** (re-open a project folder, e.g. one removed from the list).
- × on a row → **Remove from list** (asked in place). Nothing is deleted; the folder and `.orchester` stay. Unavailable
  while agents work in it.
- The **Usage** tab shows every project's usage.
- On first launch, a **Welcome** page offers *Create your first project*.

Invites to team-server projects also appear here.

### Adding a project (`/add-project`)

1. **Project name**, then **Project folder › Choose folder…** — the top folder of an existing git repository. Nothing
   is cloned or uploaded.
2. If the folder is not a repository, or has no commit: **Create a git repository here** / **Make the first commit**
   ("Commit the folder's files as the first commit" or "Start with an empty first commit").
3. **GitHub** is detected from the `origin` remote through `gh` (optional): *Detected*, *Local only*, *gh not found*,
   *Not signed in*, *Wrong account* (with *Use this account*), *Missing scope*… Without GitHub, pull request steps are
   unavailable.
4. **Create project & continue** — creates `.orchester/` and opens the Overview.

Over MCP: `add_project`.

### Overview (`/project/<id>`)

"What is moving in <project> and what needs you":

- Metrics: **Running now**, **Need your input**, **Completed today**, **Tokens today** (against the daily budget).
- A tasks panel with tabs **Running · Needs you · Queued · Failed**.
- A **Needs you** column with a card per waiting item (Approval, Question, Plan review, Decision, Stopped at limit,
  Interrupted, Ready to merge, Ready to complete, Conflicts need you, Delivery blocked, Memory…) and its action.
- **Recent activity**, notices (blocked tasks, unreadable runs, interrupted runs to recover), *Import resources* and
  *New task*.
- On an empty project: a **Ready to run** checklist (agents, pipeline, connection).

## 4. Setting a project up

The order that works: **connection → agent profiles → pipeline → task**.

### Connections (`/project/<id>/connections`)

Tabs: All, Agent providers, GitHub, MCP servers. Selecting a row opens its inspector (status, *Test connection*,
*Refresh models*, *Edit connection*, *Check again*).

**Add an agent CLI connection** (*Add connection › Agent provider*):

1. Install the CLI and sign in yourself (`claude auth login`, `codex login`). Optionally add more **accounts** in
   Settings › Accounts.
2. *Kind*: **Codex CLI** or **Claude Code CLI** (native API kinds are locked in this version).
3. *Name*; *Program* (the copy found on the PATH); the check runs automatically ("<program> <version> · signed in").
4. *Account*: Default or one of yours.
5. *Runs on*: **Its own sign-in** or **An API provider** (base URL, API key secret, models — person-only).
6. **Save connection**.

**Add an MCP server** (*Add connection › MCP server*): a name, then a local *program* (with arguments, working folder
and environment — `${secret:NAME@environment}` for secrets) or a remote *URL* (with authorization), and the *Tools that
run without asking*. Turning a server on and widening it is person-only. A server reaches only agents of profiles that
choose it.

**GitHub**: detected at setup; the inspector offers *Check again* and *Use this account*. Adding a GitHub connection in
the window is not built yet.

Over MCP: `add_cli_connection`, `set_api_provider`, `check_connection`, `list_models`, `set_connection_account`,
`add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`, `remove_mcp_server`.

### Agents hub

One **Agents** page with tabs **Profiles**, **Instructions**, **Skills**, **Tools** and **Memory**, each on its own
route (`/project/<id>/agents`, `…/instructions`, `…/skills`, `…/tools`, `…/memory`); the sidebar entries open the hub at
their tab. Each item opens as a sub-page with a back crumb. Profiles, instructions, skills and tools **autosave** their
draft; **Discard draft** returns to the published version and **Remove** deletes the item, both asked in place. Removing
something in use lists who uses it and offers **Remove and reset**, which clears it from every user (a pipeline step
loses its profile, a profile loses that skill).

**Profiles** (`/project/<id>/agents`):

1. **New agent** (needs a connection).
2. The profile page: *Profile name* (role), *Responsibility*, *Role instructions*, *Project connection*, *Model* (the
   CLI's model ids, or *Other model id…*), *Reasoning effort* (the efforts the model lists).
3. Permissions: repository access, extra writable folders, commands, network, MCP servers, approvals. Options the CLI
   can't enforce are disabled with the reason; *Connection enforcement* explains each mechanism. A widening change is
   asked in place at its field before it is saved (person-only).
4. **Publish v1**. *Also in this profile's prompts* lists the rules and skills that apply; *Used in pipelines* lists the
   steps that run it.

### Pipelines (`/project/<id>/pipelines`)

1. **New pipeline**: *Name* and *Start from* — **Starter · Feature delivery**, **Copy a pipeline** or **Empty
   pipeline** → *Create draft v1*.
2. In the editor (a graph of steps): **Add step**, then in the step inspector choose the agent profile or command
   tool, *Runs after*, *Wait for your approval*, *When changes are requested* (failure route, *Maximum retries*), *Step
   budget*, *Repository access*, *Model for this step*, step permissions, *Merge the base branch when it moves*, and
   the step's instructions.
3. Optional: the **Deliver step** (mode, merge method, integrator profile, re-check) and **Pipeline settings** (*What a
   run delivers*, sources).
4. **Save draft**, then **Publish vN**. The publish review shows *Published now* against *After publishing*; a weaker
   version asks for your approval. A pipeline whose profile only has a draft can't publish. *Version history* compares
   and restores versions.

### Instructions, Skills, Tools, Memory (Agents hub tabs), Secrets

- **Instructions** — one list in prompt order: agent role, project rules, pipeline, step, task, grouped per role,
  pipeline and step. **New instruction** chooses its scope (Project, Agent role, Pipeline or Step). Each rule has a
  target (all agents, roles, profiles), *Inject at* (start, resume, both), a Markdown text with `{{variables}}`;
  the side panel switches between *Prompt preview* and *Variables*. Drafts autosave; **Publish vN** makes new runs use
  it. *Import from repository* turns repository files into drafts.
- **Skills** — grouped by scope, with *Found in this repository*; description, `SKILL.md` body and reference files,
  scope. **Import from repository** reads `AGENTS.md`, `.claude/skills`, `.agents/skills` (the same import page as
  Instructions). Drafts save at once; publish to reach runs.
- **Tools** — **Add tool › Command**: program, arguments (one per line), timeout, exit codes that pass, output kept,
  environment variables. A tool's page has tabs *Configuration*, *Access*, *Inputs & outputs* and *Usage log*.
  Drafts autosave; **Publish vN**. HTTP, database, MCP-import and *Create pull request* tool kinds are locked.
- **Memory** — status pills **Waiting** (first, and where the tab opens while proposals wait), **Active**,
  **Deprecated**, **Superseded**, **Rejected** (with the withdrawn ones), then **Settings**. Search and filter by type,
  author and tag. Approve, edit or reject proposals on their review page; a row's hover actions edit, pin, deprecate,
  restore or delete records; a record opens as its own page; **New record**. *Settings* chooses how task and project
  memory save and how much each agent gets. Memory changes are person-only.
- **Secrets** — **Add secret** (name, environment, kind, value — kept in the OS keychain), **Replace value**,
  **Archive secret** (removes the value). The *Usage* tab shows what uses each secret.

### Import resources (`/project/<id>/import`)

Copy pipelines, agent profiles, instructions and tools from another project as **independent local drafts**:
*Select resources* → *Resolve dependencies* (new copy, rename, reuse a local one, leave unresolved) → *Create drafts*.
Connections and secrets are never copied; importing never grants access.

### Project settings

General (GitHub, repository agent files, base-moved notices, auto mode, base branch), Limits, Environment, Sources,
Code index, Sharing — see [settings.md](settings.md#2-project-settings).

## 5. Tasks

### Tasks list (`/project/<id>/tasks`)

Tabs **All · Needs you · Backlog · Todo · In progress · In review · Done** (Canceled tasks appear under All). Search
(title, key, brief), *Pipeline* and *Module* filters, *Sort* (manual order, newest, title, workflow status,
priority — sorting never changes the run queue). Columns: ID, task and tags, pipeline, priority, workflow/run state,
dependency/readiness ("Ready to queue", "Needs a brief", "Agent at work", "Waiting for you", "Raise the limit to
continue", "Merged into main", …). A row opens the task on *Details*.

Header actions: **Hold new starts** / *Release*, **New task**. Each row's *More actions*: Mark as done, Cancel task,
Reopen task, Delete task….

### New task / Edit task

1. **Brief**: *Task title*, *Describe the outcome*, *Acceptance criteria* (Enter adds one, Backspace in an empty one
   removes it, Alt+↑/↓ moves it).
2. **Context › Sources**: folders or files outside the worktree this task's agents may read without asking.
3. **Pipeline** (published only) with its *Execution plan* and approval gates.
4. **Module**, **Limits** (*Run budget*, *Time limit* — empty means no limit), **Story points** (*Automatic* or a
   value), **Merging** (*Don't merge automatically*).
5. **Create** (saves a draft; run it later with *Run*) or **Create & run** (queues and starts the first run, then opens
   the Conversation).

Unsaved edits are kept on this computer and restored. If someone saved a newer version meanwhile, you choose per field.
A task that already ran keeps its runs' briefs; edits apply to future runs.

### Task page (`/project/<id>/tasks/<KEY>/<tab>`)

**Header**: breadcrumb, title, run status chip, story points, run line ("Run 128 · attempt 2 · Feature delivery v3 ·
started 16:20"), Module select, and actions: *Edit*, *Remove worktree*, *Return to a step*, run controls, *Run again*,
*More actions*. Locked actions say why in one line under them.

| Control | When | Effect |
|---|---|---|
| **Run** | Draft with every detail | Queues the task and starts its first run |
| **Start run** | Queued, run not started | Asks the runner to start it (never a second run) |
| **Pause run** | An agent works | Pauses at the next checkpoint |
| **Resume run** | Paused, or stopped by the daily budget | Continues in the same worktree |
| **Continue run** | Interrupted | Continues from the last checkpoint |
| **Cancel run** | Until the run ends | Cancels it |
| **Run again** | Failed, canceled, or interrupted with its worktree removed | In-place confirmation, then a new run from step 1 in a new worktree |

**Tabs** (a task opens on *Details*):

- **Details** — the requirements (*At task creation*, *Run #N · at start*, *Current task*), the run setup (pipeline,
  limits) and task organization (status, priority, tags, depends on).
- **Conversation** — the latest run as a live chat: agents' messages with tool groups ("Read 4 files · ran cargo
  test"), step dividers, returns and restarts, result cards, decision cards (questions, plans, approvals), and while an
  agent works "<agent> is working" with **Stop turn**. Scrolling up stops following; *Jump to latest*.
- **Changes** — the diff of the run: *Checkpointed* (base → latest checkpoint) and *Not checkpointed* (the worktree
  beyond it). *Open file* opens the code viewer.
- **Local integration** — the guided integration into your checkout (see §7).
- **Summary** — where the task stands, one progress row per step, brief and criteria, *Runs of this task*, *This run*
  / *Run details*, and every **decision card** (plan review, raise the limit, retries, ready to merge, complete).
- **Artifacts** — empty in this version (a report run's *Report* shows on the Summary).
- **Memory** — the task's memory records and what the next agent receives.

### The composer

One box under the Conversation. *Attach files* (images, PDFs, text; paste or drop). Enter sends the run state's way;
Ctrl+Enter (⌘Enter) the second way; Shift+Enter a new line. Drafts persist across tabs and runs.

| Run state | Primary send | Other ways |
|---|---|---|
| Agent working | **Send now** (at its next tool call) or **Send after this turn** | *Stop turn and send* |
| A question waits | **Send answer** | |
| A plan waits | **Request changes** | |
| Ready for review | **Send follow-up** (the agent continues its session; the run is ready again after) | |
| Failed / waits after retries | **Continue with instructions** | |
| Paused | **Resume and send** | *Send without resuming* |
| Interrupted | **Continue run and send** | *Send without continuing* |
| Stopped at its limit | **Raise the limit and continue** (opens the Summary's card) | |
| Not started | **Send when the run starts** | |
| Ended | **Run again with this message** | |

**Stop turn** (no confirmation) ends the agent's turn, commits its work, and the agent continues with your next
message; the run is not paused.

### Closing and deleting tasks

*More actions*: **Mark as done**, **Cancel task**, **Reopen task**, **Delete task…**. Closing a task whose run hasn't
ended asks in place ("Stop the run of <task>?"). *Delete task…* lists everything that goes (runs, conversation, memory,
approval rules, links); *Also remove work that is not merged* is optional. Deleting is person-only and not available
in shared projects.

### Modules (`/project/<id>/modules`)

**New module** (name, description, start and target dates). The list shows dates, state, task count and progress. A
module page shows progress, a **Burndown** (remaining work per day, ideal line, forecast 50%/85%, added/removed) and
its tasks. Put a task in a module from the task form or the header's *Module* select.

## 6. Following and steering a run

1. After *Run*, the **Conversation** shows the agents live; the **Summary** shows step rows ("Complete · attempt 1 ·
   41k tokens · 6 min") and *This run* (pipeline, branch, base, checkpoint, model, account, budget, cached input, time
   limit, active agent time, permissions, worktree).
2. **Approvals** — answer on the card or in the Needs you panel: *Approve once*, *Deny*, *Deny with a message*, or
   more widely (*Approve similar in this run*, *Approve everything in this run*). To stop being asked: the profile's
   *Approve automatically*, or auto mode.
3. **Questions** — pick options or write your own answer, then *Send answer*; a plain question is answered from the
   composer.
4. **Plans** — on the Summary's plan review: read plan vN, then **Approve plan & continue**, or write *Changes the plan
   needs* and **Request changes**. Earlier versions are in the *Plan version* select.

### Continuing a stopped run

| How it stopped | What to do |
|---|---|
| Paused | *Resume run* (or the composer's *Resume and send*) |
| Interrupted (crash, runner lost, *Quit now*) | *Continue run* — from the last checkpoint; once its worktree is removed only *Run again* is left |
| Token or time limit | Summary › *Raise the limit*: enter a new budget/time → *Continue with N tokens / N min*, or *Remove the limit* |
| Step budget | *Raise the step budget* → *Continue* or *Remove the limit* |
| Daily project budget | *Resume run* once the day has room, or raise the daily budget |
| Account plan limit | *Wait for <account>, then continue* (resumes by itself at the reset) or *Continue now on another account* |
| Retries used up / broke again / unverified / unchanged | *Decide how the run continues*: *Allow 1 retry & continue*, *Accept <step> as it is*, *Run <step> again*, *Return to <step> with note*, *Stop and fail the run* |
| Failed | Composer *Continue with instructions*, or *Run again* |
| Ended, want more work | *Return to a step* with a note, or composer *Send follow-up* |

Over MCP: `pause_run`, `resume_run`, `cancel_run`, `continue_run`, `send_message`, `run_task`.

## 7. Merging

### Pipeline without a Deliver step

The Summary shows **Ready for your review** with *What changed* and *Verification*:

- **Merge into <branch>** — merges directly (in a temporary worktree, brought in as a fast-forward). On success the
  run's worktrees and merged branches are removed and the task becomes Done. If git refuses ("Not merged into
  <branch>"): **Resolve in editor** (the base is merged into the run's worktree and opened in VS Code or
  `ORCHESTER_EDITOR`; then *Continue* or *Abort the merge*), **Ask an agent to resolve**, or **Local integration**.
- **Create pull request** (*Open as draft* on by default) — pushes the branch and opens a PR with `gh`; needs `gh`
  signed in as the expected account with the `repo` scope.

### Pipeline with a Deliver step — the merge queue (`/project/<id>/merge-queue`)

Runs that passed every step deliver into the base **one at a time, first finished first**. The page shows the base and
checkout state, *Delivering now*, *Waiting* (*Move up*, *Move to top*, *Remove*), *Waiting for approval* (*Review &
merge*), and *Today*. With *Approve to merge*, the task's Summary shows *Ready to merge into <base>* → **Merge into
<base>** (person-only) or *Reject*. With *Auto-merge* it merges after the re-check passes. Conflicts the Integrate step
can't resolve show *Conflicts need you* → **Resolve myself** (resolve in the worktree) → *I've resolved it*. The queue
never merges into a checkout with uncommitted changes; it continues by itself once the checkout is clean.

### Local integration (task tab)

For a succeeded run whose merge would overwrite local changes or needs conflict resolution: *Review current
identities* → *Commit selected changes* or *Temporarily preserve selected* → *Sync and run fresh Check* → tick "I
reviewed this exact checked result." → *Approve and merge checked result* → *Restore preserved local changes* →
*Verify local restoration*. A built-in conflict resolver shows the common ancestor, your version, the incoming version
and an editable result. At any point: *Cancel integration*, *Pause at safe boundary*, *Reconcile actual Git state*.
Every acknowledgement is a person's.

### Report runs

**Ready to complete** → **Complete task** (Done; the report is kept). If the pipeline expected changes: *Complete
anyway*, *Return to a step* or *Run again*.

### Removing worktrees

*Remove worktree* on the header (latest run) or per run under *Runs of this task*. The in-place confirmation lists each
worktree's uncommitted files and what happens to each branch; *Discard the uncommitted changes listed above* is needed
when files would be lost.

## 8. Orchestrator chat (`/project/<id>/orchestrator`)

1. **Settings** (Orchestrator settings): choose the *Project connection*, *Model* and *Reasoning effort*, then
   **Publish v1**.
2. Write in the composer — Enter adds a line, **Ctrl+Enter (⌘Enter) sends**; *Attach* files. While it answers: *Send
   after this answer* (default), *Send now*, *Stop and send*; Esc or **Stop response** stops the answer.
3. It reads everything, saves drafts and runs safe actions directly; it **proposes** the rest as cards — *Start*,
   *Publish*, *Merge into …*, *Send answer*, *Apply*… — that you apply or dismiss. You can also approve a waiting
   proposal in words ("run it"), except click-only ones.
4. **History** lists conversations; **New conversation** starts one.
5. More autonomy: the person-only switches in Settings › Automation; the **daily report**.

Details: [orchestrator-chat.md](orchestrator-chat.md).

## 9. Code, pull requests and usage

### Files — the code viewer (`/project/<id>/files`)

A read-only view of any file at a version: *Project checkout*, *Run #N worktree*, *Base branch* or a commit. A folder
tree, syntax highlighting, line selection (click a number, Shift extends), **Go to file** (Ctrl+P / ⌘P), **Find**
(Ctrl+F / ⌘F), *Copy lines* / *Copy file* and *Open in editor* (person-only). It follows agents' changes in the shown
worktree.

### Pull requests (`/project/<id>/pull-requests`)

The linked GitHub repository's pull requests through `gh`, synced on open, on *Refresh* and every minute. Tabs **Open
· Needs your review · Agent working · Merged · Closed**; search by title, number, branch or task key. A pull request
opens on **Files changed** (diff, threads) and **Agent feedback**: select lines, then **Ask a question** (the linked
task's agent answers in its conversation) or **Request a fix** (continues the task's run). Threads stay in Orchester and
are never posted to GitHub. A PR is linked when a task's run made its branch.

### Usage (`/project/<id>/usage`, `/usage` for all projects)

Period (*Today*, *7 days*, *30 days*, *Custom…*), *By hour* / *By day*, filters (account, model, agent profile,
pipeline, tag, project). Sections: **Summary** (spent tokens = input − cached + output, runs, agent time, cache share,
failed runs, each against the previous period), **Tokens over time**, **Plan limits** (with the warning threshold),
**Where tokens go**, **Most expensive**, **Efficiency** (tokens before first edit, code index use). Every chart has a
table view.

### Delivery (`/project/<id>/usage/delivery`)

*Usage · Delivery* switch; period 30/90/365 days. **Delivery forecast** (Monte Carlo over the last 30 days of merges —
needs at least 10 days and 3 merged tasks — giving 50% and 85% dates for queued tasks and each open module),
**Throughput**, **Cycle time** and **Lead time**, **Where time goes** (active time per step and how often its work was
returned), **Cumulative flow**.

## 10. Sharing a project with a team

1. Get invited by the team server's owner (the invite arrives in Settings › Orchester › Invites and on All projects;
   two-factor must be on), or use Settings › Orchester › **Connect a server**.
2. Project settings › **Sharing** › *Link to a server project* → *Link and upload N tasks* (optionally *Write
   orchester.toml into the repository*). A project with tasks goes through the **join review** (*Join and upload N* /
   *Join without uploading*).
3. Tasks not uploaded stay *Not shared* (read-only; *Share task* on its page).
4. *Make local…* or *Leave…* to stop.

## 11. Automating Orchester

Every screen's work is also available to your own agents through Orchester's MCP server. Register it from Settings ›
Automation › *Connect an agent*, then decide what connected agents may do under *What automation may do*. See
[mcp-server.md](mcp-server.md).
