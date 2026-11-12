# The Orchestrator chat and its toolset

Each project has an **Orchestrator** — a chat with an agent that can see everything in Orchester. One person-only
switch per project, **Let the Orchestrator drive** (Project settings › Orchestrator), decides how it works. **On**, the
chat acts itself: it calls the same tools an outside agent gets from the app's MCP server, with the same rules and
refusals, nothing becomes a card, and it is woken for every wait, stop, end and event. **Off**, it changes what is safe
to change and proposes the rest as cards the person applies with a click, and no turn starts by itself. This page
describes how the chat works, which tools its agent has and what it does directly and what only as a proposal.

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
2. **Drafts are free** — tasks, modules, and drafts of profiles, instructions, skills and tools are saved without
   asking. A pipeline has no drafts (saving one publishes its next version), so the chat changes pipelines
   (`create_pipeline`, `save_pipeline`, `duplicate_pipeline`) only while it drives; while it does not, they are
   refused as `drive_only` and it tells the person what to change in Pipelines (D-033).
3. **While it drives, the chat is a client of the app** — every change is made at once by the tool's own rules, and a
   refusal is the tool's. Publishing, starting, canceling, pausing, resuming, continuing (accepting a step, raising
   limits and budgets and resuming after the daily budget included), merging, answering agents and messaging their
   runs, making the folder a git repository and writing with git all happen directly. Whatever only *Let automation …*
   allows another client stays refused: what widens what agents may do is `person_only` without *Let automation change
   agent permissions*; answering agents' questions and messages needs *Let automation answer approvals*, as for any
   client.
4. **While it does not drive, changes are cards** — publishing, starting a run (unless the project allows Autopilot),
   canceling, merging, answering agents and continuing a stopped run are proposal cards, and no turn starts by itself.
   Pipelines are the exception: there is no card for them, the chat changes none. A card to publish a pipeline kept
   from before pipelines lost their drafts still shows; it opens the pipeline, and *Apply* is refused with nothing
   published.

## Domain tools the chat has

Each goes through exactly the same tool an MCP client of the app calls ([mcp-tools-setup.md](mcp-tools-setup.md),
[mcp-tools-work.md](mcp-tools-work.md)), so it is refused or carried out the same way.

**While it drives**, the chat has every domain tool of the app except `add_project` and the app's `*_mcp_server` tools
(the chat has its own): the reads, tasks and modules, runs (`run_task`, `pause_run`, `resume_run`, `cancel_run`,
`continue_run` with every choice, `merge_run`, `complete_run`, `remove_worktree`, `return_to_draft`), questions and
approvals (`answer_question`, `send_message`, `decide_plan`, `decide_approval`), queue holds, creating, saving and
duplicating pipelines, the profile, instruction, skill, tool, service and stack drafts and their publication,
connections and accounts, the project's settings the app lets automation change (Autopilot, reviewer, environment,
limits, code index, repository agent files). Each keeps its own rules: `decide_approval` and `decide_plan` need *Let
automation answer approvals*, and what only a person may do is `person_only`, as for an outside client.

**While it does not drive**, it has these: `list_projects`, `get_project`, `list_tasks`, `get_task`, `create_task`,
`update_task`, `list_modules`, `get_module`, `create_module`, `update_module`, `list_runs`, `get_run`, `get_changes`,
`get_conversation`, `run_task`, `pause_run`, `resume_run`, `cancel_run`, `merge_run`, `continue_run`,
`answer_question`, `send_message`, `list_approvals`, `list_questions`, `get_plan`, `hold_queue`, `release_queue`,
`get_approvals`, `list_pipelines`, `get_pipeline`, `save_pipeline`, `create_pipeline`, `duplicate_pipeline` (all three
refused as `drive_only`), `list_profiles`, `get_profile`, `save_profile_draft`, `publish_profile`,
`list_instructions`, `get_instruction`, `save_instruction_draft`, `publish_instruction`, `list_skills`, `get_skill`,
`save_skill_draft`, `publish_skill`, `list_tools`, `get_tool`, `save_tool_draft`, `publish_tool`, `list_secrets`,
`list_connections`, `list_accounts`, `list_models`, `get_usage`, `get_delivery`, `get_story_point_calibration`,
`get_daily_report`.

**Never available to the chat**: `add_project`; deleting anything; the app's `*_mcp_server` tools (the chat has its own
`mcp_*` tools).

## Direct or proposal

| Tool | While it drives, in a person's turn | While it drives, in a turn the Orchestrator started itself | While it does not drive | In a daily report's turn |
|---|---|---|---|---|
| Reads | direct | direct | direct | direct |
| `create_task`, `update_task`, `create_module`, `update_module`, `save_*_draft` | direct | direct | direct | refused `report_turn` |
| `run_task` | direct (refused `task_blocked` for a task marked Blocked) | direct for a task this conversation leads, refused `not_led` for another | proposal; direct while the project allows Autopilot in a person's turn | refused `report_turn` |
| `pause_run`, `release_queue` | direct | direct | direct in a person's turn; proposal in a turn it started itself | refused `report_turn` |
| `resume_run` | direct for a paused run and for an interrupted one; stopped at a limit → `use_continue_run` (decide it with `continue_run`), canceled or failed → `person_only` | direct for a paused run; for an interrupted one only the run whose interruption started the turn, once, not within 10 minutes of the time before (`interrupted_again`) and not while more than 3 runs stopped on a busy database (`storage_contention`) | direct for a paused run; an interrupted one `person_only` | refused `report_turn` |
| `hold_queue` | direct | direct | direct | refused |
| `cancel_run` | direct | direct | proposal | refused `report_turn` |
| `continue_run` | direct, every choice | direct, every choice | proposal: only `allow_retry`, `return_to`, `resolve_conflicts`; the other choices `person_only` | refused `report_turn` |
| `merge_run` | direct | direct for a run the rule lets it (`left_to_person` otherwise; see auto-merge) | proposal | refused `report_turn` |
| `answer_question`, `send_message` | direct (deciding a waiting plan is refused `plan_decision_person_only`) | direct | proposal | refused `report_turn` |
| `publish_*` | direct, unless the draft widens agents' permissions while *Let automation change agent permissions* is off (`person_only`) | same | proposal | refused `report_turn` |
| `init_repository`, `git_*` writes, `mcp_*` | direct (`git_*` that can lose work stay `git_propose` cards; `mcp_*` that widen need *Let automation change agent permissions*) | same | cards; git writes `write_off` | refused `report_turn` |

Every change, proposal and refusal is kept in the conversation. A proposal card's *Apply* checks again
(`changed_since_proposed`); automation's synthetic click on *Apply* is refused `person_only`. A turn the Orchestrator
started itself is marked `[Orchester]`.

## The switch (Project settings › Orchestrator › Let the Orchestrator drive)

Each project has its own. Off until a person turns it on; no tool and no synthetic input can change it. It replaced six
switches — *publish drafts*, *answer agents*, *merge runs*, *merge finished runs on its own*, *continue interrupted
runs* and *write with git* — and a project that had *answer agents* on starts with it on. While it is on:

- The chat is woken for **every** wait of an agent, every run that stops, ends or succeeds and every event, **whoever
  started the run** and whatever raised it (the chat's own answer included); there is no per-run or per-step limit on
  its turns. A turn starts only while no other turn of its conversation thinks and the project's runner attached.
- The turn a run's success starts **merges** it once if the rule passes — latest run, every step passed (a person's
  accept counts, and so does the Orchestrator's own since 2026-11-11: it then says which step it accepted unverified
  and what was left unverified), task not Blocked or blocked, new starts not held, task not *Don't merge
  automatically*, the main checkout needs no local integration — one at a time, in end order; conflicts → it calls
  `continue_run` `resolve_conflicts`. A task's own *Don't merge automatically* still keeps its runs from being merged
  on their own.
- The turn of an interrupted run's end **continues** it from the last checkpoint (crash, runner gone, *Quit now*,
  database busy), unless it was interrupted again within 10 minutes of the time before (`interrupted_again`) or more
  than 3 runs stopped on a busy database within 10 minutes (`storage_contention`).
- It decides what a stopped step needs itself: it accepts when the step's report found no real mismatch and what it
  could not check is unreachable, otherwise returns the work to the right step with a note or lets it retry, and
  raises a limit or budget when the run is close to done and the work is worth it, saying by how much and why. It never
  tells the person to press a button its tools can press.
- A day's report and the summary of a watching that ended by itself start a turn that changes nothing (`report_turn`).

Related: *Let automation answer approvals* and *Let automation change agent permissions* (Settings › Automation),
*Daily report from the Orchestrator* (Settings › Notifications), *Let Orchester estimate story points* (Project settings
› Runs › Estimates).

## Chat-only tools (27)

### `init_repository`

Proposes `git init` (branch from `init.defaultBranch`, else `main`) and a first commit *Initial commit* — or only the
first commit for a repository without one. `.orchester/` is kept out via `.git/info/exclude`. Never adds a remote,
pushes or changes git settings. While the chat drives it makes the repository at once; otherwise it is **a card**; the
card shows the branch, file count, the whole `.gitignore` and files that look like secrets (e.g. `.env`).

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
  waiting task, kept in the conversation that leads it (in none when no conversation leads it). With *Let the Orchestrator
  drive* on it starts a turn there, which tells the run's agents with `send_message` (by the tool's rules as for any
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

### `watch` and `unwatch`

When the person asks in words in this conversation ("watch ORC-889 and ORC-890", "follow the Docs module", "stop
watching Docs"), the chat subscribes or unsubscribes **this conversation** with the same watch and stop the
[commands](#the-commands) apply, by the same rules: the most specific subscription wins, and stopping cancels and
pauses nothing. No command bubble is kept: the conversation shows the usual line with a quiet *at your request*
(*Leading Docs · 4 tasks at your request — …*, *Stopped watching Docs at your request — nothing is canceled or
paused*), and History's *Leads …* and the header's chips change as for a command. A target another conversation leads
is **not taken over**: `watch` answers it under `led_elsewhere` with that conversation's title, and the conversation
shows the [already-led card](#already-led-elsewhere), where only the person chooses *Take it here* (with the order
their words gave) or *Open that conversation*.

Parameters of `watch`: `targets` (req.: task keys, module names or ids, `without_module`, `project`), `order` (the
person's words for their events; left out, the default order), `project` (the chat's own). It answers `started`,
`already_here`, `led_elsewhere` and `gone`. Parameters of `unwatch`: `targets`, or `all: true`; it answers `stopped`
and `left_open`. Refusals: `event_turn` (a turn Orchester started by itself: the agent never subscribes on its own),
`target_not_found`, `other_project`, `gone`, `not_watched`, `led_elsewhere` (unwatch of what another conversation
leads), `no_target`; a turn that plans refuses both, as every change.

### MCP server tools

`mcp_add_server`, `mcp_update_server`, `mcp_set_server_enabled`, `mcp_remove_server` take the same arguments as the
app's `add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`, `remove_mcp_server`. **While the chat drives**
each is made at once by the app's own rules (what widens is `person_only` unless *Let automation change agent
permissions* is on). **Otherwise adding or widening is always a card.** Narrowing (turn off, remove one no profile
chooses, drop exempt tools or authorization) is direct only in a person's turn while *Let automation change agent
permissions* is on; otherwise a card.

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

Writes need *Let the Orchestrator drive* (otherwise `write_off`), never happen in a report's turn (`report_turn`) and,
while the chat does not drive, in a turn the Orchestrator started (`person_only`), and are refused `checkout_reserved` while local integration or a conflict
resolution holds the checkout, and `operation_in_progress` during a merge or rebase. Other codes: `git_not_found`,
`not_a_repository`, `nothing_to_clean`, `nothing_in_progress`, `detached_head`, `changed_since_proposed`.

## Turns the Orchestrator starts itself

Only subscriptions route the events of a task. By the **routing rule**, each event below is kept as **one entry** in
the conversation that **leads the task** through a subscription (see [Watching](#watching)): the task's own, else its
module's (or *Tasks without a module*'s, for a task with none), else the whole project's (*Everything in* the
project).

An event of a task **no conversation leads goes to no conversation** and starts no turn, even when a conversation
dealt with the task before: it stays where it always shows outside the chat — the task's page, *Needs you*, the
Overview and the notifications. No conversation is ever opened for an event. While such an event still stands, a
subscription that starts later takes it into its conversation. To get every event into one conversation, watch
*Everything in* the project there (`/watch-module`). Entries kept before stay in their conversations' history.

A watched task's events never reach another conversation, whoever dealt with it before. With *Let the Orchestrator
drive* on, each entry also starts **one** turn there by itself — never while another turn of that conversation thinks
(it waits for that turn to end) and only while what it is about still stands; whoever started the run and whatever
raised the event, the Orchestrator's own answer, message, merge or cancel included. With the switch off the entry only
links to its card. Such a turn is
marked `[Orchester]`; it is told the subscription the event came through and
the order that applies (see [Orders](#orders)).

- **Waits** — when an agent asks a question with suggested answers, a step waits for a message, or a plan waits for a
  decision, the conversation gets an entry linking to the card. The turn answers only what follows plainly from the
  brief, criteria, docs, AGENTS.md or the person's words, otherwise tells the person the question with options and a
  recommendation.
- **Tool requests** (since 2026-11-11) — a tool request that came to the person gets an entry too, and with *Let the
  Orchestrator drive* on starts one turn, once: the turn reads the request (`list_approvals`, the run, the brief) and
  approves it once with `decide_approval` only when it plainly serves the task and is safe (inside the run's
  worktree, nothing irreversible, nothing that reaches another system or person, no secret), denies it with a
  `message` that tells the agent why only when it is plainly wrong, and otherwise decides nothing and tells the person
  what is asked, why it hesitates (*Not sure: …*) and what it recommends (*Recommends …*). In that turn
  `decide_approval` decides only that request (`not_this_request` for another), takes the chat's own `reason` (one
  sentence, required) and never allows a tool for good; allowing similar, all of a run or the project's stays the
  person's, on the card. While the chat drives, *Let the Orchestrator drive* is its consent to answer: *Let automation
  answer approvals* is not needed for the chat (the person's decision of 2026-11-11), and other MCP clients still need
  it. The request's line in the conversation says where it stands: *Orchestrator is reading*; *Allowed by the
  Orchestrator* or *Denied by the Orchestrator*, with what was asked and its reason, or the words it told the agent,
  nested under it; *Waits for you*, with the Orchestrator's words and, under them, the request's card from the task's
  Conversation, answered here exactly as there; and, once the person answered, how (*Allowed once by you*, *Denied by
  you*, …). Requests that arrive together get a turn each, one at a time per conversation.
- **Run ends** — a task's latest run that succeeds, fails, stops for a decision, stops at a limit or is interrupted
  gets one entry. The turn: success → merges it under the merge rule above (or says why it is left to the person), and
  once merged, where an order applies (see [Orders](#orders)), does what it asks of the work it leads that can start
  now, such as starting with `run_task`, once each, the tasks the merge unblocked or a slot refused before (a refusal
  such as `no_agent_slot` is told, not retried; the next merged success comes back to it); a
  decision wait or a limit → decides with `continue_run`; failure → may return the work to a step with `continue_run`
  and explains; an interruption → continues it (see the switch) or explains the person's choices.
- **No progress** — a run an agent works on that made no progress (no run event such as a step change, no transcript
  event such as a tool call, no agent launch) for the minutes of *A run without progress for* (Project settings ›
  Orchestrator › *Watching*; 20 by default, 1–240, or off). Raised once per stall; progress again re-arms it. A run
  that waits for a person, is queued, paused, asked to pause, waits for a workspace or waits on another task never
  stalls. The turn reads the run, tells the person what it was doing and their choices, and may send its agents a
  message; it starts, resumes, cancels or merges nothing. The entry links to the task's Summary.
- **Merge conflict** — a merge of a task's run that stopped at conflicts (from the task page, `merge_run` or the merge
  queue), once per attempt; only the newest attempt of a run still not merged. The turn calls `continue_run`
  `resolve_conflicts` naming the files, or points to the Local integration page; it does not call `merge_run` again.
  The Orchestrator's own merge that conflicted starts its turn like any other, once the turn that merged ended.
- **Waited task merged** and **waited task closed** — the task a run waits on through [`wait_on_task`](#wait_on_task)
  was merged into the run's base branch, or canceled or closed without that merge. See there.
- **A subscription ended by itself** — see [Ending](#ending).
- **Daily report** — at the time set in *Daily report from the Orchestrator*, each open project's latest conversation
  gets one *Daily report* entry per local day with activity (missed days are delivered at the next start, at most 7
  back). With *Let the Orchestrator drive* on, a turn adds a summary and recommendations and **changes nothing** (every change
  is refused `report_turn`). `get_daily_report` reads a day's report. The daily report is not
  an event of a task: the routing rule does not apply to it.

## Watching

A conversation **leads** what the person tells it to watch: its subscriptions decide where the events of their tasks
go (the routing rule above). Subscriptions are **the person's alone**: the person subscribes with the commands, which
the app applies itself, or by asking the Orchestrator in words, whose [`watch` and `unwatch`](#watch-and-unwatch)
apply the same for this conversation only, never on the agent's own initiative and never in a turn Orchester started.
The agent sees its subscriptions and their orders in its standing instructions and in every turn an event starts. A
watch command sent by automation (synthetic input) is not applied.

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
field; empty by default, meaning no order: the switch decides). A subscription without its own text follows
the default order as it is when an event comes. Watching a target the conversation leads already with other text
renews its order.

An order is quoted to the agent as data and **can only narrow** what the person-only switch allows: whether it acts
at once or only proposes stays as the switch and the tables above say, whatever the order's text says. While the
switch is on, an order such as *start tasks as they become ready* is carried out in the turn of a run's success the
Orchestrator merged: it starts the tasks of what the conversation leads that can start now, once each. A start an
agent slot refused waits for the next merged success of what it leads; no other event brings it back.

### Already led elsewhere

Choosing a target another conversation leads applies the rest of the command and shows a card under its line:
*Webhooks v2 is already led in «…»*, since when, with **Open that conversation** (nothing moves; the line reads *…
stays led in «…»*) and **Take it here** (the subscription moves here with this command's order; that conversation gets
the line *… is now led in «…»*). Only a person chooses on the card: synthetic input is refused.

### Seeing and stopping

- The conversation's header shows what it leads right after its title, after an eye: a chip per target. Those
  without room fold into *+N*, which opens the whole list with since when for each; × on a chip or on a row of that
  list stops that subscription.
- History shows *Leads …* on a conversation that leads something.
- A module's Summary and a task's Summary properties show **Led in** with the leading conversation.
- Stopping (×, *Stop watching*, or asking the Orchestrator in words) ends the subscription at once and **cancels or
  pauses nothing**: the line *Stopped watching …* lists what was left open (waiting, running, queued, stopped at a
  limit); its later events go to the conversation of a less specific subscription that covers it, or to none.

### Ending

A subscription **ends by itself** when its task is Done or Canceled, or when every task of its module is (a module
with no task does not end). *Tasks without a module* and the whole project never end by themselves. Its conversation
gets the line *Every task of Mobile release is Done — watching ended* (or *KON-41 is Done*, *… is Canceled*, *… is
Done or Canceled*). With *Let the Orchestrator drive* on, one turn writes the module's or task's summary under
it and **changes nothing**, as a daily report's turn. A status changed anywhere ends it; reopening the task afterwards
does not bring the subscription back.

### Created tasks

A task the chat creates (`create_task`) in a conversation that watches something, outside what it watches, becomes a
**task subscription** of that conversation, so its events come back there. It takes the order of the subscription
whose event started the turn; in a person's turn, none of its own (the default order applies). Nothing is added when
the conversation watches nothing, when one of its subscriptions already covers the task, or when another conversation
leads the task's module or the tasks without a module — then the task's events go there, and `create_task`'s answer
(`watch`) and the action's row say which.
