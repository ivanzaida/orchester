# Concepts and glossary

This page defines the things Orchester works with, as the app (version 0.3.x, `crates/desktop-app`) implements them.
Screens and step-by-step workflows are in [user-guide.md](user-guide.md); settings in [settings.md](settings.md).

## Projects

**Project.** A local git repository that Orchester manages. *Everything operational belongs to exactly one project*:
tasks and runs, pipelines, agent profiles, instructions, skills, tools, secrets, connections, memory, modules and
settings. Nothing is a shared workspace resource; to reuse something in another project, copy it with *Import
resources*.

- The project's data lives in the repository's **`.orchester/`** folder, excluded from git through
  `.git/info/exclude` (your `.gitignore` is never touched). It holds the project database `orchester.db`,
  `worktrees/`, `logs/`, `blobs/`, `index/`, `skills/`, `tools/`, `attachments/` and `runner.lock`.
- The **application database** `orchester.db` (beside the program on Windows) keeps the list of projects, app
  settings, accounts and plan-limit readings.
- Inside the app a project is addressed by its id, so links survive renames. People see its **name** and **project
  key** — a short prefix for task keys (`KON`, `ORC`) suggested from the name (initials of a multi-word name, else the
  first three letters).
- **One runner process** (`orchester-runner`) runs per open project. It executes the project's runs and agents. A
  project is open while a tab shows it, or while its runner still has work.

## Tasks

**Task.** A brief that agents carry out: a title, a description ("Describe the outcome", Markdown), numbered
**acceptance criteria**, the chosen **pipeline**, and optional **limits** (run token budget, time limit). It may also
have sources (extra folders agents may read), a module, story points, organization fields and delivery overrides.

**Task key.** `<PROJECT KEY>-<number>`, numbered from 1 per project (e.g. `ORC-129`).

**Draft and queued.** A task is a **draft** until it is submitted. A draft may be incomplete. Queueing needs a title,
a brief, acceptance criteria and a published pipeline; the task list says what is missing ("Needs a brief", "Ready to
queue", …). *Run* (or *Create & run*) queues the task and starts its first run.

**Task phase** (derived from the latest run, never stored): Draft, Queued, Running, Waiting / Stuck (shown as *Needs
input*), Paused, Stopped at limit, Interrupted, Succeeded (shown as *Completed* in lists, *Ready for review* on the task
page, *Ready to merge* on the Overview), Merged, Failed, Canceled, and *Unreadable* when the run record cannot be read.

**Workflow status.** Backlog, Todo, In progress, In review, Done, Canceled — the state of *your* work, independent of
the run label. Orchester moves it by itself: a step starting → In progress; a succeeded run → In review; a merge or
*Complete task* → Done. New tasks start in Backlog. In the window a person changes it on the **Tasks board** (drag a
card to another column, or *Move to*), or closes and reopens a task through *More actions* (Mark as done, Cancel task,
Reopen task); the task page shows it read-only. MCP `update_task` can set any status.

**Priority.** Urgent, High, Medium, Low, No priority. Used for sorting and display only — it never changes the run
queue. Set with the *Priority* chip of New/Edit task, on the board (card menu, bulk bar, or a drop when columns or
rows are by priority), or over MCP.

**Story points.** 1, 2, 3, 5, 8, 13, 21. Shown as "5 pt" (set by a person) or "~5 pt" (Orchester's estimate). With
*Let Orchester estimate story points* on (a per-project switch in Project settings › Runs › Estimates), Orchester estimates new and changed tasks using the reviewer's model (Runs › Approvals)
and the project's merged tasks as examples; a person's value always wins. Points are effort sizes, not money or
tokens: no budget reads them. Delivery metrics show them.

**Tags.** Free, case-insensitive project tags. Rows show the first two plus "+N". Existing tags are added and removed
on the board (card menu, bulk bar); new tags are created only over MCP (`tags`).

**Blocked and blocked by.** Two separate things:

- A manual **Blocked** flag with a reason — informational: it keeps no run from starting, but the Orchestrator chat
  will not start the task or merge its runs on its own.
- **Blocked-by links** to other tasks (gate: *merge*) — the task cannot run, resume or continue until each linked task
  is finished; its controls lock with "Unavailable until … finishes." Cycles are refused.

Both are set over MCP (`blocked_reason`, `blocked_by`); the window shows them read-only (the *Blocked* notice and
*Depends on*).

**Don't merge automatically.** A task flag: the Orchestrator never merges this task's runs on its own; it tells you
the run is ready and the merge stays yours.

**Modules.** Epics, milestones or sprints that group tasks: name, optional description, optional start and target
dates, state Open/Closed. A task is in at most one module; every move in or out is kept, so the burndown shows scope
changes. A module page shows progress, a burndown and a 50%/85% delivery forecast.

**Hold new starts.** Holds new run starts in the whole project (or a reviewed task group). Running runs go on. A hold
a person set is released only by a person.

## Runs

**Run.** One execution of a task's pipeline in its own **git worktree** and branch. A run pins a snapshot when it
starts — pipeline, profile, model, instruction, skill and tool versions, the brief and criteria, limits and sources —
so publishing a new version never changes a run already started. A task has one active run at a time. *Run again*
starts a new run of the same task.

**Run number.** `#128` — unique within the project, not per task.

**Run branch.** `orchester/<task key lower-case>-run-<n>`, e.g. `orchester/orc-98-run-1`. A step running beside
others works on `<run-branch>--<step>-<attempt>`.

**Step.** A node of the pipeline graph. Each time a step runs it is a *step execution*. Steps whose dependencies
passed run **in parallel** as far as agent slots allow; a parallel step gets its own worktree, merged back into the
run's branch when it passes. Step states: Not started, Working…, Needs you, Paused, Stopped at limit, Interrupted,
Complete, Failed, Skipped, Canceled.

**Attempt.** A pass over the pipeline from some step onward. A new attempt starts when a failure route returns the
work, a broken step restarts, a person allows a retry or returns the work, or a follow-up runs. At most 10 automatic
attempts follow each other; then the run waits for a person.

**Checkpoint.** A commit of the worktree on the run's branch, made when a step passes, at a pause, at a limit stop, on
*Stop turn* and before *Apply now*. Continuing, resuming and raising a limit pick the run up from its last checkpoint.
Work after the last checkpoint is never claimed as done.

**Worktree.** `<repo>/.orchester/worktrees/<run-id>/` on the run's branch. Agents work only there — *your checkout
is not touched*. A worktree stays after the run ends until a person removes it, a merge cleans it up, or *Run again*
is told to remove it.

**Base branch.** The branch a run starts from and delivers into. Project settings › General › Repository › *Base branch*:
*Checked-out branch* (default — the run starts at the checkout's HEAD) or a named branch (every run starts from its
tip). Each run records its own base, and merge, merge queue, local integration, pull requests and base-moved notices
follow that record. The setting is kept per computer. A chosen branch that no longer exists refuses new runs.

**Base moved notice.** When a run's base branch moves while the run is unfinished, the run's next writing agent is
told to merge the base into its worktree after its turn (never mid-edit). Moves within two minutes coalesce. Can be
turned off in Project settings.

## Pipelines

**Pipeline.** The steps a task's run goes through, who runs each one, and where the run waits for a person. Versioned:
tasks can only choose a **published** pipeline.

**Step kinds.**

- **Agent step** — runs an agent of a chosen *published* profile. Uses the profile's model unless the step overrides
  it. Effective permissions are profile ∩ step: a step can only narrow.
- **Tool step** — calls a project **command tool**; its exit code decides the step.

**Dependencies.** Each step's *Runs after* set; the first level hangs under *On task start*. Steps ready together run
in parallel.

**Approval gate.** *Wait for your approval*: the step proposes its result as **plan vN** and the run waits.
*Approve plan & continue* lets it go on; *Request changes* (with a note) resumes the step's agent, which writes plan
vN+1.

**Failure route.** *When changes are requested*: return the work to an earlier step (with *Maximum retries*) or fail
the run. When the retries are used up, the run waits for a person. A *breakage* (the CLI crashed or could not start,
commit or merge) is different: the step restarts once, and a second breakage makes the run wait.

**Step budget.** Tokens one execution of the step may use; reaching it stops the run *at its limit*.

**Deliver step.** An optional last step. Modes: *Approve to merge* (default — the run waits in Needs you), *Auto-merge*
(merges once every step and the re-check passed); *Pull request* is "Coming later". Merge method: *Merge commit* or
*Squash* (titled `KEY: <task title>`). *Resolve conflicts with*: the profile of the **Integrate** step. *Re-run Check on
the merged result* (on; always on for Auto-merge). A run of a pipeline with a Deliver step joins the **merge queue**.

**What a run delivers.**

| Option | Meaning |
|---|---|
| Repository changes (default) | Its commits are merged. A run that committed nothing is flagged and waits for a person. |
| A report | The last step's report is the result; the run is *completed*, not merged. |
| Work outside the repository | Work done through MCP tools: the report plus the record of approved MCP calls. |
| Changes or a report | Merged when it has commits; otherwise its report is the result. |

A task may override its pipeline's choice.

**Merge the base branch when it moves.** Per-step switch deciding which step takes base-moved notices. On by default
for writing steps, except review/verify steps and steps whose failure route returns to a writing step.

**Starter pipeline — Feature delivery.** Research → Plan (approval gate) → Implement → Run checks ∥ Review → Create
pull request. Implement writes; the other agent steps read only. Run checks and Create pull request are tool steps.
Run checks and Review return to Implement on failure, at most 2 retries.

## Agents

**Agent profile.** A role for pipeline steps: *Profile name* (the role, e.g. *Builder*, which role-scoped
instructions and skills match), *Responsibility*, *Role instructions* (Markdown added to every run of the profile),
*Project connection*, *Model*, *Reasoning effort*, **permissions**, and up to 20 MCP servers. Versioned (the draft
autosaves; Publish vN); runs use only published versions.

**Permissions.**

- *Repository access*: Read-only, or Edit files in the task worktree (then *Extra writable folders* may be added).
- *Commands*: Ask before running commands; Allowed commands (a prefix list; known-safe read-only commands also run);
  Any command in the worktree.
- *Network access*: On or Off.
- *MCP servers*: a switch per project server.
- *Approvals*: *Approve automatically (don't ask me)*; *Ask before MCP actions that require approval*.

Options the connection's CLI cannot enforce are disabled with the reason; there is no bypass mode.

| Option | Codex CLI | Claude Code CLI |
|---|---|---|
| Allowed commands | unavailable | yes |
| Any command in the worktree | yes (sandbox) | unavailable |
| Network off | macOS sandbox only; unavailable on Windows | unavailable |

**Connection.** How a project's agents run and what they reach:

- **Codex CLI** (runs `codex` as App Server) and **Claude Code CLI** (runs `claude -p`) — each runs on the CLI's own
  sign-in through an *account*, or on an **API provider**.
- **Anthropic API / OpenAI API (native)** — Orchester's own agent loop; shown but locked in this version.
- **GitHub** through the `gh` CLI — the project's linked repository and expected account.
- **MCP server** — a program the agent's CLI starts over stdio, or a remote streamable-HTTP server, with optional
  authorization (Bearer, Basic, pasted Basic credential, custom header) whose secret part is always a project secret.
  Given only to agents of profiles that choose it.

**Account.** A folder where a CLI keeps its own sign-in (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`). Accounts are app-wide and
belong to the person; each CLI has a **Default** account (its own folder). A connection chooses its account; runs pin
it. Orchester never signs in for you: it opens a terminal running the CLI's own login.

**API provider.** A Claude Code or Codex connection can *Run on: An API provider* instead of its own sign-in: a base
URL (Claude Code: Anthropic Messages API; Codex: OpenAI Responses API), an API key from a project secret, and models
typed by hand. Only on the Default account; no plan limits are read; the reviewer does not run on it.

**Plan limits.** How much of each account's 5-hour and weekly windows is used. Codex reports with each turn and is
also read on its own; Claude Code reports during a turn or on *Check now*. A run whose account hits its limit stops
*Stopped at plan limit*: wait for the reset (resumes by itself) or continue now on another account.

## Instructions, skills, tools, secrets

**Instructions.** Versioned prompt rules. Scopes: Project rules, Agent roles, Pipeline overrides, Step overrides.
Targets: all agents, chosen roles or chosen profiles. *Inject at*: Agent start, Agent resume, or both. Assembly order:
agent role → project rules → pipeline → step → task context. Variables such as `{{task.title}}` come from a fixed
catalog; secrets are never variables.

**Skills.** Versioned know-how an agent loads when needed: a description, a Markdown body (`SKILL.md`) and reference
files. Scoped like instructions. *Import from repository* makes drafts from `AGENTS.md`, `.claude/skills` and
`.agents/skills`.

**Repository agent files.** Whether the CLIs may also load the repository's own `AGENTS.md`, `CLAUDE.md` and skills
folders (on by default). Claude Code is started without project settings and loads neither in either state.

**Tools.** Project actions that tool steps call. Only **command tools** run in this version: a program (on the search
path or an absolute path outside the repository), arguments (no shell), timeout (default 600 s), passing exit codes
(default 0), environment variables, output kept (default 256 KiB). A command step runs **with your rights, network
access and no sandbox**, on files the agents before it wrote. HTTP and database tools are listed but unavailable.

**Secrets.** Credentials kept in the OS keychain (Windows Credential Manager / macOS login keychain); only metadata is
in the project database. Name `NAME` (capitals, digits, `_`); environment Development, Staging or Production (one name
per environment), referenced as `${secret:NAME@environment}`. Kinds: Access token, API key, Database connection,
Deployment token, Password. Used today by API-provider connections and MCP servers. A secret value never reaches
prompts, logs, MCP output or the screen after saving.

## Limits and budgets

All budgets are in **tokens**, never money. Counted tokens = input + output (reasoning included); cached input is
shown separately and never counted. Values are what the CLI reports, never estimates.

| Limit | Where | Default | Effect |
|---|---|---|---|
| Run budget | Task (default from Project settings › Runs › Limits) | no limit | Stops the run *at the token limit* at a checkpoint |
| Time limit | Task (default from Project settings) | no limit | Counts **active agent time only** (waiting, paused or stopped time doesn't count) |
| Step budget | Pipeline step | no limit | Stops the run at the step's budget |
| Daily budget | Project settings › Runs › Limits | 20,000,000 tokens | Runs, the Orchestrator chat, auto-mode reviews and estimates all count; new work waits once it is used |
| Agents at once | Project settings › Runs › Limits | 4 (1–16) | Agent slots shared by all runs; applies at once |
| QA apps at once | Project settings › Runs › Limits | 2 (1–16) | QA copies of the app runs may have open |

There is no maximum: a raised limit only has to be above what was used. *Remove the limit* continues with none.
Raising applies to that run only.

## Waiting for the person

**Needs you.** Everything that waits for you: tool and command approvals, agents' questions, steps waiting for a
message, plans, runs waiting for a decision, interrupted runs, delivery decisions and memory records waiting for
approval. Counted in the title-bar pill, the status bar, the Overview, tab marks and All projects.

**Approval.** A command or MCP tool call that the agent's permissions leave to a person. A card in the task's
Conversation and the Needs you panel shows a category chip (Read-only, Worktree, Network, Outside write, Unknown,
History / destructive, Credential read), the command, the agent, the working directory and the agent's note.
Answers: *Approve once*, *Deny*, *Deny with a message*, and under *Or answer more widely*: *Approve similar in this
run*, *Approve everything in this run*, and (for Orchester's QA tool only) *Approve for every run of this project*.
There is no time-out: the agent's active time stops while it waits.

**Auto-approval.** Rule-based: per profile (*Approve automatically*) or per run (*Approve everything in this run*).
Orchester answers commands its classifier knows to read or stay in the worktree, and MCP tool calls; it never
approves network, writes outside the worktree, credential reads, installs, history rewrites or unparsed commands.

**Approval mode.** Each agent profile is **Autopilot** or **Manual** (the default). Autopilot approves the agent's
tool requests at once, except work outside the run's worktree, its Sources and the agent's temp folders, pushes to a
remote and refs other than the run's branch, which still come to the person. Manual runs the profile's allowed
commands, asks the project's **reviewer** about the rest, and leaves what it denies or is unsure about to the person.
A step is in Autopilot only when its profile says so, the project allows it (*Allow Autopilot for agents in this
project*, Project settings › Runs, off until a person turns it on) and the run was not switched to Manual; a run can
switch itself to Manual, never back. Questions, plans, limits and conflicts always come to the person.

**Reviewer.** A separate model judges the requests a Manual step's rules leave to a person and approves only what
clearly serves the task. Hard rules go to the person first (credential reads, unparsed parts, history rewrites,
deletes outside the worktree, anything outside the step's permissions); the reviewer fails closed. Its tokens count
toward the run's and the daily budget. It never answers `run_check` or `start_qa_app`.

**Question.** An agent asks the person. A plain question ends the agent's turn and is answered from the composer; a
question with options shows a card (radios or checkboxes plus *Or write your own answer…*) and the agent waits in
its turn. A question asking for a secret is refused automatically.

**Plan.** The result of a gated step (plan vN). Decided on the task's Summary or the Conversation's plan card; who
decided is recorded.

**Decision after retries.** *Allow 1 retry & continue*; for unverified or unchanged steps *Accept … as it is* or *Run
… again*; *Return to … with note*; *Stop and fail the run*.

## Merging and delivery

There are three ways a finished run's work reaches the base branch:

1. **Direct merge** (pipelines without a Deliver step): *Merge into <branch>*. It merges in a temporary worktree and
   brings the result into your checkout as a fast-forward that leaves uncommitted and untracked files it doesn't
   touch as they are. A conflict offers *Resolve in editor*, *Ask an agent to resolve*, or *Local integration*. Or
   *Create pull request* with `gh`.
2. **Merge queue** (pipelines ending in a Deliver step): runs deliver one at a time, first finished first: sync the
   base → Integrate step resolves conflicts → re-run checks on the merged result → your approval (or auto-merge) →
   merge → clean up and set the task Done. Waiting for approval never holds the queue. The queue never merges into a
   checkout with uncommitted changes ("Delivery is blocked").
3. **Local integration** (task tab): finishes integrating a succeeded run when your checkout has local changes the
   merge would overwrite or conflicts need resolving — review local files → commit or temporarily preserve them →
   sync and fresh check → approve the exact checked result and merge → restore preserved changes. Every
   acknowledgement is a person's.

**Complete task.** A run that passed every step and committed nothing, for pipelines delivering a report, outside
work or changes-or-report: *Complete task* sets the task Done and keeps the last step's report. If changes were
expected: *Complete anyway*, *Return to a step* or *Run again*.

**Merged outside Orchester.** A succeeded run whose own commits are all in the base reads *Merged* ("merged without
Orchester"). Squash-merged or rebased pull requests cannot be seen by git.

**Remove worktree.** For runs with no live work. Committed work stays on the branch; a branch whose commits are merged
is deleted. An interrupted run whose worktree is removed can never continue.

## Knowledge and reporting

**Code index.** An index per git revision: tracked files, symbols, outlines, Markdown headings and a package map,
built when a run starts from a revision without one. Agents get a built-in skill and the `find_symbol` tool that
search it with ripgrep. **Off by default.**

**Memory.** Records agents and people keep, separate from conversations. Types: Fact, Decision, Observation,
Artifact, Handoff, To do, Warning, Summary. Statuses: Proposed (waiting for approval), Active, Superseded, Deprecated,
Rejected, Withdrawn. Only Active records reach agents; pinned records come first. Each step's handoff is kept as task
memory. Task memory saves automatically by default, project memory after approval. Per invocation: 8 records and 3,000
tokens by default. Only people approve, edit, deprecate or delete records.

**Orchestrator chat.** A project conversation with an agent CLI that sees everything and acts through Orchester's
tools with the same checks as the app. See [orchestrator-chat.md](orchestrator-chat.md).

**Daily report.** At a set time each open project's Orchestrator conversation gets a report of the day.

**Usage.** Tokens and active agent time by period, grouped and filtered; plan limits; where tokens go; most expensive
tasks and runs; efficiency. **Delivery metrics**: Monte Carlo forecast, throughput, cycle and lead time, where agent
time goes, cumulative flow. No money anywhere.

**Pull requests.** The linked GitHub repository's pull requests through `gh`, with *Files changed* and **Agent
feedback** threads (ask the linked task's agent a question or request a fix — kept in Orchester, never posted to
GitHub).

**Notifications.** OS notifications (installed app only): *Needs you*, *Stopped*, *Finished and info*; a taskbar or
Dock badge with the Needs-you count.

## Orchester Cloud and team servers

**Orchester Cloud sign-in.** The app is locked until this computer signs in to Orchester Cloud
(`https://orchester.cloud`, API at `https://api.orchester.cloud`; both can be changed in Settings › Orchester
Cloud › Advanced) with email and password (plus a TOTP or recovery code with two-factor on). After that it
works offline. While signed out every MCP tool answers `sign_in_required` except `sign_in_state`. Only a person signs
in.

**Team servers and shared projects.** A team runs its own `orchester-server`. A computer joins through an invite
(needs two-factor) or *Connect a server* (address, check-code comparison for self-signed certificates, then the
Orchester account or an 8-character invite code). A project links to a server project in Project settings › Sharing › *Share…*.

- **Shared**: tasks' title, brief, criteria, organization, holder and short run progress; published configuration.
- **Stays on each computer**: runs, worktrees, conversations, memory, secrets, connections, accounts and the base
  branch. The server never receives transcripts, prompts, diffs, file or branch names, code or secrets.
- Tasks not uploaded when joining stay *Not shared* (read-only, *Share task*). Starting work on a shared task claims it;
  claims go stale 15 minutes after the last heartbeat. Members and invites are managed in the server's browser admin.
- Shared projects have no modules, no *Don't merge automatically*, and no task deletion.
