# Orchester — processes and local storage

Date: 2026-09-23. Status: agreed direction for the local (single-user) version; details marked
*open* still need a decision. A future server replaces the local store behind the same
`orchester-core` traits; nothing here is a server contract.

## Decisions

| Topic | Decision |
| --- | --- |
| Project data | `<repo>/.orchester/`, local only, never committed |
| Application data | `orchester.db` next to the executable |
| Structured data | SQLite (WAL) |
| Large content | content-addressed blob files |
| Secrets | OS keychain only; databases store references |
| Execution | separate runner process; agents are runner child processes |
| Agent worktrees | inside the repository, `<repo>/.orchester/worktrees/`; a step running beside other steps of its run has its own, merged back when it passes |
| Closing with active runs | confirmation listing the runs: wait for checkpoints, or quit now |
| Quit now or crash | next start reports the abnormal end and offers to continue from the last checkpoint |
| Logs and transcripts | kept in full, no retention limit |
| Pull request diffs | cached per revision |

## Processes

```
orchester.exe (desktop, UI)
 └─ orchester-runner.exe  (one per open project)
     └─ agent CLIs (Codex, Claude, …), one per active step, each in its own worktree
```

- The desktop owns the window and all user-authored changes. It starts one runner
  per project that has work, and talks to it over a local IPC channel (named pipe on Windows):
  commands down (start, pause, resume, cancel, deliver message, approval decision), events up
  (state changes, progress, output). It reads state directly from the project database.
- The runner owns execution: scheduling, agent processes, checkpoints, tool calls, evidence. It
  resolves secrets from the keychain at tool-call time; neither the desktop nor agents see values.
- Runner and agents run inside a Windows Job Object created by the desktop with
  kill-on-close, so they can never outlive the desktop process, whether it quits or crashes.

### Lifecycle

Closing the window is closing the application; there is no background mode. (A tray icon that
keeps work running is a possible later improvement, not part of this version.)

- **Close with no active runs** → the application exits cleanly.
- **Close with active runs** → the close request is vetoed and a modal lists the tasks in
  progress (task, current step, agent). It offers:
  - **Wait for agents to save** — the runner stops scheduling, lets every active step reach its
    next checkpoint, marks the runs paused and exits; then the application closes. The modal
    shows per-task progress while waiting and still offers Quit now.
  - **Quit now** — runner and agents are terminated immediately, without a checkpoint.
  - **Cancel** — keep the application open; nothing changes.
- **Start** → the desktop reads the previous session:
  - after *Wait for agents to save*, the runs are simply paused and can be resumed as usual;
  - after *Quit now* or a crash (power loss, process killed), it reports that the session ended
    abnormally, lists the interrupted runs with the reason, the last persisted checkpoint of
    each and whether its worktree has uncommitted changes, and offers to continue each run, or
    all of them, from its last checkpoint. Nothing restarts automatically: the modal offers
    only continuing all of them or none. Runs not continued there stay interrupted and each can
    be continued on its own later from its task conversation (user decision, 2026-09-24). Work after the
    checkpoint is not claimed as done; uncommitted worktree changes are kept and shown, never
    discarded silently.
- **Runner drops while the application is open** → the desktop reconnects automatically with a
  bounded backoff and tells the user (reconnecting, reconnected, or could not restart). Runs
  interrupted by the drop continue automatically from their last checkpoint as soon as the runner
  is back; nothing after the checkpoint is claimed as done. If the runner cannot be restarted, the
  runs stay interrupted until it is.

lurq already supports vetoable close requests whose decision can be held for an asynchronous
dialog (`on_close_requested` / `CloseRequest`), which is what the wait path needs.

### Step graphs

The runner starts every step execution whose dependencies have passed, as far as the agent slots
allow; the slots (an application setting, 4 by default) are shared by all runs of the runner, and each agent
at work takes one.

- **Where a step works.** A step that starts alone — no other step of the run is running or ready —
  works in the run's worktree on the run's branch, as a sequential pipeline always has; nothing else
  can start while it runs, since only a passing step makes another one ready. A step that starts
  beside others, read-only or writing, gets a worktree of its own on `<run-branch>--<step>-<attempt>`,
  created from the run's branch as it is then (it holds every step it depends on). Parallel writes
  in one worktree would overwrite each other, and a merge would change files under an agent at work.
- **Joining.** When such a step passes, its work is committed on its branch and merged into the
  run's branch in the run's worktree, one merge at a time, with the repository's git identity (fast
  forward when possible, else a merge commit); its worktree and branch are then removed. A conflict
  is undone, the step fails with *merge conflict* (the files are named in the transcript) and its
  failure route decides: retry from the route's target, or fail the run. Its branch and worktree stay.
- **Failures.** A step fails when its agent fails, when its structured result says `failed`, when it
  cannot start or when its work cannot be committed or merged. A failure ends its attempt: agents of
  the attempt still at work are ended and their executions skipped. What a failed step changed is
  committed as a checkpoint of its execution, never discarded. Failures for a used step budget,
  recorded by earlier versions, are still read.
- **Step budgets.** A step whose execution uses its budget does not fail: its agent is ended after
  its turn, its work is committed as a checkpoint in its worktree, and the run stops at its limit
  (*Stopped at limit*, naming the execution). Agents of other steps finish their invocations, as at a
  pause, and nothing new starts; no retry is taken. Raising that step's budget for the run (above
  what the execution used, at most the project's maximum run budget) continues the stopped agent's
  CLI session. The raised budget applies to this run only; the published pipeline keeps its own.
- **Outputs.** A new agent session ends with a structured result (Codex `--output-schema`,
  `{outcome: done|failed, summary}`); its summary — or, without one, the final message — is the
  step's output, and the prompts of the steps that depend on it include it with its source (step,
  attempt). A continued session is started without the schema (the probe verified it for new
  sessions only), so its CLI exit and final message decide.

## Storage layout

```
<install dir>/
  orchester.exe
  orchester-runner.exe
  orchester.db                 application database

<repo>/.orchester/             excluded through .git/info/exclude (the project's .gitignore is not touched)
  orchester.db                 project database
  blobs/<aa>/<sha256>          zstd-compressed content
  worktrees/<run-id>/          agent git worktrees: the run's, on its branch
  worktrees/<execution-id>/    a step running beside others, on `<run-branch>--<step>-<attempt>`
  step-result.schema.json      the JSON schema new agent sessions end with (Codex `--output-schema`)
  logs/runner-<YYYY-MM-DD>.log runner process log (UTC date), kept in full
  runner.lock                  one runner per project
```

*Open:* a directory next to the executable is not writable under `Program Files` and may be
replaced by an installer. The path is resolved in one place so a per-user fallback can be added
when an installer exists.

## Conventions

- **Identity:** UUIDv7 for every row id (time-ordered, merge-safe for a future server). Display
  keys (`ORC-129`, run `#128`) come from per-project counters and are never used as references.
- **Time:** UTC, milliseconds since the Unix epoch.
- **Budgets:** whole numbers of tokens (input + output as the CLIs report it; cached input is not
  counted), step budgets included. Published pipeline versions from before step budgets were tokens
  keep their money amounts in their bodies (they are immutable); such a step reads as having no step
  budget, and the amount is kept in `money_step_budgets_before_tokens`.
- **Time limits:** milliseconds of active agent time (the time agent CLIs ran); pauses, stops at a
  limit and waits for a person do not count.
- **Revisions:** full 40-character commit SHA.
- **Structured bodies:** JSON columns with a `schema` field for resource bodies, so a body can
  evolve without a table migration.
- **Idempotency:** every user action that creates something (task batch, queueing, message,
  feedback, run start) carries a client-generated key with a unique constraint; a retry returns
  the existing result.
- **Append-only:** run events, messages, audit events and published versions are never updated
  or deleted. Current state is stored alongside for fast reads and must be derivable from them.
- **Migrations:** each database has a `meta.schema_version`; migrations are forward-only and run
  at open. A newer schema than the binary understands refuses to open instead of guessing.

## Application database

| Table | Purpose |
| --- | --- |
| `meta` | schema version |
| `projects` | id, name, repository path, added / last opened |
| `settings` | application settings (locale, runner limits, agent slots) as key → JSON |
| `sessions` | id, pid, started / ended, `end_kind`: clean, runs_paused, quit_now; still open means crash |
| `ui_state` | last project, window placement, drafts not tied to a project |

A session row is written at start with `ended_at = NULL`. A clean exit fills it; a row still
open at the next start means the previous session crashed.

## Project database

### Project

| Table | Purpose |
| --- | --- |
| `meta` | schema version, project id |
| `project` | key (`ORC`), name, repository, default branch |
| `counters` | next task number, next run number |
| `project_limits` | at most one row: maximum run budget, default run budget, daily project budget, default time limit; without it the defaults apply (10M, 1.4M, 20M tokens, 30 min) |
| `money_budgets_before_tokens` | the money budgets tasks and runs had before budgets became tokens, kept by the migration that replaced them with the default run budget |
| `money_step_budgets_before_tokens` | the money step budgets of published pipeline versions and drafts from before step budgets became tokens, per version or draft and step |
| `repository_link` | at most one row: GitHub connection, git remote, host / owner / name, default branch, permission, expected login, checked at |

### Versioned configuration

Agent profiles, instructions, pipelines and tools share one lifecycle: one editable draft,
immutable published versions `v1..vN`. Runs pin versions; publishing affects new runs only.

| Table | Purpose |
| --- | --- |
| `resources` | id, kind (`agent_profile`, `instruction`, `pipeline`, `tool`), slug, name, archived, import provenance |
| `resource_drafts` | resource id, body, based-on version, updated at |
| `resource_versions` | id, resource id, number, body, content hash, published at; unique (resource, number) |
| `connections` | kind (GitHub, agent provider, MCP), name, settings, keychain reference, status, last check |
| `secrets` | name, environment, type, keychain reference, created / rotated / archived; unique (name, environment) |

Tool access policy (allowed pipelines and steps, environment, approval mode, access window,
denied behavior) is part of the tool version body. "Used by" is derived by query, never stored.

Agent permissions (repository access, commands, network) are part of the agent profile version body
since body schema 2. Bodies of schema 1 are upgraded when read, never rewritten: they get read-only
access, commands confined to the worktree and network off (`AgentPermissions::before_permissions`).
A pipeline step body may carry a `permissions` narrowing (commands, network); steps without one run
with their profile's permissions.

### Tasks and scheduling

| Table | Purpose |
| --- | --- |
| `tasks` | key, title, brief, acceptance criteria, pipeline resource, limits, status, idempotency key |
| `task_dependencies` | task, prerequisite, condition (`ready_for_review`, `merged`) |
| `run_groups` | mode (parallel with limit / sequential with gate), budget cap, failure policy, scheduling paused, pinned versions |
| `run_group_items` | group, task, position |

### Execution (written by the runner)

| Table | Purpose |
| --- | --- |
| `runs` | task, number, group, version snapshot, status and reason (a pause is requested or at a limit; at a step's budget with the execution that used it), branch, base revision, worktree, token budget and time limit at creation and in force, tokens used, pause requested |
| `run_step_budgets` | step budgets raised for one run after it stopped at them, per run and step |
| `attempts` | run, number, status, reason |
| `step_executions` | attempt, pipeline step, agent profile version, effective model, status, failure reason (agent failed, reported failed, not started, merge conflict, checkpoint failed; step budget only in records of earlier versions) |
| `step_outputs` | step execution, run, step, outcome (done, failed, or none when the final message was not a structured result), text — what the step left for the steps after it, read into their prompts with its source; append-only |
| `plans` | run, version, body blob, decision (proposed, approved, changes requested) |
| `checkpoints` | run, step execution, revision, agent state blob |
| `run_events` | run, sequence, time, kind, payload — append-only |
| `evidence` | run, revision, kind (criteria, tests, review, browser), result, blob |
| `artifacts` | run, name, blob |
| `invocations` | run, step execution, number, model, CLI session id, started / ended, input / cached input / output tokens, outcome — one row per agent CLI launch |
| `agent_events` | run, sequence, step execution, invocation, time, kind, payload — the agent transcript, append-only |

### Conversations and pull requests

| Table | Purpose |
| --- | --- |
| `conversations` | scope (project, task, PR thread), task, title |
| `messages` | conversation, idempotency key, author, recipient, run / step, body, attachments, delivery state and times |
| `proposals` | conversation, version, proposed task set (not tasks until created) |
| `pull_requests` | number, title, state, branches, head revision, linked task, synced at |
| `pull_request_syncs` | repository (host / owner / name) and when its open pull requests were last read; `pull_requests` rows of a repository are replaced with every sync (a cache of GitHub, desktop-written) |
| `pr_revisions` | pull request, revision, diff blob, synced at |
| `feedback_threads` | pull request, conversation, mode (question, fix), base / head revision, file, side, line range, excerpt, target profile, state |

### Access

| Table | Purpose |
| --- | --- |
| `approvals` | tool version, environment, run, step, action, revision, decision, decided by, expires at |
| `audit_events` | actor, capability, scope, run / step, decision, outcome — never secret values |

### Blobs

| Table | Purpose |
| --- | --- |
| `blobs` | sha256, size, stored size, media type, created at |

Files are written to a temporary name and renamed after the hash is verified; the row is
inserted after the file exists. Blobs are never deleted (full retention).

## Write ownership

SQLite serializes writes between the two processes; ownership keeps meaning unambiguous.

- **Desktop** writes user-authored data: configuration, connections, secret metadata, tasks,
  dependencies, run groups, user messages, feedback threads, approval decisions, settings.
- **Runner** writes execution data: runs, attempts, steps, plans, checkpoints, events,
  evidence, artifacts, usage, agent messages, message delivery state, audit events.
- Neither process edits the other's rows. A user request that changes execution (pause, cancel,
  continue) is an IPC command; the runner applies it and records the event.

## State machines

- **Task:** draft → queued → running → awaiting plan approval ⇄ running → ready for review →
  merged / done; any active state → paused, cancelled, failed. Blocked is derived from
  dependencies, not stored.
- **Run:** pending → running ⇄ paused → succeeded / failed / cancelled; running → interrupted
  (session ended) → running on continue. Interrupted keeps its last checkpoint. A pause is
  requested first and recorded as paused once the active agent invocation ended and its work was
  committed. A run that reaches its token budget or time limit is paused as stopped at limit
  after its agent was ended and its work committed; raising the limit continues it.
- **Step execution:** pending → running → waiting (approval, plan, message) → passed / failed /
  skipped.
- **Message:** draft (desktop only) → queued → delivered → acknowledged; failed keeps the draft.
- **Feedback thread:** draft → queued → working → answered / ready to verify → resolved;
  resolved can be reopened.
- **Approval:** requested → allowed / denied → expired.

Transitions live in `orchester-core` as typed functions; stores persist the result and the event.

## Open questions

1. IPC transport details, framing and versioning between desktop and runner.
2. Agent state inside checkpoints: what each provider can actually resume from.
3. Keychain naming scheme and behavior when a project folder is moved or cloned.
