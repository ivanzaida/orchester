# Orchester — shared server

Date: 2026-10-02. Status: **agreed direction**; S1a and S1b (server only) handed to Codex on 2026-10-03 (`docs/plan.md`, *S · Shared server*). The user's decisions, including the answers to the
first draft's open questions, are recorded in [Decisions (2026-10-02)](#decisions-2026-10-02). Rows marked *proposed*
are the architect's recommendations: the user has not objected to them, but has not confirmed them one by one either.
[Open questions](#open-questions) lists what still needs the user. Nothing here changes the local (single-user)
version in `storage.md` except where a section says so.

**Goal (user, 2026-10-02).** Several people work on one project in Orchester. They share the task list and can see
who took a task into work.

Today a project lives in `<repo>/.orchester/orchester.db` on one computer. This document adds a server a team runs
itself. It holds a shared project's task list, who has taken each task (the *claim*) and a short live status of the
run working on it. Runs themselves (worktrees, agents, transcripts) stay on the computer that runs them.

## Decisions (2026-10-02)

The user set rows 1–3 and answered the first draft's open questions on 2026-10-02 (rows 15–22); those answers also
settle rows 5, 6, 10 and 12. The other rows are the architect's recommendations, still *proposed*.

| # | Topic | Decision | State |
| --- | --- | --- | --- |
| 1 | Hosting | Self-hosted: one server binary a team runs itself, Docker is fine. No cloud service, no GitHub OAuth | user |
| 2 | Accounts | Accounts live on the server; an admin creates users. A user signs in from the desktop and receives a token, kept in the OS keychain like every secret. Passwords are hashed with argon2id; tokens can be revoked; the first admin is created on the server host ([Accounts](#accounts-and-tokens)) | user |
| 3 | Transport security | HTTPS, from a reverse proxy or the server's own TLS. The desktop accepts `http://` only for a loopback host, or for another host after an explicit opt-in for that server | user |
| 4 | v1 scope | Shared: tasks (title, brief, criteria, pipeline by name, limits, status), the claim, a short live run status per task. The server assigns task numbers. Local: runs, worktrees, transcripts, Activity, conversations, memory, secrets, agent profiles and pipelines. The domain has no priority or tags, so none are shared | proposed |
| 5 | Claim | Atomic "take if free" on the server. Starting work on a task takes the claim. Heartbeat every 60 s; no heartbeat for 15 min makes it stale; a stale claim can be taken over after confirmation, and the takeover is recorded. Manual Take and Release exist ([Claims](#claims)) | user |
| 6 | Concurrency | Every shared task has a revision. A change names the revision it was made on; a mismatch returns a conflict with the current task, and the desktop shows the other person's change instead of overwriting it | user |
| 7 | Live updates | Server-Sent Events with a resume cursor; the desktop reconnects with backoff ([Live updates](#live-updates)) | proposed |
| 8 | Offline | Without the server, a shared project's task list is read-only from the local cache, in a visible offline state. Changes wait for the connection; nothing is queued silently | proposed |
| 9 | Configuration | Application level: Settings › Servers (several servers, each signed in; the status bar shows the connection). Project level: Project settings › Sharing, *Local only* (default) or *Shared on <server>* | proposed |
| 10 | Project identity | A server project is identified by the normalized URL of the repository's `origin` remote, unique per server; the server enforces it | user |
| 11 | Discovery | An optional committed `orchester.toml` at the repository root (`server = "https://…"`, no secrets) offers *Sign in and join*. Adding a server by hand always works | proposed |
| 12 | Numbers on Share | The first person to share keeps their task numbers; the server continues after the highest | user |
| 13 | Framework, storage | `axum` on the workspace's tokio; SQLite through rusqlite for v1 | proposed |
| 14 | Trait boundary | `TaskStore`, with a revision on updates; no new core trait ([The desktop's trait boundary](#the-desktops-trait-boundary)) | proposed |
| 15 | Claim after a run ends | Kept when the run succeeds or fails (review, merge and recovery happen on the holder's computer); released on cancel; completed on merge | user |
| 16 | Who may share | Any user. The first to share a repository creates its server project and becomes its owner; everyone after joins it ([Two people share one repository](#two-people-share-one-repository)) | user |
| 17 | Visibility | Explicit membership per project. Owners and admins add members by username | user |
| 18 | Editing a claimed task | Any member may edit any task; revisions prevent overwriting | user |
| 19 | Removing a server project | Not in v1 | user |
| 20 | Joining with local tasks | A review list of the joiner's local tasks, all ticked; ticked ones are uploaded with new numbers and keep their id and former key; unticked ones are archived as *Not shared* ([Two people share one repository](#two-people-share-one-repository)) | user (flow); archive: architect's choice |
| 21 | Token | The server generates it at sign-in; nobody is shown it; the desktop keeps it in the keychain; when it expires, the person signs in again with their password. Expiry after 30 days without use | user; expiry *proposed* |
| 22 | Two computers, retention | A heartbeat from any of a person's computers keeps their claims fresh. Events are kept 30 days; an older cursor makes the desktop reload the full list | user |

## Processes and crates

```
orchester.exe (desktop) ── HTTPS ──> orchester-server (Linux / Docker)
 └─ orchester-runner.exe                └─ orchester-server.db (SQLite, WAL)
     └─ agent CLIs
```

- **Only the desktop talks to the server.** It owns user-authored data already (`storage.md`, "Write ownership"),
  holds the server token, and learns about runs from the runner's IPC events. The runner keeps reading tasks from the
  project database and never sees a server token. Closing the desktop closes the runner (`storage.md`, "Lifecycle"),
  so nothing works on a shared task without the desktop being there to report it.
- **The project database is the cache.** A shared project's tasks stay in the `tasks` table of
  `<repo>/.orchester/orchester.db`, written by the desktop from what the server confirmed. Runs keep their foreign key
  to the task, the runner and every local screen keep working unchanged, and the list can be read offline.

| Crate | Role | Depends on |
| --- | --- | --- |
| `orchester-core` | Gains the shared-task rules (`sharing`), `TaskRevision`, `RepositoryIdentity`; no HTTP or database | as today |
| `orchester-api` (new) | The JSON contract: request, response, error and event types. A `client` cargo feature adds the HTTP client the desktop uses | core, serde; `client`: reqwest |
| `orchester-server` (new) | Binary `orchester-server`: HTTP API, SQLite storage, accounts, event stream, admin command line | core, api, axum, rusqlite |
| `orchester-store` | Cache tables and the shared-task writes of the project database; the `servers` table of the application database | as today |
| `orchester-desktop` | `RemoteTaskStore`, sync, Settings › Servers, Project settings › Sharing, claim actions | api (`client`) |

**Why two crates.** The server is a real boundary (AGENTS.md: "for example the server"). The contract is shared by
two programs built from this workspace, as `orchester-ipc` is between desktop and runner, so a change to it breaks the
build on both sides at once. The `client` feature keeps reqwest out of the server build; the server never needs a
client.

**Core stays free of HTTP and databases.** The server reuses core's rules (task completeness, queueing, the edit
lock, `Name`, `TaskLimits`, claim transitions, staleness) by calling them; storage and HTTP live in
`orchester-server`. Core only gains types, pure functions and serde derives, which it already uses.

### Dependencies (AGENTS.md rule 17)

Each goes into `[workspace.dependencies]` with its reason.

| Crate | Where | Why it is needed |
| --- | --- | --- |
| `axum` | server | HTTP routing, JSON extraction and SSE responses on tokio; built on `hyper` 1, already in the lock file through lurq |
| `argon2` | server | Password hashing (argon2id); not something to write by hand |
| `tokio-rustls` | server | The server's own TLS; same `rustls` the client brings, no OpenSSL on any platform |
| `reqwest` (rustls, OS trust store) | api `client` | The HTTP client already chosen for `orchester-agent` (`native-agent.md` §3.0); the second user adds nothing new |

Tokens come from `getrandom` (already in the lock file through `uuid`) and are hashed with `sha2` (already a workspace
dependency). The configuration is TOML (`toml`, already there). The command line has three subcommands and is parsed by
hand. The SSE parser on the desktop is the ~100-line parser `native-agent.md` §3.0 plans. Whichever phase lands first
writes it where both can use it; nobody writes it twice.

### Framework and storage

- **axum**: tokio-native, on the hyper already built for lurq, with SSE and JSON built in and no macro DSL.
- **SQLite for v1, not Postgres.** One binary and one data file, nothing else to run or back up, and the team already
  knows rusqlite, WAL and the forward-only migrations of `orchester-store`. Writes are small and rare (a task edit, a
  claim, one heartbeat per person per minute). SQLite serializes them, which also makes claims and numbers atomic
  without extra locking. A team of tens of people is far inside what one SQLite writer handles. Postgres becomes worth
  it only for several server instances. The storage is then a module of the server to replace; there is no trait for
  it now (rule 8).
- One write connection behind a `parking_lot::Mutex`, used only inside `spawn_blocking`, and a few read connections.
  Time comes from one `Clock`: the system clock in the binary, a settable clock in the server's tests. Rule 8 names a
  clock as a real boundary; staleness is untestable without one.

## The desktop's trait boundary

**`TaskStore` is the boundary.** The desktop's task code is already generic over it (`tasks/data.rs`: `save_draft`,
`save_to`). A shared project gets `RemoteTaskStore` (desktop, `src/sharing/`). It implements `TaskStore` like
`ProjectDb` does:

| Method | `RemoteTaskStore` |
| --- | --- |
| `tasks`, `task` | Read from the cache in the project database; work offline |
| `create_tasks` | `POST …/tasks` with the batch key; the server assigns ids and numbers; the confirmed tasks are written to the cache |
| `update_task`, `save_task` | `PATCH …/tasks/{id}` with the revision read and only the fields that differ from the cached task; the confirmed task is written to the cache |

`TaskStore` is synchronous. The desktop already calls stores on blocking workers, never on the UI thread, and
`RemoteTaskStore` waits for its request there. Offline, the write methods fail with an *offline* error at once, and
the screens never offer them in that state.

**Change to `TaskStore` (both implementations).** `Task` gains `revision: TaskRevision`, which every stored change
raises. The update methods report a conflict as a result, not as an error, as `ResourceStore::save_draft` returns
`DraftSaved`:

```rust
pub enum TaskUpdate { Saved(Task), Conflict { current: Task } }
fn update_task(&mut self, id: TaskId, read: TaskRevision, details: TaskDetails) -> Result<TaskUpdate, Self::Error>;
fn save_task(&mut self, task: &Task) -> Result<TaskUpdate, Self::Error>; // task.revision() is the revision read
```

The local store uses the revision too: two window tabs can show one task.

**No new trait.** Claims, heartbeats and run status exist only for shared projects. On `TaskStore` they would need a
local implementation that does nothing, a silent no-op (rule 6). A trait of their own would have one implementation
(rule 8). The desktop calls the `orchester-api` client for them directly. Their rules (`Claim::take`, `release`,
`take_over`, `heartbeat`, `is_stale`, the run phases that end a claim) live in `orchester_core::sharing`, so the
server enforces and the desktop displays the same rules.

**Pipelines by name.** Pipelines are not shared, so a shared task names its pipeline by slug. The cache keeps the
slug, plus the local pipeline of that slug when the person has one (`TaskDetails.pipeline`). If they have none, the
task shows its pipeline as missing on their computer and cannot be queued or run there, which today's preflight
already reports. Because a change sends only the fields that differ, a person who lacks the pipeline does not clear
it by editing the title. The pinned version stays local: a published version number means nothing on a colleague's
computer. Whoever starts a run pins their own latest published version of the named pipeline, as Run again does
today.

## Claims

A claim says who has taken a task into work. A task has at most one; the server keeps it and its history.

| Event | Effect |
| --- | --- |
| *Take* (manual) | Takes the claim if the task has none: an atomic check-and-set on the server. Held by someone else: refused with the holder, since when and whether the claim is stale |
| Starting work on a shared task: Create & run, Start, Run again, continuing a paused or interrupted run, Send & continue, a follow-up message | The desktop takes the claim first, or confirms that this person holds it, then asks the runner. If the claim is held by someone else, nothing starts and the desktop names the holder. Pause and cancel never need the claim |
| Heartbeat | Every 60 s while the desktop runs and is signed in, one request per server names the claimed tasks in the projects this desktop has joined. The server refreshes those the caller still holds and returns the ones they lost |
| Stale | No heartbeat for 15 min. Shown as stale, not as free |
| *Take over* | Only a stale claim, after an explicit confirmation that names the holder and the last heartbeat. The request names the claim it saw; if that claim changed or was refreshed meanwhile, the request is refused. The takeover is recorded (from, to, when) and the old holder is told on their next connection |
| *Release* | By the holder, while none of their runs of the task has an agent at work. A paused or stopped run stays on their computer; its status is no longer shared |
| Run cancelled | The claim is released |
| Run merged | The claim is completed: the task shows *Merged* with who merged it. A merged task cannot be claimed again |
| Run succeeded or failed | The claim is **kept**: review and merge, Send & continue and Run again all happen on the holder's computer, where the run's worktree is. The holder releases it if they leave it |
| Member removed or user disabled | Their claims are released, and the release is recorded |

A succeeded run can only be merged from the computer that ran it, so releasing the claim there would invite a second
person to redo finished work (decision 15).

**Two computers.** A heartbeat from any computer where the person has joined the project keeps all their claims in
it fresh, even while the run is on another computer that is off (decision 22).

**Editing.** A claim does not lock the task: any member may edit it, and the revision keeps two edits from
overwriting each other (decision 18).

**Lost claim while working.** A person offline for more than 15 minutes can find their claim taken over. Their local
run goes on: Orchester never stops work on its own. Their desktop tells them who took it, and the server refuses their
run status reports for that task.

**Time.** Staleness is decided by the server's clock. Responses carry the server time, so the desktop computes
*stale* with the same core function and an offset, without polling.

## Live run status

The holder's desktop reports a short status for the task's latest run whenever the runner says that run changed
(`Event::RunChanged`, `WorkSummary`). Only the holder's reports are accepted.

| Field | Content |
| --- | --- |
| `number` | The run's number on the holder's computer (`#3`); shown with the holder's name, since run numbers are per computer |
| `phase` | `queued`, `running`, `waiting`, `paused`, `stopped_at_limit`, `interrupted`, `succeeded`, `merged`, `failed`, `cancelled`: `TaskPhase` as the holder's desktop derives it |
| `step` | The current step's name, or none |
| `started_at`, `phase_since`, `updated_at` | Milliseconds since the Unix epoch, UTC |

Other members see the task's phase from this status (*Running · Review · Alice · since 14:02*). For their own runs,
the desktop keeps deriving the phase from the local run, as today.

**What the server never stores about runs:** transcripts, prompts, messages, plans, diffs, file names, commits,
branch and worktree names, token usage, agent or model names, connection names, environment variables or secrets. The
status above is all it gets.

## Live updates

**Server-Sent Events**, one stream per shared project the desktop has open:
`GET /api/v1/projects/{id}/events`.

- **Why SSE.** The push only goes one way; changes are ordinary requests, which need their status codes and conflict
  bodies anyway. SSE is plain HTTP, so the bearer header, TLS and reverse proxies work unchanged. Resuming is part of
  the protocol (`id:` and `Last-Event-ID`). axum serves it without another crate. WebSocket would need a crate on both
  sides and its own resume and ping logic.
- **Events.** The server appends one row to `events` in the transaction of every change and wakes the streams.
  Each stream then reads the events after its cursor from the database, so a slow client misses nothing; the wake-up
  carries no data, as with the IPC notifications. Event `id` is the event's sequence number.
- **Kinds.** `task` (the full task snapshot, with claim and run status), `members` (the project's members changed),
  `project` (name or key changed), `reset` (the cursor is older than the retention, 30 days: the desktop reloads the
  list and continues from the cursor the list returns), and `revoked` (the session or membership ended; the stream
  closes).
- **Keep-alive.** A comment line every 20 s, so proxies do not close an idle stream; also `X-Accel-Buffering: no`
  for nginx.
- **Desktop.** On open, the desktop subscribes from the cache's cursor; the server replays what it missed. Reconnects
  back off from 1 s, doubling to 60 s, with jitter. `401` means *signed out*, and the desktop stops retrying until the
  person signs in. `403` or `revoked` means *no longer a member*. Each event is applied to the cache through the same
  store code a local edit uses, so the runs of an edited task keep the task content they started with
  (`run_task_snapshots`).
- **Connection state** per server: *connected*, *reconnecting*, *offline*, *signed out*. The status bar shows it for
  the servers of open shared projects. A shared project's task list says *Offline — read-only* with the time it was
  last synced.

## API

Base path `<server URL>/api/v1`. JSON bodies, UTF-8. Times are milliseconds since the Unix epoch, UTC. Ids are
UUIDv7 strings. Every call except `GET /server` and sign-in sends `Authorization: Bearer <token>`. The token never
appears in a URL, including the event stream. Requests are limited to 1 MiB. The server sends no CORS headers: its
only client is the desktop.

### Shapes

```text
User        { id, username, display_name, admin: bool, disabled: bool }
UserRef     { id, display_name }
Session     { id, device, created_at, last_used_at, current: bool }
Project     { id, key, name, remote, role: "owner"|"member", next_number }
Member      { user: UserRef, role, added_at }
TaskContent { title, brief, criteria: [string], pipeline: slug|null,
              limits: { budget_tokens, max_duration_ms }|null }
Task        { id, number, key: "ORC-42", former_key: "KON-12"|null, revision, ...TaskContent,
              status: "draft"|"queued", has_run: bool,
              created: { by: UserRef, at }, updated: { by: UserRef, at },
              claim: Claim|null, run: RunStatus|null }
Claim       { id, holder: UserRef, kind: "manual"|"run", since, heartbeat_at, stale: bool,
              taken_over_from: UserRef|null }
RunStatus   { holder: UserRef, number, phase, step|null, started_at, phase_since, updated_at }
Changes     { title?, brief?, criteria?, pipeline?, limits?, status?: "queued" }
Error       { error: code, message, ...details }   // message is English for logs; the desktop shows its own i18n text by code
```

Every response carries `Orchester-Server-Time` (the server's clock, for staleness).

### Endpoints

Auth column: **none**; **user** (any signed-in user); **member** (of the project); **owner** (owner of the project
or admin); **admin**; **holder** (member holding the task's claim).

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `GET /server` | none | | `{ name, api_version: 1, needs_bootstrap }` | |
| `POST /auth/sessions` | none | `{ username, password, device }` | `201 { token, session, user, password_change_required }` | `401 invalid_credentials`, `429 rate_limited` |
| `GET /auth/me` | user | | `{ user }` | `401` |
| `GET /auth/sessions` | user | | `{ sessions: [Session] }` | |
| `DELETE /auth/sessions/current` | user | | `204` | |
| `DELETE /auth/sessions/{id}` | user (own sessions) | | `204` | `404` |
| `POST /auth/password` | user | `{ current_password, new_password }` | `204`; the user's other sessions are revoked | `401 invalid_credentials`, `422 invalid` |
| `GET /projects?remote=<identity>` | user | | `{ projects: [Project] }` the user is a member of | `403 not_member { owners: [UserRef] }` when a project for that remote exists |
| `POST /projects` | user | `{ idempotency_key, name, key, remote, tasks: [UploadTask] }` | `201 { project, tasks: [Task], cursor }`; the caller is owner | `409 remote_taken { project: id, member: bool, owners: [UserRef] }` (the desktop goes on with Join), `422 invalid` |
| `GET /projects/{id}` | member | | `{ project }` | `404` |
| `PATCH /projects/{id}` | owner | `{ name?, key? }` | `{ project }` | `422 invalid` |
| `GET /projects/{id}/members` | member | | `{ members: [Member] }` | |
| `POST /projects/{id}/members` | owner | `{ username, role }` | `201 { member }` | `404 no_such_user`, `409 already_member` |
| `DELETE /projects/{id}/members/{user}` | owner, or the member themself | | `204`; their claims are released | `409 last_owner` |
| `GET /projects/{id}/tasks` | member | | `{ tasks: [Task], cursor }` | |
| `GET /projects/{id}/tasks/{task}` | member | | `{ task }` | `404` |
| `POST /projects/{id}/tasks` | member | `{ idempotency_key, tasks: [TaskContent] }` | `201 { tasks: [Task] }`, consecutive numbers; a repeated key returns the first result | `422 invalid` |
| `POST /projects/{id}/tasks/import` | member | `{ idempotency_key, tasks: [UploadTask] }` | `201 { tasks: [Task] }`, new numbers, ids and former keys kept | `409 id_taken`, `422 invalid` |
| `PATCH /projects/{id}/tasks/{task}` | member | `{ revision, changes: Changes }` | `{ task }` | `409 revision_conflict { current: Task }`, `422 not_editable { reason }`, `422 not_queueable { missing }`, `422 invalid` |
| `POST /projects/{id}/tasks/{task}/claim` | member | `{ kind }` | `{ task }`; also when the caller holds it already | `409 claim_held { claim }`, `409 task_merged` |
| `DELETE /projects/{id}/tasks/{task}/claim` | holder | | `{ task }` | `409 not_holder { claim }` |
| `POST /projects/{id}/tasks/{task}/claim/take-over` | member | `{ claim: id }` | `{ task }` | `409 claim_not_stale { claim }`, `409 claim_changed { claim }` |
| `PUT /projects/{id}/tasks/{task}/run` | holder | `RunStatus` without `holder`, `updated_at` | `{ task }`; `merged` completes the claim, `cancelled` releases it | `409 not_holder { claim }`, `422 invalid` |
| `POST /heartbeat` | user | `{ tasks: [id] }` | `{ lost: [{ task, claim: Claim\|null }] }` | |
| `GET /projects/{id}/events` | member | `Last-Event-ID` header | `text/event-stream` | `401`, `403` |
| `GET /admin/users` | admin | | `{ users: [User] }` | |
| `POST /admin/users` | admin | `{ username, display_name, admin }` | `201 { user, initial_password }` | `409 username_taken`, `422 invalid` |
| `PATCH /admin/users/{id}` | admin | `{ display_name?, admin?, disabled? }` | `{ user }`; disabling revokes sessions and releases claims | `409 last_admin` |
| `POST /admin/users/{id}/password` | admin | | `{ initial_password }`; revokes the user's sessions | |
| `DELETE /admin/users/{id}/sessions` | admin | | `204` | |

`UploadTask` is `{ id, number?, former_key?, ...TaskContent, status, created_at, claim: bool }`. `number` is only
used when a project is created: Share keeps numbers. Import ignores numbers and assigns new ones, and keeps
`former_key`, the key the task had on the uploader's computer (`KON-12`), for everyone to see. `claim: true` claims the
task for the uploader in the same transaction; the desktop sets it for tasks with a run that has not ended on its
computer.

`not_editable` reasons come from core's `EditError`. A queued task with no run is locked; a task with a run must stay
complete. "Has a run" is `has_run` on the server: someone took a `run` claim on the task, which starting work always
does. Claiming a task one holds already as `run` turns a manual claim into a run claim.

Common errors on every endpoint: `400 bad_request` (malformed body), `401 unauthorized` (no, unknown, revoked or
expired token), `403 password_change_required` (a session from an initial password, before the change; only
`POST /auth/password` and sign-out work), `403 forbidden`, `404 not_found`, which is also the answer for a project
the caller is not a member of.

## Accounts and tokens

**Where the token comes from.** The server generates it when the person signs in with username and password, and
returns it once, in the sign-in response, to the desktop. Nobody is shown it: not the person, not an admin, not a
log. The desktop puts it straight into the OS keychain and sends it with every request. When it expires or is
revoked, the server answers `401`, the desktop shows the server as *signed out*, and the person simply signs in again
with their password, which issues a new token. There is nothing to copy, paste or rotate by hand.

- **Passwords.** argon2id with the OWASP parameters (19 MiB, 2 iterations, 1 lane) and a random salt, in the PHC
  string format. 12 to 256 characters, no composition rules. Hashing runs in `spawn_blocking` behind a small semaphore,
  so a burst of sign-ins cannot exhaust the server. An unknown username is verified against a fixed dummy hash, so it
  answers like a wrong password, in about the same time.
- **Initial passwords.** An admin never chooses another person's password. Creating a user, or resetting a password,
  returns a random initial password once. A session signed in with it can only change the password
  (`password_change_required`).
- **Tokens.** 32 random bytes, sent as `orc_` + 64 hex characters (the prefix lets secret scanners find leaked
  tokens). The server stores only their SHA-256, which is enough for a random 256-bit value; argon2 is for passwords.
  A token expires after 30 days without use (*proposed*). It is revoked by signing out, by the session list, by a
  password change (all other sessions), by an admin, or by disabling the user. Open event streams check their session
  at every wake-up and keep-alive and close when it ends.
- **On the desktop.** The token is kept in the OS keychain under service `orchester-server:<server id>`, account
  `token` (Windows Credential Manager; the macOS login keychain), through `orchester-vault` like project secrets. It is
  a `SecretValue`: redacted in `Debug`, no `Display`, never logged or copied. The application database keeps only the
  server entry.
- **Bootstrap.** On an empty data directory the server starts and answers `GET /server` with `needs_bootstrap: true`;
  nobody can sign in. On the server host, `orchester-server user create --admin <username>` (in Docker:
  `docker compose exec server orchester-server user create --admin alice`) creates the first admin. It prints the
  initial password once to that command's own output, never to the log. `orchester-server user reset-password
  <username>` is the way back in when no admin can sign in.
- **Rate limiting of sign-in.** In memory, per username and per client address. 5 failures for a username in 15
  minutes block it for 15 minutes; 20 failures from one address in 15 minutes block that address. Both answer
  `429 rate_limited` with `Retry-After`. The client address is the connection's, or the `X-Forwarded-For` hop added
  by a proxy listed in `trusted_proxies`; otherwise the header is ignored. A restart clears the counters, which is
  acceptable for v1.
- **Audit.** The server records sign-ins and failures (username and address, never the password), session revocations,
  password changes, user and membership changes, and claim takeovers, in an append-only `audit_events` table.

## Security

| Concern | Rule |
| --- | --- |
| Tokens and passwords | Never in logs, errors, URLs or `Debug` output, on either side. The request log has method, route template, status, user id and duration, never headers or bodies. A server test signs in, works and asserts that neither the token nor the password appears in the captured log |
| TLS | *Recommended:* a reverse proxy with automatic certificates (Caddy in the shipped compose example), server listening on the proxy's network. *Or:* the server's own TLS from a PEM certificate and key in its configuration. The desktop trusts the OS trust store (a team's own CA is added there); there is no "ignore certificate errors" switch |
| Plain HTTP | The desktop accepts `http://` for `localhost`, `127.0.0.0/8` and `[::1]`. Any other `http://` server needs the person's explicit opt-in for that server, after a warning that the password and token can be read on the network. The opt-in is stored with the server entry. Redirects are never followed; a redirect is an error naming its target |
| Membership | A project is visible only to its members: everything else answers `404`. Members read and change tasks, claim, and report status for tasks they hold. Owners also manage members and the project's name and key. Admins manage users and the members of every project, but see a project's tasks only as members |
| Discovery file | `orchester.toml` comes from the repository, so it is untrusted. The desktop never contacts its URL until the person confirms adding that server, with its host shown. The plain-HTTP rule applies the same way |
| Runs | The server stores only the [live run status](#live-run-status): never transcripts or secrets |
| At rest | The server's SQLite file is not encrypted; securing the host and backing up its data volume belong to the team running it |

## Server data model

`orchester-server.db`, SQLite (WAL), with `meta.schema_version` and forward-only migrations as in `storage.md`.
UUIDv7 ids, UTC milliseconds.

| Table | Purpose |
| --- | --- |
| `meta` | schema version |
| `users` | id, username (unique, case-insensitive), display name, password hash, admin, disabled at, must change password, created / password changed |
| `sessions` | id, user, token hash (unique), device, created / last used / revoked, from an initial password |
| `projects` | id, key, name, remote identity (unique: one server project per repository), next task number, created by / at |
| `memberships` | project, user, role (owner, member), added by / at; unique (project, user) |
| `tasks` | id (the desktop's `TaskId`, kept on upload), project, number (unique per project), former key, title, brief, criteria (JSON array), pipeline slug, budget tokens, time limit, status, revision, has run, created by / at, updated by / at |
| `task_batches` | project, idempotency key, user, task ids; unique (project, key): a repeated create or upload returns the first result |
| `claims` | the current claim per task: id, task (unique), holder, kind, since, last heartbeat, taken over from |
| `claim_events` | append-only: task, claim, action (taken, released, completed, taken over), by, from (for a takeover), at |
| `run_status` | per task: holder, run number, phase, step, started at, phase since, updated at |
| `events` | append-only: sequence, project, kind, payload (JSON), at; older than 30 days removed |
| `audit_events` | append-only: actor, action, target, address, at |

**Task numbers** come from `projects.next_task_number`, read and raised in the transaction that inserts the tasks, so
concurrent creates never share or skip a number. As in `storage.md`, numbers are for people; every reference is an id.

**Project identity.** `RepositoryIdentity::from_remote_url` in core normalizes the `origin` fetch URL. Credentials
(already dropped by `strip_userinfo`), scheme and port are dropped. scp-like `git@host:owner/name` becomes
`host/owner/name`. The host is lowercased, a trailing `.git` and `/` are removed, and the path is compared without
regard to case. HTTPS and SSH clones of one repository therefore match. A repository whose `origin` is a local path,
or which has no `origin`, cannot be shared, and the desktop says why.

## Desktop data

**Application database.** A new table `servers`: id, base URL (origin plus an optional path prefix), the server's
name, plain HTTP allowed, the signed-in user (id, username, display name), signed in at. The token is in the keychain.

**Project database** (desktop-written, like `tasks`):

| Table | Purpose |
| --- | --- |
| `tasks` (changed) | adds `revision`, and for a shared project the pipeline slug, former key, created by and updated by (name and time), and `sharing` (`shared` or `not_shared`; numbers are unique among the shared tasks) |
| `sharing` | at most one row: server id, server project id, server key, joined at, event cursor, last synced at. Without it the project is local |
| `task_claims` | the cached current claim per task |
| `task_run_status` | the cached run status per task |

`storage.md` gets these rows when S2 lands.

**Where it is configured** (screens to be drawn after this document is approved; only what they must hold is listed):

- **Settings › Servers** (application level). The application has no settings screen yet, so this is a new page.
  It lists the servers with their connection state and signed-in account. *Add server* takes a URL and checks
  `GET /server`, then *Sign in* asks for username and password, then for a new password when required. Each server
  has *Sign out* and the person's sessions. An admin sees the server's users there: create (showing the initial
  password once), disable, reset password.
- **Project settings › Sharing.** *Local only* (default) or *Shared on <server>*. Actions: *Share on…* (choose a
  signed-in server), *Join* when the repository is shared already (with the review list), the not-a-member state with
  *Check again*, *Make local* (leave), and the former-key mapping. Owners and admins manage members by username.
- **Discovery.** A local project whose checkout has `orchester.toml` with a server URL offers *Sign in and join* in
  that project; it adds the server, signs in and joins, each step confirmed.
- **Status bar.** One item for the connection to the servers of open shared projects.
- **Tasks.** The *Not shared* filter and *Share task*; *was KON-12* on the task page; holder and claim state on rows
  and the task page; *Take*, *Release*, *Take over* (with its
  confirmation); the conflict state of the task editor; the offline notice; the notice that a claim was taken over.

## Migration

**Share (local → shared, the first person).** `POST /projects` creates the server project from the local one and
uploads every task in one transaction. Task ids and numbers are kept, and the server's counter continues after the
highest number, so branches (`orchester/orc-14-run-1`), conversations and memory that mention `ORC-14` stay right.
Tasks with a run that has not ended are claimed for the uploader. The desktop then writes the `sharing` row and offers
to write `orchester.toml` into the checkout. It never commits it; the person does.

**Join (a colleague).** See [Two people share one repository](#two-people-share-one-repository). The local project
id does not change, since the keychain names project secrets by it (`storage.md`, "Secrets").

**Make local (any member, on their own computer).** The person's claims are released first when the server can be
reached; otherwise they go stale on their own. The `sharing` row is removed. The cached tasks, everyone's, become the
project's local tasks with their ids and numbers; the local counter continues after the highest. Claims and other
people's run statuses are dropped from the cache. *Not shared* tasks return to the list with new numbers after the
highest, and the desktop shows the mapping. The server project is unchanged for everyone else. Bringing a local copy
back is a Join, with the same review of the tasks created locally meanwhile.

### Two people share one repository

Two users of one server each have the same repository checked out, each with different local tasks, and both want to
share it.

**Identity and creation.** A server project is identified by the normalized `origin` URL
([Project identity](#server-data-model)), and the server holds at most one project per identity (a unique column).
The first person to share creates it and becomes its owner, keeping their numbers. If two people share at the same
moment, the server's uniqueness decides: the second `POST /projects` gets `409 remote_taken`, changes nothing, and
the second desktop goes on with Join.

**Finding the project.** When a person chooses *Share on…*, and whenever a local project is opened, the desktop asks
the servers the person is signed in to, plus the one named in `orchester.toml` once added, whether a project exists
for the repository's identity (`GET /projects?remote=`). The identity goes only to servers the person added. If one
exists, Project settings › Sharing offers **Join** instead of *Share*, and the project shows that it is shared there.

**Membership.** Joining needs membership (decision 17). An owner of the project, or an admin, adds a member by exact
username in Project settings › Sharing › Members (`POST …/members`). A person who is not a member sees, in place of
*Join*, that the repository is shared on that server, the owners' names, the username to give them, and *Check again*.
Their project stays local and nothing is uploaded. v1 has no access requests or invitations, since there are no
notifications; the owner is told by other means. Once added, *Check again* or the next opening of the project offers
Join.

**Review before joining.** If the joiner's local project has tasks, Join opens a review list of them, all ticked.

- Each row shows the local key and title. Below it, as a hint only, the server's tasks with similar titles, from a
  pure function in core (`sharing::similar_titles`: case-folded words, a large share of them in common). Nothing is
  merged automatically.
- The person unticks tasks that duplicate tasks already on the server.
- A task with a run that has not ended cannot be unticked: work on a shared task needs a claim. Its row says why, and
  it is uploaded claimed for the joiner.

**Upload and numbers.** Confirming joins and uploads the ticked tasks in one `POST …/tasks/import`. Each gets a new
number from the server, keeps its local id, and records its former key (`former_key`, e.g. `KON-12`). The task page
shows it to every member as *was KON-12*. The local project takes the server project's key. After the join the desktop
shows the old-to-new mapping (`KON-12 → ORC-41`), which stays reachable from the project's sharing settings. What
carries the old key keeps working:

| Carries the old key | What happens |
| --- | --- |
| Local runs, their checkpoints, conversation and memory | Attached by task id, so unchanged |
| Run branches (`orchester/kon-12-run-1`) | Keep their names: a branch is named once, when its run starts, and the run stores it. The next run is `orchester/orc-41-run-2`, since runs of a task are numbered on (`BranchName::for_next_run`). A merge after the join is titled with the new key |
| Worktrees | Named by run id (`.orchester/worktrees/<run-id>/`), so unaffected |
| References in briefs, criteria and messages (`see KON-12`) | Orchester does not rewrite text. The task's *was KON-12* and the mapping explain them, and searching the task list for a former key finds the task |

**Unticked tasks: archived as *Not shared*.** They stay in the project database with their runs, conversation and
memory, read-only and out of the task list, behind a *Not shared* filter, with their old key. They cannot be edited,
run or claimed. *Share task* on one of them uploads it later through the same import, with a new number.

The other option was to keep them as live local-only tasks beside the shared list. That was rejected: a shared
project's list then has two sources, and the rules for them have no good answers. Could such a task run without a
claim? Which numbers may it use? Is it editable offline? What does Make local merge? The person unticked these tasks
as duplicates, so archiving loses nothing, keeps every run readable, and can be undone one task at a time.

**Not shared in v1:** daily token budgets and usage (per computer, as today), conversations and memory (tied to local
runs), pipelines, profiles, instructions, tools, connections, secrets, environment and limits settings.

## Platforms

- **Desktop, Windows and macOS.** The client is reqwest with rustls and the OS trust store on both. The token goes
  through `orchester-vault`, which already covers Windows Credential Manager and the macOS login keychain. Nothing
  platform-specific is new; the macOS side is checked with `cargo check`/`clippy --target aarch64-apple-darwin` as
  usual.
- **Server.** Supported hosting is Linux (x86_64 and aarch64) and Docker. The repository ships a Dockerfile and a
  compose example with Caddy, and the data directory is a volume. The binary is plain Rust plus bundled SQLite, so it
  also builds and runs on Windows and macOS. It must, because it is a workspace member (AGENTS.md rule 0), and the
  server's tests start it on loopback there. Windows and macOS are not supported as production hosts in v1: nothing
  stops it, but the documentation and the Docker image cover Linux only.
- **Server configuration** (`server.toml`, or the same keys from the environment): `listen` (address and port),
  `data_dir`, `name`, optional `tls_cert` and `tls_key` (PEM), `trusted_proxies`.

## Phasing

Each phase is one Orchester task. Every phase ends with `tools/check.ps1` passing and a clean
`cargo check --target aarch64-apple-darwin`. Phases with UI pass design verification (rule 15) once their screens are
drawn. Server tests live in `crates/server/tests/` and start a real server on loopback with a temporary data
directory.

**S1a · Server and accounts.**
- [ ] `orchester-api` (types only) and `orchester-server` build on Windows, macOS and Linux; the Docker image and the
  compose example build and start on Linux.
- [ ] On an empty data directory `GET /server` reports `needs_bootstrap`. `user create --admin` creates the first
  admin and prints the initial password only to its own output. `user reset-password` works.
- [ ] Sign-in, `password_change_required`, password change (other sessions revoked), session list and revoke,
  sign-out, and the 30-day idle expiry with the test clock.
- [ ] The admin user endpoints work; non-admins get `403`; the last admin cannot be removed or disabled.
- [ ] Rate limits answer `429` with `Retry-After`. An unknown username answers like a wrong password.
  `X-Forwarded-For` counts only from a trusted proxy.
- [ ] With a certificate configured the server speaks TLS, tested with a test certificate. A log test finds neither
  the token nor the password.

**S1b · Projects, tasks, claims and events on the server.**
- [ ] Core: `sharing` (claim rules and staleness with tests over explicit times), `RepositoryIdentity` (HTTPS, SSH
  and scp-like forms of one repository match; local paths refused).
- [ ] Projects: create with upload (numbers kept, counter after the highest), lookup by remote (`not_member` with
  owners), members added and removed by owners and admins only, `404` for non-members. Two simultaneous creates for
  one repository: exactly one succeeds; the other gets `409 remote_taken` and changes nothing.
- [ ] Tasks: 5 clients creating 50 tasks each get numbers 1–250 with no gap or duplicate. A repeated batch key
  returns the first tasks. A stale revision gets `409 revision_conflict` with the current task. The edit lock and
  queueing follow core's rules. Import gives new numbers and keeps ids and former keys.
- [ ] Claims: of two simultaneous claims exactly one wins. A heartbeat refreshes and reports lost claims. A claim is
  stale after 15 minutes of test clock. Take-over is refused while fresh or changed, and recorded when done.
  Removing a member or disabling a user releases their claims.
- [ ] Events: a stream resumed with `Last-Event-ID` receives exactly the missed events. A cursor beyond retention gets
  `reset`. A revoked session or membership closes the stream.

**S2a · Desktop: servers and sign-in.**
- [ ] `servers` table; Settings › Servers adds a server by URL, signs in (with the new password when required), signs
  out and lists sessions; several servers.
- [ ] The token is stored in and read from the keychain on Windows and macOS. A revoked token shows *signed out*.
- [ ] `http://` to a non-loopback host is refused without the opt-in; redirects are errors.
- [ ] The status bar shows each server's connection. Admins can create, disable and reset users.
- [ ] Client tests against a real `orchester-server` on loopback; the token never appears in `Debug` output or logs.

**S2b · Share and make local.**
- [ ] `TaskRevision` and `TaskUpdate` in core, in `ProjectDb` and in `RemoteTaskStore`; the cache tables.
- [ ] Share uploads with numbers kept; Make local keeps every cached task. A shared project's runner starts runs from
  cached tasks unchanged.
- [ ] Pipelines are sent and stored by slug; a missing local pipeline shows as missing; an edit does not clear it.
- [ ] Project settings › Sharing with members (owners and admins add by username); `orchester.toml` offers *Sign in
  and join* only after confirmation.
- [ ] Starting or continuing work on a shared task takes a run claim first and names the holder when it cannot;
  heartbeats run every minute.
- [ ] Offline: the list is read-only from the cache with the offline notice and the last sync time; no change is
  sent later without the person.

**S2c · Join a shared repository.**
- [ ] Opening a local project, or *Share on…*, finds an existing server project for its identity and offers Join; a
  `409 remote_taken` on Share leads to the same place.
- [ ] A non-member sees the owners, their own username and *Check again*; nothing is uploaded.
- [ ] The review list shows every local task ticked, with the server's similar titles as a hint (core
  `sharing::similar_titles`, tested in core); tasks with a run that has not ended cannot be unticked.
- [ ] Ticked tasks are imported with new numbers, ids and former keys; the mapping is shown after the join and stays
  in the sharing settings; the task page shows *was KON-12*; searching a former key finds the task. Local runs stay
  attached, and the next run's branch uses the new key.
- [ ] Unticked tasks are *Not shared*: read-only, out of the list, behind the filter; *Share task* uploads one later.
  Make local returns them with new numbers.

**S2d · Live task list and claims.**
- [ ] One SSE stream per open shared project, resumed from the cached cursor, with backoff and `reset` handled; two
  desktops on one project see each other's changes within a second on loopback.
- [ ] A conflicting edit shows the other person's change, and the person's own text is kept until they choose.
- [ ] Take, Release and Take over (with its confirmation) on the task row and page; a lost claim is announced.

**S3 · Live run status.**
- [ ] `PUT …/run` on the server, with merged completing and cancelled releasing the claim, and reports from a
  non-holder refused.
- [ ] The holder's desktop reports phase, step and times on every run change. Other members see them on the task row
  and page; the holder's own view still comes from the local run.
- [ ] A takeover while the old holder's run works leaves that run going, tells its owner, and their reports stop
  being accepted.
- [ ] A test asserts that the server's database holds no run data beyond the status fields.

**Later.** Sharing pipelines, profiles and instructions (by version, with publishing on the server); comments on
tasks; notifications; Postgres for several server instances; single sign-on, if a team needs it.

## Open questions

1. **Token expiry.** Proposed: a token expires after 30 days without use, and the person signs in again
   (decision 21). Confirm, or choose no expiry.
2. **Unticked tasks.** Archived as *Not shared* is the architect's choice
   ([Two people share one repository](#two-people-share-one-repository)); the user asked for one to be picked, not
   yet confirmed it.
3. **Rows marked *proposed*** in [Decisions](#decisions-2026-10-02) (scope, SSE, offline, configuration, discovery,
   framework and storage, trait boundary): the user has not objected; a confirmation settles them.
