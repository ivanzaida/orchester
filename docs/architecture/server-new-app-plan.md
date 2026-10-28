# Shared server in the new app — plan

> Since 2026-10-27 the team server, its web app and the tests and QA examples that start it live in the private
> platform repository `ivanzaida/orchester-platform` ([server.md](server.md), "Where the code is"); the paths below
> under `crates/server`, `web/` and those tests mean that repository.

Date: 2026-10-15. Status: **proposed**, for the person's review. The design is
[server.md](server.md) (decisions 1–65); this document only orders the work and names its files, frames and tests.
When a phase starts, its entry goes into `docs/plan.md` (*S · Shared server*) as usual.

## Starting point

- **Team server.** S1a and S1b are merged. Not built: `PUT …/run`, the staleness events, idempotency keys, every
  configuration endpoint, and everything of rows 39–47 (its own certificate, codes, invites, device joins, the cloud
  link, the removal of passwords).
- **Orchester Cloud.** Does not exist (rows 41–48): run by the person for everyone, email and password sign-up,
  identity and invites only.
- **Addresses.** Team servers run at their owner's address and hold several projects, each at its own address
  (`orchester.userdomain.com/project-a`), created by the owner by name and slug; the project address is the identity,
  the repository remote a hint (row 49). One connection per server, a link per local project, access per project
  (row 50).
- **Service.** S2a–c's service is in `crates/workspace/src/sharing/` (servers, password sign-in, connection monitor,
  shared projects, `RemoteTaskStore`, Share, Join, Make local, claims, heartbeats, the minute sync). Its tests are in
  `crates/workspace/tests` (`sharing`, `shared_projects`, `joined_projects`, `shared_statuses`) with a real server on
  loopback (`server_support`) and a forwarding port that can count or drop traffic (`sharing_support/forwarder.rs`).
- **New app.** Saves tasks through `TaskWriter` and takes run claims before work; starts the monitor and the sync
  (`services/mod.rs`). It has no sharing screen, and gets only member actions (row 63, below).
- **Server admin.** Every admin action is in the team server's own web app (row 62, phase SA), which shares
  `web/ui` with the cloud's web front (row 64).
- **Old app.** Frozen as of 2026-10-15 and never updated again (row 31). No phase touches `crates/desktop`. Its S2a–c
  sharing screens stay exactly as they are; after N6b they show read-only offline states that no longer apply, and
  after S1c its password sign-in stops working, both by design. Every phase touches only the new app,
  `orchester-workspace`, core, api, the team server and the cloud.
- **Skills.** Already versioned in the project database, with reference files in blob files; the runner places them
  in `<repo>/.orchester/skills/<execution-id>/` and keeps them; repository skill folders are a live source while
  *Repository agent files* is on. Row 37 changes all three (phase SK).

## What the desktop does, and what it does not

Row 63, in the person's words: "the desktop app stays the same, all admin actions are done in selfhosted-web". The
desktop does only what a member needs:

- sign in to the cloud;
- connect to a server;
- accept an invite and join, linking a checkout;
- work with shared tasks and configuration, and sync;
- *Make local*;
- show the connection state.

It has **no admin action at all**: no project creation on a server, no member or role management, no invites, no
revoking other people's devices, no server settings, no owner setup. Where one would be, it shows at most a link that
opens the server admin in the browser (*Manage members and invites*, *Open the server admin*). Every admin action is in
the server admin (phase SA). No phase or frame below puts an admin action in the app.

## Design frames

**`data/orchester.pen`, section `yYqBE` (old app, 10.01–10.44)** is the approved reference for behavior and copy of
S2a–S3, except where row 63 moves an action to the server admin. **`data/orchester2.pen` (new app)** already has,
read-only check of 2026-10-15:

| Area | Frames in `orchester2.pen` | Components |
| --- | --- | --- |
| Settings › Servers | `wratq`, `W4DV7T`, `uLJmj`, `Vc8Rw`, `di7Ul`, `S6uDhx`, `uEqJy`, `k3Hf9`, `Xn3yt`. **Obsolete** by rows 40, 46 and 63 (no app-wide server list, no passwords, no admin in the app); the address check `di7Ul`/`S6uDhx` may be reused for connecting, the users and sessions pieces for the server admin | `S5CZ57` Server list row, `euFSs`, `MgIb4` sessions, `MWQRb`, `wJ8Bn` users |
| Project settings › Sharing | `CHSKS` local only, `KSnOW` Share on Kontur team · page, `zRz47` shared · owner, `woioY` members, Make local and refusals (in place), `oOGMT` repository states and joining, `AiCwO` Join KTR · review page. The member and role controls of `zRz47` and `woioY` move to the server admin; in the app, members are read-only | `GFbX0`, `f2kjDI` members, `izogn`, `cA02b` join |
| Edit task | `PenUu` conflict · newer version | |

**Not drawn yet:** the cloud's web pages (C1b) and the server admin's pages (SA), in a *Cloud web* and a *Server
admin* layer of `data/orchester2.pen`; Settings › Orchester (cloud sign-in, invites, connected servers, connecting with
the check code); linking a local project to a project address with the remote mismatch warning; accepting an invite;
the status bar's server item; the shared task list and task page (badge, Holder, Not shared, claims, lost claim,
another member's run); everything of rows 27–30 and 35–37. Each phase lists what must be drawn before its UI is
built. New desktop frames go into the domain layers (*Settings*, *Project settings*, *Tasks*, *Task page*, *Status
bar*, and the library layers), confirmations in place, no modals. A designer draws them; a separate agent verifies
the built screens (rule 15). No read-only offline frame is drawn (row 38).

## Phases

Every phase is mergeable and testable on its own, ends with `tools/check.ps1` and the macOS check, and passes an
independent review; phases with UI pass design verification. Tests carry name filters. "Loopback" means a real
`orchester-server` (and, from C1a, `orchester-cloud`) started by the test with a temporary data directory and a
settable clock: `crates/workspace/tests/server_support`, extended with a cloud, and made reachable from
`crates/desktop-app/tests` (the app gains `orchester-server` and `orchester-cloud` as dev-dependencies, `workspace =
true`).

```
C1a ─ S1c ─ N1 ─ N2 ─ N3        (C2: magic link, Google, GitHub, later)
 │     └─ SA (web/server, after C1b's web/ui and the admin's design approval)
 └─ C1b (web/ui + web/cloud, after the person approves its Pencil screens)
               └── N4 ─ N5
                    └── N6a ─ N6b
SK ─────────────────────────────── N7a ─ N7b ─ N7c
N8: one slice per phase from N4 on
```

### C1a · Orchester Cloud backend (non-UI, new crate)

- **Scope.** `crates/cloud` (`orchester-cloud`) on `actix-web` (row 60; team servers stay on axum), the JSON API the
  desktop and the web front use: sign-up with email and password, email confirmation, password
  reset by email, sign-in (argon2id and S1a's limiters, moved from the team server), accounts, devices, account
  deletion; the root key host command, signing keys, signed key sets, identity
  tokens; the team server registry from a server-signed link statement; invites in flight with routing and email;
  rate limits; Docker image. `orchester-api`: the `cloud` module (types, client) and the `identity` feature (tokens,
  key sets, Ed25519 through `ring`). Storage on PostgreSQL (row 54): `tokio-postgres` with a `deadpool-postgres`
  pool and `tokio-postgres-rustls`, forward-only SQL migrations under an advisory lock. New dependencies `lettre`
  (SMTP), `actix-web` and those three, each stated in the commit. Every address and credential comes from the
  environment (row 59: `ORCHESTER_CLOUD_PUBLIC_URL`, `https://orchester.cloud` since the move of 2026-10-25, the database, the
  SMTP relay, the key-encryption key); nothing is hard-coded. `deploy/cloud/compose.yaml` with Caddy, the backend and
  an optional PostgreSQL. Magic link, Google and GitHub are a later phase (C2), not v1.
- **Files.** `crates/cloud/src/{main,config,http,accounts,devices,keys,servers,invites,email}.rs`, `db/` (pool,
  migration runner, queries) and `crates/cloud/migrations/*.sql`;
  `crates/api/src/cloud/`, `crates/api/src/identity/`.
- **Frames.** None.
- **Tests.** `crates/cloud/tests`, against a PostgreSQL named by `ORCHESTER_CLOUD_TEST_DATABASE_URL` (a local Docker
  `postgres`; each test makes and drops its own database; without the variable the database tests fail with that
  message): migrations applied once by two instances, a newer database refused, sign-up, confirmation (with the
  sign-up password, row 65: a wrong password or unknown link gives one answer at equal cost, a repeated sign-up keeps
  earlier links valid, confirming one ends the rest) and reset links
  through a test SMTP relay, a link or identity token used once under concurrent requests, sign-in limits,
  an unknown email answering like a wrong password, key rotation and key set expiry with the settable clock, identity token audience, replay and expiry, invite routing
  to a registered and an unregistered email, deletion after an invite ends, each rate limit, and a log test that finds
  no email, link, code or token; no configuration value compiled in. `crates/api/tests`: identity tokens and key
  sets over explicit keys and times.
- **Depends on.** Nothing.

### C1b · Orchester Cloud web front (UI, Next.js)

- **Scope.** The `web/` npm workspace (row 64): `web/package.json` with workspaces `ui`, `cloud` and later `server`;
  `web/ui` (`@orchester/ui`: the components, the design tokens generated from the Pencil variables into one CSS file,
  the i18n helpers; no words of its own: shared components take every string from the app); `web/cloud/`
  (server-rendered Next.js): sign-up, sign-in, the email confirmation landing, forgot password
  and reset password, against C1a's API called from the server side only; the `__Host-orchester_session` cookie
  (`HttpOnly`, `Secure`, `SameSite=Lax`), `Origin` and per-session CSRF token checks, `Referrer-Policy: no-referrer`
  and a strict CSP; strings in `web/cloud/locales/en.json`, styles from the design variables; the `web` service and
  Caddy's routes (`/api/*` to the backend, the rest to the web app). Later: account and devices. The desktop's
  *Create an account* and *Forgot password* open these pages (N1).
- **Files.** `web/package.json`, `web/package-lock.json`; `web/ui/` (`src/components/`, `src/tokens/` with its
  generator script and output, `src/i18n.ts`, tests); `web/cloud/` (`app/`, `lib/api.ts`, `lib/session.ts`,
  `locales/en.json`, tests); `deploy/cloud/` (compose and Caddyfile); `tools/check.ps1` runs `npm --prefix web run
  check` (lint, type check, tests of every package) when Node is present and says so when it is not. AGENTS.md's
  code-quality, i18n and design-token rules apply to all of `web/`. The cloud web implementer restructures the work in
  progress into `web/ui` and `web/cloud`.
- **Frames.** **To draw first, for the person's approval** (`data/orchester2.pen`, a *Cloud web* layer using the
  file's variables): each page with its states (empty, errors, rate limited, link expired or used, check your email,
  done), at desktop and narrow widths. The web app is built only after approval; C1a does not wait for it.
- **Tests.** `web/ui`: components render with the app's words only, token file in step with the Pencil variables,
  locale parity helpers; `web/cloud`: locale parity, unit tests of the session and CSRF checks; end-to-end against a local C1a and a test SMTP relay
  (sign up, confirm from the email's link with the sign-up password, sign in, forgot and reset); no token in logs or referrers; design
  verification of each page against its frame at the same size, in a browser.
- **Depends on.** C1a, and the person's approval of the frames.

### S1c · Team server: certificate, devices, codes, invites, cloud link, project addresses (non-UI)

- **Scope.** The generated certificate (`rcgen`, new dependency), its check code, `tls` modes and host commands;
  setup, invite, account and pairing codes; `POST /auth/cloud`, `POST /auth/code`, `POST /auth/pairings`, invites,
  `…/server/cloud`; the cloud link and key set caching; projects by slug and address (`POST /projects` for admins,
  `GET /projects?slug=`, `POST …/link`, `remote_hint`); the migration of S1a accounts (passwords and `argon2` removed
  from the server) and of projects made under row 10 (slugs from their repository names). API 3's sign-in shapes. Client: the pinned-certificate verifier. Workspace: `sharing::cloud` (the cloud
  session, invites, the device's servers), joining a team server by identity token or code, one device token per
  server reused by every linked project (row 50), project addresses parsed into server and slug, the `cloud_account` table and the `servers` table's trust columns; the
  password sign-in service is removed.
- **Files.** `crates/server/src/{tls,codes,devices,invites,cloud_link}.rs`, `accounts/` (password parts removed),
  `http/{auth,invites,cloud}.rs`, `main.rs` (host commands), `db/` migration; `crates/api/src/{accounts,invites}.rs`,
  `client/pinned.rs`; `crates/store/src/` (application tables); `crates/workspace/src/sharing/{cloud,account,
  connection}.rs`.
- **Frames.** None.
- **Tests.** Server: the certificate and check code on first start; the pinned client refuses another certificate;
  every code kind with lifetimes, one-time use and rate limits; cloud joins against tokens from a loopback cloud
  (audience, replay, expiry, key set replaced by one an app carries, expired key set, *off*, re-pin); invites by email
  and code; projects created by slug, found by `?slug=`, linked with numbers kept into an empty project; the S1a
  migration of a populated database keeps users, memberships and device tokens and gives existing projects slugs.
  Workspace: a device connects once and two local projects linked to two projects on that server share its token; an
  invite to project-a gives nothing in project-b; a log test finds no token or code.
- **Depends on.** C1a (a test cloud to verify against; the code path does not need it).

### SA · Server admin (rows 62–64) · UI, Next.js

- **Scope.** `web/server/`, the team server's admin in the browser, admin only: first setup (the setup code, the
  first admin's name, linking the server to the owner's Orchester Cloud account through the cloud's `/authorize`);
  projects (create by name and slug, the slug fixed; rename; the repository remote hint); members and roles per project
  with the last-owner refusal; invites by email through the cloud and by code, open invites, revoke; people (disable,
  enable, account codes, admin flag); devices joined to the server, revoke; settings (join approval, the cloud link and
  its root key, the certificate check code). Sign-in through the cloud (`/authorize`, added to `web/cloud` in this
  phase) or a code; the `__Host-orchester_admin` session cookie and CSRF checks as in C1b; the team server's API
  called from the server side only. Team server: the reverse proxy of non-API paths to the admin, reserved slugs,
  `GET/PATCH /server/settings`, `GET/DELETE /admin/sessions`, `remote_hint` in `PATCH /projects/{id}`.
- **Files.** `web/server/` (`app/`, `lib/api.ts`, `lib/session.ts`, `locales/en.json`, tests), `web/cloud/app/authorize/`;
  `crates/server/src/http/{admin_proxy,settings}.rs`, `config.rs` (`admin_url`); `deploy/server/` (compose with
  `server` and `admin`, a service unit example for a host without Docker).
- **Frames.** **To draw first, for the person's approval** (`data/orchester2.pen`, *Server admin* layer, on `web/ui`'s
  components): setup (setup code, first admin, link to the cloud), sign-in (cloud or code), projects (list, create with
  slug, rename and hint), members and roles (with the in-place confirmations of `woioY`), invites (by email, by code
  shown once, open, revoke), people, devices, settings; desktop and narrow widths; the cloud's *Sign in to Kontur
  team's admin?* page.
- **Tests.** The team server proxies admin paths and refuses reserved slugs; end-to-end against a loopback server and
  cloud: setup with the setup code and linking, sign-in through the cloud and with a code, creating a project, inviting
  by email (the invite reaches the cloud) and by code, changing a role, revoking a device, turning join approval on;
  the session and CSRF checks; no code or token in logs; an air-gapped run makes no outside request; design
  verification of each page in a browser.
- **Depends on.** S1c, C1b's `web/ui`, and the person's approval of the frames.

### N1 · Signing in, connecting servers, linking projects · UI (member actions only)

- **Scope.** Settings › Orchester: sign in to the cloud (email and password, through the API), *Create an account* and
  *Forgot password* opening the cloud's web front in the browser, waiting invites, and the connected servers, each with
  its linked projects on this computer, *Add a computer* (a pairing code for the person's own next computer), *Sign in
  to the admin* (a pairing code for the server admin's sign-in, for admins on an offline server), *Open the server
  admin* (a link) and *Sign out* naming those projects. *Connect to a server* with an invite code: the address, the
  certificate check code to compare, the code, a username for a new account. Project settings › Sharing for a local
  project: *Link to a project* by address (or from `orchester.toml`), with the remote mismatch warning. The status
  bar's server item. A notice in All projects for waiting invites. No owner setup, no server settings, no invites, no
  device revocation for others: those are SA's.
- **Files.** `crates/desktop-app/src/screens/settings/orchester/` (new: account, invites, servers, connect),
  `settings/mod.rs` (the tab), `screens/project_settings/sharing/{mod,link}.rs` (new), `project_settings/page.rs`
  (the tab), `shell/status_bar/server.rs` (new), `screens/all_projects/` (the invites notice), `routes/`; locales
  `cloud.*`, `sharing.connect.*`, `statusBar.server.*`; theme roles.
- **Frames.** **To draw:** Settings › Orchester (signed out with *Create an account* and *Forgot password* leading to
  the web, sign-in with its refusals, signed in with invites and connected servers); *Connect to a server* (address,
  check code to compare, invite code, choose a username, the refusals: wrong code, expired, rate limited, a different
  certificate); accepting an invite; the server entry with *Add a computer*, *Sign in to the admin* and *Open the
  server admin*; Sharing › *Link to a project* with the mismatch warning; the status bar server item (connected,
  reconnecting, offline, signed out, two servers), after old 10.43 `hYres`.
- **Tests.** App against loopback server and cloud: sign-in (an account confirmed through the API in the test), the
  web links opened in the browser, accepting an invite (pin from the invite, no check code asked), connecting with an
  invite code (check code confirmed), linking a local project by address with and without a remote mismatch, pairing a
  second device, signing out names every project on that server, the status bar states, and that no admin endpoint is
  ever called by the app; i18n and theme parity.
- **Depends on.** S1c. A server and projects for the tests are set up through the API, as SA would.

### N2 · Shared project, read-only members, Make local, discovery (S2b in the new app) · UI

- **Scope.** Project settings › Sharing once linked: the first checkout linked to an empty project uploads its tasks
  with their numbers (the old Share confirmation page); the shared state (server, project address, role); members and
  roles **read-only**, with *Manage members and invites* opening the server admin in the browser; Make local (reachable
  and not); `orchester.toml` prefilling the address. The shared task list's header and the *no longer a member* state;
  *<slug> · not on this computer* for a pipeline this computer lacks (until N7b). The conflict view for shared saves.
- **Files.** `screens/project_settings/sharing/{shared,members,make_local,discovery}.rs`, `screens/tasks/{header,
  states}.rs`, `screens/task_form/` (the conflict view gains the server's names); locales `sharing.*`,
  `tasks.conflict.*`.
- **Frames.** `CHSKS`, `KSnOW` (as the first-link upload), `zRz47` and `woioY` with their member and role controls
  replaced by a read-only list and *Manage members and invites*, the Make local parts of `woioY`, the discovery part of
  `oOGMT`, `PenUu`. **To draw:** that read-only members section, the shared task list header and the *no longer a
  member* list (old 10.44 `Y6H8A`). No read-only offline frame (row 38).
- **Tests.** App against loopback: link a checkout with tasks to an empty project (numbers kept), link a second
  checkout (new numbers), members shown read-only and *Manage members and invites* opening the server admin's URL,
  Make local keeps every task, the discovery file is not contacted until confirmed, a conflicting edit shows the
  field-by-field view. Workspace `shared_projects`, `shared_statuses`.
- **Depends on.** N1.

### N3 · Join (S2c in the new app) · UI

- **Scope.** Accepting a project invite (B: download, sign up on the web or sign in, accept) connects to the server
  when needed and opens the join review in the local project whose `origin` matches the project's hint, or asks which
  local project or checkout to link; linking by address without an invite (member, or not a member: *No project you're a
  member of here* and *Ask for an invite* from whoever shared it, since the server answers a non-member like no
  project; the app sends nothing); the review page with similar titles and
  locked rows; *Join and upload N*; a failed join keeping the ticks; *Keys before joining*; the *Not shared* filter
  and rows; a Not shared task with *Share task*; *was KON-12*; search by former key.
- **Files.** `screens/project_settings/sharing/join/` (new page and rows), `screens/tasks/` (filter, rows),
  `screens/task/header/` and `summary/`, routes (`…/settings/sharing/join`); locales `sharing.join.*`.
- **Frames.** `AiCwO`, `oOGMT` (not a member, after joining). **To draw:** the invite landing (which local project,
  or add the checkout), the task list with the Not shared filter (old 10.23 `l9jYZS`), a Not shared task with Share
  task in place (10.24 `FAKNF`, 10.26 `m7jOn9`), a joined task's *was KON-12* (10.25 `RuBhQ`).
- **Tests.** App against loopback with two people: an invite lands in the join review; join with ticked and unticked
  tasks; a task with a running run cannot be unticked; the mapping shows and stays; a lost join answer does not
  upload twice (forwarder); Share task later. Workspace `joined_projects`.
- **Depends on.** N2.
- **Built (addresses and automation).** The join review is `/project/<id>/settings?tab=sharing&view=join&server=<server
  id>&project=<slug>`, the invite landing `/settings/invite?server=…&project=…`, a task kept *Not shared*
  `/project/<id>/tasks/kept/<key>/<tab>` (a shared task may have its key) and the list's filter `?show=not-shared`.
  Opening the review or the landing reads the server (the project, its tasks, and nothing else) and only reads: it
  is open to MCP clients like any other navigation. What sends is a person's only: *Join and upload*, the review's
  ticks and *Share task*. A checkout linked already is never reviewed or joined again: the review's address shows the
  Sharing of the project it is linked to, and refuses any other project before anything is read or sent.

### N4 · Live task list and claims (S2d) · server, non-UI and UI

- **Scope.** Server: the staleness sweep and its `task` events. API: the SSE parser moved from `orchester-agent` to
  `orchester-api::sse`; `Client::events(project, cursor)`. Workspace: `sharing::sync::{stream, apply, state}`; the
  minute poll goes. Screens: the Holder column (1440 and 960), the task page's *On Kontur team* panel with every claim
  state, *Take*, *Release*, *Take over* confirmed in place, the lost-claim notice, *A new computer joined as you*
  (row 47) on the other devices.
- **Files.** `crates/server/src/sharing/claims.rs` and a sweep beside the event pruning; `crates/api/src/sse.rs`
  (moved), `crates/api/src/client/events.rs`; `crates/agent` uses the moved parser;
  `crates/workspace/src/sharing/sync/{mod,stream,apply,state}.rs`, `sharing/project/work.rs`; app
  `screens/tasks/{row,table}.rs`, `screens/task/summary/`, `screens/task/header/controls/`; locales `claims.*`.
- **Frames.** **To draw:** the shared list with Holder at 1440 and 960 and the lost-claim notice (old 10.31 `jakf4`,
  10.32 `ByIkx`), the claim panel states and refusals (10.34 `omHGh`), Take over in place (10.35 `U4GzjB` without a
  dialog), the new-computer notice.
- **Tests.** Server: a claim's event when it becomes stale and when refreshed, none for a fresh refresh. API: the
  parser's tests move with it; the client resumes a stream with `Last-Event-ID`. Workspace loopback with two
  workspaces: an edit reaches the other's cache within a second; `reset` reloads; a revoked membership stops the
  stream; a takeover is announced to the old holder. App: Take, Release, Take over, lost claim.
- **Depends on.** N3 for the screens; its server and workspace parts may start after S1c.

### N5 · Shared run progress (S3, decisions 29 and 34) · server, non-UI and UI

- **Scope.** Server: `PUT …/run` with the fields of row 29. Core: `sharing::progress::of(run, steps, merge, pull
  request)`, with *unchanged* as Verify-flow's `Unchanged` (row 34). Workspace: `sharing::sync::progress` reports
  every run and pull request change of a held task (directly; N6b moves the reports into the outbox). Screens: another
  member's run on the row and the task page: phase, step, waiting kind, the step rail with results, branch, pull
  request link.
- **Files.** `crates/server/src/http/claims.rs`, `sharing/progress.rs`, `db/` migration; `crates/api/src/tasks.rs`;
  `crates/core/src/sharing/progress.rs`; `crates/workspace/src/sharing/sync/progress.rs`; app
  `screens/task/summary/`, `screens/tasks/row.rs`; locales.
- **Frames.** **To draw:** a task page showing another member's run with the step rail and its results, the waiting
  kind, branch and pull request (old 10.33 `NikWo`, extended), and the row text in the shared list.
- **Tests.** Core: every phase, each waiting kind and each step result (including a step that waits `Unchanged`).
  Server: a non-holder refused, merged completes and cancelled releases the claim, invalid shapes refused, the
  database holds no run data beyond the fields. Workspace loopback: a run on one workspace shows its progress on the
  other. App: the task page and row.
- **Depends on.** N4.

### N6a · Outbox groundwork (S4, non-UI)

- **Scope.** Server: `idempotency_key` on task changes and tag creates, `change_keys`, `409 tag_exists { tag }`.
  Core: `sharing::outbox` (entries, merging of waiting changes, `next_sendable`), `TemporaryTaskKey`,
  `held_offline` (row 28). Store: `outbox`, `shared_tasks_confirmed`, `temporary_keys`; the migration folding
  `shared_automatic_status` in. No behavior change yet.
- **Files.** `crates/server/src/sharing/{task_write,tags}.rs`, `db/` migration; `crates/api/src/tasks.rs`;
  `crates/core/src/sharing/{outbox,offline}.rs`, `project.rs`; `crates/store/src/`.
- **Frames.** None.
- **Tests.** Server: a repeated key returns the first answer and writes one history row; a refused change is not
  remembered. Core: holding and merging over explicit lists; `held_offline` over explicit times; temporary keys never
  parse as task keys. Store: tables and the migration of a populated database.
- **Depends on.** N5 (API 3).

### N6b · Offline-first (S4, decisions 27–28, 32–33) · non-UI and UI

- **Scope.** Workspace: `RemoteTaskStore` writes the person's view and records entries; `sharing::sync::{outbox,
  sender, temporary}`: ordering, coalescing, keys, backoff, `409 revision_conflict` merge, conflicts and refusals for
  the person, temporary keys and the rewrite of waiting texts; tag creates offline, rename, merge and delete refused
  offline (row 33); the offline start rule; Create & run offline only saves, and *Start* is offered once the number
  and the claim can come, never started on its own (row 32); progress through the outbox. Screens: the status bar's
  counts, the offline notice, the sync marks, the conflict view for a waiting change, the refused change with *Edit
  again* and *Discard*, the temporary-key notice with *Start*, *was ORC-T1*, the offline start refusals, *Waiting to
  send* in Sharing, Manage tags offline.
- **Files.** `crates/workspace/src/sharing/project/{store,claims,automatic}.rs`, `sharing/sync/{outbox,sender,
  temporary}.rs`, `tasks/{writer,conflict}.rs`, `app_mcp/tools/{tasks,task_write,run_task}.rs`; app
  `shell/status_bar/server.rs`, `screens/tasks/{states,row}.rs`, `screens/task/header/`,
  `screens/project_settings/sharing/outbox.rs`, the tag manager, `routes/` (temporary keys resolve); locales
  `sharing.outbox.*`.
- **Frames.** **To draw:** the task list offline with waiting marks and the offline notice, a task page with a
  waiting change, a conflict found on send and a refused change; the temporary-key notice with *Start*; *Waiting to
  send* with Discard in place; the status bar item with counts; the offline start refusals; Manage tags offline.
- **Tests.** Workspace loopback (server stopped and started, forwarder dropping answers): edits made offline are sent
  in order; ten edits send one change; a dropped answer is applied once; two people editing one task offline end
  with organization merged and both-changed content in `conflict`; a refusal keeps the person's version; a task
  created offline gets its number, the waiting texts naming it are rewritten, and no run starts until *Start*; a tag
  created offline maps to an existing one of the same name, and a rename offline is refused; the offline start rule
  with the settable clock; a takeover found on reconnect drops the reports and announces it; Make local with waiting
  entries keeps the person's view and reports the unsent count. App: each state above.
- **Depends on.** N6a, N4, N5.

### SK · Skills in the database (decision 37) · non-UI and UI

- **Scope.** Core: `SkillLimits` (1 MiB a part, 16 MiB a version, 256 parts, row 55) and an executable
  flag per part. Store: `skill_files` and the migration copying every part of every skill version and draft from its
  blob (a part over the limit kept and marked *too large to share*). Runner: place a step's skills in the run's
  private folder (`private_file` root, `<project>/skills/<execution-id>/<slug>/`) at each start of the execution,
  remove them when the step ends, remove leftovers when a runner starts, remove `<repo>/.orchester/skills/` once.
  Repository skill folders stop being live: Codex disables them at every start, the native agent and
  `list_skills`/`read_skill` offer Orchester's skills only. Workspace: the skill editor and Import from repository at
  the new limits. Screens: the executable mark on a part, the size refusals, *Repository agent files* reworded to
  `AGENTS.md`.
- **Files.** `crates/core/src/resource/skill.rs`, `agent_files/mod.rs`, `agent_files/skills/native.rs` (removed or
  reduced), `orchester_tools/skills/{mod,repository}.rs` (the repository source goes); `crates/store/src/`;
  `crates/runner/src/execution/step/skills.rs`, `execution/step/start/native.rs`, `private_file/`;
  `crates/workspace/src/skills/{data,editor,repository}.rs`; app `screens/skills/`,
  `screens/project_settings/general/`; locales.
- **Frames.** **To draw:** the skill editor's parts with the executable mark and the size refusal; Project settings ›
  General's *Repository agent files* with its new text.
- **Tests.** Store: the migration of a populated project keeps every part by hash. Runner: a step's skills exist in
  the private folder while it runs, are rewritten on a resumed start and are gone when it ends; a leftover folder is
  removed at runner start; nothing is written under the repository. Core: a repository `SKILL.md` is never offered
  (Codex disables it with the setting on). Workspace: import and editor limits. App: the editor states.
- **Depends on.** Nothing; before N7a.

### N7a · Shared configuration on the server (S5, non-UI)

- **Scope.** Server: `resources`, `resource_versions`, `version_references`, `files`, the configuration endpoints of
  API 3 including `PUT …/files/{sha256}`, body checks with core's rules per kind, tools of every kind (row 52), `resources`
  events. Core: `sharing::references` (references of a body, resolution through bindings, the reverse when
  publishing), reusing `import`'s walk; `widens(previous, next)` (row 35). API: shapes and client calls.
- **Files.** `crates/server/src/sharing/{resources,files}.rs`, `http/resources.rs`, `db/` migration;
  `crates/api/src/resources.rs`, `client/resources.rs`; `crates/core/src/sharing/{references,widens}.rs`,
  `import/references.rs` (made reachable, not copied).
- **Frames.** None.
- **Tests.** Server: create, publish, `version_conflict` from one base, slug and id clashes, HTTP and database tools
  with their secret references, references that do not match the body refused, a part missing (`missing_files`) and a part over 1 MiB
  refused, import keeps numbers, events. Core: resolution and reverse over explicit bodies; `widens` for permissions,
  a new MCP server or secret, a changed command, address or access policy of a tool, a first version.
- **Depends on.** N6a, SK.

### N7b · Shared configuration on the desktop (S5, decisions 30, 35–36) · non-UI and UI

- **Scope.** Store: `shared_version_bodies`, `reference_bindings`, the resource columns. Workspace:
  `sharing::sync::config` (resource sync, local bodies, bindings by name and role, person-only bindings for MCP
  servers and secrets, a widening version waiting for *Use v<N> here* while runs keep the previous one),
  `RemoteResourceStore` (drafts local, publish through the outbox, an offline publish used by the person's own runs at
  once, `version_conflict` for the person), skill parts uploaded by hash, Share uploading resources with their history,
  Make local keeping them. Screens: shared marks in the five libraries, *Publish for the team* with the publisher on
  versions, *missing on this computer* with the binding picker, *v5 widens what agents may do here* with *Use v5
  here*, *Published here, not sent*, the publish conflict.
- **Files.** `crates/store/src/`, `crates/workspace/src/sharing/sync/config.rs`,
  `sharing/project/{share,make_local}.rs`, the resource services under `pipelines/`, `agents/`, `instructions/`,
  `skills/`, `tools/`; app: the library and editor headers of `screens/{pipelines,agents,instructions,skills,tools}/`,
  `screens/project_settings/sharing/bindings.rs`; locales `sharing.config.*`.
- **Frames.** **To draw:** a library with shared resources and publishers; an editor header for a shared resource
  (draft on v4, Publish for the team, published here not sent, conflict with a newer version, *Use v5 here*); the
  binding picker for a connection, an MCP server and a secret with the person-only confirmation; *What sharing sends*
  updated for configuration.
- **Tests.** Workspace loopback with two workspaces: Share uploads resources with numbers kept; a profile published
  on one runs on the other with its connection bound by role; an MCP server reference stays unbound until a person
  binds it and no MCP tool can bind it; a widening version waits and *Use v5 here* cannot be pressed by a tool; a
  binding change rewrites local bodies; an offline publish is used by the person's own run and gets its number on
  reconnect; two publishes from one base give the second a conflict; a skill with parts round-trips by hash; Make
  local keeps every version. App: each screen state.
- **Depends on.** N7a, N6b.

### N7c · Configuration in Join (S5) · UI

- **Scope.** The join review's configuration part: matching by kind and slug then name, the three choices, *missing
  on this computer* listed and bindable there, references rewritten before upload; *Not shared* resources with
  *Share* later.
- **Files.** `crates/workspace/src/sharing/project/join.rs` split first (381 lines) with `join/resources.rs` beside
  it; app `screens/project_settings/sharing/join/`; locales.
- **Frames.** **To draw:** the join review page with its configuration section (extending `AiCwO`), a Not shared
  resource.
- **Tests.** Workspace loopback: equal content adopts; different content keeps the person's as a draft on the team's
  latest; *Upload as a new one* with a typed slug; an unmatched resource uploaded with its versions; references
  between adopted and uploaded resources rewritten; past runs unchanged. App: the review page states.
- **Depends on.** N7b, N3.

### N8 · The Orchestrator reads shared data

Not a phase of its own: each phase from N4 on adds its part to the domain MCP tools in `crates/workspace/src/
app_mcp/tools/` (the Orchestrator chat uses the same tools, `docs/design/orchestrator-toolset.md`). The coordinator
gives these edits to the stream that owns those files (stream B).

| Phase | What the tools gain |
| --- | --- |
| N4 | `get_task`, `list_tasks`: `shared` (server name, project address and key), `holder`, `claim` (kind, since, stale), `former_key`; `list_projects`/`get_project`: sharing state and connection |
| N5 | `get_task`, `list_tasks`: `progress` of another member's run (phase, step, waiting kind, step results, branch, pull request with `url`); `list_runs`/`get_run`/`get_changes` stay local-only and say so for a task whose run is on someone else's computer, with the task's `link` |
| N6b | `sync` per task (`synced`, `waiting`, `conflict`, `refused`), temporary keys resolvable as task names and returned with `temporary: true`; `create_task`/`update_task` answer with the sync state instead of failing offline; `run_task` follows row 28 (`claim_needs_server`, `claim_stale_offline`) and refuses a task with a temporary key |
| N7b | `list_pipelines`, `list_profiles`, `list_instructions`, `list_skills`, `list_tools`: `shared`, each version's publisher, `missing_here` references, `waits_for_person` for a widening version; publishing a shared resource is always a proposal card that says it publishes for the team (row 35) |

Rules for the chat: like the desktop, it has no admin action (row 63); it reads only the local cache, so it works offline and needs no token; it never takes or takes
over a claim, resolves a conflict, discards a waiting change, binds a reference, presses *Use v<N> here*, publishes a
shared resource directly (*Let the Orchestrator publish drafts* does not cover them), redeems or makes an invite or
code, or starts Share, Join or Make local; a conflict is reported with a `link` to the task's conflict view.

**Tests.** `crates/workspace/tests` (MCP tool tests over a shared project on loopback): each field above, a refusal
for each forbidden action, and the router parity test for links to temporary keys.

## Questions for the person

None. The person settled every question on 2026-10-15 (server.md rows 31–64), including the server admin (62), the
desktop's member-only scope (63) and the shared web package (64). What remains for the person is approving the
frames of C1b, SA and every UI phase when they are drawn.
