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
  every call is refused.
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
`get_delivery`, `get_story_point_calibration`, `get_daily_report`, `list_pull_requests`, `get_pull_request`.

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

## Person-only switches (Settings › Automation › The Orchestrator may)

All off until a person turns them on; no tool and no synthetic input can change them. The window labels them
shortly (*Publish drafts*, *Answer agents*, …); their full names, used below and in the logs, are *Let the
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

## Chat-only tools (23)

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

- **Waits** — when an agent asks a question with suggested answers, a step waits for a message, or a plan waits for a
  decision, the conversation that last dealt with the task (else the project's latest, else a new one) gets an entry
  linking to the card. With *Let the Orchestrator answer agents* it also starts **one** turn: it answers only what
  follows plainly from the brief, criteria, docs, AGENTS.md or the person's words, otherwise tells the person the
  question with options and a recommendation.
- **Run ends** — a task's latest run that succeeds, fails, stops for a decision, stops at a limit or is interrupted
  gets one entry. With *answer agents* on, it starts a turn: success → says it is ready (merges only under the
  merge-finished rule); a decision wait → may propose `continue_run`; failure/limit/interruption → explains the
  person's choices.
- **Daily report** — at the time set in *Daily report from the Orchestrator*, each open project's latest conversation
  gets one *Daily report* entry per local day with activity (missed days are delivered at the next start, at most 7
  back). With *answer agents* on, a turn adds a summary and recommendations and **changes nothing** (changes are only
  proposed; anything else is refused `report_turn`). `get_daily_report` reads a day's report.
