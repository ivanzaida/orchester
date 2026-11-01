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
| Ctrl+K | ⌘K | Open or close *Jump to* |
| Ctrl+, | ⌘, | Open the project's settings |
| Mouse back/forward buttons | same | Back / Forward |

**Links** (sidebar entries, breadcrumbs, rows that open a page, *Open …* buttons, links in chats):

- Plain click, or Enter/Space → opens in the active tab.
- **Ctrl+click (⌘-click) or middle-click** → a new tab to the right; the current tab stays active.
- **Ctrl+Shift+click (⌘⇧-click)** → a new tab that becomes active.

An item of *another* project (a Needs-you item, a notification, a chat link) never replaces your current tab: a tab
of that project is used or opened, and Back there returns you once to where you came from.

### Sidebar

At the top, the **project switcher**: recent projects (with *Open in new tab*), then *Project settings* (Ctrl+, /
⌘,), *All projects*, *Settings*, *Open folder…* and *Add project*. Under it, **Jump to…** (Ctrl+K / ⌘K). Then the
project's places:

- **Overview, Tasks, Modules, Orchestrator, Review, Files** — Review is one page with two tabs, *Merge queue* and
  *Pull requests*.
- *Configure*: **Pipelines, Agents** — Agents is one page with the tabs *Profiles, Instructions, Skills, Tools,
  Memory*. Click the *Configure* heading to fold it (*Configure · 2*); each project keeps its own fold. Connections
  and Secrets are tabs of Project settings.
- A **Running** section while runs work, with how many: "<key> · <step>" with the step's live agent time; a hand when
  the run waits for you.
- At the bottom, the **usage card**: *Tokens today* "used / budget" with a meter; click it for Usage, or its ⚙ for
  Settings.

Counts show beside Tasks and Review (runs in the merge queue, amber).

**Jump to** (Ctrl+K / ⌘K anywhere, or the row under the switcher) finds the project's tasks by key or title, its
modules, every page and hub tab, and actions such as *New task*. ↑ ↓ move, Enter opens, Ctrl+Enter (⌘Enter) opens in a
new tab, Esc closes. Files keeps Ctrl+P for *Go to file*.

### Status bar

It belongs to the active tab's project. Left: project name, **runner** state (*Runner online*, *Reconnecting…*,
*Runner offline* with why; click for the runner log; *Restart runner* when it can't start), and an **activity slot**
(operations under way, the newest failure, the latest event). Right:

- **Agents** N/M (agents at work / slots), **Queued** N, **N need you**. Today's tokens are in the sidebar's usage
  card.
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
Configure them in Settings › Notifications.

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

Orchester works without an account. Sign in to **Orchester Cloud** only to share to a team, see invites or reach your
account's team servers: each of those says so and offers the sign-in in place, and Settings › Orchester Cloud has it too.

1. Enter *Email* and *Password*, press **Sign in**.
2. With two-factor on: enter the 6-digit code and **Verify**, or *Use a recovery code instead*.
3. *Create an account* and *Forgot password?* open the cloud's pages in your browser.

Signing in needs the internet once; after that Orchester works offline and re-checks when it can. *Advanced · cloud
address* sets a test cloud. Only a person can sign in, never over MCP. *Use a code* in Project settings › Sharing joins a
team server without an account.

**Whose a project is.** All projects' *Where* column says *This computer*, *Your account* or the team it is shared with.
A project added while nobody is signed in is this computer's and stays so when you sign in. Hover a row to *Move to
account* or *Move to this computer*; a shared project stays where it is until you make it local. After you sign out, or
when a sign-in ends, only this computer's projects show: All projects says how many of your account's wait, and their
runners keep working. Another account signed in here sees this computer's projects and its own, not yours.

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

### Connections (Project settings › Connections)

`/project/<id>/connections` — a tab of Project settings. Groups **Agent providers** (with **Add provider**),
**GitHub**, and **MCP servers** (with **Add MCP server**). A row opens the connection's own page. An empty project
offers *Connect an agent* rows for each CLI found on the PATH and *A CLI on an API provider*.

**Add an agent CLI connection** (**Add provider** → *Add an agent provider*):

1. Install the CLI and sign in yourself (`claude auth login`, `codex login`). Optionally add more **accounts** in
   Settings › Accounts.
2. *Kind*: **Codex CLI** or **Claude Code CLI** (the native Anthropic API / OpenAI API kinds are locked in this
   version).
3. *Name*; *Program* (the copy found on the PATH, checked at once — "<program> <version> · signed in").
4. *Account*: Default or one of yours.
5. *Runs on*: **Its own sign-in** or **An API provider** (base URL, an API key from a project secret or *New secret…*,
   models as chips — person-only).
6. **Save connection**.

An agent CLI connection's page has **Check again**, **…** (*Rename*, *Edit connection*), and the groups *Runs on*,
*Models it lists* (*Refresh models*) and *Used by*.

**Add an MCP server** (**Add MCP server** → *Add an MCP server*): a name, then *A program here* (program, arguments,
folder, variables — `${secret:NAME@environment}` for secrets) or *A remote server (HTTP)* (endpoint, authorization
secret), and the tools that *Run without asking* → **Add server**. A server's page has an on/off switch, *Edit*, **…**
(*Remove*), and the groups *Server*, *Sends*, its tools and *Used by*. Turning a server on and widening it is
person-only. A server reaches only agents of profiles that choose it.

**GitHub**: detected at setup; its page offers *Check again* and *Use this account*. Adding or editing a GitHub
connection in the window is not built yet.

Over MCP: `add_cli_connection`, `set_api_provider`, `check_connection`, `list_models`, `set_connection_account`,
`add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`, `remove_mcp_server`.

### Agents hub

One **Agents** page with tabs **Profiles**, **Instructions**, **Skills**, **Tools** and **Memory**, each on its own
route (`/project/<id>/agents`, `…/instructions`, `…/skills`, `…/tools`, `…/memory`); the sidebar's *Agents* opens it on
Profiles, and *Jump to* on any tab. Each item opens as a sub-page with a back crumb. Profiles, instructions, skills and tools **autosave** their
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

The list groups pipelines as **Published** and **Draft only**; each row shows its steps ("Research → Plan → …"), its
version state and the tasks using it, and a **…** menu (*Open*, *Duplicate*, **Make default** — the pipeline New task
starts with, marked *Default*). Header: **Import from project**, **New pipeline**.

1. **New pipeline** (a page): *Name*, *Slug*, *Description* and *Start from* — **Starter · Feature delivery**, **Copy
   a pipeline** or **Empty pipeline** → **Create draft v1**.
2. The pipeline page has tabs **Steps · Settings · Versions** and, in the header, **Save draft**, **Publish vN** (or
   *Version history* with no draft) and *Run* (always unavailable: "Create a task to run this pipeline.").
3. **Steps** — a **vertical flow**: *On task start*, the step cards in dependency order, steps on the same level in a
   *Run in parallel* bracket, the Deliver card last, and **Add step**. Selecting a step opens a column beside the
   flow:
   - **Step** — *Executor* (a published profile or a command tool), *Model for this step*, *Instructions* (a page of
     its own);
   - **Flow** — *Runs after*, *When changes are requested* (failure route), *Maximum retries*, *Step budget*, *When the
     step fails* ("Restart once, then ask");
   - **Gate and access** — *Wait for your approval*, *Repository access* (Read-only / Can write), *Merge the base branch
     when it moves*, *Sources* and *Permissions* (pages that can only narrow the profile: a subset of its commands,
     network off, ask when a tool needs approval).
   Rename a step in place; *Duplicate step*, *Remove step*.
4. The **Deliver step** (from its card): *Mode* (*Approve to merge*, *Auto-merge*; *Pull request* coming later),
   *Merge method* (*Merge commit* / *Squash*), *Resolve conflicts with*, *Re-run Check on the merged result*.
5. **Settings** — *General* (name, slug, description), *Runs* (starts on task start; when a step fails), *Sources for
   every step*, *Delivery* (*What a run delivers*), *Deliver step*.
6. **Save draft**, then **Publish vN**. The publish review shows *Published now* against *After publishing*; a weaker
   version asks for your approval. A pipeline whose profile only has a draft can't publish.
7. **Versions** — every published version; open one read-only, *Compare* it with the draft or the published version,
   or *Restore vN as draft*.

### Instructions, Skills, Tools, Memory (Agents hub tabs)

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

### Secrets (Project settings › Secrets)

`/project/<id>/secrets` — a tab of Project settings. Segments **All · Development · Staging · Production**; secrets are
grouped by environment, each row with its name, what uses it and a masked value with the date it was set.

- **Add secret** → *New secret*: name, type, value, environment → **Save secret**. The value goes to the OS keychain and
  is never shown again.
- A secret's page: **Replace value** (asked in place), **…** (*Copy name*, *Archive* — removes the value), and the
  groups *Details* and *Used by*.
- *Import from project* explains that secrets are not imported: a value stays in its own project.

### Import resources (`/project/<id>/import`)

Copy pipelines, agent profiles, instructions and tools from another project as **independent local drafts**:
*Select resources* → *Resolve dependencies* (new copy, rename, reuse a local one, leave unresolved) → *Create drafts*.
Connections and secrets are never copied; importing never grants access.

### Project settings

**General** (project, GitHub, base branch, base-moved notices, repository agent files), **Runs** (approvals: Allow
Autopilot and the reviewer, approved-for-every-run rules, story-point estimates, limits, environment), **Context**
(sources, code index),
**Connections**, **Secrets**, **Sharing** — see [settings.md](settings.md#2-project-settings-projectidsettings).

## 5. Tasks

### Tasks list and board (`/project/<id>/tasks`)

**Header** (one row): **Tasks**, **Search tasks** (press `/` to focus; searches titles, keys and briefs), a **List |
Board** switch, **Filter**, **Display**, **…** (*More actions*: *Hold new starts* / *Release new starts*; *Create
multiple*, locked as planned) and **New task**. List or Board is remembered per project.

**Views**, each with a count:

| View | Holds |
|---|---|
| **Needs you** | Tasks whose run waits for you |
| **Active** (default) | Todo, In progress and In review, plus anything that waits for you |
| **Backlog** | Backlog tasks that don't wait for you |
| **Done** | Done and Canceled tasks |
| **All** | Every task |

While new starts are held, a *New starts on hold* chip with **Release** sits at the end of the views.

**Filter** is a menu with its own search and checkbox groups **Pipeline** (with *No pipeline*), **Priority**,
**Module** (with *No module*) and **Tag**; chosen values show as removable chips with **Clear**. There is no status
filter — the views do that job. **Display** chooses the sort (*Manual order, Newest first, Title, Workflow status,
Priority* — sorting never changes the run queue) and, on the board, its layout.

#### List

One-line rows: status icon, key, title with its first two tags, **one state phrase** saying what happens next
("Approve `cargo add rand`", "Answer a question", "Review plan v1", "Stopped at its token limit", "Ready to merge",
"02 Plan", "Ready to queue", "Needs a brief", "Merged into main"…), the pipeline, a priority icon and the age.
*Active* is grouped by what happens next — **Needs you, Running, Queued, Ready to start, Stopped, Finished**; *All*
is grouped by workflow status; group headers fold. A row opens the task on its **Summary**. On hover the row shows the
waiting task's one action (*Review, Answer, Review plan, Decide, Raise the limit, Review & merge…*) and a **…** menu
(*Mark as done, Cancel task, Reopen task, Delete task…*). The list has no selection.

#### Board

A Kanban board. **Columns** by status (Backlog, Todo, In progress, In review, Done, and Canceled when shown), or —
from *Display options* — by priority, module or pipeline; **Rows** (swimlanes) by module or priority. The view
chooses which status columns appear (Active: Todo, In progress, In review; All: every column). Columns and swimlanes
fold. *Display options* also choose the **card properties** shown (key, priority, points, module, tags, run status,
readiness, agent, age, blocked, pipeline), *Show empty columns* and *Show Canceled column*; they are saved per
project.

**Drag and drop** a card to another column to change it. Columns that refuse the card are dimmed with the reason
("Run not ended: stop it from the task", "Needs a brief: add it before queuing", "Has a run: can't go back to
Backlog", "Pipelines change on the task's page"), and a hint under the lifted card says what the drop will do. Order
inside a column is always by task number. The status rules:

- **In progress / In review** only change the status.
- **Todo** queues a complete draft (asks first if it would start now); for a task whose run ended it means *Run
  again* (asks first).
- **Backlog** returns a queued task whose run never started to draft.
- **Done / Canceled** close the task (a run that hasn't ended is stopped after confirmation; Done asks first when
  work waits to be merged). The Done column offers two drop zones, *Mark as done* and *Cancel task*.
- A task whose run hasn't ended can go only to Done or Canceled.
- By priority or module, a drop sets the priority or module; by pipeline, drops are refused.

Questions appear in a small popover at the card (Enter confirms, Esc keeps it).

**Card menu** (`…`, or `.`): *Open*, *Move to*, *Priority*, *Tags* (existing project tags), *Copy key*, *Run again*,
*Mark as done* / *Cancel task* / *Reopen task*, *Delete task…*.

**Selection and bulk actions**: Shift+click or X selects cards; a bar at the board's foot offers **Move to**,
**Priority**, **Tags**, **Mark as done**, **Cancel task** and **Delete…** (person-only).

**Quick add**: Backlog's **+** (or its *New task* foot) opens a title field; Enter adds a draft and keeps the field
open, Esc closes it.

| Board key | Action |
|---|---|
| Arrows | Move focus between cards |
| Enter | Open the task (or confirm a question) |
| `[` / `]` | Move the card a column left / right |
| X | Select |
| `.` | Open the card menu |
| M / P / T | Move to / Priority / Tags |
| Ctrl+C (⌘C) | Copy the key |
| Esc | Put down a lifted card, keep a question, or clear the selection |

### New task / Edit task

The form is a **composer**:

1. Type the **title** and **the outcome** ("what should change, and why?" — Markdown works) directly on the page.
2. **Acceptance criteria** — *Add criterion*; Enter adds the next one, Backspace in an empty one removes it, Alt+↑/↓
   moves it.
3. **Property chips**:
   - **Pipeline** — published pipelines with their steps and approval gates (and *Manage pipelines*); once chosen, a
     plan line shows the steps, where approval is needed and *View pipeline ↗*.
   - **Delivers** — *As the pipeline says, Code changes, A report, Outside the repository, Code changes or a report*.
   - **Limits** — *Run budget*, *Time limit* (empty = no limit).
   - **Sources** — folders or files this task's agents may read without asking (*This task*, *From the project*, *From
     the pipeline*, *Add*).
   - **Module** (when the project has modules), **Priority**, **Points** (*Automatic* or 1–21).
   - **…** — *Don't merge automatically*.
4. **Create** saves a draft and opens Tasks with the row highlighted *New*; **Create & run** (Ctrl+Enter / ⌘Enter)
   queues and starts the first run and opens the Conversation. When editing: **Discard changes** and **Save changes**
   (Ctrl+S / ⌘S); a draft keeps *Create & run*.

The bar says what is missing ("To create, add a title. To run, also the outcome, one criterion and a pipeline.").
Unsaved edits are kept on this computer and restored; if someone saved a newer version meanwhile, you choose per field.
A task that already ran keeps its runs' briefs; edits apply to future runs. `/tasks/new?module=<id>` starts with that
module chosen.

### Task page (`/project/<id>/tasks/<KEY>`)

**Header**: `← Tasks / KEY`; the title with **one primary action** for the task's state and a **…** menu; a status
line ("Interrupted at 03 Implement · Run 8 · attempt 2 · started 15:04 · 5 pt"); the **Module** select; notices
(Blocked, a runner refusal, a stale read) and in-place confirmations.

The **primary action** is what the state calls for: *Approve* or *Answer* (opens the Conversation at the card),
*Review* or *Merge* (opens the Summary at the decision card), *Share task* for a Not shared task, or else **Run**,
**Start run**, **Resume run** or **Continue run**. A locked action says why in a few words under it and in full in its
tooltip ("Runner not connected", "Blocked by KON-3", "Worktree removed", "Today's token budget is used"…).

**More actions** (…): *Edit task* (key **E**), *Copy link*, *Open worktree in Files*, *Memory for this task*; *Pause
run*, *Run again*, *Return to a step*; *Mark as done* / *Cancel task* / *Reopen task*; *Remove worktree*, *Cancel
run*, *Delete task…*. Items that apply but can't be used stay visible, locked with why.

| Control | When | Effect |
|---|---|---|
| **Run** | Draft with every detail | Queues the task and starts its first run |
| **Start run** | Queued, run not started | Asks the runner to start it (never a second run) |
| **Pause run** | An agent works | Pauses at the next checkpoint |
| **Resume run** | Paused, or stopped by the daily budget | Continues in the same worktree |
| **Continue run** | Interrupted | Continues from the last checkpoint |
| **Cancel run** | Until the run ends | Cancels it |
| **Run again** | Failed, canceled, or interrupted with its worktree removed | In-place confirmation "Run KEY again as run #N" → **Start run #N**: a new run from step 1 in a new worktree |

**Tabs** — a task opens on **Summary**:

- **Summary**, top to bottom:
  - the **decision card**, if one waits — plan review, raise the limit, plan limit, retries, *Ready for your review*,
    merge queue, *Ready to complete*, or *Return the work to an earlier step*;
  - **Run** — the pipeline and run number, a **rail of step chips** (each chip's tooltip has the agent, state,
    attempt, tokens and time), the last event and *Conversation →*;
  - **Brief** (with a version chip: *As created*, *As run #N started*, *Current task*) and **Acceptance criteria**;
  - **Output** — each step's report and the run summary;
  - **Runs** — one row per run, each with *Open its conversation* and *Remove worktree*.

  A read-only **properties column** shows Status, Priority, Pipeline, Tags, Depends on, Limits, Branch and Created
  ("· changed" when the task differs from what the latest run started with), the run's facts (base, checkpoint,
  tokens, active time, model, worktree — a click copies; account, cached input, duration, agents and permissions
  behind a disclosure) and the **memory notes** link, which opens the task's **Memory** view (search, filters, *New
  record*, what the next agent receives).
- **Conversation** — the latest run as a live chat: agents' messages with tool groups ("Read 4 files · ran cargo
  test"), step dividers, returns and restarts, result cards, decision cards (questions, plans, approvals), and while an
  agent works "<agent> is working" with **Stop turn**. Scrolling up stops following; *Jump to latest*.
- **Changes** (with a file count) — *Checkpointed* (base → latest checkpoint) and *Not checkpointed* (the worktree
  beyond it); *Open file* opens the code viewer; **Try locally** opens **Local integration** in the tab's place.

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

**Mark as done**, **Cancel task**, **Reopen task** and **Delete task…** are in the task's *More actions*, a list row's
**…**, the board's card menu and bulk bar (and dropping on Done or Canceled). Closing a task whose run hasn't ended asks
first ("Stop the run of KEY?"). *Delete task…* lists everything that goes (runs, conversation, memory, approval rules,
links); *Also remove work that is not merged* is optional. Deleting is person-only and not available in shared
projects.

### Modules (`/project/<id>/modules`)

- The list has views **Open · Closed**, **Search modules** (`/`) and **New module**. Rows show the name and
  description, the forecast for open modules ("Likely Oct 30", "· after target", "Not enough history", "All done"),
  the dates and "N of M tasks done".
- **New module** and **Edit** are pages: the name as the title, the description under it, and chips for **State**,
  **Start date** and **Target date** (a calendar date picker) → *Create module* / *Save changes*.
- A **module page** has **New task in module**, a **…** menu (*Edit*, *Close module* / *Reopen module*), a status line
  (state, dates, progress, forecast) and tabs:
  - **Summary** — description, tasks grouped by status, "N remaining" with the forecast and *Burndown →*, and
    properties (state and dates editable in place);
  - **Burndown** — remaining work per day with the ideal line, 50%/85% forecasts and work added and removed, as a
    chart or a table.
- Put a task in a module with the form's *Module* chip, the task header's *Module* select, or by dropping its card on
  the board when columns or rows are by module. Modules can't be deleted, only closed.

## 6. Following and steering a run

1. After *Run*, the **Conversation** shows the agents live; the **Summary** shows the step rail, the last event and
   the run's facts; **Changes** shows the diff.
2. **Approvals** — the header's **Approve** opens the card. Answer there or in the Needs you panel: *Approve once*,
   *Deny*, *Deny with a message*, or more widely (*Approve similar in this run*, *Approve everything in this run*). To
   be asked less: the profile's *Approve automatically*, or its approval mode set to Autopilot while the project
   allows Autopilot (Project settings › Runs). Under Manual the project's reviewer answers before you. A run's
   *Switch to Manual for this run* takes it out of Autopilot for good.
3. **Questions** — the header's **Answer** opens the card; pick options or write your own answer, then *Send answer*.
   A plain question is answered from the composer.
4. **Plans** — the header's **Review** opens the Summary's plan review: read plan vN, then **Approve plan & continue**,
   or write *Changes the plan needs* and **Request changes**. Earlier versions are in the *Plan version* select.

### Continuing a stopped run

| How it stopped | What to do |
|---|---|
| Paused | **Resume run** (or the composer's *Resume and send*) |
| Interrupted (crash, runner lost, *Quit now*) | **Continue run** — from the last checkpoint; once its worktree is removed only *Run again* is left |
| Token or time limit | Summary › *Raise the limit*: enter a new budget/time → *Continue with N tokens / N min*, or *Remove the limit* |
| Step budget | *Raise the step budget* → *Continue* or *Remove the limit* |
| Daily project budget | *Resume run* once the day has room, or raise the daily budget |
| Account plan limit | *Wait for <account>, then continue* (resumes by itself at the reset) or *Continue now on another account* |
| Retries used up / broke again / unverified / unchanged | *Decide how the run continues*: *Allow 1 retry & continue*, *Accept <step> as it is*, *Run <step> again*, *Return to <step> with note*, *Stop and fail the run* |
| Failed | Composer *Continue with instructions*, or *More actions › Run again* |
| Ended, want more work | *More actions › Return to a step* (with a note), or composer *Send follow-up* |

Resume and Continue are the header's primary action when no decision card waits; otherwise they are in *More actions*.
Over MCP: `pause_run`, `resume_run`, `cancel_run`, `continue_run`, `send_message`, `run_task`.

## 7. Merging

### Pipeline without a Deliver step

The Summary shows **Ready for your review** (the header's **Merge** or **Review** opens it) with *What changed* and
*Verification*:

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

### Local integration (Changes › Try locally)

A view of the **Changes** tab (`/tasks/<KEY>/changes?view=local-integration`), opened by **Try locally** or by the
Ready card's **Local integration** after git refused the merge. For a succeeded run whose merge would overwrite local
changes or needs conflict resolution: *Review current identities* → *Commit selected changes* or *Temporarily preserve
selected* → *Sync and run fresh Check* → tick "I reviewed this exact checked result." → *Approve and merge checked
result* → *Restore preserved local changes* → *Verify local restoration*. A built-in conflict resolver shows the common
ancestor, your version, the incoming version and an editable result. At any point: *Cancel integration*, *Pause at
safe boundary*, *Reconcile actual Git state*. Every acknowledgement is a person's.

### Report runs

**Ready to complete** → **Complete task** (Done; the report is kept). If the pipeline expected changes: *Complete
anyway*, *Return to a step* or *Run again*.

### Removing worktrees

**More actions › Remove worktree** for the latest run, or a run's **…** › *Remove worktree* in the Summary's **Runs**
section. The in-place confirmation lists each worktree's uncommitted files and what happens to each branch; *Discard
the uncommitted changes listed above* is needed when files would be lost.

## 8. Orchestrator chat (`/project/<id>/orchestrator`)

The page is a chat first: the conversation's title and start time, **New** and a **History** toggle in the header,
the turns in one column, and the composer at the foot.

1. **Choose what it runs on.** Until it is set up, the composer says the Orchestrator can't answer yet and offers
   **Set up the Orchestrator**. The composer's *runs-on* chip (CLI · account · model) opens a picker: connection,
   *Model*, *Effort*, **Publish vN**, and *Settings ↗* for the full Orchestrator settings page.
2. **Write.** Enter adds a line, **Ctrl+Enter (⌘Enter) sends**; the paperclip attaches files. While it answers: *Send
   after this answer* (default), *Send now*, *Stop and send*; the working line's **Stop** (or Esc) stops the answer, and
   a stopped turn offers **Ask again**.
3. **Read what it did.** Every action shows as one of:
   - an **event line** — done, refused ("· not done") or dismissed, with *Open ↗*;
   - an **agent wait line** — *Waits for you*, *Orchestrator is reading*, *Answered by the Orchestrator*, *Answered by
     you*, *No longer waits*;
   - a **run-end line** — with *Review ↗*, *Raise ↗*, *Continue ↗* or *Summary ↗*;
   - a **digest** — "N waiting for you";
   - a **decision card**, only for what a person decides: its primary button is the action (*Start*, *Publish vN*,
     *Merge into …*, *Send answer*, *Send message*, *Cancel run*, *Allow retry*, *Return to step*, *Pause*, *Resume*,
     *Release*, *Create repository*…), plus *Dismiss*. "Only you can apply"; a decided card folds into an event line.
   - the **daily report** card ("Daily report · <day>").

   You can also approve a waiting proposal in words ("run it"), except click-only ones.
4. **New** starts a conversation ("What should we plan?" with four suggestions); **History** lists earlier ones (search,
   *Today*, *Yesterday*, *Earlier*).
5. More autonomy: the person-only switches in Settings › Automation (*The Orchestrator may*); the daily report in
   Settings › Notifications.

Details: [orchestrator-chat.md](orchestrator-chat.md).

## 9. Code, pull requests and usage

### Files — the code viewer (`/project/<id>/files`)

A read-only view of any file at a version: *Project checkout*, *Run #N worktree*, *Base branch* or a commit. A folder
tree, syntax highlighting, line selection (click a number, Shift extends), **Go to file** (Ctrl+P / ⌘P), **Find**
(Ctrl+F / ⌘F), *Copy lines* / *Copy file* and *Open in editor* (person-only). It follows agents' changes in the shown
worktree.

### Pull requests (`/project/<id>/pull-requests`)

The *Pull requests* tab of the **Review** page (the sidebar's *Review*), beside the *Merge queue* tab
(`/project/<id>/merge-queue`); each tab keeps its route. The linked GitHub repository's pull requests through `gh`,
synced on open, on *Refresh* and every minute. Tabs **Open
· Needs your review · Agent working · Merged · Closed**; search by title, number, branch or task key. A pull request
opens on **Files changed** (diff, threads) and **Agent feedback**: select lines, then **Ask a question** (the linked
task's agent answers in its conversation) or **Request a fix** (continues the task's run). Threads stay in Orchester and
are never posted to GitHub. A PR is linked when a task's run made its branch.

### Usage (`/project/<id>/usage`, `/usage` for all projects)

Opened from the sidebar's usage card (*Tokens today*).

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

1. Get invited by the team server's owner (the invite arrives in Settings › Orchester Cloud › *Invites* and on All
   projects; two-factor must be on), or use Settings › Orchester Cloud › *Team servers* › **Connect a server**.
2. Project settings › **Sharing** › *Share on a team server* › **Share…** → choose the server and a project there (or
   its address) → *Link to this project…* → **Link and upload N tasks** (optionally *Write orchester.toml into the
   repository*). A project with tasks goes through the **join review** (*Join and upload N* / *Join without
   uploading*).
3. Tasks not uploaded stay *Not shared* (read-only; *Share task* on its page).
4. *Leave* › **Leave…** or **Make local…** to stop.

## 11. Automating Orchester

Every screen's work is also available to your own agents through Orchester's MCP server. Register it from Settings ›
Automation › *Connect an agent*, then decide what connected agents may do under *What automation may do*. See
[mcp-server.md](mcp-server.md).
