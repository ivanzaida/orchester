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
| Agent worktrees | inside the repository, `<repo>/.orchester/worktrees/` |
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

## Storage layout

```
<install dir>/
  orchester.exe
  orchester-runner.exe
  orchester.db                 application database

<repo>/.orchester/             excluded through .git/info/exclude (the project's .gitignore is not touched)
  orchester.db                 project database
  blobs/<aa>/<sha256>          zstd-compressed content
  worktrees/<run-id>/          agent git worktrees
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
- **Money:** integer micro-units plus an ISO currency code; never floating point.
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
| `runs` | task, number, group, version snapshot, status, branch, base revision, worktree, budget cap, spent |
| `attempts` | run, number, status, reason |
| `step_executions` | attempt, pipeline step, agent profile version, effective model, status |
| `plans` | run, version, body blob, decision (proposed, approved, changes requested) |
| `checkpoints` | run, step execution, revision, agent state blob |
| `run_events` | run, sequence, time, kind, payload — append-only |
| `evidence` | run, revision, kind (criteria, tests, review, browser), result, blob |
| `artifacts` | run, name, blob |
| `usage` | step execution, provider, model, tokens, cost |

### Conversations and pull requests

| Table | Purpose |
| --- | --- |
| `conversations` | scope (project, task, PR thread), task, title |
| `messages` | conversation, idempotency key, author, recipient, run / step, body, attachments, delivery state and times |
| `proposals` | conversation, version, proposed task set (not tasks until created) |
| `pull_requests` | number, title, state, branches, head revision, linked task, synced at |
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
  (session ended) → running on continue. Interrupted keeps its last checkpoint.
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
