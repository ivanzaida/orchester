# The Orchestrator chat and its toolset

Each project has an **Orchestrator** — a chat with an agent that can see everything in Orchester, change what is
safe to change, and propose the rest as cards the person applies with a click. This page describes how the chat
works, which tools its agent has, what it may do directly and what only as a proposal, and the switches that widen
that.

## How a turn runs

- Every turn — a person's message, a wait's turn, a run end's turn, a daily report's turn — runs **Claude Code** or
  **Codex** on the connection chosen in *Orchestrator settings › Profile*, started by the project's runner in the
  project folder. A connection on an API provider can be chosen; its key is passed only to the CLI process.
- The app serves the chat's tools on a **loopback MCP server of their own, one per turn**, named `orchester` for the
  agent, with its own token handed only to the runner. It serves no window tool. Signed out of Orchester Cloud,
  a project an account holds answers `sign_in_required`; this computer's projects answer as usual.
- The chat agent has **no shell and no edit tool**. Claude Code runs with `--strict-mcp-config`, only the tools
  `Read`, `Glob`, `Grep` and the `orchester` server, reads outside the project folder blocked; Codex runs without
  shell, image viewing, web search or hooks, with a read-only sandbox, the repository's agent files and skills left
  out.
- Limits: 30 agent turns and 10 minutes per answer.
- **Writing while it answers**: *Send after this answer* (queued), *Send now* (steers the answer under way), *Stop and
  send*.

## Decisions that shape the toolset

1. **Reading is open** — tasks, runs, changes, approvals, questions, plans, conversations, pipelines, profiles,
   instructions, skills, tools, secrets (names and use only), connections, accounts, models, usage and pull requests,
   in **every** project. Changes stay in the chat's own project (another → `other_project`).
2. **Drafts are free** — tasks, modules, and drafts of pipelines, profiles, instructions, skills and tools are saved
   without asking.
3. **Publishing** is a proposal card unless the person turned on *Let the Orchestrator publish drafts*. Widening what
   agents may do additionally needs *Let automation change agent permissions*.
4. **Starting runs** is a proposal card by default; while the project allows Autopilot (*Allow Autopilot for agents
   in this project*, `get_approvals`) the chat starts them directly. **Canceling** a run is always a proposal.
5. **Answering agents and merging** are proposals, or direct under the person's switches (below).

## Domain tools the chat has (60)

Each goes through exactly the same tool an MCP client of the app calls ([mcp-tools-setup.md](mcp-tools-setup.md),
[mcp-tools-work.md](mcp-tools-work.md)), so it is refused or carried out the same way.

`list_projects`, `get_project`, `list_tasks`, `get_task`, `create_task`, `update_task`, `list_modules`, `get_module`,
`create_module`, `update_module`, `list_runs`, `get_run`, `get_changes`, `get_conversation`, `run_task`, `pause_run`,
`resume_run`, `cancel_run`, `merge_run`, `continue_run`, `answer_question`, `send_message`, `list_approvals`,
`list_questions`, `get_plan`, `hold_queue`, `release_queue`, `get_approvals`, `list_pipelines`, `get_pipeline`,
`save_pipeline_draft`, `create_pipeline`, `duplicate_pipeline`, `publish_pipeline`, `list_profiles`, `get_profile`,
`save_profile_draft`, `publish_profile`, `list_instructions`, `get_instruction`, `save_instruction_draft`,
`publish_instruction`, `list_skills`, `get_skill`, `save_skill_draft`, `publish_skill`, `list_tools`, `get_tool`,
`save_tool_draft`, `publish_tool`, `list_secrets`, `list_connections`, `list_accounts`, `list_models`, `get_usage`,
`get_delivery`, `get_story_point_calibration`, `get_daily_report`.

**Not available to the chat**: `decide_approval`, `decide_plan` (approvals and plan decisions stay the person's);
`continue_run`'s `raise_limits`, `raise_step_budget`, `resume` and `accept`; project setup (`add_project`,
`set_project_environment`, `set_repository_agent_files`, `update_project_limits`, `set_code_index`); connections
(`add_cli_connection`, `set_api_provider`, `check_connection`, `set_connection_account`); `set_allow_autopilot`,
`set_reviewer`, `set_run_manual` (and the retired auto mode tools), `remove_worktree`, `return_to_draft`; the app's
`*_mcp_server` tools (the chat has its own);
deleting anything.

## Direct or proposal

| Tool | In a person's turn | With switch / mode | In a turn the Orchestrator started itself | In a daily report's turn |
|---|---|---|---|---|
| Reads | direct | — | direct | direct |
| `create_task`, `update_task`, `create_module`, `update_module`, `save_*_draft` | direct | — | direct | refused `report_turn` |
| `run_task` | proposal (refused `task_blocked` for a task marked Blocked) | direct while the project allows Autopilot | proposal | proposal |
| `pause_run` | direct | — | proposal | proposal |
| `resume_run` | direct for a paused run; an interrupted run only with *continue interrupted runs*; stopped at a limit, canceled or failed → `person_only` | — | proposal (see run ends) | proposal |
| `hold_queue` | direct | — | direct | refused |
| `release_queue` | direct | — | proposal | proposal |
| `cancel_run` | always proposal | — | proposal | proposal |
| `continue_run` | always proposal: only `allow_retry`, `return_to`, `resolve_conflicts` | — | proposal | proposal |
| `merge_run` | proposal | direct with *Let the Orchestrator merge runs* | proposal (see auto-merge) | proposal |
| `answer_question`, `send_message` | proposal | direct with *Let the Orchestrator answer agents* (a message that would move an idle run is always a proposal; deciding a waiting plan is refused `plan_decision_person_only`) | direct under the switch | proposal |
| `publish_*` | proposal | direct with *Let the Orchestrator publish drafts*, unless the draft widens agents' permissions while *Let automation change agent permissions* is off | proposal | proposal |

Every change, proposal and refusal is kept in the conversation. A proposal card's *Apply* checks again
(`changed_since_proposed`); automation's synthetic click on *Apply* is refused `person_only`. A turn the Orchestrator
started itself is marked `[Orchester]`.

## Person-only switches (Project settings › Orchestrator › The Orchestrator may)

Each project has its own. All off until a person turns them on; no tool and no synthetic input can change them.
Settings › Automation › *The Orchestrator* names the open project's switches that are on and links to them. The window
labels them shortly (*Publish drafts*, *Answer agents*, …); their full names, used below and in the logs, are *Let the
Orchestrator …*.

| Switch | What it enables |
|---|---|
| **Let the Orchestrator publish drafts** | Publishes pipelines, profiles, instructions, skills and tools it drafts, without a card (widening still needs *Let automation change agent permissions*). |
| **Let the Orchestrator answer agents** | `answer_question` / `send_message` directly. Also makes waits, run ends and the daily report **start an Orchestrator turn by itself**. |
| **Let the Orchestrator merge runs** | `merge_run` merges runs the person asked it to merge. |
| **Let the Orchestrator merge finished runs on its own** | With *merge runs* and *answer agents*: the turn a run's success starts merges it once, if the rule passes — latest run, every step passed (a person's accept counts, automation's does not), task not Blocked or blocked, new starts not held, task not *Don't merge automatically*, the main checkout needs no local integration. One at a time, in end order; conflicts → proposes `continue_run` `resolve_conflicts`. |
| **Let the Orchestrator continue interrupted runs** | `resume_run` continues interrupted runs (crash, runner gone, *Quit now*, database busy) from the last checkpoint. With *answer agents*, an interruption's turn continues it once — not again within 10 minutes (`interrupted_again`), not while more than 3 runs stopped on a busy database within 10 minutes (`storage_contention`). |
| **Let the Orchestrator write with git** | The git write tools below (otherwise `write_off`). |

Related: *Let automation change agent permissions* (Settings › Automation), *Daily report from the Orchestrator*
(Settings › Notifications), *Let Orchester estimate story points* (Project settings › Runs › Estimates).

## Chat-only tools (25)

### `init_repository`

Proposes `git init` (branch from `init.defaultBranch`, else `main`) and a first commit *Initial commit* — or only the
first commit for a repository without one. `.orchester/` is kept out via `.git/info/exclude`. Never adds a remote,
pushes or changes git settings. **Always a card**; the card shows the branch, file count, the whole `.gitignore` and
files that look like secrets (e.g. `.env`).

Parameters: `project` (req.), `first_commit` (req.: `files` | `empty`), `gitignore` (whole `.gitignore` text, written
only where none exists). Refusals: `has_commits`, `gitignore_exists`, `git_not_found`, `folder_not_writable`,
`git_failed`, `nothing_to_commit`, `staged_files`, `git_stopped`; on apply `author_unknown`, `changed_since_proposed`.

### `apply_proposal`

Applies a waiting proposal for the person when the person says so in the chat ("yes, do it"). Without `proposal`
it lists the waiting proposals (number, what it does, whether only a click applies it). With `proposal` (integer) it
applies that proposal exactly as *Apply*, recorded as the person's decision with the approving message quoted. Only
in a turn the person's own message started. Refusals: `not_persons_turn`, `automation_input`, `not_pending`,
`click_only` (publications that widen, MCP servers added/widened/turned on, `git_propose` operations, a first commit
with secret-looking files).

### `wait_on_task`

Makes a run that waits for a message or a decision wait on another task of the project — typically one the chat just
created for the work the run needs (screen 04 of the Orchestrator watch design: KON-41's run waits on KON-52). It
changes no run, so it is **direct in any turn** (a turn that plans refuses it, as every change). The conversation
keeps its row, *KON-41 · #17 waits on KON-52*. A run waits on one task at a time.

Parameters: `run` (req.: id or number), `task` (req.: the waited task's key or id), `project` (the chat's own).
Refusals: `own_task`, `other_project`, `not_waiting_for_person` (a tool call's approval is answered at once),
`already_waits`, `no_base_branch` (the run has no base branch its work merges into).

What ends the wait, read by the waits follower:

- **The waited task's run is merged into the waiting run's base branch** — a *waited task merged* event of the
  waiting task, kept in the conversation that leads it (else by the usual rule). With *Let the Orchestrator answer
  agents* on it starts a turn there, which tells the run's agents with `send_message` (under the switches as for any
  message); when the run goes on, the base-moved path merges the base branch into its worktree.
- **The waited task is canceled, or closed without that merge** (a merge into another branch counts as closed) — a
  *waited task closed* event, routed the same way, whose turn tells the person and offers the choices (go on without
  that work, run that task again, or cancel this run) and decides none of them; the run keeps waiting for them.
- **The person answers the run** on its own wait card (a message, or a decision on its plan) — the wait ends without
  an event.
- **The person drops the wait** — *Drop the wait* on the waiting task's Summary and Conversation, and on the wait's
  row in the leading conversation. The run goes on at once without that work, with no confirmation: an agent that
  waits for a message gets one telling it to go on; a plan waiting for a decision goes on once the person decides it.
  No agent turn starts.
- **The waiting run ends** — the wait ends without a word.

### MCP server tools

`mcp_add_server`, `mcp_update_server`, `mcp_set_server_enabled`, `mcp_remove_server` take the same arguments as the
app's `add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`, `remove_mcp_server`. **Adding or widening is
always a card.** Narrowing (turn off, remove one no profile chooses, drop exempt tools or authorization) is direct
only in a person's turn while *Let automation change agent permissions* is on; otherwise a card.

### Git tools

Every git tool also takes `run` (optional: a run's id or number — work in that run's worktree instead of the
project's checkout; writes there are refused `run_active` until the run ended). Unknown fields are refused. Local
writes time out after 5 minutes, network operations after 2; no credential prompts (`auth_failed`), no editor.

| Tool | Kind | Parameters |
|---|---|---|
| `git_status` | read | — (branch, upstream, ahead/behind, operation in progress, changed/staged/conflicted/untracked files) |
| `git_log` | read | `range` (default `HEAD`, e.g. `main..feature`), `path`, `limit` (default 30, max 200), `all` |
| `git_diff` | read | `from`, `to`, `paths`, `staged`, `stat` |
| `git_show` | read | `rev` (req.), `file`, `stat` |
| `git_branches` | read | — |
| `git_blame` | read | `path` (req.), `rev`, `start`, `end` |
| `git_remotes` | read | — (URLs with credentials removed) |
| `git_commit` | write | `message` (req.), `paths` or `all`; else what is staged |
| `git_create_branch` | write | `name` (req.), `start` (default HEAD), `switch` |
| `git_switch` | write | `branch` (req.); keeps local changes, else `would_lose_work` |
| `git_pull` | write, network | `remote`, `branch`, `merge` (default fast-forward only) |
| `git_push` | write, network | `remote`, `branch`, `set_upstream`; never forced |
| `git_merge` | write | `branch` (req.), `no_ff`, `message`; a conflict is undone → `conflict` |
| `git_revert` | write | `commit` (req.), `mainline` |
| `git_tag` | write | `name` (req.), `rev`, `message` (annotated) |
| `git_fetch` | write, network | `remote`, `all`, `prune` |
| `git_propose` | proposal | `operation.kind`: `reset` {`to`, `mode` hard/mixed/soft}, `clean` {`paths`, `directories`, `ignored`}, `delete_branch` {`branch`, `remote`?}, `force_push` {`remote`?, `branch`?} (`--force-with-lease`), `discard_changes` {`paths`}, `switch_discarding` {`branch`}, `rebase` {`onto`}, `delete_tag` {`tag`, `remote`?}, `abort_merge` — **always a click-only card**; work over 10 000 files or 256 MB is not proposed (`too_large`) |

Writes need *Let the Orchestrator write with git*, never happen in a turn the Orchestrator started (`person_only`) or
a report's turn (`report_turn`), and are refused `checkout_reserved` while local integration or a conflict
resolution holds the checkout, and `operation_in_progress` during a merge or rebase. Other codes: `git_not_found`,
`not_a_repository`, `nothing_to_clean`, `nothing_in_progress`, `detached_head`, `changed_since_proposed`.

## Turns the Orchestrator starts itself

Every event of a task below is kept as **one entry** in exactly one conversation, chosen by the **routing rule**:

1. the conversation that **leads the task** through a subscription (see [Watching](#watching)): the task's own, else
   its module's (or *Tasks without a module*'s, for a task with none), else the whole project's;
2. with no subscription covering the task, the conversation that last dealt with the task or its run;
3. else the project's latest conversation, else a new one, titled after the task and what waits.

A watched task's events never reach another conversation, whoever dealt with it before. With *Let the Orchestrator
answer agents* on, each entry also starts **one** turn there by itself — never while another turn of that conversation
thinks (it waits for that turn to end), only while what it is about still stands, and not when the Orchestrator's own
answer, message, merge or cancel raised it. With the switch off the entry only links to its card. Such a turn is
marked `[Orchester]`; in a conversation that leads something it is told the subscription the event came through and
the order that applies (see [Orders](#orders)).

- **Waits** — when an agent asks a question with suggested answers, a step waits for a message, or a plan waits for a
  decision, the conversation gets an entry linking to the card. The turn answers only what follows plainly from the
  brief, criteria, docs, AGENTS.md or the person's words, otherwise tells the person the question with options and a
  recommendation.
- **Run ends** — a task's latest run that succeeds, fails, stops for a decision, stops at a limit or is interrupted
  gets one entry. The turn: success → says it is ready (merges only under the merge-finished rule); a decision wait →
  may propose `continue_run`; failure/limit/interruption → explains the person's choices.
- **No progress** — a run an agent works on that made no progress (no run event such as a step change, no transcript
  event such as a tool call, no agent launch) for the minutes of *A run without progress for* (Project settings ›
  Orchestrator › *Watching*; 20 by default, 1–240, or off). Raised once per stall; progress again re-arms it. A run
  that waits for a person, is queued, paused, asked to pause, waits for a workspace or waits on another task never
  stalls. The turn reads the run, tells the person what it was doing and their choices, and may send its agents a
  message; it starts, resumes, cancels or merges nothing. The entry links to the task's Summary.
- **Merge conflict** — a merge of a task's run that stopped at conflicts (from the task page, `merge_run` or the merge
  queue), once per attempt; only the newest attempt of a run still not merged. The turn proposes `continue_run`
  `resolve_conflicts` naming the files, or points to the Local integration page; it does not call `merge_run` again.
  The Orchestrator's own merge that conflicted starts no turn.
- **Waited task merged** and **waited task closed** — the task a run waits on through [`wait_on_task`](#wait_on_task)
  was merged into the run's base branch, or canceled or closed without that merge. See there.
- **A subscription ended by itself** — see [Ending](#ending).
- **Daily report** — at the time set in *Daily report from the Orchestrator*, each open project's latest conversation
  gets one *Daily report* entry per local day with activity (missed days are delivered at the next start, at most 7
  back). With *answer agents* on, a turn adds a summary and recommendations and **changes nothing** (changes are only
  proposed; anything else is refused `report_turn`). `get_daily_report` reads a day's report. The daily report is not
  an event of a task: the routing rule does not apply to it.

## Watching

A conversation **leads** what the person tells it to watch: its subscriptions decide where the events of their tasks
go (the routing rule above). Subscriptions are **the person's alone**: the app applies the commands itself, and the
agent has no tool that subscribes or stops — it only sees its subscriptions and their orders, in its standing
instructions and in every turn an event starts. A watch command sent by automation (synthetic input) is not applied.

### The commands

In the composer's slash menu, after *New conversation* and *History*:

| Command | Label | What it does |
|---|---|---|
| `/watch-module` | *Watch modules* — "Their tasks' events come here" | Opens a picker of the project's open modules (those with a task left to watch first), then *Tasks without a module* and *Everything in* the project. Each row says how many tasks, running and waiting, and who leads it. |
| `/watch-task` | *Watch tasks* — "Only these tasks' events" | Opens a picker of the project's tasks by key or title, with their workflow status, module and who leads them. |
| `/unwatch` | *Stop watching* | Offered while the conversation leads something. Opens a picker of only what this conversation watches (*Watched here*), each with what is still open there, then *All of them*; Enter adds the focused row as a chip, the text after the chips only filters, and Ctrl Enter (Cmd Enter) stops the chosen subscriptions at once, with no agent turn. You can also stop one with × on its header chip or *Stop watching* in a `/watching` answer. |
| `/watching` | *What I watch* — "What this conversation leads and what waits there" | Faded while the conversation leads nothing. Keeps the command with Orchester's answer under it, read from the live state: each subscription (target, kind, since when, its order) with what waits there, and *Stop watching* on each. |

In a picker, Enter adds the focused row as a chip, Backspace removes the last chip, **Ctrl Enter (⌘Enter) watches**,
Esc closes. A target whose subscription would end at once is faded: a module whose every task is Done or Canceled
(*Nothing left to watch*), a Done or Canceled task (*nothing to watch*). Text written after the chips is the
subscription's **order**.

The commands spend nothing and start no agent turn, and they work while an answer is under way. Each keeps the
person's command and the app's line in the conversation: *Leading Webhooks v2 · 12 tasks — their waits, run ends,
conflicts, limits and stalls start a turn here*. A target closed or deleted meanwhile is left out and named; with
nothing left the command is not applied (*Nothing was watched*) and nothing changes. `get_watches`
([mcp-tools-work.md](mcp-tools-work.md#get_watches--read-only)) reads the same as `/watching`.

### Targets and specificity

A conversation can watch several targets, each with its own order:

- **a task**;
- **a module**, including the tasks added to it later;
- **the tasks without a module**;
- **everything in the project** ("What no other subscription takes").

One conversation leads each target at most. An event of a task goes to the **most specific** subscription that covers
it: the task, then its module (or the tasks without a module), then the project. Watching a task inside a module
another conversation leads is no conflict: the task subscription wins, and the module's conversation gets a line
(*KON-41 is now led in «…» — watched there as a task; the rest of Webhooks v2 stays here*). When that subscription
stops, the less specific one's conversation gets *… is led here again*.

### Orders

The **order** is the person's text after the chips: what they want done with the target's events. A command without
text takes the project's **Default order for watching** (Project settings › Orchestrator › *Watching*, a multi-line
field; empty by default, meaning no order: the person's switches decide). A subscription without its own text follows
the default order as it is when an event comes. Watching a target the conversation leads already with other text
renews its order.

An order is quoted to the agent as data and **can only narrow** what the person-only switches allow: whether it acts
at once or only proposes stays as the switches and the tables above say, whatever the order's text says.

### Already led elsewhere

Choosing a target another conversation leads applies the rest of the command and shows a card under its line:
*Webhooks v2 is already led in «…»*, since when, with **Open that conversation** (nothing moves; the line reads *…
stays led in «…»*) and **Take it here** (the subscription moves here with this command's order; that conversation gets
the line *… is now led in «…»*). Only a person chooses on the card: synthetic input is refused.

### Seeing and stopping

- The conversation's header shows **Leads** with a chip per target (those without room fold into *+N*); × on a chip
  stops that subscription.
- History shows *Leads …* on a conversation that leads something.
- A module's Summary and a task's Summary properties show **Led in** with the leading conversation.
- Stopping (× or *Stop watching*) ends the subscription at once and **cancels or pauses nothing**: the line *Stopped
  watching …* lists what was left open (waiting, running, queued, stopped at a limit), which goes back to the usual
  rule.

### Ending

A subscription **ends by itself** when its task is Done or Canceled, or when every task of its module is (a module
with no task does not end). *Tasks without a module* and the whole project never end by themselves. Its conversation
gets the line *Every task of Mobile release is Done — watching ended* (or *KON-41 is Done*, *… is Canceled*, *… is
Done or Canceled*). With *Let the Orchestrator answer agents* on, one turn writes the module's or task's summary under
it and **changes nothing**, as a daily report's turn. A status changed anywhere ends it; reopening the task afterwards
does not bring the subscription back.

### Created tasks

A task the chat creates (`create_task`) in a conversation that watches something, outside what it watches, becomes a
**task subscription** of that conversation, so its events come back there. It takes the order of the subscription
whose event started the turn; in a person's turn, none of its own (the default order applies). Nothing is added when
the conversation watches nothing, when one of its subscriptions already covers the task, or when another conversation
leads the task's module or the tasks without a module — then the task's events go there, and `create_task`'s answer
(`watch`) and the action's row say which.
