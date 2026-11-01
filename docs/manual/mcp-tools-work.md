# MCP reference — work tools

These tools drive a project's work through data: projects and limits, tasks and modules, runs, the waits of a run
(approvals, questions, plans, messages), approval modes, worktrees, pull requests, and usage and delivery reports.
Conventions (naming, strict schemas, links, errors, gates) are in [mcp-server.md](mcp-server.md); project setup
tools are in [mcp-tools-setup.md](mcp-tools-setup.md).

Gate abbreviations: **AP** = *Let automation change agent permissions*; **AA** = *Let automation answer
approvals*. Both are person-only switches (see [mcp-server.md › Permission gates](mcp-server.md#permission-gates)).

Most read tools that take an optional `project` read **across projects** when it is left out: each item then names
its project, and `unreadable_projects` lists the projects that could not be read, with why (for example
`project_not_opened_yet` for a project this app version has not opened yet — list tools only read and never upgrade a
project's data).

---

## 1. Projects and project settings

### `list_projects` — read-only · no arguments

The workspace's projects, most recently opened first: id, name, folder, whether each uses the code index, its limits
(token budgets, default time limit, agents working at once) and its app link (Overview). Links are the app's route
paths; a person opens one with a click.

### `set_code_index` — changes data

Turns the project's code index (*Use code index*) on or off for agents that start from now on, as Project settings
does. Returns the new state.

| Parameter | Type    | Req. | Description                                       |
|-----------|---------|------|---------------------------------------------------|
| `project` | string  | yes  | The project's id, or its name.                    |
| `enabled` | boolean | yes  | Whether agents that start from now on get it.     |

### `update_project_limits` — changes data

Changes a project's limits, as Project settings › Runs › Limits does. Limits left out stay as they are; invalid ones are
refused with why. There is **no maximum** for token or time limits; `null` means *no limit*. Raising the daily budget
or setting it to `null` lets runs it stopped go on, a person's decision: refused as `automation_not_allowed` (nothing
saved) unless a person turned on *Let automation answer approvals*; lowering it is not. Returns the limits as saved.

| Parameter                     | Type            | Req. | Description                                                                                  |
|-------------------------------|-----------------|------|----------------------------------------------------------------------------------------------|
| `project`                     | string          | yes  | The project's id, or its name.                                                               |
| `default_run_budget`          | integer \| null | no   | Tokens prefilled in *New task*; `null` = no default budget.                                  |
| `default_time_limit_minutes`  | integer \| null | no   | Whole minutes (> 0) of active agent time prefilled in *New task*; `null` = no default.       |
| `daily_budget`                | integer \| null | no   | Tokens all runs of the project may use per day; `null` = no limit.                           |
| `agent_slots`                 | integer \| null | no   | Agents working at once across runs, **1–16**. Applied at once by the runner: more lets queued work start; fewer stops no agent and holds new work. |
| `qa_slots`                    | integer \| null | no   | QA apps (`start_qa_app`) runs may have open at once, **1–16**. Applies from the next start; a start beyond it waits until one closes. |

### Approval modes

Each agent profile has an **approval mode** (`save_profile_draft`'s `permissions.approval_mode`, see
[setup](mcp-tools-setup.md#save_profile_draft--changes-data--gate-ap-for-widening)): **Autopilot** approves every
tool request at once, except work outside the run's worktree, its Sources and the agent's temp folders, pushes to a
remote and refs other than the run's branch, which still come to the person; **Manual** runs the profile's allowed
commands, asks the project's reviewer about the rest, and leaves what the reviewer denies or is unsure about to the
person. A run's step is in Autopilot only when its profile says Autopilot, the project allows it (*Allow Autopilot for
agents in this project*, Project settings › Runs) and the run was not switched to Manual. Decisions (questions, plans,
limits, conflicts) always come to the person in both modes.

### `get_approvals` — read-only

A project's approvals. Parameter: `project` (req.). Answers `approvals` with:

- `allow_autopilot` — *Allow Autopilot for agents in this project*.
- `reviewer` — the connection, model and effort that review Manual's requests (`default` when no one chose another),
  or `unavailable` with why the project has none.
- `runs` — each run of the project that has not ended: `id`, `number`, `task`, `link`, and `approval_mode`: `{"mode":
  "autopilot"}`, or `{"mode": "manual", "because": …}` with `profile` (its profile is Manual), `project` (the project
  does not allow Autopilot) or `run` (the run was switched to Manual), the first that says so.

### `set_allow_autopilot` — changes data · person only to turn on

Turns the project's *Allow Autopilot for agents in this project* on or off. `false` is always taken: every agent of
the project then runs as Manual. `true` is refused as `person_only` whatever automation may do: only a person allows
Autopilot, in Project settings › Runs (asking for it while it is on already changes nothing). Parameters: `project`,
`allow_autopilot` (boolean) — both required. Answers as `get_approvals`.

### `set_reviewer` — changes data · gate AA, and AP for another reviewer

Chooses the reviewer Manual asks before the person. Refused as `automation_not_allowed` unless AA is on; a reviewer
other than the current one also needs AP (`person_only`), and one on another account says so among the reasons.
Answers as `get_approvals`.

| Parameter    | Type           | Req. | Description                                                                                                   |
|--------------|----------------|------|---------------------------------------------------------------------------------------------------------------|
| `project`    | string         | yes  | The project's id, or its name.                                                                                |
| `connection` | string \| null | no   | An agent connection: Claude Code, Codex, or a native one on the Anthropic API; left out = the default reviewer, the first Claude Code connection with Haiku 4.5. |
| `model`      | string \| null | no   | A model of that connection; its default when left out.                                                        |
| `effort`     | string \| null | no   | An effort the model takes; the connection's default when left out.                                            |

### Retired: `get_auto_mode`, `set_auto_mode`, `set_run_auto_mode`

Auto mode was replaced by approval modes. For one release these names stay listed, change nothing and answer the
error `retired`, whose message names the tool to use instead: `get_auto_mode` → `get_approvals`; `set_auto_mode` →
`set_allow_autopilot` and `set_reviewer` (*New runs use auto mode* and *approve everything in each run* are gone: a
profile's approval mode and the project's *Allow Autopilot* take their place); `set_run_auto_mode` → `set_run_manual`.

---

## 2. Tasks

A task has a **key** (`KON-12`), title, brief, acceptance criteria, a pipeline, limits, and organization fields:
workflow status, priority, story points, tags, Blocked reason, blocking tasks, module, *Don't merge automatically*
and `delivers`. A task is a **draft** until it is submitted (queued) to run.

Enumerations:

- **Workflow status**: `backlog`, `todo`, `in_progress`, `in_review`, `done`, `canceled`.
- **Priority**: `urgent`, `high`, `medium`, `low`, `none`.
- **Story points**: 1, 2, 3, 5, 8, 13, 21.
- **Blocker gate**: `merge` (the blocking task is finished once its work is merged).
- **Delivers**: `changes`, `report`, `outside`, `changes_or_report` (see
  [setup › Pipelines](mcp-tools-setup.md#5-pipelines)).

### `list_tasks` — read-only · cross-project

A project's tasks as its task list finds them: key, title, workflow status, priority, story points (`points`,
`set_by` person/automation, automation's reason; `null` for none), tags, `blocked_reason`, `held_by_blocking_tasks`
(unfinished blocking tasks keep it from running), pipeline, where it stands, link. For a named project git is asked
which runs were merged outside Orchester; across projects, data is read as stored.

| Parameter | Type                    | Req. | Description                                                        |
|-----------|-------------------------|------|--------------------------------------------------------------------|
| `project` | string \| null          | no   | Left out = every project.                                          |
| `search`  | string \| null          | no   | Words the title, key, id or brief contain.                         |
| `status`  | Workflow status \| null | no   | Only tasks with this workflow status.                              |

### `get_task` — read-only

One task: title, brief, criteria, draft or submitted, workflow status, priority, story points, tags, module,
`base_branch`, `blocked_reason`, `blocked_by`, `no_automatic_merge`, `delivers` (`null` = as its pipeline says),
pipeline, limits, where it stands, `result` (for a latest run that passed every step: what it delivers, whether it
committed nothing, its completion, its report and the MCP tool calls it was let make outside the repository; `null`
otherwise), its runs and its link (Details tab; each run links to the Summary).

`base_branch` holds `choice` (`inherit`, `checked_out` or `branch`) with the `branch` set, and `next_run`: the branch
its next run takes as things stand (`null` = whatever the checkout has checked out), with its `source` (`task`,
`module` with that `module`'s id, or `checked_out`). Each run's own base is in `get_run`.

Parameters: `project` (req.), `key` (req.: key or id).

### `create_task` — changes data

Saves a new **draft** task. Limits left out take the project's defaults (as the *New task* form fills them); `null`
is no limit. Any published pipeline may be chosen. Organization is checked as the task form checks it. The task's
history records the organization as automation's.

A task's **base branch** is the branch its runs start from and deliver into. It is kept on this computer only: in a
shared project the tools set and read this computer's choice, which is never sent to the server, and teammates may
choose differently.

| Parameter            | Type                       | Req. | Description                                                                                       |
|----------------------|----------------------------|------|---------------------------------------------------------------------------------------------------|
| `project`            | string                     | yes  | The project's id, or its name.                                                                    |
| `title`              | string                     | yes  | Title.                                                                                            |
| `brief`              | string \| null             | no   | The task's description for agents.                                                               |
| `criteria`           | array<string>              | no   | Acceptance criteria, one per entry; blank ones dropped.                                           |
| `pipeline`           | string \| null             | no   | A published pipeline's id or name.                                                                |
| `budget_tokens`      | integer \| null            | no   | Token budget of each run; `null` = none; left out = project default.                              |
| `time_limit_minutes` | integer \| null            | no   | Active agent minutes per run; `null` = none; left out = project default.                          |
| `workflow_status`    | Workflow status \| null    | no   |                                                                                                   |
| `priority`           | Priority \| null           | no   |                                                                                                   |
| `story_points`       | integer \| null            | no   | 1/2/3/5/8/13/21, recorded as automation's. Left out: estimated when the project's *Let Orchester estimate story points* is on. |
| `tags`               | array<string>              | no   | Tag names, case-insensitive; unknown names are created as tags.                                   |
| `blocked_reason`     | string \| null             | no   | Marks it *Blocked* (informational; keeps no run from starting, but the Orchestrator chat won't start it). Must not be blank. |
| `blocked_by`         | array<{`task`: string, `gate`?: `merge`}> | no | Blocking tasks of the same project, by key or id.                                      |
| `module`             | string \| null             | no   | Module id or name (`list_modules`).                                                               |
| `no_automatic_merge` | boolean                    | no   | *Don't merge automatically*: the Orchestrator won't merge its runs on its own. Default `false`.   |
| `delivers`           | Delivers \| null           | no   | Overrides the pipeline's `delivers`.                                                              |
| `base_branch`        | string \| null             | no   | `inherit` (its module's; default), `checked_out` or a branch name; a name only a remote has (`release` or `origin/release`) creates its local branch, tracking it. |

Refusals: `task_not_found` (unknown blocker), `invalid_organization` (blank reason, cycle), `shared_task`
(`no_automatic_merge` in a shared project), `module_not_found`, `branch_not_found` (the repository has the base branch
nowhere; nothing is saved).

### `update_task` — changes data · gate AP for switching pipelines

Changes a task's details or organization; fields left out stay as they are; a `null` limit is no limit. Same fields
as `create_task` (all optional) plus `key` (req.). Specifics:

- **Pipeline**: moving a task to *another* pipeline is a person's decision — `person_only` unless AP is on (a task
  without a pipeline may get any published one).
- **`workflow_status: done | canceled`** closes the task (*Mark as done* / *Cancel task*), whatever its latest run
  did. Refused as `stop_run_first` while a run of the task has not ended (`cancel_run` first). A closed task leaves
  *In progress*, *Needs you* and the queue. `backlog` or `todo` reopens it.
- **`workflow_status: todo`** on a task whose latest run ended is the Tasks board's *Move to Todo*: only the status
  changes; no run starts and nothing is queued. `run_task` runs it again.
- **`blocked_reason`**: a string sets *Blocked*; `null` clears it.
- **`blocked_by`** / **`tags`**: the whole list; `[]` clears.
- **`module`**: `null` takes it out of its module.
- **`no_automatic_merge`**: `true`/`false`; in a shared project refused as `shared_task` before anything is saved.
- **`delivers`**: `null` follows the pipeline again.
- **`base_branch`**: the task's next runs take it; runs that started keep the base they recorded.
- **`story_points`**: refused as `persons_points` when a person set them. After a brief/criteria change without
  points, Orchester re-estimates when the switch allows.
- **Deleting a task** is a person's alone, in the app — no tool deletes one.

### `get_story_point_calibration` — read-only

How story points tracked reality: per points value (1…21) the merged tasks with it and their median tokens, active
agent minutes and attempts; and each merged task with its points (who set them) and what it actually took over all
its runs. These are the examples estimates read. The estimator's own requests are reported by `get_usage` under
`estimates`. Parameter: `project` (req.).

### `return_to_draft` — changes data

Returns a **queued** task whose run never started to Draft (task list's *Return to drafts*) — only a task its group
holds for good (a task of the group failed, the group's cap, refused by preflight or the runner, blocked or
canceled). Parameters: `project`, `key` (both req.).

---

## 3. Modules

Modules are epics, milestones or sprints that group a project's tasks. Shared projects have no modules.

A module's **base branch** is the branch its tasks that inherit theirs start from and deliver into (`null` = whatever
the checkout has checked out when a run starts). Like a task's, it is kept on this computer only and never sent to a
server.

### `list_modules` — read-only

Oldest first: name, description, start and target dates (`YYYY-MM-DD`, local days), state (`open`/`closed`),
`base_branch`, task count, progress (scope and done in its unit — tasks, until tasks have story points; Canceled tasks
are out of scope), link. Parameter: `project` (req.).

### `get_module` — read-only

One module as its page shows it: details (`base_branch` among them), progress, tasks (key, title, workflow status,
link), and its **burndown** by local day from its start (or its first task's day) to today or its target date (at most
366 days): scope and remaining work at each day's end (`null` for future days), the ideal line (`null` without a
target), and work added and removed that day. *Done* is the task's Done (merging its run sets it, in Orchester or found
merged outside by git, as does a person). `forecast`: 50% and 85% delivery dates, or `not_enough_history`.

Parameters: `project` (req.), `module` (req.: id or name).

### `create_module` — changes data

Makes a new open module in a **local** project (refused as `shared_task` in a shared project).

| Parameter     | Type           | Req. | Description                               |
|---------------|----------------|------|-------------------------------------------|
| `project`     | string         | yes  |                                           |
| `name`        | string         | yes  | Unique among the project's modules.       |
| `description` | string \| null | no   |                                           |
| `start_date`  | string \| null | no   | `YYYY-MM-DD`.                             |
| `target_date` | string \| null | no   | `YYYY-MM-DD`, not before the start.       |
| `base_branch` | string \| null | no   | `checked_out` (default) or a branch name; a name only a remote has (`release` or `origin/release`) creates its local branch, tracking it. |

Put tasks in it with `create_task`/`update_task`'s `module`. Refusals: `branch_not_found` (the repository has the
branch nowhere; nothing is made).

### `update_module` — changes data

Changes `name`, `description`, `start_date`, `target_date` (`null` = no date), `state` (`open` | `closed`) or
`base_branch` (as `create_module` takes it; the next runs of its tasks that inherit theirs take it, runs that started
keep theirs); fields left out stay. Its tasks and history stay. Parameters: `project`, `module` (req.) + the fields.

---

## 4. Runs

A **run** is one execution of a task's pipeline. Runs have an id and a per-project **number** (`#7`). Every tool that
takes `run` accepts the id or the number (`7` or `#7`).

### `run_task` — changes data

Runs a task as the app allows: queues and starts a draft, starts a queued task, or runs a task again after its latest
run ended. Refusals name why (e.g. the runner's `task_not_queued`, a held queue, missing base branch, limits).
Parameters: `project`, `key` (both req.).

### `list_runs` — read-only · cross-project

Runs, newest first, as each task's Summary lists them: id, number, task (key, title), phase (`merged` also when git
proves its work merged outside Orchester), state and why (with `runner_failure` as in `get_run`), `delivery` (merge
queue state, `null` if it never joined), what happened, tokens used and budget, start and last change, link. A run
that cannot be read has no phase/state/tokens; its detail says why. While steps of a run wait for an agent slot, its
detail ends by naming them (`… · Review waits for an agent slot`).

| Parameter | Type            | Req. | Description                                   |
|-----------|-----------------|------|-----------------------------------------------|
| `project` | string \| null  | no   | Left out = every project.                     |
| `task`    | string \| null  | no   | Only this task's runs (needs `project`).      |
| `limit`   | integer \| null | no   | 1–200; default 50. `more` tells if older exist. |

### `get_run` — read-only

The complete state of a run:

- **Steps**, **state** and **tokens** (the reviewer's included and shown apart), **active time**. A step's
  `status` is its `state` and `waits_for`: while it is `waiting`, what for; `agent_slot` on a `pending` step that is
  ready and held back only because every agent slot is in use (raising *Agents at once* starts it), which the online
  runner reports and nothing stores; `null` on a step whose dependencies have not passed.
- For a run the runner interrupted after its own failure: reason `runner_failed` with `runner_failure`
  (`storage_contention` — project database stayed busy; `storage_failure` — database unavailable; `other`; `null`).
- **What it waits for** (`awaiting`): agents' waiting questions with their options; steps waiting for a message with
  what each agent wrote last (`awaiting.messages`); the plan waiting for a decision (version, step, whole text,
  excerpt).
- **Decided plans** with decision, note, time and `decided_by` (person, or automation through MCP).
- **`base`**: the base the run started from and delivers into, as it recorded it when it started — `branch` (`null`
  when it took a detached HEAD's), `revision`, and `source`: `task` (the task's base branch), `module` (its module's,
  with that `module`'s id), `checked_out` (whatever the checkout had checked out) or `project` (a run recorded before
  tasks and modules had a base branch). A later change of the task's or module's base branch leaves it alone.
- **`merge`**: `merged` by `orchester` (with the recorded merge) or `outside` (git proves the base holds the run's
  commits), `not_merged`, `nothing_to_merge`, or `null` for a run that has not succeeded.
- **`base_moved`**: notices that the base branch moved — branch and revision, tasks and commits that landed, files
  both sides changed, and state: `waits_for_writing_step`, `waits_until_work_goes_back` (with `waits_for_step`, when
  only non-merging steps like Review/Verify are left), `kept_until_run_goes_on`, `waits_for_follow_up`, `delivered`
  (with the agent and step that read it), or `merged_by_runner` (with `merged_by_runner.before_step` and
  `merge_commit`: the move overlapped none of the run's changes, so Orchester merged it itself before that step).
- **`delivery`** (merge queue; `null` without a Deliver step or if it never joined): `lane` (the base branch it
  delivers into: the queue has a lane per base branch, each delivering one run at a time on its own), `queued` (with
  `place`, its place in its lane), `syncing`, `rechecking`, `integrating` (`integrate_step`, `files`),
  `waiting_for_approval`, `blocked`
  (`blocked.why`: `uncommitted_changes` + files, `other_branch_checked_out`, `operation_in_progress`,
  `local_integration`), `failed` or `needs_you` (`reason.why`: `conflicts` + files, `check_failed`,
  `base_keeps_moving` + heads, `stopped`, `later_run`), `merged` (revision, kind `merge_commit`/`squash`,
  `worktree_removed`, `branch_deleted`) or `removed`; plus `since`, `resyncs`, `approved_at`, `checked_on`.
- Links: the run links to its task's page; waits link to their cards on the task's Conversation.

Parameters: `project` (req.), `run` (req.).

### `pause_run` / `resume_run` / `cancel_run` — changes data

- `pause_run`: pauses a run at its agents' next checkpoint.
- `resume_run`: resumes a paused run, or continues an interrupted one from its last checkpoint.
- `cancel_run`: cancels a run that has not ended.

Parameters (each): `project` (req.), `run` (req.). Runner refusals: `run_not_running`, `shutting_down`,
`runner_unavailable`, `runner_reply_lost`, …

### `continue_run` — changes data · gate AA for retry/return/accept/resolve

Continues a run that **stopped** with one of the choices its task page offers for how it stopped. Every limit value
is given explicitly (no maximum); nothing raises or lifts a limit by itself.

| Parameter | Type   | Req. | Description             |
|-----------|--------|------|-------------------------|
| `project` | string | yes  |                         |
| `run`     | string | yes  |                         |
| `choice`  | Choice | yes  | One of the kinds below. |

| `choice.kind`       | Extra fields                                   | When / what                                                                                                   | Gate |
|---------------------|------------------------------------------------|---------------------------------------------------------------------------------------------------------------|------|
| `raise_limits`      | `budget_tokens`, `time_limit_minutes` (both req.) | Stopped at its token budget or time limit: continue with these limits, above what was used.               | —    |
| `raise_step_budget` | `budget_tokens` (req.)                         | Stopped at a step's budget: continue with this budget for that step.                                        | —    |
| `resume`            | —                                              | Stopped by the project's daily budget (*Resume*). The runner refuses while today's budget is still used up. | —    |
| `allow_retry`       | —                                              | A step used up its retries or broke again: *Allow 1 retry & continue*. A step waiting unverified/unchanged: run it again. | AA |
| `return_to`         | `step`, `note` (both req.)                     | Return to an earlier agent step with a note. For an ended run (succeeded or failed): return its work to any agent step it executed, which runs again with the note, then the steps after it. | AA |
| `accept`            | —                                              | A step could not check everything (*unverified*) or was not re-run on an unchanged revision: accept it; the next steps run. | AA |
| `resolve_conflicts` | `step`?, `note`?                               | The last merge stopped at conflicts (`merge_run`'s `merge_failed`; *Not merged into main*): *Ask an agent to resolve*. Work returns to a writing step (default: the step that implemented what conflicts, else the last one) with a note to merge the base in and resolve, plus the conflicting files. | AA |

Refusals: `run_merged`, `worktree_removed`, `worktree_in_merge`, `run_not_ended`, `merge_not_conflicted`,
`step_cannot_write`, `no_writing_step`, `integration_running`, `automation_not_allowed`. Decisions are recorded as
automation's.

### `set_run_manual` — changes data

Switches one run to Manual for itself, as its status line's *Switch to Manual for this run* does: its agents'
requests no longer go through Autopilot but through the profile's allowed commands, the reviewer, then the person. A
narrowing, so always taken, whatever the person's switches; a run never goes back to Autopilot. Refused as `run_ended`
for a run that ended. Parameters: `project`, `run` — both required. Answers the run's `id`, `number` and
`approval_mode` (as `get_approvals` gives it).

### `remove_worktree` — changes data

Removes an ended run's worktree (task page's *Remove worktree*): committed work stays on the branches; only merged
branches are deleted. Refused while the run works, while a merge or removal of its task runs, when the worktree holds
uncommitted changes (a person decides that on the task page), when already removed, or `run_changed` when the run
worked again in other worktrees since this was read. Parameters: `project`, `run`.

`get_changes`, `merge_run` and `complete_run` are in [setup › Ending a run's work](mcp-tools-setup.md#10-ending-a-runs-work).

---

## 5. Queue

### `hold_queue` / `release_queue` — changes data

`hold_queue` holds **new starts** in the whole project, or — with `task` — in the reviewed group that task belongs
to. Running runs go on; queued tasks wait and a start is refused until `release_queue` lets them go on. Both answer
what holds now.

| Parameter | Type           | Req. | Description                                                        |
|-----------|----------------|------|--------------------------------------------------------------------|
| `project` | string         | yes  |                                                                    |
| `task`    | string \| null | no   | A task of the group to hold/release; left out = the whole project. |

---

## 6. What agents wait for: approvals, questions, plans, messages

### `list_approvals` — read-only · cross-project

Tool requests waiting for a decision, oldest first (kind, command or tool, arguments, agent, run, task, since when),
and the plans runs wait on (version, step, excerpt) with decisions on their earlier versions and `decided_by`. Each
links to its card on the task's Conversation. A request the reviewer still judges has `reviewing: true`: it
waits for the reviewer, not for a person, until the review ends (`get_run`'s `awaiting.approvals` mark it the same).
A request the run's approval mode brought to the person says why in `came_to_you`: `reason` is the card's code
(`outside`, `notAllowed`, `reviewerUnsure`, `reviewerCouldNotRun`, `reviewerDenied`, `push`, `refs`) and `detail` what
the card names with it (the path, the ref, or the reviewer's reason); for `reviewerCouldNotRun`, `failure` says how the
review failed (`could_not_run`, `timed_out`, `unreadable`; absent for a request recorded before it was kept, whose
`auto_mode` still says it), and `autopilot: true` marks one that came by Autopilot's rules. `get_run`'s `requests`
lists every request of the run, oldest first, with `answered` (`kind`: `autopilot`, `profile`, `reviewer` with its
`reason`, `person`; absent while it waits) and `came_to_you`.

Parameters: `project` (string | null), `run` (string | null — needs `project`).

### `decide_approval` — changes data · gate AA

Approves a waiting request once, or denies it. A denial may carry a `message` that the agent gets as the reason
(Codex, whose decline carries no words, gets it as the next message of its turn); the decision record keeps it.

| Parameter  | Type                       | Req. | Description                                         |
|------------|----------------------------|------|-----------------------------------------------------|
| `project`  | string                     | yes  |                                                     |
| `approval` | string                     | yes  | The request's id from `list_approvals`.             |
| `decision` | `approve_once` \| `deny`   | yes  |                                                     |
| `message`  | string \| null             | no   | Only with `deny`; non-empty; ≤ 2000 characters.     |

### `list_questions` — read-only · cross-project

Agents' questions waiting for an answer, oldest first: id, agent, when asked, whether the agent waits in its turn,
the questions with suggested answers, run, task, link. Under `waiting_for_message`: steps of tasks' latest runs that
wait for a message, each with what its agent wrote last (answer those with `send_message`).

Parameters: `project` (string | null), `run` (string | null — needs `project`).

### `answer_question` — changes data · gate AA

Answers a waiting question with suggested answers, as its card does: the option labels picked (one, or several where
allowed) and/or free text. Recorded as automation's. Refused for an unknown label, several answers to a single-choice
question, or an empty answer.

| Parameter  | Type           | Req. | Description                                            |
|------------|----------------|------|--------------------------------------------------------|
| `project`  | string         | yes  |                                                        |
| `run`      | string         | yes  |                                                        |
| `question` | string \| null | no   | The question's id; left out = the run's only waiting question. |
| `choices`  | array<string>  | no   | Labels of the options picked.                          |
| `text`     | string \| null | no   | An answer in your own words.                           |

### `get_plan` — read-only

A plan a gated step proposed, as its card shows it: version, step, agent, time, whole text, whether the run waits on
it, its decision (`approved`, or `changes_requested` with note; `decided_by`, when), the step's versions (no text),
run and link. Refused as `no_plan` / `plan_not_found`.

| Parameter | Type            | Req. | Description                                                                                  |
|-----------|-----------------|------|----------------------------------------------------------------------------------------------|
| `project` | string          | yes  |                                                                                              |
| `run`     | string          | yes  |                                                                                              |
| `step`    | string \| null  | no   | Step key or name; left out = the step waiting for a decision, else the latest plan's step.   |
| `version` | integer \| null | no   | Left out = the step's latest.                                                                |

### `decide_plan` — changes data · gate AA

Decides the plan a gated step waits with, as the plan card does. Recorded as automation's; the card says so.

| Parameter  | Type                             | Req. | Description                                                                         |
|------------|----------------------------------|------|-------------------------------------------------------------------------------------|
| `project`  | string                           | yes  |                                                                                     |
| `run`      | string                           | yes  |                                                                                     |
| `decision` | `approve` \| `request_changes`   | yes  | *Approve plan & continue* or *Request changes*.                                     |
| `version`  | integer \| null                  | approve | The version you decide on (as `get_run` shows it); optional with `request_changes`. |
| `note`     | string \| null                   | request_changes | What should change. `approve` takes no note.                         |

Refusals: `automation_not_allowed`, `run_not_found`, `no_plan_waiting`, `run_paused`, `plan_already_decided`,
`plan_version_changed`, `version_required`, `invalid_version`, `blank_note`, `note_not_taken`, and the runner's
`run_not_running`, `shutting_down`, `runner_unavailable`, `runner_reply_lost`.

### `send_message` — changes data

Sends a message to the agents of a task's **latest** run, as the Conversation composer does in the run's state: to
the agent at work (now or after its turn), as the answer to a waiting question or the changes a waiting plan needs,
as a follow-up of a run ready for review, as instructions that continue a failed run or one waiting for a decision,
resuming a paused or interrupted run, or running an ended task again with it. A run stopped at a limit keeps the
message until a person raises the limit. Refused when no agent reads it and the task does not run again.

| Parameter | Type                          | Req. | Description                                                                                       |
|-----------|-------------------------------|------|---------------------------------------------------------------------------------------------------|
| `project` | string                        | yes  |                                                                                                   |
| `run`     | string                        | yes  | Must be the latest run of its task.                                                               |
| `text`    | string                        | yes  | The message.                                                                                      |
| `timing`  | `now` \| `after_turn` \| null | no   | While an agent works on a connection that takes mid-turn messages: `now` (at its next tool call) or `after_turn` (default). If the CLI cannot take it mid-turn, automation's message is queued (`queued`). |

### `get_conversation` — read-only

A task's conversation as its Conversation tab shows it for the latest run, oldest first: what the person and the
Orchestrator chat wrote to agents (recipient, delivered or not), what agents wrote, questions written in the
conversation, questions with suggested answers (options, state, answers, `answered_by`: person, automation or
orchestrator), plans (text, waiting or not, decision), `run_continued` entries (`returned`, `returned_with_note`,
`restarted`; step, note, attempt, author), `base_moved` notices and how each step ended. Each entry has author, step,
run, time, `cursor` and link. Texts are stored text with secrets redacted.

| Parameter | Type            | Req. | Description                                                      |
|-----------|-----------------|------|------------------------------------------------------------------|
| `project` | string          | yes  |                                                                  |
| `task`    | string          | yes  | Task key or id.                                                  |
| `limit`   | integer \| null | no   | Newest N entries; default 50, max 200.                           |
| `before`  | string \| null  | no   | An entry's `cursor`: read older entries (pagination). `more` tells if older remain. |

---

## 7. Pipelines (copy)

### `duplicate_pipeline` — changes data · gate AP unless identical

Copies a pipeline's latest published version into a new pipeline and publishes it as v1, optionally giving agent
steps other published profiles. A copy that is not identical to its source (other profiles included) is
`person_only` unless AP is on.

| Parameter       | Type                 | Req. | Description                                                                         |
|-----------------|----------------------|------|-------------------------------------------------------------------------------------|
| `project`       | string               | yes  |                                                                                     |
| `pipeline`      | string               | yes  | Source pipeline (id or name).                                                       |
| `name`          | string               | yes  | New pipeline's name.                                                                |
| `step_profiles` | object<string,string>| no   | Step (key, name, or 1-based number) → published profile name.                       |

---

## 8. Pull requests

### `list_pull_requests` — read-only · cross-project

Pull requests of the project's linked GitHub repository, as the *Pull requests* page lists them: number, title,
state (`open`, `draft`, `merged`, `closed`), review, GitHub checks on the head, branches, changes, merged/closed time,
fork or not, the task and run whose branch it is (`agent_working` while that run runs), app link and GitHub url.
They are what the last successful sync read (open ones plus the 50 most recently updated closed ones). Each
repository reports its sync: `sync` (`synced`, `rate_limited`, `github_unreachable`, `paused_other_account`,
`gh_unavailable`, …), `synced_at`, `attempted_at`, `paused_until`.

| Parameter | Type           | Req. | Description                                                                          |
|-----------|----------------|------|--------------------------------------------------------------------------------------|
| `project` | string \| null | no   | Left out = every project.                                                            |
| `refresh` | boolean        | no   | Sync through `gh` first (needs `project`); skipped if a sync ran in the last 30 s or a rate limit holds. |
| `tab`     | `open` \| `needs_review` \| `agent_working` \| `merged` \| `closed` \| null | no | As the page's tabs. |
| `search`  | string \| null | no   | A number (`42`, `#42`), title words, a branch, or a task key.                        |

### `get_pull_request` — read-only

One pull request, with the repository's sync state. One the last sync did not read is refused as
`pull_request_not_synced`; with `refresh` it is read from GitHub on its own (once per 30 s). Errors:
`github_rate_limited` (until `paused_until`), `github_unreachable`, `pull_request_not_read`,
`pull_request_not_found`.

Parameters: `project` (req.), `number` (integer, req.), `refresh` (boolean).

---

## 9. Usage, delivery and reports

### `get_usage` — read-only

Tokens (**spent = input − cached input + output**, as each CLI reported them) and active agent time over a range of
local days, as the *Usage* page reads them: totals and grouped rows. `unmatched` names filter values nothing in the
range matches; `link` opens the Usage page with the same query; each row has its own link. `unread_runs` counts runs
left out because their record is damaged. `estimates` (one project only) is the story-point estimator's own usage.

| Parameter  | Type           | Req. | Description                                                                                   |
|------------|----------------|------|-----------------------------------------------------------------------------------------------|
| `project`  | string \| null | no   | Left out = every project.                                                                     |
| `from`     | string \| null | no   | First local day `YYYY-MM-DD`; without it, the 7 days ending with `to`.                       |
| `to`       | string \| null | no   | Last local day (today included up to now); default today. Max range 366 days.                 |
| `group`    | string \| null | no   | `hour` (≤ 7 days), `day` (default), `task`, `pipeline`, `step`, `agent`, `model`, `account`. |
| `task`     | string \| null | no   | Filter: task key or id.                                                                       |
| `run`      | string \| null | no   | Filter: run id.                                                                               |
| `pipeline` | string \| null | no   | Filter: pipeline id or name.                                                                  |
| `step`     | string \| null | no   | Filter: step name in any pipeline.                                                            |
| `agent`    | string \| null | no   | Filter: agent profile id or name.                                                             |
| `model`    | string \| null | no   | Filter: model.                                                                                |
| `account`  | string \| null | no   | Filter: CLI account id, or `none` (native agents and reviewers, command tools, older runs).   |

### `get_delivery` — read-only

How delivery goes in a project over its last `days` local days (default 30, max 366), as the *Delivery* view beside
Usage reads it, in tasks (no money estimates):

- **Throughput** — tasks merged per day (per week past 31 days), in Orchester or found merged outside by git, with
  their story points and how many had none.
- **Cycle time** (first agent start → merge) and **lead time** (creation → merge) per merged task, with median,
  quartiles and 85th percentile in hours, and attempts.
- **Where agent time goes** — active hours per pipeline step, executions, returns by a failure route or a person,
  return percent, attempts per delivered task.
- **Cumulative flow** — tasks per workflow status at each day's end (Canceled left out).
- **Forecasts** — Monte Carlo over recently merged tasks for the queued tasks and each open module: 50% and 85%
  dates, remaining tasks, the history used; or `not_enough_history` / `nothing_left`.

Parameters: `project` (req.), `days` (integer | null), `module` (string | null — only that module's forecast beside
the queued work's).

### `get_daily_report` — read-only

A project's report of one local day (default today, up to now): the one the Orchestrator conversation got for that
day if it was delivered (`delivered: true`), else read now. Contents: tasks merged that day; the latest runs as they
stand (ready to merge, stopped — failed, at a limit, waiting for a decision, interrupted — with `end` and step, or at
work); what waits for the person (tool requests, questions, messages, plans, the Orchestrator's proposals); what the
Orchestrator did (`way`: direct, itself, applied by the person); the day's tokens and active time by task and by
account against the daily budget; the queued tasks in order; and `merge_queue` (runs in the queue now and those
merged, failed or removed since the day began). Every item has its app link.

Parameters: `project` (req.), `day` (`YYYY-MM-DD` | null).
