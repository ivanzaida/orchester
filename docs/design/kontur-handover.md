# Orchester redesign in Kontur: product brief

Written 2026-10-06 for the design agent that draws Orchester from scratch in Kontur. This is your only source about
the product. Read it whole before you start.

## 1. Purpose

- **A new, modern visual design of Orchester**, drawn in **Kontur**, the user's own design editor.
- **It is also a real-world test of Kontur.** Kontur is still in development. Drawing a full product design in it
  shows what works and what does not. Every gap you hit is a result: record it (section 6).
- **You choose the visual language.** Nothing here prescribes colours, type, sizes, radii or icons. Before drawing
  screens, propose **2–3 directions as small style tiles** (palette, type scale, a few components, one screen
  fragment each) and let the user pick one.
- **Desktop app for Windows and macOS.** Orchester draws its own window chrome (title bar with tabs, status bar). The
  macOS window keeps the system's three window buttons at the left; Windows has minimize, maximize and close at the
  right.
- **Two reference window widths: 1440 and 960.** 960 is the minimum width. Draw key screens at both.
- **A dense, power-user tool.** People keep it open all day and scan many runs at once. No marketing whitespace.
- **English UI copy.** Quoted labels in this brief are the app's real wording; reuse them, or note any change you
  make and why.
- **Illustrative fixture data only.** Use a sample project named *Kontur* with tasks *KON-1*, *KON-2*, …, a second
  project such as *Atlas* or *Personal website*, agent profiles *Explorer*, *Planner*, *Builder*, *Reviewer*,
  made-up branches, hashes, paths, times and token counts. Never real projects, people, accounts or secrets, and
  nothing that looks like a shipped default.

## 2. What Orchester is

Orchester orchestrates coding agents per project, on the user's own computer. A **project** is a local git
repository; everything operational belongs to it. The person writes a **task** (a brief and acceptance criteria) and
starts a **run**. A run follows a **pipeline**: an ordered graph of **steps**, each an agent step (an **agent
profile** does the work) or a tool step (a command such as the test suite). Steps can run in parallel, wait for the
person's approval (an approval gate, e.g. "approve the plan"), and send failed work back to an earlier step with
retries. Pipelines, agent profiles, instructions, skills and tools are versioned: you edit a **draft**, then
**publish vN**; a run pins the versions it started with.

- **Agent profile:** a role (name, responsibility, role instructions), a **connection** and a model with a reasoning
  effort, and **permissions** (repository access, commands, network, MCP servers, extra writable folders, whether
  it auto-approves).
- **Connection:** how an agent runs. *Codex CLI* and *Claude Code CLI* use the CLI the person is signed in to on this
  computer (Orchester never signs in or sees tokens). *Anthropic API (native)* and *OpenAI API (native)* are
  "native" connections: Orchester runs the agent loop itself against the model API with a project API key. Other
  connection kinds: *GitHub* (through the `gh` CLI) and *MCP servers*.
- **Instructions** (project rules, role rules, pipeline and step overrides) are assembled into each agent's prompt.
  **Skills** are know-how an agent loads when needed. **Tools** are callable actions (command tools today).
- **Secrets** are project credentials stored in the system keychain, grouped by **environment** (Development,
  Staging, Production), used by tools and MCP servers, never shown to agents. **Environment** variables are plain
  text added to every agent and tool.
- **Approvals:** when an agent wants to run a command or call a tool its permissions do not cover, the request waits
  for the person (*Needs you*). Answers: Approve once, Approve similar in this run, Always allow, Deny, or Decide later.
  **Auto-approval** lets Orchester answer safe requests itself; **auto mode** lets a separate reviewer model judge
  the requests the rules leave to the person. Dangerous requests (credential reads, history rewrites, writes outside
  the worktree, installs, network) always reach a person unless a person widened the answer.
- **Worktrees and branches:** every run works in its own git worktree on its own branch; the person's checkout is
  never touched. Work is saved as **checkpoints** (commits). When a run is *Ready for review*, the person **merges**
  it into the base branch, or asks the agent for a follow-up. Worktrees are removed explicitly.
- **Pull requests:** the project's GitHub pull requests are listed and read in Orchester (review feedback to agents
  is planned).
- **Code index:** Orchester indexes the repository per revision so agents find code faster; it needs ripgrep.
- **Memory** (planned screens): agents keep handoffs and decisions as versioned records per task and per project.
- **Limits:** every run has a **token budget** and a **time limit** (active agent time only); the project has a
  **daily token budget**. **Plan limits** are the CLI accounts' own usage windows (5-hour, weekly), read live.
- **The runner:** one background process per open project that runs agents. It can start, go offline and reconnect;
  runs interrupted by a runner drop continue from their last checkpoint. Closing the window stops everything (there is
  no background mode).

## 3. App shell and navigation

### 3.1 Window and title bar

- The title bar holds **tabs**, like a browser. A tab is an independent location: *All projects*, or a project and a
  page (Overview, Tasks, a task, a pipeline, Settings…). Each tab has its own Back / Forward history (mouse back and
  forward buttons, Alt+Left/Right, Cmd+[ / Cmd+]).
- **Tab label:** `<project> · <page>`, e.g. *Kontur · Tasks*, *Kontur · KON-12*, *Kontur · Feature delivery*. The
  tooltip adds the object's title (*Kontur · KON-12 · Add retry to webhook sender*). An All projects tab shows a
  folders glyph instead of a project avatar.
- **Tab states:** active, inactive, hover, close hovered, being dragged, and **attention**: a marker on every tab of a
  project that has something waiting for the person or an unresolved failure (tooltip: *Personal website · 1 needs
  you*, or the failure).
- **Actions:** **+** (new tab, Ctrl/Cmd+T) opens All projects next to the active tab; close with ×, middle-click or
  Ctrl/Cmd+W; **drag to reorder** along the strip (Escape cancels). Closing the last tab opens a fresh All projects
  tab; the only All projects tab has no close. Closing a tab with unsaved edits asks *Discard changes?* (*Keep open* /
  *Discard and close*).
- **Overflow:** tabs shrink together, then the strip scrolls sideways with faded edges, and a **tab list** button
  shows the count and opens a list of every tab (click switches, middle-click closes).
- **Shortcuts:** Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+1…8, Ctrl+9 (last). Tabs restore on next start.
- The empty parts of the title bar are the window's drag area. A dialog belongs to its tab's page: while one is open,
  the tabs and status bar do not react.

### 3.2 Project sidebar

- **Project selector** at the top: the project's name (long names truncate). It opens the **project switcher**: a
  *RECENT* list (up to five projects, the current one checked), *All projects*, *Add project*. A hovered row offers
  *Open in new tab · Ctrl+click*.
- **Sections** (in order): *Overview*, *Orchestrator* (project chat, **planned**: shown but unavailable), *Tasks*
  (with a count), *Pull requests*, *Pipelines*, *Instructions*, *Skills*, *Agents*, *Tools*, *Secrets*,
  *Connections*.
- **Planned sections** that the design should leave room for: *Merge queue* (after Tasks, with a count), *Memory*
  (after Instructions, count of records waiting for approval), *Usage* (after Pull requests).
- **Application group:** *All projects*, *Import resources* (opens a dialog over the current page), *Settings* (app
  settings).
- **Runner state** near the bottom: *Starting local runner* / *Local runner online · 2 of 4 agent slots in use* /
  *Agent slots not reported yet* / *Reconnecting to runner · attempt 2 of 5* / *Runner offline · could not restart* /
  *Stopping local runner* / *Local runner stopped*. Then *Project settings*.
- Sidebar items are pointer-only (not keyboard stops). Ctrl/Cmd+click or middle-click opens a page in a new tab.

### 3.3 Status bar

Always at the bottom, for the active tab's project. Every item shows only measured data; nothing is faked.

- **Runner:** *Runner online*, *Starting runner…*, *Reconnecting to runner · attempt 2 of 5 · next in 8 s*, *Runner
  offline · 5 attempts since 14:02* with *Restart runner* and *Open runner log*, *Stopping runner…*, *Runner
  stopped*. Click opens the runner log.
- **Activity slot** (one line, priority: runner problem > unresolved failure > running operations > latest event):
  - one operation: *Creating worktree for KON-29…* with elapsed time (never a percentage);
  - several: *3 operations · Publishing Feature delivery v4…* opens a popover *Background operations* (running rows
    with project · object and elapsed time; *RECENT* finished events; *Open runner log*);
  - latest event: *Run #128 ready for review · 14:32*, *Plan ready for review · KON-12*, *Builder asks · KON-12*,
    *Review needs your decision · KON-28*; a click opens the object;
  - failure, pinned until opened or dismissed: *Could not create worktree for KON-31 · branch kon-31 already exists*
    with *Open task* and dismiss;
  - "not done" (quiet, not pinned): *Pull requests not synced*.
- **Agents** `2/4` (slots in use / configured; a dash while unknown), **Queued** `2`, **Needs you** `3` (plans,
  questions, decisions, waiting approvals; `0` stays visible). Counts open Tasks or the Overview's Needs you.
- **Tokens today** `3.8M / 20M` (this project's daily budget).
- **Plan limits:** one entry per agent CLI the project uses, e.g. *Codex 65% · Claude 42%*; a hint lists each window
  (5-hour, weekly) with a meter, used %, reset time, plan type, credits or extra usage, and which turn reported it.
  States: known, near the limit (80 %+ or the CLI warns), limit reached (*Codex limit reached · resets 16:40*), not
  read yet, not signed in, no working connection, after a reset (*read again with the next turn*).
- **Approvals:** *3 approvals waiting · Builder asks to run cargo add serde · KON-42* with **Review**; *Request
  ended* when an agent stopped waiting.
- **Auto-approving** · *Run 142 · KON-42* (or *2 runs*) with **Stop**.
- **MCP** `127.0.0.1:52341`: the port of Orchester's own automation server. Its popover: *Orchester MCP server*, what
  it is for, where clients find the access token (never shown), and the switch **Let automation answer approvals**
  (see App Settings). *MCP unavailable* when the server did not start.
- **Server connection** (shared server; built): *Kontur team offline · since 14:31 · next try in 32 s*, *Signed out
  of Kontur team · Sign in*, *Not a member of ORC*.
- **Version** (`v0.1.0`), with build info in its tooltip.
- **Narrow window:** the bar is one line that gives way in a fixed order: spare room, then the approval's words,
  times, the version, *Tokens today*, count labels; the hand, counts, *Review*, *Stop*, plan limits and MCP never
  give way. Truncated text keeps its full value in a tooltip.
- On All projects, project items (runner, agents, queued, needs you, tokens) are hidden; the activity names the
  project (*Kontur · Run #128 ready for review*).

### 3.4 Global states

- **First launch (Welcome):** no projects yet. *WELCOME TO ORCHESTER*, *Build with a team of AI agents.*, a short
  description, **Create your first project**, and *THREE STEPS TO YOUR FIRST TASK*: *Connect a repository*, *Assign
  your agents*, *Start a task*. No fake recent projects or activity.
- **Project setup** (a page with a step rail *1. Project & repository*, *2. Runner & agents*, *3. Ready to start*;
  only step 1 is built, steps 2–3 are **planned**):
  - *Create your project*: *Project name*, *Project folder* (*Choose folder…*; must be a git repository; nothing is
    cloned or uploaded), a note that everything stays in this project, **Create project & continue** (disabled with
    *Choose the project folder to continue.*).
  - **GitHub detection block** under the folder (GitHub is optional and never blocks): *Checking…*, *Detected*
    (repository, remote, default branch, account, *Check again*, *Change GitHub account or access*), *Local only*
    (no GitHub remote), *Not GitHub*, *gh not found* (*Choose program…*), *Not signed in*, *Wrong account* (*Signed
    in as X, expected Y*, *Use this account (X)*), *No access*, *Unrecognized* gh output, *Missing scope: repo*,
    *Not a repository* (the only state that disables Create). States with a fix show the exact terminal command in
    a copyable command row (e.g. `gh auth login --hostname github.com`), introduced by "Run this in a terminal, then
    check again."
  - *Change GitHub account or access* is a small panel: host, signed-in account, scopes, expected account, the `gh`
    commands with Copy, *Close*, *Check again*. Orchester never opens a browser or switches accounts.
- **Startup failure:** *Orchester cannot open its database*, with the reason; nothing else loads.
- **Recovery after an interrupted session** (dialog on the next start after *Quit now* or a crash): *The last session
  ended before agents could save*; why and when; a table of interrupted runs (task key and title, project · run,
  *Interrupted during* step · agent · provider · model, *Reason*, *Last checkpoint*, *Worktree*: *3 files not
  committed* with *View changes*, *No uncommitted changes*, *Could not read the worktree*); a warning when a run
  stopped during an external action (outcome unknown). All or nothing: **Continue all 3 from checkpoints** or
  **Don't continue** (runs stay interrupted; each can be continued from its task page); *Open runner log*.
- **Close while working** (the window's close is held back): *Close Orchester while agents are working?*; the tasks
  with an agent at work (key, title, step · agent, provider · model); tasks that only wait are noted as unaffected;
  **Wait for agents to save** (primary: agents stop at their next checkpoint, runs are paused, then the app closes),
  **Quit now** (set apart: stops immediately; work after the last checkpoint is not counted), **Cancel**.
  While waiting: *Saving before closing*, *1 of 2 runs saved*, rows move from *Saving at next checkpoint…* to
  *Saved · Paused at 9d07b3f*, the primary becomes *Closes after 1 more run*, *Quit now* stays.
- **Runner connection problems:** reconnecting is ambient (status bar only); *Could not restart the runner* is an
  inline notice on the Overview with **Restart runner** and *Open runner log*, because it blocks all work.

## 4. Areas

Each area lists purpose, content, actions, states and key flows. "Unavailable" means drawn visibly disabled with a
reason; "planned" means not built: draw it only where noted, and label it as planned in your progress file.

### 4.1 Overview

- **Purpose:** what is moving in the project and what needs the person. The default page of a project.
- **Header:** project name, *Project key*, *Repository*, **New task**.
- **Metrics:** *Running now* (*2 agents working*, or *2 runs interrupted*, or *Waiting for the runner to report*),
  *Need your input* (*Review & decisions*), *Completed today* (*Runs with every step passed*), *Tokens today* (*of
  20M daily project budget*; *Usage not read yet*). Counts are plain numbers; a dash means "not known yet".
- **Task list** with tabs *Running*, *Needs you*, *Queued*, *Failed*, each with a count. Rows: title, key, workflow
  status, run label and what it means now (see 5.1), priority and tags. *Failed* lists tasks whose latest run failed;
  empty: *No failed tasks.* Footer *4 active tasks · 12 completed* and *View all tasks →*.
- **Needs you panel** (beside the list on wide windows, under it on narrow ones): one card per open decision:
  - *REVIEW PLAN* · task · *Plan v1 from Planner waits for your decision.* · **Review plan**;
  - *CLARIFICATION* · the question · *Builder is waiting · KON-12* · **Answer question →**;
  - *NEEDS INPUT* · *KON-28 · Review still fails after 2 retries* · **Open task →**;
  - *APPROVAL* · *Builder asks to run cargo add serde* · *KON-42 · Run 142 · Implement · Network · since 14:06* ·
    **Review**;
  - empty: *Nothing needs you right now. Plan decisions, questions and approvals appear here.*
- **Recent activity:** *Run #128 completed*, *Run #129 needs your input*, *Run #131 failed*… with task · time.
- **States:** empty project (*No tasks yet* with what to do), loading, runner could not restart (blocking notice),
  interrupted runs, a blocked task notice (*Blocked · KON-7*), an unreadable run (*No run to show: the latest run of
  a task could not be read. The task list marks it.*).

### 4.2 Tasks

- **Purpose:** the project's backlog; select tasks and start runs, organize them.
- **Header:** title, description, **New task**, *Create multiple* (**unavailable**: bulk creation is planned).
- **Workflow tabs** with counts: *All*, *Needs you*, *Backlog*, *Todo*, *In progress*, *In review*, *Done*.
  *Canceled* is reached through *All* and the Status filter.
- **Query line 1:** search (title, key, brief), pipeline filter, sort (*Manual order*, *Newest first*, *Title*,
  *Status*, *Priority*). Sorting never changes the run queue.
- **Query line 2:** *Status*, *Priority*, *Tags* filters (multi-select; an active one reads *Status · 2* and shows
  chips), *Clear filters*, *Manage tags* at the right. Tags filter offers *Match any* / *Match all selected tags*.
- **Columns:** checkbox, *ID*, *TASK / TAGS* (title with tag chips, *+N* when they don't fit), *PIPELINE*,
  *PRIORITY*, *WORKFLOW / RUN* (workflow status + run label, *⊘ Blocked* when flagged), *DEPENDENCY / READINESS*,
  open. A click on a row opens the task; the checkbox only selects. Rows have no menu.
- **Readiness texts:** *Ready to queue*, *Needs a brief*, *Blocked · KON-29 unfinished*, *Waiting for its run to
  start*, *Agent at work*, *Waiting for you*, *Raise the limit to continue*, *Every step passed*, *Merged into main*,
  *Run #131 could not be read*, held reasons (*Held · a task failed*, *Waiting for an agent slot*, *Waiting for the
  daily budget*, *Waiting for the previous task*, *Canceled · not started by its group*).
- **Selection bar** (hidden until something is selected): select-all (all, some, none), *3 selected*, the readiness
  summary (*3 ready to queue · 0 incomplete · 2 already queued or started*), then *Return to drafts* (only for held,
  never-started tasks), *Set pipeline*, primary **Run selected · 3**; second line *Set status*, *Set priority*, *Add
  tag*, *Remove tag*. Outcomes appear under it: one summary line for success (*Status set to Todo for 3 tasks.*), one
  line per task that failed, and problem lines (*2 tasks held after a task failed · Return to drafts*). *Run
  selected* is unavailable unless every selected task is a complete draft with a published pipeline and no run.
- **Run selected → scheduling review** (a page): *Execution order* (drag, or *Task to reorder* with *Move up*, *Move
  down*, *Undo*), *Execution mode* as two option cards **Parallel** (*Run ready tasks together.*, *Maximum concurrent
  tasks*) and **Sequential** (*One task at a time, in order.*, *Start next task when: Previous task is ready for
  review*), *If a task fails: Pause remaining tasks*, *Token budget / task*, *Time limit / task*, *Group token cap*,
  the pinned *Pipeline version* per task, a notice that pipeline approvals stay in place, a footer with task count
  and concurrency, **Queue N tasks** and *Back to tasks*. The review stacks into one column on narrow windows.
- **Tag management** (*Manage tags*, a page): project tags with task counts; *New tag*, *Rename*, *Merge* (into
  another tag, confirm), *Delete* (confirm; tasks keep their other tags); duplicate and blank name errors; load error;
  empty list.
- **Bulk confirmations:** *Set status → Canceled* with active runs names the runs that will stop and that work stays
  on their branches; partial failures name each task.
- **States:** loading, load error (never shown as an empty list), empty project, no matches for the query (*No tasks
  in this tab match the search or pipeline filter.* with *Clear search and filter*), a newly created task's row
  highlighted, narrow window (drop Pipeline, then tag chips, then Priority; the title truncates; the selection bar
  wraps to three lines), long titles, held groups, unreadable runs.

### 4.3 New task and Edit task

- **New task** (*What would you like to build?*), one page:
  - *Project · current context*; *Task title* (*Name the outcome*); *Describe the outcome* (brief: *What should
    change, and why?*); *Acceptance criteria* (a list; *+ Add criterion*; remove × on hover or focus).
  - **Organization:** *Status* (default Backlog), *Priority* (default No priority), *Tags* (picker: search, select,
    create while tagging; duplicates offer the existing tag), *Blocked* switch with a required *Reason* and optional
    *Blocking tasks* (search, add, remove; a task can't block itself or form a cycle).
  - *Context*: *Attach context* (**unavailable**, planned).
  - **Pipeline** select (published pipelines only; *No published pipelines in this project*), its *Execution plan*:
    each step with its executor (*Explorer · Claude / Opus*, *Tool · Run tests*, *· waits for approval*), *View
    pipeline ↗*, and the approval note (*Approval is required for Plan.* / *This pipeline has no approval gates.*).
  - **Limits:** *Run budget* (whole number with a *tokens* unit, grouped while typing, read-back *= 1.4M tokens ·
    input + output*), *Time limit* (*30 minutes*, read-back *active agent time*). Both are optional with no maximum:
    empty reads back *No limit* (user decision 2026-10-09).
  - Footer: *Project, agent and pipeline instructions are included automatically.* with *Preview* (unavailable).
  - Actions: primary **Create & run**, and a quiet **Create** (saves a draft and returns to Tasks with the new row
    highlighted). Unavailable reasons appear under the actions (no title, invalid limits, blocked by unfinished tasks:
    *Run unavailable*).
  - **Top status:** *Not saved yet*, *Creating the task…*, *Task queued*, *Starting the run…*.
  - **Validation:** *Use digits only, for example 1,400,000.*, *Enter a budget above 0 tokens.*, *This number is too
    large for a token budget.*; an empty budget or time limit is no limit, not an error.
- **Queued, run not started:** when the runner refuses to start the run, the form stays: *Queued · run not
  started*, fields locked, the runner's reason under the action (*The repository has no commit to start the run
  from.*), primary **Retry run** (never creates a second run); while retrying *Starting the run…*; if the runner stops
  answering *Task saved · run status unconfirmed*. Reopened later it says *No run is recorded for this task…*.
- **Edit task** (*Edit task*, *Changes apply to draft KON-7. Nothing runs until you choose Create & run.*): the same
  fields filled; *Save changes* (unavailable until something differs; topbar *Unsaved changes* / *Draft saved
  locally*); a draft keeps *Create & run*. A task that already ran: changes apply to future runs only, no *Create &
  run*.
- **Leaving with unsaved changes:** *Discard changes?* · *Keep editing* / *Discard changes*.
- **Conflict** (shared projects; built): someone saved a newer version; both versions of each field side by side,
  one choice per field.

### 4.4 Task page

The task page has a **header**, then tabs **Summary · Conversation · Changes · Artifacts** (Artifacts **unavailable**,
planned; *Memory* tab planned).

#### Header and organization

- Breadcrumb *Kontur / Tasks / KON-12*, title, run line *Run #128 · Feature delivery v3 · Finished at 14:32* (or *Run
  128 · Attempt 1 · Implement · Builder working*).
- **Organization row:** status picker (six statuses; choosing Canceled with an active run confirms first, naming the
  run and that work stays on its branch; *Stopping…*, stop failure with *Retry*), priority picker, tag chips with
  *Edit tags*, Blocked flag. The run label sits beside the status (*In progress · Needs you*).
- **Run controls** (depend on state): *Pause run*, *Resume run*, *Continue run*, *Cancel run*, *Retry run*, *Run
  again* (failed or cancelled runs; confirmation *Start run #132 of KON-31?* listing pipeline (latest published),
  base, new branch, new worktree, limits, and a switch to remove the old run's worktree), *Remove worktree*, *Edit*.
  An unavailable control stays in place with a lock and a reason line (*Run again is unavailable: run #132 of this
  task is running.*, *…Feature delivery has no published version any more.*, *…the daily token budget is used.*).
- **Auto-approving** line under the heading while the run auto-approves, with **Stop**.
- **Notices** under the heading: unreadable earlier runs, blocked, *Daily token budget used* refusal.

#### Summary

The run's state and decisions, top to bottom:

- **State notice:** *Implement is running* (the agent works in the run's own worktree; your checkout is not
  touched), *This task is a draft*, *The run has not started*, *Paused at checkpoint 4b92ad0*, *Stopped at the token
  limit · checkpoint 7c1e9f2*, *The run was interrupted at Implement*, *The run failed at Run checks*, *The run was
  cancelled*, *Builder asks you a question*, *Your approval is needed*, *Builder waits for your answer · docs
  update_page* with **Review request**, *Run #131 could not be read*.
- **Step progress:** one row per step: *Research · Explorer* · *Complete · attempt 1 · 84k tokens · 3 min*,
  *Working…*, *Waiting*, *Needs you*, *Waiting for you · cargo add · since 14:06*, *Paused*, *Stopped at limit*,
  *Interrupted · can't continue*, *Failed*, *Skipped*, *Not started*.
- **Brief** and **Acceptance criteria** (numbered) of this run.
- **This run** side panel: Pipeline, Branch, Base, Last checkpoint, Model, Budget (*412k / 1.4M tokens*), Cached
  input (*not counted*), Time limit, Active agent time, **Permissions** row (*Running with · Builder · Edit files · 3
  commands · Network off · MCP asks · change ↗*; *Last ran with*, *Next run uses*), Worktree.
  - **Approvals** section: the switch *Approve everything in this run* with its note; the **Auto mode** option with
    *Reviewed by <model> on <connection>* (or why it is unavailable); counts by who answered (*Approved by you*, *By
    similar rule*, *Always allowed*, *Allowed by the profile*, *Auto-approved*, *Approved by auto mode*, *Auto mode
    asked you*, *Approved by automation*, *Denied by you*, *Denied by automation*, *Waiting*); *Approved as similar in
    this run* with **Revoke** per rule (*revoked at 12:22 · approved 3 before*); *Always allowed during this run*.
- **Plan review** (an approval gate waits): *Plan v1 · <task>* rendered as Markdown, *Plan version* select when there
  are several, the task's criteria, a note box for changes, **Request changes** (needs a note) and **Approve plan &
  continue**.
- **Raise the limit** (stopped at a token, time or step budget): what was used (*Used 1.4M of 1.4M · Implement stopped
  mid-step*, *Used 24 of 30 min of active agent time*), a whole-number input with read-back (*= 2M tokens · 600k more
  than now*), what continuing does, primary **Continue with 2M tokens**. Stopped at the **daily project budget**: no
  panel, just **Resume run**; resuming while still used shows the refusal.
- **Run waits for a person after retries** (*Review still fails after 2 retries*, or *Implement stopped again without
  a result*): the evidence (*Latest attempt · 6a71bd4*, the steps of that attempt, *What Review reported* in the
  agent's words, *Changes and messages are preserved*) and a decision panel: *Tokens used*, *Remaining budget*,
  **Allow 1 retry & continue**, *Return the work to* (step select, default the step a retry resumes) with a required
  note and *Return to Implement with note*, and **Stop and fail the run**; a refusal from the runner appears above it.
- **Ready for review:** the card *Ready for your review · Every step of the run passed. Its changes wait for your
  review.*, the stages, *What changed* (*4 files changed · +120 / −18*), *Verification* (*commit 8f3a2c1*, each step's
  result), *Run details* (project, base revision, working branch, duration, tokens used, agents on this run), then
  actions **Ask agent**, **Merge into main**, *Review PR* (**unavailable**: *Review PR needs a pull request, which no
  pipeline step opens yet.*). Merge states: *Merging…*, refused or conflict (*Not merged into main · The run's changes
  conflict with your branch in src/auth/github.ts… The merge was undone.*), unavailable (*this run didn't record the
  branch your checkout had*), merged (*Merged into main · main moved forward to fa7e675 at 16:35. Its worktrees and
  merged branches were removed.*). Native runs add *What Orchester could not enforce for Builder*.
- **Runs of this task** (every task with a run): newest first, each *Run #N* (*this run*), its status, what happened
  (*Cancelled at Implement · work kept on kon-switcher*), the worktree line (*Worktree · 2 files not committed ·
  kon-github-auth*, *Worktree removed Oct 3, 16:40 · kon-x kept*, *Worktree in use · Remove worktree comes back when the
  run ends*, *No worktree*, *Folder left behind…*), when, and **Remove worktree** for ended runs. One removal at a time;
  *Removing…*; git's error under the row. At 960 the row stacks.
- **Organization history** at the end: who changed status, priority, tags or blocking, when, from and to; automatic
  changes are attributed to Orchester.
- **Unreadable run:** *Run #131 could not be read* with what could not be read and that the database is unchanged; a
  lock line (*Cancel, pause, continue, merge and Remove worktree are unavailable for run #131 until it can be read.*);
  the other runs listed; no side panel.
- The Summary column scrolls; decision actions stay visible.

#### Conversation (a chat with the run)

One timeline per run, where the person follows the agents and talks to them.

- **Layout:** the chat on the left, a **step rail** on the right (*Steps* with a count; *All steps* first, then one row
  per step: status icon, number, name, one meta line such as *Explorer · 41k · 6 min*, *Planner · waiting for
  approval*, *Tool · exit 1 · 2 min*, *Reviewer · returned to Implement*, *Builder · not started*). Picking a row
  filters the chat (*Showing 03 Implement only · 01, 02 and 04–07 are hidden · Show all steps*). The rail collapses
  to a narrow strip of step numbers with status (by hand, and by default on narrow windows); at 960 the strip opens
  the full rail over the chat.
- **Toolbar:** *Show* segmented control **All / Messages / Tools**, *Search this run…*, the run's state chip
  (*Following live*, *Not following*, *Waiting for you*, *Waits for your decision*, *Queued*, *Ready for review*,
  *Failed*, *Paused*, *Stopped at limit*, *Interrupted*, *Merged*, *Cancelled*) and **Details**.
- **Messages:** chat rows with an avatar (agent, you, a question, a plan, a tool step), name, step and time, text.
  Your messages carry a **delivery line**: *Sending now · Builder gets it at its next tool call*, *Delivered mid-turn
  to Builder · 12:05*, *Queued · Builder reads it when its turn ends*, *Delivered at end of turn to Builder*, *Queued ·
  waiting for the run to start*, *Queued · delivered when the run continues*, *Could not deliver to Builder: <why>.
  Your message is kept here.* (with *Send after this turn* and *Edit in composer*), *Not delivered · the run's
  worktree was removed*.
- **Tool chips:** an agent message is followed by one chip per group of tool calls (*Read 4 files · searched 2 times ·
  ran pnpm test · edited 3 files · 12 calls*); the running one shows *running pnpm test* with a loader; a failed
  command marks the chip (*· exit 1*). A chip opens in place as a list of tool rows with times (7 rows, then *Show N
  more · 4 reads, 1 search*). Tool rows: commands with exit code and duration and collapsible output (open on
  failure, long output shows its end first with *Show all*), file reads, searches, file changes (*M · open in
  Changes ↗*), MCP calls, memory searches, reasoning summaries, usage reports (*in · out · cached (not counted)*),
  unparsed lines (counted, never dropped). Each asked-about command carries a **decision mark** (*approved by policy
  · Worktree*, *approved by you · Outside write*, *approved by similar rule · Network*, *always allowed · cargo add*,
  *allowed by the profile · Read-only*, *auto-approved · Worktree*, *approved by auto mode · Network* with the
  reviewer's reason under it, *asked you · Unknown*, *denied by you · the agent continued*, *not asked · exempt tool
  of docs*).
- **Step dividers:** a thin divider starts a step (*07 Docs & cleanup · Builder · Codex / Sol · started 16:31 · 38k
  tokens*, status; the tokens open Details on that step) and ends it (*06 Fix notes done · 16:30 · Changes · 3 files
  ↗*). Finished steps fold into one line (*01–05 · five steps done · Show them*). Steps that never started share one
  muted divider. **Returns and restarts** are dividers too: *Review returned the work to 03 Implement · attempt 2 ·
  15:12*, *Restarted Implement · it stopped without a result*, *Returned to Implement with your note* (your note
  quoted). The run's end: *Run 128 is ready for your review · 16:52*.
- **Cards inside the asking agent's message:** a **question** (*Builder asks you* · *Waiting for your answer* →
  *Answered*); a **plan** (*Plan v1 · <task>* · *Waiting for approval*, the first lines, *Show the whole plan · 24
  lines*, **Approve plan & continue** with what approving starts; changes are requested from the composer); a **tool
  approval** (see 4.5; *Ended without an answer* when its agent stopped waiting). A step's **result card**: *Review
  passed* / *Run checks failed* / *Research done*, what was reported, duration and tokens, links *Changes ↗*,
  *Summary ↗*.
- **Problem rows** never fold into a chip: *Command denied* (what the agent was told), *Working without MCP server
  docs* (why), runner problems, *The step's handoff was not saved to task memory*.
- **Working line** at the end: *Builder is working · last event 2 s ago* (a real time); while an approval waits
  *Builder waits for your answer · since 12:19, its active time stopped*.
- **Messages view** hides chips and counts them (*31 tool calls in 4 groups hidden · Show all*). **Tools view** is the
  flat transcript of every tool event with times, and a *Show raw lines* switch.
- **Search:** matches messages, commands, paths and output; non-matching items hide; a matching chip opens with its
  matches (*2 of 12 calls match*, *Show 10 without a match*); the rail shows match counts per step (*no match*);
  nothing found: *Nothing in Run 128 matches "refresh token"* with *Clear search*.
- **Following live:** while the end is in view the chat follows new items. Scrolling up shows *Not following* and a
  floating **Jump to latest · 3 new**; jumping or sending resumes. Opening the tab starts at a card that waits for
  you, else at the end.
- **Details pane** (replaces the rail; *Steps* brings the rail back): the selected step's invocation: step and
  attempt, agent and model, started / ran, *Code index* (*4d2b8e0 · main*, *Off · project setting*, *Unavailable ·
  still building*), *Memory given* (planned), tokens (input, output, cached, counted), run budget, *Tokens before
  first edit*, the permissions it ran with, **Command decisions** (totals and a table by category; a row filters the
  Tools view), similar rules with *Revoke*. Closed by default; remembered per person.
- **States:** loading (*Loading activity for Run 128*, no fake skeleton entries), couldn't load (*Open runner log*,
  *Try again*), queued run not started (*Nothing in this conversation yet* with the steps on one line and why it
  waits; toolbar controls unavailable), a task with no run (messages wait for the first step), unreadable run
  (*Nothing this run did can be shown until its record can be read.*), narrow window (toolbar wraps to two lines;
  rail is a strip).

#### The composer

Always visible at the bottom of the Conversation tab: one box with the text field on top and a row inside it.

- **Row:** **+** Attach (**unavailable**: *Attaching files is not available yet*), the **permissions chip** (this
  run's permissions, *Read-only* / *Edit files · asks for the rest*; it must stand out once the run allows more than
  its profile, e.g. *Network on · approves similar (2)*; it opens the run permissions popover), the **recipient** as a
  quiet selector (*Builder · working*; a lock when fixed), and a round **send** button whose icon says what happens
  (send now, or queue).
- **Recipient menu:** *To: Orchestrator · this task*, then each agent of the run's pipeline with its state.
- **Send now or after this turn:** while the agent is mid-turn, Send / Enter sends now (it reaches the agent at its
  next tool call); a chevron beside Send offers *Send now · Enter* and *Send after this turn · Ctrl+Enter* for that
  message only. Shift+Enter adds a line. Connections that can't take a message mid-turn offer only *Send after this
  turn* (Send now unavailable with the reason).
- **Context lines above the box** (with × to leave the mode): *Answering Builder's question from 16:42* (recipient
  fixed, **Send answer**), *Changes to plan v1 · Planner writes plan v2 from your note* (**Request changes**),
  *Continue 03 Implement with your instructions · attempt 3* (failed or stopped run: **Continue with instructions**;
  *Run again* stays in the header), *Waiting for your decision · Open the decision →*.
- **Per state:** working (send now / after turn), tool step or other agent (queues: *Queued · waiting for Reviewer's
  next turn*), ready for review (**Send follow-up**: the agent resumes its session; the run is ready again when done),
  paused or interrupted (queues until it continues), queued not started (**Send when the run starts**), closed
  (merged, worktree removed, no agent step: no controls, a lock and the reason, e.g. *This run's work was merged.
  Messages no longer reach its agents.*).
- **Placeholders:** *Message Builder…*, *Message an agent of this run…*, *Describe the changes the plan needs…*, *Ask a
  question or request a follow-up…*, *Write to the first agent of the run…*.
- One quiet line under the box only when it changes the outcome (queued, follow-up, continue starts a new attempt);
  other help lives in the send button's tooltip. Drafts survive failed sends; a retry never sends twice.
- **Continue a failed run:** recipient limited to agents that already ran; *Your message continues this run from its
  saved work. The agent resumes its session when available; dependent steps run again.*; a run with no agent step
  can't be continued (*This run has no agent step to continue. Its saved work remains on its branch.*).

#### Run permissions popover

Opened from the permissions chip or the *change ↗* link: agent, step, model, profile version; *Running with …*;
Repository access, Commands (allowed list chips), Network, *Ask before MCP actions*, MCP servers (from the profile,
not changeable per run); changed values highlighted; the timing rule (*changes apply from the next agent invocation;
the invocation running now keeps the permissions it started with*); **Apply from next invocation** and *Apply now*
(restarts the step from a checkpoint); *Also save to the Builder profile*; notices *Saved for the next invocation ·
pnpm build added*, *Applying pnpm build added at the next checkpoint…* (*Keep for next invocation instead*),
*Restarting with the new permissions…*, *New permissions not active: Builder did not restart*; an **Auto mode**
section (*Use auto mode in this run*, *Saves at once…*). An ended run's popover is read-only with only *Close*. On
short windows it scrolls.

#### Changes

- The run's diff from its base revision to its latest checkpoint: a file list (path, change type, +/−) and a unified
  diff with line numbers and syntax colouring; a toolbar naming both revisions, the checkpoint's step and time,
  *Checkpointed*.
- *NOT CHECKPOINTED*: what the worktree holds beyond the checkpoint, read again while an agent works.
- The tab label counts files (*Changes 4*); no count while reading or on failure.
- **States:** *No checkpoint yet*, too large to show (*more than 300 files or 20,000 lines*, measured not shown),
  couldn't read the changes not checkpointed, *The run's worktree was removed* (only committed work shown), long
  lines cut with *Line shortened*.

### 4.5 Approvals

#### The approval dialog and inline card

A request appears as a **modal dialog** when the person is elsewhere, and as an **inline card** in the
asking agent's message on the task's Conversation tab.

- **Title:** *Approve command?* or *Approve tool action?*; a one-sentence explanation of what the policy asks about.
- **Subject line:** a **category chip** for a command (*Read-only*, *Worktree*, *Network*, *Outside write*, *History /
  destructive*, *Credential read*, *Unknown*) or the MCP server's chip; what made it ask (*Not an allowed command ·
  Builder v3*, *Asks before commands*, *Asks to leave the sandbox*, *Asks before MCP actions*); *waiting since 12:11*.
- **Request:** the reason (*Writes outside the worktree: /tmp/worker_orig.rs*, *Installs a package from the network:
  crates.io · serde*); the **whole command** or tool *Action*, *Target* and *Arguments* (pretty-printed JSON), never
  shortened: long values sit in a scrolling monospace box with **Copy** on the label row (capped height, sideways
  scroll); *Agent* (*Builder · Implement · Run 142 · KON-42*), *Working directory*, *Agent's note*. For a compound
  command, the parts are listed with their categories and the part that forced the ask is marked (*asks you · forced
  by part 2 of 3*). For Unknown: *What Orchester read*, part by part, and where it stopped. A native shell command
  shows the shell (*pwsh · -NoProfile -NonInteractive -Command*), the time limit and *Runs outside any sandbox*.
- **Auto mode line** when auto mode left it to the person: *Auto mode asked you: "…"* or *Auto mode left this to you:
  it reads a credential location.*
- **Deny's message, word for word:** *Deny tells Builder: "Denied by the person reviewing this run. Continue without
  it."* Deny never fails the step.
- **Answers:**
  - footer: **Decide later** (Esc) at the left, **Deny**, primary **Approve once** (Enter, focused on open);
  - *Or answer more widely* — two tiles, each saying what it covers before it is chosen:
    - **Approve similar in this run** (*cargo add commands in Network, until Run 142 ends. Flags and packages may
      differ…* / *docs update_page, every call until Run 146 ends. Starts now, for this invocation too.*); hover
      or focus shows its scope (*Runs without asking* / *Still asks you* with examples);
    - **Always allow…** (*Add cargo add to Builder's Allowed commands, in a new profile version* / *Add update_page
      to docs's tools that run without asking, for every profile that uses docs*);
  - *Or stop asking for this run* — **Approve everything in this run** (shown on and unavailable when the run already
    auto-approves, with why this request still waits).
  - Unavailable tiles stay in place, faded, with a lock and the reason (*Several parts ask: approve once or deny.*,
    *Unavailable for a command that reads credentials*, *Its prefix would also allow riskier forms…*), so the dialog
    keeps its shape.
- **Always allow, second step:** *Always allow cargo add for Builder?*; the prefix to add (from program + subcommand to
  the exact command; dangerous prefixes locked); *Adds cargo add to Builder › Permissions › Allowed commands,
  published as Builder v4*; *Runs that start from now use v4. Run 142 keeps v3 and also allows cargo add until it
  ends.*; the unsaved draft is kept; pipelines that use Builder. For a tool: what is added, where (*Connections › MCP
  servers › docs › Tools that run without asking*) and which profiles it affects. **Back** (Esc) and **Add to v4 and
  approve** / **Add to docs and approve**.
- **Several waiting:** one request at a time, oldest first, *1 of 3 waiting* with ‹ ›. Answering shows the next; the
  dialog closes when none is left. A request whose agent stopped waiting drops out with *The previous request ended
  without an answer: its agent is no longer waiting.*
- **Decide later:** closes the dialog without an answer; the agent keeps waiting with its active time stopped; the
  request stays in *Needs you*, on the task page (*Review request* card, step row *Waiting for you · …*) and in the
  status bar (*3 approvals waiting · … · Review*). A new request then does not reopen the dialog by itself. A click on
  the backdrop does nothing.
- **Inline card** keeps a one-row set of answers (*Always allow…*, *Deny*, *Approve similar in this run*, *Approve
  once*) that wraps on narrow chats; once decided it reads *Approved*, *Denied*, *Answered by the profile*, *Answered
  by automation* or *Ended without an answer*.
- **Automation** (Orchester's MCP clients) may only approve once or deny, and only what auto-approval would approve:
  *Only a person can approve this request: it reaches the network. Automation may deny it.*

#### Approvals in a run and auto-approval

- Per run: *Approve everything in this run* (switch in the run's Approvals section, or the dialog's fifth answer).
  Orchester then answers that run's requests itself: commands that read or stay in the worktree and MCP calls. It
  never auto-approves network commands, writes outside the worktree, credential reads, installs, history rewrites or
  anything it can't read to the end.
- Per profile: *Approve automatically (don't ask me)* under the profile's *When the agent asks*.
- While a run auto-approves: *Auto-approving* under the task heading and in the status bar, each with **Stop**.
- **Auto mode:** a separate **reviewer model** judges the requests the rules leave to the person and approves only
  those that clearly serve the task; it fails closed (timeout, unreadable answer → asks the person). Turned on per
  run (*Use auto mode in this run*) or for new runs in Project settings. Answers read *approved by auto mode*, with
  the reviewer's reason; requests it leaves to the person say why. Unavailable states: *Auto mode needs an agent
  connection in this project to review with…*, *The reviewer connection chosen in Project settings is gone.*
- **Planned:** a per-category policy (*Allow · Ask me · Deny* for each of the seven categories, per profile, narrowed
  per step and per run, with *Reset to defaults*); a waiting-request timeout.

### 4.6 Questions, plans and Ready for review

- **Question:** an agent asks; the run waits (*Needs input*). It counts in *Needs you*; the Overview card *Answer
  question →* and the status bar *Builder asks · KON-12* open the Conversation with the question card and the composer
  in answer mode. The answer resumes the agent at once.
- **Plan:** a gated step proposes *Plan vN*; the run waits. Review it on the Summary (Markdown, versions, criteria) or
  in the Conversation card. **Approve plan & continue** starts the next step; **Request changes** (a note is
  required) sends the plan back for vN+1. Earlier versions read as *Earlier version*. Pause is not offered while a
  run only waits.
- **Ready for review:** every step passed. The person merges (*Merge into main*), asks for a follow-up (*Ask agent*,
  which opens the composer), removes the worktree, or runs again. The status bar's event *Run #128 ready for review*
  opens it.
- **Planned delivery:** a *Deliver* step at the end of a pipeline (*Approve to merge*, *Auto-merge*, *Pull request*
  coming later; merge commit or squash; re-check on the merged result), a per-project **Merge queue** (queued,
  syncing with main, re-checking, integrating conflicts, waiting for approval, blocked by uncommitted changes in the
  checkout, failed, merged), a *Ready to merge* dialog (changes, checks, commits; *Reject*, *Open changes*, *Merge
  into main*) and an *Integrate failed* dialog (*Resolve myself*). Overview cards *READY TO MERGE* and *CONFLICTS NEED
  YOU*.

### 4.7 Pipelines

- **Library:** rows with name, step summary (*Research → Plan → Implement → Run checks ∥ Review → Create pull
  request*), step count, version state (*Published v1*, *v3 · Draft v4 saved*, *Draft v1 · not published*), tasks that
  use it, last published. *New pipeline*, *Import from project*. Side note *How versions work*: *Draft saved →
  Publish vN → Runs keep their version*.
- **Empty library:** *No pipelines yet*, what a pipeline is, the starter *Feature delivery* with its steps, **Create
  from starter** and *Start empty* (both create draft v1 only).
- **New pipeline** dialog: *Name* (*e.g. Dependency updates*), the generated *Slug*, *Start from* (option cards:
  *Starter · Feature delivery* with its chain, *Empty pipeline*), **Create draft**. Error: *The slug dependency-updates
  is already used by the pipeline Dependency updates. Choose another name.*
- **Editor:** a canvas of step cards starting from *ON TASK START*; steps in sequence and *RUN IN PARALLEL* groups
  (*Both steps must pass*); returning edges for failure routes. Each card: name, executor (profile or tool), model,
  badges (*Waits for your approval*, *Can change files* / *Reads the repository*, *Its exit code decides the step*).
  Toolbar: *Add step*, *Pipeline settings*, *Version history*. Top: version and save status (*Draft saved ·
  Published v3*, *Saving draft…*), **Run v3** (runs the published version), primary **Publish v4**.
- **Step inspector:** *Project agent profile* or *Tool* select, *Model for this step* (inherits the profile's by
  default; override shows the effective connection and model), *Instructions*, *Runs after*, *When changes are
  requested* / *When the step fails* (route to a step, or *Fail the run*), *Maximum retries*, *Step budget* (tokens),
  approval gate, *Repository access* (*Read-only* / *Can write*), **Permissions** (*Commands · narrowed*, Network,
  *Ask when a tool needs approval*; *Steps can only narrow profile permissions.*, and a note of what the profile also
  allows), *Duplicate step*, *Remove step*. Missing pieces: *No agent profile*, *Choose a profile before publishing*,
  *Profile not published*, *This tool can't run in this version yet*.
- **Pipeline inspector** (nothing selected): name, slug, description, *Starts · On task start*, *Published version*,
  *Used by*, *Steps*, and *Not available to tasks yet* until v1 is published.
- **Empty pipeline:** a slot *Add first step · Agent profile or tool*, *Use starter steps*; Run and Publish
  unavailable (*Add a step before you can publish*).
- **Publish blocked:** a strip *Publishing is blocked by 2 problems* (each problem selects its step or edge: a step
  without a profile, a cycle), the cycle edge labelled on the canvas, Publish unavailable, *Run v3* still available.
- **Publish confirmation:** *Published now: v3* → *After publishing: v4*, the changes, and that started or queued runs
  keep v3. **Published v4:** the label changes, the next edit starts draft v5, Publish becomes *Version history*.
- **Version history** (a panel in place of the inspector; the canvas becomes read-only, *Back to editing*): versions
  newest first (*v4 · LATEST · 2d ago · 3 changes · 2 runs*), the selected one's changes and *Runs that pinned it*,
  *Compare with draft v5* (changed steps tagged on the canvas, *Hide comparison*), *Restore v4 as draft* (asks again
  when it would replace a draft). States: loading, failed, first version, draft with problems can't be compared.
- **Planned:** presets (*Add from preset*: built-in bundles of profiles, pipelines and tool placeholders created as
  drafts, a preview with renames, a *Finish setting up* checklist, *Save as preset*).

### 4.8 Agents

- **Profile list:** name, responsibility, connection and model (*Claude · Kontur / Opus 5.5*), version state,
  *Missing connection*. *New agent*, *Import agents*. Empty: *No agent profiles yet* (*Open connections* when there is
  no connection).
- **Editor header:** save status and actions follow the draft: *Published v2* (Publish v3 unavailable), *Draft saved ·
  Published v2* (Publish v3), *Unsaved changes · Published v2* (*Save draft*; publish after saving), *Draft based on
  v1 · Published v2*, *Not published*. A one-line permissions summary stays visible on every tab (*Edits files in the
  task worktree · 3 allowed commands · Network off*).
- **Tabs:** *Profile*, *Permissions*, *Used in pipelines* (count).
- **Profile tab:** *Profile name*, *Responsibility*, *Connection & model* (*Project connection* select, *Model* select
  with *Other model id…*, *Reasoning effort* with *CLI default*; a notice when the CLI is too old for the model),
  *Role instructions*. *Runs keep the exact assignment.*
- **Permissions tab:**
  - *Repository access*: option cards *Read-only* / *Edit files in the task worktree*.
  - *Extra writable folders* (only with Edit files): field, *Add folder*, one entry per line with ×; refusals (*Your
    home folder cannot be writable.*).
  - *Commands*: *Ask before running commands* / *Allowed commands* (chips with ×, *Add command*, *e.g. pnpm test*;
    entries match by prefix) / *Any command in the worktree*.
  - *Network*: switch (*Off* / *On*).
  - *MCP servers*: one switch per project server with where it runs; a reason under a server agents would work
    without; *Add an MCP server* when there are none.
  - *When the agent asks*: *Approve automatically (don't ask me)*; *MCP approvals*: *Ask before MCP actions that
    require approval*.
  - **Unavailable options:** an option the connection's CLI can't enforce is disabled with a lock and the reason
    (*Unavailable on this connection: Claude Code can't confine commands to the worktree.*); a setting may be locked on
    (*Network stays on*). If no option is enforceable, publishing is blocked with *Change connection*.
  - **Enforcement panel** (*Enforced by Codex* / *Claude Code* / *Enforced by Orchester*, *checked 14:02*): each
    setting with the mechanism and whether it is enforced (*sandbox · workspace-write*, *allowed tools · 3 rules ·
    others ask*, *Network stays on · off is not enforceable*, *Only the chosen MCP servers run*, *Servers work outside
    these limits*, *Repository agent files off · enforced*, *Your own Codex rules still apply*). No bypass or "full
    auto" exists anywhere; the panel says so. *Applies to new runs after publishing.*
- **Used in pipelines:** the pipelines and steps that use the profile.
- **Flow:** edit → *Save draft* → **Publish v3** → new runs use v3; running runs keep their version.

### 4.9 Instructions, Skills, Tools, Secrets, Connections, MCP servers

#### Instructions

- **Scopes** (tabs): *Project rules*, *Agent roles*, *Pipeline overrides*, *Step overrides*. A list of rules per
  scope with version state.
- **Editor:** the rule text, which agent roles it targets, the injection event (e.g. agent start / resume), draft and
  publish (*Draft saved · Published v3*, **Publish v4**). *Import from repository*.
- **Effective prompt preview:** for a chosen task, agent and step, the sources in assembly order (agent role →
  project rules → pipeline → step → task context), each with its version; excluded rules named (*Review checklist
  v2 targets Reviewer*). *View full prompt* opens a read-only viewer with sources; *Copy full prompt* (never
  secrets).
- **Variables catalog** beside the editor: search, grouped by Task, Project, Agent, Pipeline, Run, Step; each with
  type, explanation, current value and availability; **Insert** at the cursor (`{{task.title}}`). A variable not
  available at the chosen event says so and offers a later event; it never becomes empty text.

#### Skills

- **Library:** name, description, *Files*, *Version*, *Scope* (*Project*, *Role · Reviewer*, *Step · Run checks*,
  *Pipeline · Feature delivery*), *Last published*; tabs *All skills*, *Project*, *Agent roles*, *Pipelines & steps*;
  *New skill*, *Import from repository*.
- **Editor:** *Name*, *Scope*, *Description* (what an agent reads to decide whether to load it), *Body* (Markdown),
  reference files (add, remove), where it was imported from, *Publishing applies to new runs. Active runs keep v3.*
- **Import from repository** dialog: the found items (*AGENTS.md*, `.claude/skills/*`, `.agents/skills/*`) in two
  groups (instructions, skills) with checkboxes and what each becomes; a preview of the open item; items with nothing
  to import disabled with why; *Creates drafts only: nothing is published and no repository file changes*; *4 of 6
  selected*; **Create 4 drafts**. Result: *4 drafts created*, the list, *NOT IMPORTED* with why, *Open Instructions*,
  *Review skills*.
- **Empty:** *No skills in Kontur yet*, with a *Found in this repository · 6 items* card when agent files exist.

#### Tools

- **Library:** tools with kind (*Command*, *HTTP request*, *Database*), status (*Ready*, *Draft*, *Unavailable*),
  secrets (*No secrets*, *Needs a secret*), used by; filter *All types*; *Add tool* (*Command*, *HTTP request*,
  *Database query*, *Import from MCP*, *Create pull request*), *Import from project*. Only **command tools** are
  editable today; other kinds are visibly unavailable (*This tool can't be edited in this version*).
- **Command tool editor:** *Name*, *Description*, *Program*, *Arguments · one per line*, *Working directory* (*The
  worktree of the step that runs it*), *Timeout · seconds*, *Exit codes that pass*, *Output kept · KiB*, *Variables ·
  NAME=value per line*; tabs *Configuration*, *Inputs & outputs*, *Access*, *Usage log*; *Save draft*, publish.
- **Inspector:** *Owner project*, *Kind*, *Secrets*, *Access* (*Pipeline steps that call it*; *Access rules · not
  applied yet*), *Version*, *Used by*; *Runs keep the tool version they started with.*
- **Planned:** HTTP and database tools with credential bindings and access rules (allowed pipelines and steps,
  environment, per-call approval, access window).

#### Secrets

- **List:** name, kind (*Access token*, *API key*, *Database connection*, *Deployment token*, *Password*),
  environment (*Development*, *Staging*, *Production*), used by (*1 tool*, *2 MCP servers*, *3 resources*, *Unused*);
  filter *All environments*; tabs *Secrets*, *Usage log*; *Add secret*, *Import from project*.
- **Inspector:** the value masked, project, environment, *Last rotated* (*Never · added Sep 20*), *USED BY* (tools,
  then *MCP · name* links; which side edits the binding), *Recent access*, **Replace value**, *Archive secret*.
- **Add secret** dialog: *Name*, *Type*, *Secret value* (masked), *Project*, *Environment*, **Save secret**.
  Duplicate name: Save unavailable with *Open secret* for the existing one.
- **Archive** confirmation: who uses it (*Tools and MCP servers use this secret (2 total). They are not ready until
  they select another secret.*).
- Secret values never appear anywhere in clear after entry.

#### Connections

- **Tabs:** *All connections*, *Agent providers*, *GitHub*, *MCP servers*. Rows: name, what it is (*gh · github.com ·
  signed in as …*, *Codex CLI*, *Native · OpenAI API · Official · api.openai.com*), type, status.
- **Add connection:** *Kind* as four option cards: *Codex CLI* (*Runs the codex program, signed in on this
  computer.*), *Claude Code CLI*, *Anthropic API (native)*, *OpenAI API (native)* (*Orchester runs the agent on the
  Responses API with a project API key.*). CLI kinds: name, program (found on PATH with its version, or chosen;
  disabled options say *Not found on PATH*), the check result. **Native** form: *Preset* (*Official*; *DeepSeek* and
  *Custom* locked: *Compatible endpoints come in a later version*), *Base URL* (read-only until *Advanced*), *API key ·
  project secret* (pick one, or *New secret…* to create it in place), *Models* table (model, source *From the
  endpoint* / *Added by hand* / *From the preset*, context, max output, efforts) with *Fetch models* (unavailable) and
  *Add a model by hand*, *Default model*, *Default effort*, *Provider fallback* (switch, off). *Test connection* is
  unavailable for native kinds today. Field errors, the first reason beside the actions, **Save connection**.
- **CLI connection inspector:** name, status, *Provider*, *Program file* (path shortened in the middle), *Runs with*,
  *Script* (npm launchers only), *Version* (*Not checked yet*, *Unknown*), *Execution · This computer*,
  *Authentication · CLI login*, *Last checked*, *Available models* (chips) with *Refresh models*, *Used by* (profile
  links). Pinned actions: **Test connection**, *Edit connection*, and a status sentence. States: signed in, not signed
  in, CLI not found, CLI too old for a model, checking.
- **Native connection inspector:** *Provider*, *Preset*, *Endpoint* in full, data lines (*Prompts, repository content
  and command output are sent to api.deepseek.com.*), *Authentication · API key · KEY_NAME (Staging)*, *Execution ·
  This computer (Orchester runs the agent)*, *Model traffic*, *Balance*, *Last checked*, *Models*, *Used by*.
  Statuses: *Connected*, *Key missing* (*Choose a secret*, *Open Secrets*), *Key rejected*, *Billing problem*,
  *Endpoint unreachable*.
- **Models dialog** (*Models · Lab gateway · Kontur*): rows with id, default label, source, limits, *Edit*, *Remove*;
  editing in place (*Model id*, *Context window*, *Max output*, effort chips *low medium high xhigh max*); a model a
  profile or pipeline uses can't be removed and says who uses it; *Add a model*; *Save models*.
- **GitHub connection:** host, program and version, path, account, scopes, last checked, *Used by Kontur* (expected
  account, repository, remote, used by tool); *Check again*, *Change account or access*, *Edit connection*, *Choose
  program…*, *Use this account (X)*; states signed in, not signed in, gh not found, wrong account, unrecognized
  output, missing scope, each with its command row. *Add GitHub connection*: name, host (*github.com* or *Other
  host…*), program, *Uses gh's own sign-in*, the check row.
- **Planned:** *Send Kontur's work to DeepSeek?* data-location acknowledgement before the first run on a flagged
  preset; OAuth reconnect for remote MCP servers.

#### MCP servers

- **List:** name, *Program · npx* or *Remote MCP · mcp.example.dev*, how many agents get it, *On* / *Turned off*.
- **Inspector:** *Type* (*Program on this computer* / *Remote MCP*), *Program*, *Arguments*, *Starts in*, *Started by
  · The agent's CLI, with each agent* (or *Endpoint*, *Authentication · Bearer token from KNOWLEDGE_TOKEN (Staging)*,
  *Reached by*), *Variables* (secret ones by name and environment only), *Tools that run without asking* (or *None*),
  *Discovered tools* (*Orchester does not connect to MCP servers itself, so it cannot list their tools yet.*; *Refresh
  tools* unavailable), *Used by*. Pinned: *Test connection* (unavailable), *Edit connection*, *Turn off*.
- **Add / Edit MCP server** dialog: *Name* (*Agents know the server and its tools by this name.*), *Type* (*Program on
  this computer* / *Remote MCP*), *Program*, *Starts in*, *Arguments* (one per line), *Variables* (NAME=value per
  line, `${secret:NAME@environment}`) with *Add a variable from a secret…* (or *This project has no secrets*),
  *Endpoint*, *Authentication* select, *Tools that run without asking* (one name per line), the notice *Secret values
  stay out of arguments and logs*, *Choose which agent profiles get this server on their Permissions tab.* Errors:
  *Use small letters, digits, - and _, starting with a letter*, *Use a folder inside the worktree, or an absolute
  path*, *This project has no secret DOCS_KEY@Staging*, *Keep user names and passwords out of the URL; use a bearer
  token from a secret*.
- In a run, a server the agent works without is a problem row with the reason.

### 4.10 Pull requests

- **Inbox:** the project's one linked repository as a read-only label; sync status (*Synced just now*, *Syncing…*,
  *Last synced 14:02 · GitHub unreachable*, *GitHub is having trouble*, *Rate limited · resets in 12 min* with the
  local time in a tooltip and Refresh disabled, *Paused · gh signed in as sam-work*); **Refresh**; search and sort;
  tabs *Open*, *Needs your review*, *Agent working*, *Merged*, *Closed* with counts; columns *PULL REQUEST / LINKED
  TASK*, *STATE & REVIEW* (*Open*, *Draft*, *Merged*, *Closed*), *GITHUB CHECKS* (counts by state), *Open in GitHub*.
  Footer *4 of 8 open pull requests* while filtered. Sync runs on open, on Refresh and every minute while visible.
- **Unavailable:** *Pull requests are unavailable* (*Kontur isn't linked to a GitHub repository…*, *Open Connections*),
  no tabs. Wrong account and missing scope show their command row, *Check again*, *Use this account (X)*.
- **Detail:** title, number, branches, state, *Open in GitHub*, *Refresh*; tabs *Files changed* (file list, unified
  diff with syntax colouring and line numbers), *Agent feedback*, *Agent checks*, *Overview* (the last three
  **unavailable**, planned). States: reading, *Too large to show here*, *No changes*, couldn't reach GitHub, diff not
  readable, long files cut with a count.
- **Planned agent feedback:** select diff lines, *Ask a question* or *Request a fix* addressed to the linked task's
  agent (comments stay in Orchester, never posted to GitHub), threads with states (*Queued*, *Delivered*, *Working*,
  *Answered*, *Ready to verify*, *Resolved*), anchors that follow new commits, a draft kept when the PR changes.

### 4.11 Project settings

Tabs, each saving on change (*All changes saved* / *Saving…* / failure):

- **General**
  - *Project*: *Project name* (renaming not available yet), *Project folder* (*Git repository · moving the project is
    not available yet*).
  - *GitHub repository* (*github.com · origin · default branch main*, or *Not linked · Local only · pull request steps
    are unavailable* with *Detect repository*), *Check again*.
  - *Expected GitHub account* with the read-back (*gh is signed in as sam-work · pull requests blocked*) and *Use
    signed-in account (sam-work)* plus its explanatory notice, shown only on a mismatch.
  - *Agents and the repository* — **Repository agent files** (*Agent files*): switch; *On · agents may read them* with
    *Found: AGENTS.md, CLAUDE.md, 4 skills folders* and a panel of what each CLI loads; *Off · agents load none of
    them*; *Import from repository*.
  - **Auto mode:** *New runs use auto mode* (On / Off) and *Reviewer*: connection select (*Default · Claude · Kontur*),
    model select, effort select, *Reviewed by Haiku 4.5 on Claude · Kontur*, or a lock line saying why no connection can
    review.
- **Limits** (*New runs*): *Default run budget* (empty: no limit), *Daily project budget* (20M), *Default time limit*
  (select: *No limit*, the default, then 15, 30, 45, 60, 90, 120, 180, 240 minutes); no maximum run budget (user
  decision 2026-10-09); notice *Changes apply to new runs*.
  Out-of-range errors name the value still saved.
- **Environment:** *Variables · NAME=value per line* in a monospace area; *Stored and shown in plain text, so never
  put secrets here.*; *Changes apply to the next agent or command that starts*; invalid line error.
- **Code index:** *Use code index* switch (*On · agents search the index* / *Off · agents get no index*); *Indexes*
  list (*3 indexes · 3.2 MB in total*): per revision the short hash, what uses it (*main · KON-38 · Run 2*), state
  *Ready* (size, files, symbols, built when; *Rebuild*), *Building* (current phase and elapsed, never a percentage),
  *Failed* (reason and what it means for the run; *Retry*), *Outdated*; *None kept*. *Search tool · ripgrep*:
  *Found · ripgrep 14.1.1* with where from, or *Not found* with *Agents fall back to slower search*, the exact install
  command, **Install ripgrep**, *Check again*; installing (elapsed, last output, *Cancel*), install failed, cancelled,
  timed out, Homebrew / winget missing.
- **Writable folders** are set per agent profile (Agents › Permissions), not here; link to them if useful.
- **Memory** (**planned**): saving policy per scope (*Save automatically* / *Save after approval*), records and tokens
  per invocation.
- **Sharing** (shared server; built: share, members and roles, join, *Make local*, offline read-only; the live task list and claims are **planned**): *Local only* (default) or *Shared on <server>*; *Share on…* with a
  confirmation (tasks uploaded, numbering, *Write orchester.toml into the repository*); members with roles (*Make
  owner*, *Make member*, *Remove*, *Leave*; the last owner refused); *Make local…*; *Review and join…* when the
  repository is already shared; the join review page (tick local tasks, similar-title hints, *Join and upload 4*);
  *Not shared* tasks; offline read-only; claims (*Take*, *Release*, *Take over…*, holder and heartbeat, stale after 15
  minutes); revision conflicts.

### 4.12 App Settings

Application-wide settings, opened from the sidebar's *Settings*.

- **Automation:** **Let automation answer approvals** (off by default): *Clients may approve a waiting tool request
  once or deny it, recorded as answered by automation. Wider answers stay yours.* Today it lives in the status bar's
  MCP popover; keep a way to reach it from there. Show the Orchester MCP server's address and where the token is
  (never the token).
- **Servers** (shared server; built): a list of servers (name, URL, *Connected* / *Signed out* / *Offline* /
  *Not signed in*, account); *Add server* (address check, plain-HTTP warning with opt-in, certificate and redirect
  errors), *Sign in* (username, password; forced password change), *Sign out?*; details with tabs *Account* (signed in
  as, role, *Change password*, *Your sessions* with *Revoke*) and *Users* (admins: *Create user*, first password shown
  once with Copy, *Reset password*, *Disable* / *Enable*).
- **Planned:** the Plan limits warning threshold (with the Usage dashboard).

### 4.13 All projects and Import resources

- **All projects** (also the new-tab page; outside any project, no sidebar): *The projects Orchester knows on this
  computer…*; **Add project** (opens setup), *Open project folder…*; rows most recently opened first: initial tile,
  name, folder, status (*Runner not started*, *Runner starting*, *Runner online*, *Reconnecting*, *Runner stopping*,
  *Runner stopped*, *Runner offline*, *Folder missing* with a hint, *Can't be opened* with the reason), *Active runs*,
  *Needs you* (a dash until read, never a fake 0), *Last opened* (*2d ago*), × to remove. A click opens the project's
  Overview in the tab; Ctrl/Cmd+click in a new tab.
  - *Remove Kontur from the list?* (Orchester forgets it; the folder and its `.orchester` data stay on disk;
    unavailable while agents are at work: *Agents are at work in Kontur. Remove it once its runs are paused or
    finished.*).
  - Errors under the heading: *Couldn't open D:\work\notes — it has no .orchester project data. Use Add project to set
    it up as a new project.*, newer data, unreadable data.
  - Empty: *No projects in the list*.
  - Planned: a *Usage* tab beside *Projects*.
- **Import resources** (dialog over the current page): source project select; kind tabs *All*, *Pipelines*, *Agents*,
  *Instructions*, *Tools*, *Secrets*; search; selectable resources with version. Then **Resolve dependencies**: one
  mapping row per dependency (source → target: *Reuse local Builder v2*, *Copy*, *Rename copy* with a name field, *Import
  secret*, *Supply a new value*, *Authorize in Kontur*) with a resolution column (*Name resolved*, *Name needed*, *Name
  taken*); the footer names the row that blocks import. Then *Ready to create local drafts*: what is created, reused
  and authorized; **Import resources**. Copies are independent drafts; nothing runs; no credential value is shown.
  Failure keeps mappings and retries without duplicates.

### 4.14 Other planned areas (draw only if time allows, marked planned)

- **Orchestrator** (project chat): new conversation, history, the coordinator proposes several tasks with briefs and
  settings; *Review & create tasks*; composer as on the task page.
- **Create multiple tasks:** paste one title per line, edit each brief, criteria, pipeline and limits; *Create 10
  tasks*.
- **Memory:** a task's Memory tab and a project Memory page: records grouped by type (handoffs, decisions, warnings,
  facts…), proposed records to *Approve* / *Reject*, *Next agent receives* panel, record editor, delete confirmation.
- **Usage dashboard:** tokens over time by CLI, plan limits history, where tokens go (by profile, model, pipeline),
  most expensive tasks and runs, efficiency; chart / table switch; loading, no runs, filters match nothing.

## 5. Cross-cutting patterns

### 5.1 Status vocabulary

Two separate things, often shown side by side (*In progress · Needs you*):

- **Workflow status** (the person's organization, fixed list): *Backlog*, *Todo*, *In progress*, *In review*,
  *Done*, *Canceled*. Orchester also moves it: run starts → In progress; ready for review → In review; merged → Done.
- **Run label** (what the latest run is doing): *Draft*, *Queued*, *Running*, *Needs input*, *Paused*, *Stopped at
  limit*, *Interrupted*, *Completed*, *Merged*, *Failed*, *Canceled*, *Unreadable*. Step rows add *Waiting*,
  *Working*, *Complete*, *Skipped*, *Not started*, *Interrupted · can't continue*.
- **Other vocabularies:** *Blocked* (a flag, independent of status); priorities *Urgent*, *High*, *Medium*, *Low*, *No
  priority*; version states *Draft v1 · not published*, *Published v3*, *v3 · Draft v4 saved*, *Needs setup*
  (planned); connection states; approval categories and decision marks (4.5); delivery lines (4.4).
- Every status is an icon **and** a word. *Unreadable* and failures must be findable at a glance; *Paused* is only a
  pause the person asked for (a limit stop is *Stopped at limit*).

### 5.2 Time and token displays

- **Tokens:** below 1,000 exact (`980`); then `k`, `M`, `B` with one decimal below 100 and none above, no trailing
  `.0` (`12.5k`, `18k`, `412k`, `1.4M`, `20M`). Add "tokens" where the label doesn't say it. Spend reads *used /
  budget* (`412k / 1.4M tokens`). Tooltips give the exact grouped count (`412,345 tokens`). Cached input is shown
  apart and *not counted*. A run may end slightly above its budget; show the real figure.
- **Budget inputs** take whole numbers only, grouped while typing (`1,400,000`), with a compact read-back.
- **Counts** are plain numbers (`2`, `0`), never padded; a dash only means "not known yet".
- **Time:** clock times in the person's locale (`14:32`); relative ages in lists (*2d ago*, *8 min ago*); elapsed for
  work in progress (*4 s*, *6 min*), never a percentage; reset times (*resets in 2 h 10 min*, *resets Mon 09:00*).
  Time limits count active agent time only.
- **Identifiers** (keys, hashes, branches, paths, commands, model ids, ports) are monospace-style data.

### 5.3 Truncation and wrapping

- **One line with ellipsis + full value in a tooltip:** task titles in tables, tab labels, project names in the
  selector and breadcrumb, the status bar activity, recovery table cells.
- **Wrap:** briefs, notices, errors (the icon stays with the first line), plan text, chat messages.
- **Never shortened:** a command, tool arguments or shell string in an approval (scrolls inside a box), secret names
  in Authentication rows (wrap instead). Paths in inspectors are shortened **in the middle** (`C:\…\npm\claude.cmd`).
- **Composite values** (*Builder · Edit files · 3 commands · Network off · MCP asks*) break only at the "·"
  separators; their link is never clipped.
- **Narrow windows:** collapse optional columns and details first in a stated order; keep actions whole; move
  secondary panels under the main column or behind a toggle.

### 5.4 Keyboard

- **Enter:** submits a form or dialog's primary action (Create, Approve once). In the composer Enter sends (now, when
  mid-turn); Shift+Enter adds a line. In multi-line fields elsewhere Enter adds a line.
- **Ctrl/Cmd+Enter:** in the composer, *Send after this turn* while an agent works, else the state's primary send.
- **Esc:** closes popovers, menus and dialogs without a change; in the approval dialog it is *Decide later*, never
  Deny; in a dialog's second step it is *Back*; in the composer it leaves a context mode.
- **Tab** moves only between form controls (fields, selects, switches, option cards, a dialog's buttons) and wraps;
  dialogs trap focus. Navigation, list rows, tabs, toolbars and the status bar are pointer-only.
- Tab and history shortcuts are in 3.1.

### 5.5 Focus and accessibility

- A clear, consistent **focus ring** on every focusable control, distinct from hover, pressed and selected, and never
  hidden by them.
- **Contrast:** body text meets WCAG AA; check small labels and disabled-with-reason text too.
- **Colour is never the only signal:** status = icon + word; diff lines carry +/− signs; charts use labels and line
  styles; the permissions chip's "wider than profile" state also changes its words.
- Select menus: keyboard highlight, a check on the chosen value, disabled options with a reason.
- Every chart or diagram needs a table or text alternative.

### 5.6 Unavailable vs hidden

- Something **not built** or **not allowed right now** is shown in place, visibly unavailable, with a lock or similar
  cue and a one-line reason (*Attaching files is not available yet*, *Review PR needs a pull request…*). It is never
  hidden in a way that moves the layout, and never faked as working.
- Something **irrelevant to the state** is simply absent (no *Remove worktree* on a running run's row; the selection
  bar only with a selection).
- No placeholder controls without behaviour (no search box or notification bell in the project bar until defined).
- No fake progress: no percentages, no skeletons that pretend to be content, no "success" before it happened.

## 6. Deliverables

- **A Kontur document named `Orchester redesign`** containing:
  1. the 2–3 style tiles and the chosen direction;
  2. the **design system**: variables (colour, type, spacing, radius, elevation; light/dark modes if you choose), text
     styles, and components with **all states** (rest, hover, pressed, focus, selected, disabled with reason, error,
     loading): buttons, icon buttons, inputs, text areas, selects and menus, checkbox, switch, radio, option cards,
     segmented controls, tabs, title-bar tabs, sidebar items, status bar items, chips, badges, status labels, table and
     list rows, notices, popovers, tooltips, modals, the approval dialog and card, chat message, tool chip, step
     divider, rail row, composer, command/value box, diff line;
  3. **screens at 1440 and 960** built from those components, in this order: app shell → Tasks and Overview → task
     page (Conversation, then Summary and Changes) → approval dialog → New task → Pipelines and editor → Agents → Project
     settings and App Settings → Instructions, Skills, Tools, Secrets, Connections → Pull requests → All projects;
  4. **key states** per screen: empty, loading, error, long text, narrow, running, waiting for the person, auto mode.
- **Review checkpoints:** stop and ask the user to review after (a) the app shell, (b) the Tasks list and (c) the task
  page, before going wide.
- **`.tmp/kontur-design/kontur-gaps.md`** — the main output of the test: everything Kontur could not do, did wrongly,
  slowly or confusingly. Per gap: severity (*blocks the design*, *workaround exists*, *papercut*), the tool or UI
  steps, the operations JSON, expected vs actual, the receipt or error. Tell the user about *blocks the design* gaps
  right away. Watch especially: auto layout and constraints, components and variants, variables and modes, text
  styles and fonts, icons and images, effects, vector and boolean shapes, render fidelity (`node_png_v1` vs the
  editor), undo/redo, many frames, save and reopen, MCP ergonomics.
- **`.tmp/kontur-design/progress.md`**: each screen and component with its Kontur page and id, status (drawn,
  reviewed by the user, changed after review), which section of this brief it covers, and notes (including anything
  marked planned).
- Check your own work: render every frame with `node_png_v1` and look critically at spacing, alignment, contrast,
  overflow and consistency; compare each screen against its section here so nothing it must show is missing.

## 7. Kontur access

- **The app:** Kontur Desktop is installed at `E:\projects\kontur\kontur-desktop.exe` and is usually running. **Never
  stop it.** If it is not running, ask the user to start it.
- **MCP discovery:** `%LOCALAPPDATA%\lurq\mcp\<pid>.json` with `"app": "kontur-desktop"`. Streamable HTTP on
  `127.0.0.1:<port>/mcp` with a bearer token from that file: initialize, keep the `Mcp-Session-Id` header, then
  `tools/call`. **Never print, log or copy the token.** Run `tools/list` first; tool names change.
- **Document tools** (prefixed `kontur_local_`):
  - `documents_list_v1`; `document_create_v1 {name, page_name}`; `document_open_v1 {document_id}` (opened is not
    loaded: read until the document is loaded).
  - `document_read_v1` (authored nodes) and `document_layout_v1` (evaluated geometry), paginated with `next_offset`.
  - `document_commands_v1 {document_id, expected_revision, operations_json, request_id}`: one atomic array of
    operations; `request_id` is a fresh UUIDv4. The write is pending until `document_receipt_v1` says it is
    **durable**: poll it. **Never blindly retry** a pending, unconfirmed or lost write; read the document state first.
  - `node_png_v1 {document_id, expected_revision, node_id, page_id, scale}`: renders a node to PNG (up to 1024 px) to
    check your work.
  - `resource_import_v1`: imports fonts (TTF/OTF) and PNG/JPEG images by absolute path; wait for its receipt.
  - `library_read_v1`, `library_commands_v1`: components, variants and placements.
  - `text_effective_font_v1`: which face a text actually uses.
  - The lurq UI tools (`lurq_screenshot`, `lurq_read_tree`, `lurq_interact`, …) also work on the editor. Prefer the
    document tools; when you must use the editor UI, record that as a gap.
- **Specs to read first** (read-only), in `H:\projects\pencil-web\docs\architecture\`: `document-command-api.md`,
  `document-operations-v1.md`, `document-model-v1.md`, `mcp-document-reads.md`, `mcp-authored-geometry-v1.md`,
  `k22-component-contract.md`, `library-parity-v1.md`. Also `H:\projects\pencil-web\docs\agents\local-desktop-replacement-plan.md`
  for what Desktop supports today.
- **Do not use Kontur's import tools.** This is a design from features, not a copy. **Do not edit Kontur** or work
  around its gaps; record them instead.

## 8. Rules

- Never touch the installed Orchester at `E:\Orchester` (or the former install at `E:\projects\orchester`) or its
  processes.
- Do not change Orchester's source, its design files or this brief.
- Scratch files (scripts, dumps, renders) go in `.tmp/kontur-design/` inside `H:\projects\orchester`.
- Commit nothing unless the user asks.
- Report to the user **in Ukrainian, briefly**.
