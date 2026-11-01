# Orchester — shared server

> New [S1 workspace role contracts](workspace-roles.md) are additive. The existing Owner/Member contracts described
> here remain supported; the new workspace-authority types live separately in `orchester_api::v2`.

> **Where the code is (2026-10-27).** The team server (`crates/server`), its web app (`web/server`), the shared web
> kit (`web/ui`), `deploy/server`, `docs/server-hosting.md` and the end-to-end tests against a real server (its
> `crates/e2e`) live in the private platform repository `ivanzaida/orchester-platform`; paths below under those
> folders, and the tests that start a server, mean that repository. The desktop's side (`orchester-api`,
> `orchester-workspace`'s `sharing`, `orchester-app`'s screens) stays here.

Date: 2026-10-02, revised 2026-10-15 and 2026-10-22. Status: **agreed direction**. The user's decisions are recorded in
[Decisions](#decisions). Rows 1–26 were settled on 2026-10-02/03; rows 27–30 (offline-first, offline claims, shared
progress, shared team configuration) were added by the user on 2026-10-15 and replace or extend the rows marked in the
table. Rows 31–38 answer the open questions of that revision (2026-10-15, the person's or the coordinator's, as
each row says); one of them, row 37, moves skills into the project database. Rows 39–47 (the person, 2026-10-15)
make the team server work without the internet, move the connection into each project, and replace passwords with
Orchester Cloud identity and email invites, with codes as the fallback without the internet. Rows 48–53 (the
person's overall vision, 2026-10-15) win where they differ: email and password sign-up on the cloud, team servers at
the owner's own address holding several projects each at its own address, one connection per server, and every kind
of tool shared. Row 54 puts the cloud on PostgreSQL; rows 55–61 accept the remaining proposals and set the cloud's
address, its actix-web backend and its Next.js web front. Rows 62–64 give each team server a web admin, keep the
desktop to linking and working, and share one web component package. Rows 66–71 (ORC-78, 2026-10-22) share
modules ([Modules](#modules)). The plan for building
the desktop side in the new app is
[server-new-app-plan.md](server-new-app-plan.md). Nothing here changes the local (single-user) version in
`storage.md` except where a section says so.

**Goal (user, 2026-10-02, extended 2026-10-15).** Several people work on one project in Orchester. They share the
task list and can see who took a task into work and how far its run got. They share the team's configuration:
pipelines, agent profiles, instructions, skills and tools. Each person keeps working when the server cannot be
reached; their changes are sent when it is back. A team server runs at its owner's own address and holds several
projects, each at its own address; Orchester Cloud, run by the person for everyone, says who people are and carries
invites (rows 48–53).

Today a project lives in `<repo>/.orchester/orchester.db` on one computer. This document adds a server a team runs
itself. It holds a shared project's task list, who has taken each task (the *claim*), a short progress report of the
run working on it, and the published versions of the team's configuration. Runs themselves (worktrees, agents,
transcripts) and everything that holds access (connections, accounts, secrets, environment) stay on the computer of
the person who owns them.

## What is built

| Phase | State (2026-10-15) | Where |
| --- | --- | --- |
| S1a · Server and accounts | Merged | `crates/server`, `crates/api` |
| S1b · Projects, tasks, claims, tags, roles, events on the server | Merged 2026-10-04 | `crates/server`, `crates/api`, core `sharing` |
| S2a · Servers and sign-in | Merged; **screens only in the deprecated app**; its password sign-in and Settings › Servers are superseded by rows 39–46 | service: `crates/workspace/src/sharing/`; screens: `crates/desktop/src/screens/project/servers/` |
| S2b · Share, members, Make local, read-only offline, conflict | Merged; **screens only in the deprecated app** | service: `crates/workspace/src/sharing/project/`; screens: `crates/desktop/src/screens/project/settings/sharing/`, `tasks/shared.rs` |
| S2c · Join | Merged 2026-10-05; **screens only in the deprecated app** | service: `sharing/project/join.rs`; screens: `crates/desktop/src/screens/project/join/`, `tasks/not_shared.rs` |
| S1c · Certificate, codes, devices, invites, cloud link, projects by slug | Merged | `crates/server`, `crates/api`, `crates/workspace/src/sharing/` |
| C1a, C1b · Orchester Cloud backend and web front | Merged | `crates/cloud`, `web/cloud`, `web/ui` |
| SA · Server admin | On branch `claude/server-admin`, not merged | `web/server`, `crates/server/src/http/{admin_proxy,internal,settings,managed}.rs`, `crates/cloud/src/{servers,invites}.rs`, `web/cloud/app/authorize/`, `deploy/server/` |
| ORC-78 · Modules in shared projects (rows 66–71) | Built 2026-10-22 (schema 7, the `modules` event kind); offline module writes wait for S4 | server: `crates/server/src/sharing/modules.rs`, `http/modules.rs`, `db/modules.sql`; API: `crates/api/src/modules.rs`, `client/modules.rs`; cache: `crates/store/src/project_db/shared_modules.rs`; service: `crates/workspace/src/sharing/project/modules.rs` and `TaskWriter` |
| S2d, S3, S4, S5 | Not built | this document; [server-new-app-plan.md](server-new-app-plan.md) |

The new app (`crates/desktop-app`) already uses the S2 service where it writes tasks and starts work: its task form
and task page save through `TaskWriter` (local or shared) and take run claims before work, and it starts the
connection monitor and the shared projects' sync (`services/mod.rs`). Its Modules page, the task form's and task
page's module picker and the domain tools' `create_module`, `update_module` and the module of `create_task` and
`update_task` write a shared project's modules through `TaskWriter` too (ORC-78). It has none of the sharing screens:
Settings ›
Servers says it is not built yet, and there is no Sharing tab, server status item or shared task list. The service
tests are in `crates/workspace/tests` (`sharing`, `shared_projects`, `joined_projects`, `shared_statuses`,
`shared_modules`, with a
real server on loopback in `server_support` and a forwarding port in `sharing_support`); the screens' tests are in
`crates/desktop/tests/form_keyboard` (`servers`, `sharing`, `joining`).

Not built on the server: `PUT …/run` (S3's server half), claim staleness events, the idempotency key on task changes
(`change_keys` keeps module creates and changes only),
every configuration endpoint (S5), and everything of rows 39–47: the generated certificate, codes, invites, device
joins, the cloud link and the removal of passwords (S1c). Orchester Cloud (C1) does not exist yet.

## Decisions

| # | Topic | Decision | State |
| --- | --- | --- | --- |
| 1 | Hosting | Self-hosted: one server binary a team runs itself, Docker is fine. ~~No cloud service, no GitHub OAuth~~ **Changed by 41:** self-hosted team servers, plus one cloud service the person runs (Orchester Cloud) that holds identity and invites only, for everyone; sign-up with email and password (row 48) | user (changed 2026-10-15) |
| 2 | Accounts | Accounts live on the server; an admin creates users. A user signs in from the desktop and receives a token, kept in the OS keychain like every secret. Passwords are hashed with argon2id; tokens can be revoked; the first admin is created on the server host ([Accounts](#accounts-and-tokens)). **Replaced by 41–46:** identity comes from Orchester Cloud or, without the internet, from codes; passwords go away | user (replaced 2026-10-15) |
| 3 | Transport security | HTTPS, from a reverse proxy or the server's own TLS. The desktop accepts `http://` only for a loopback host, or for another host after an explicit opt-in for that server. **Extended by 39:** the server's own TLS is a certificate it generates, pinned by the desktop | user |
| 4 | v1 scope | Shared: tasks (title, brief, criteria, pipeline, limits, status), their organization (workflow status, priority, tags, blocking links and the manual Blocked reason, with its history; rows 23–24), the claim, the run's progress (row 29), and the team's configuration (row 30). The server assigns task numbers. Local: runs, worktrees, transcripts, Activity, conversations, memory, connections, accounts, secrets, environment, limits settings and drafts. **Extended by 29, changed by 30** | user (confirmed 2026-10-03; organization added 2026-10-03; changed 2026-10-15) |
| 5 | Claim | Atomic "take if free" on the server. Starting work on a task takes the claim. Heartbeat every 60 s; no heartbeat for 15 min makes it stale; a stale claim can be taken over after confirmation, and the takeover is recorded. Manual Take and Release exist ([Claims](#claims)). **Extended by 28** | user |
| 6 | Concurrency | Every shared task has a revision. A change names the revision it was made on; a mismatch returns a conflict with the current task, and the desktop shows the other person's change instead of overwriting it. **Extended by 27:** a change waiting in the outbox keeps the revision it was made on | user |
| 7 | Live updates | Server-Sent Events with a resume cursor; the desktop reconnects with backoff ([Live updates](#live-updates)). **Extended by 73:** a snapshot on connect, then deltas that say who did what | user (confirmed 2026-10-03) |
| 8 | Offline | ~~Without the server, a shared project's task list is read-only from the local cache, in a visible offline state. Changes wait for the connection; nothing is queued silently~~ **Replaced by 27** | user (confirmed 2026-10-03; replaced 2026-10-15) |
| 9 | Configuration | ~~Application level: Settings › Servers (several servers, each signed in; the status bar shows the connection).~~ Project level: Project settings › Sharing, *Local only* (default) or *Shared on <server>*. **The application level is replaced by 40** | user (confirmed 2026-10-03; changed 2026-10-15) |
| 10 | Project identity | ~~A server project is identified by the normalized URL of the repository's `origin` remote, unique per server; the server enforces it~~ **Replaced by 49:** the project's address is its identity; the remote is a hint | user (replaced 2026-10-15) |
| 11 | Discovery | An optional committed `orchester.toml` at the repository root (`server = "https://…"`, no secrets) offers *Sign in and join*. Adding a server by hand always works. **Changed by 49:** the file names the project's address (`project = "https://orchester.example.com/kontur"`) and prefills it when the person links the checkout | user (confirmed 2026-10-03; changed 2026-10-15) |
| 12 | Numbers on Share | The first person to share keeps their task numbers; the server continues after the highest. **Under 49:** the first checkout linked to an empty server project keeps its numbers | user |
| 13 | Framework, storage | `axum` on the workspace's tokio; SQLite through rusqlite for v1. **Under 54:** for team servers; the cloud uses PostgreSQL | user (confirmed 2026-10-03) |
| 14 | Trait boundary | `TaskStore`, with a revision on updates; no new core trait ([The desktop's trait boundary](#the-desktops-trait-boundary)). Shared configuration (row 30) goes through the existing `ResourceStore` the same way | user (confirmed 2026-10-03) |
| 15 | Claim after a run ends | Kept when the run succeeds or fails (review, merge and recovery happen on the holder's computer); released on cancel; completed on merge | user |
| 16 | Who may share | ~~Any user. The first to share a repository creates its server project and becomes its owner; everyone after joins it~~ **Changed by 49:** the server's owner (and admins) create projects by name and slug; the creator is the project's owner; people join by invite ([Two people share one repository](#two-people-share-one-repository)) | user (changed 2026-10-15) |
| 17 | Visibility | Explicit membership per project. Owners and admins add members by username. **Extended by 43:** and by invites | user |
| 18 | Editing a claimed task | Any member may edit any task; revisions prevent overwriting | user |
| 19 | Removing a server project | Not in v1 | user |
| 20 | Joining with local tasks | A review list of the joiner's local tasks, all ticked; ticked ones are uploaded with new numbers and keep their id and former key; unticked ones are archived as *Not shared* ([Two people share one repository](#two-people-share-one-repository)) | user (flow 2026-10-02, archive confirmed 2026-10-03) |
| 21 | Token | The server generates it at sign-in; nobody is shown it; the desktop keeps it in the keychain; when it expires, the person signs in again with their password. Expiry after 30 days without use. **Replaced by 42, 45 and 46:** the team server issues a device token when a device joins with a cloud identity token or a code; there is no password to sign in again with | user (expiry confirmed 2026-10-03; replaced 2026-10-15) |
| 22 | Two computers, retention | A heartbeat from any of a person's computers keeps their claims fresh. Events are kept 30 days; an older cursor makes the desktop reload the full list (**under 73:** the stream sends it the snapshot). **Extended by 44:** a new computer joins through the cloud or a pairing code | user |
| 23 | Task organization | ORC-39's organization is shared: workflow status, priority, tags, blocking links (a task and its gate, Review or Merge) and the manual Blocked reason are fields of a shared task, changed under its revision, with core's rules (`task::organization`). Its history is shared: every change is recorded with who made it and whether a person or Orchester did ([Task organization](#task-organization)) | user (2026-10-03) |
| 24 | Tags | Tags belong to the server project. Members create, rename, merge and delete them under ORC-39's local rules (names unique without regard to case; merge unions into the target; delete and merge rewrite the affected tasks and record their history) | user (2026-10-03) |
| 25 | Member roles | Owners and admins change a member's role (`PATCH …/members/{user}`); demoting or removing the last owner is refused (`409 last_owner`), so a sole owner first makes someone else an owner and can then step down or leave | user (2026-10-03) |
| 26 | Organization on a conflict | When a change meets a newer revision, the desktop merges the organization fields with core's `TaskOrganization::rebase` (each field the person did not change keeps the current value; tags and links merge by what was added and removed) and sends again without asking; only title, brief, criteria, pipeline and limits changed by both reach the person ([Task organization](#task-organization)). **Applies to every change the outbox sends (27)** | user |
| 27 | Offline-first | Without the server, a shared project works as usual. Every change goes into a local outbox with the revision it was based on, and is sent in order after reconnect. Organization fields merge automatically, as in 26; title, brief, criteria, pipeline and limits that both sides changed go to the person. A task created offline has a temporary number until the server assigns one; its links, references and keys are remapped, and the person sees the change ([Offline and the outbox](#offline-and-the-outbox)). **Replaces 8** | user (2026-10-15) |
| 28 | Claims offline | Offline, a run may start only on a task whose claim is already the person's; row 5's staleness applies: once the claim would be stale by the last heartbeat the server answered, nothing new starts until the server confirms the claim. Taking a new task needs the server ([Claims](#claims)) | user (2026-10-15) |
| 29 | Shared progress | Shared: who took the task, the run phase, which pipeline step it is on, what it waits for (a person, a limit, a question, an approval: kinds only, no content), each step's result (passed, failed, unverified, unchanged), the branch and the pull request. Local only: transcripts, diffs, prompts, tool calls and secrets ([Shared run progress](#shared-run-progress)). Both ways (row 53). **Extends S3** | user (2026-10-15) |
| 30 | Shared team configuration | Shared: pipelines, agent profiles, instructions, skills and the project's ~~command~~ tools (**changed by 52:** every kind). Publishing creates a version on the server; drafts stay local until published (**changed by 72:** only admins, the project's owner or the server's admin, publish and keep drafts of shared resources; members read and run with them). (**Changed by 74:** besides the team's pipelines, a person may keep pipelines of their own that stay on their computer and are never sent.) Per person and local: connections, accounts, MCP servers' credentials, secrets and environment. A published version names what it needs by name and role, resolved on each computer, with a visible *missing on this computer* state ([Shared configuration](#shared-configuration)). **Changes 4** | user (2026-10-15; changed 2026-10-29) |
| 31 | The deprecated app | `crates/desktop` is frozen as of 2026-10-15 and never updated again (AGENTS.md). Its S2a–c sharing screens stay exactly as they are; after S4 they show read-only offline states that no longer apply, by design. Every phase touches only the new app, `orchester-workspace`, core, api and server | user (2026-10-15) |
| 32 | Create & run offline | The task is saved with its temporary key and nothing more. Once it has its number and the server can give the claim, the task and the temporary-key notice offer *Start*; the run starts only when the person presses it. Nothing starts on its own | user (2026-10-15) |
| 33 | Tags offline | Creating a tag works offline (it waits in the outbox); renaming, merging and deleting need the server, since they rewrite other people's tasks ([Tags](#tags)) | coordinator (2026-10-15) |
| 34 | *Unchanged* | A step result *unchanged* is Verify-flow's: the step was not run again because its latest failed or unverified result was for the revision it would run on now and nothing a person asked for changed since (core `RunStatus` reason `Unchanged`) ([Shared run progress](#shared-run-progress)) | coordinator (2026-10-15) |
| 35 | A teammate's version that widens | A version published from another computer that widens what this computer's agents may do (broader permissions, a newly referenced MCP server or secret) applies here only after the person's person-only *Use v<N> here*; until then runs keep the previous version. Versions that do not widen apply at once. The Orchestrator chat always proposes a card for a shared resource; *Let the Orchestrator publish drafts* does not cover shared resources ([Shared configuration](#shared-configuration)) | user (2026-10-15) |
| 36 | Offline publishing | A publish made offline is used by the person's own new runs at once, sent after reconnect, and if someone published from the same base meanwhile, the person decides ([Shared configuration](#shared-configuration)) | user (2026-10-15) |
| 37 | Skills in the database | Skills are no longer folders of files Orchester reads from disk. Every skill Orchester manages, with all its parts (the `SKILL.md` body, reference files, scripts), is stored in the project database, versioned like the other resources and shared through the server like them. The runner creates a step's skill files at the step's start in the run's private CLI folder (never in the repository or the person's CLI home) and removes them when the step ends. Repository skill folders (`.claude/skills`, `.agents/skills`, `.codex/skills`) are an import source only, never a live source ([Skills in the database](#skills-in-the-database)) | user (2026-10-15) |
| 38 | Offline screens in the new app | No read-only offline frame is drawn for the new app; S4's offline-first notice is the only offline state of the shared task list | coordinator (2026-10-15) |
| 39 | A server without the internet | The server needs no internet: it runs on a machine in the team's own network, even an air-gapped one, with no dependency on public DNS or a public certificate authority. On first start it generates its own certificate and prints its check code to its console; the desktop pins that certificate, from a cloud invite's fingerprint or after the person confirms the check code. Plain `http://` stays allowed only through row 3's opt-in. Docker with Caddy and a public domain remains an option, trusted through the OS trust store ([Transport and certificates](#transport-and-certificates)). **Extends 1 and 3** | user (2026-10-15; the certificate design is the architect's) |
| 40 | Connection per project | **Wording replaced by 50 (one connection per server, a link per project).** Each project's Sharing settings hold its own server address and this device's sign-in for that project. There is no application-wide Settings › Servers as the place to connect; the new app does not build S2a's screens. The application database keeps its `servers` table as this device's internal record of servers, with no screen; projects on one server share this device's one token there ([Servers and project addresses](#servers-and-project-addresses)). **Replaces 9's application level, changes 11** | user (2026-10-15; one token per server is the architect's) |
| 41 | Orchester Cloud | One cloud service the person runs, for identity and invites only. ~~Sign-in by email magic link, Google or GitHub.~~ **Changed by 48:** sign-up and sign-in by email and password in v1; magic link, Google and GitHub later. It holds accounts (email, display name, linked providers), devices, which team servers exist and who owns them, and invites in flight. It never holds or relays project data: tasks, code, runs, configuration ([Orchester Cloud](#orchester-cloud)). **Changes 1, replaces 2** | user (2026-10-15) |
| 42 | Identity tokens | A team server is linked to its owner's cloud account at setup. The cloud signs short-lived identity tokens with asymmetric keys; the team server verifies them locally with the cloud's cached public keys, so day-to-day work needs no internet once a device has joined. The team server then issues the device its own device token, kept in the OS keychain ([Identity tokens and keys](#identity-tokens-and-keys); key rotation, caching and expiry are the architect's). **Replaces 21** | user (2026-10-15) |
| 43 | Invites by email | A project owner or admin invites an email address in the ~~app~~ server admin (row 63); the team server creates the invite and sends it to the cloud. A registered invitee gets it in their app and by email; an unregistered one gets an email to register first. Accepting in the app gives the team server's address, its certificate fingerprint and a one-time token; the app connects to the team server directly, which issues a device token and opens the project's join review. Server-level invites by an admin work the same way ([Invites](#invites)). **Extends 17** | user (2026-10-15) |
| 44 | New computers | Signing in to the cloud with the same identity on a new computer lists the person's servers and projects; each team server issues that device its own token ([New computers](#new-computers)). **Extends 22** | user (2026-10-15) |
| 45 | Without the internet | On an air-gapped network, address plus an 8-character code works: the server's one-time setup code for the owner (printed to its console, never logged), invite codes from the team server, pairing codes for new devices. Such accounts are local to the team server and can be linked to a cloud identity later ([Codes](#codes)) | user (2026-10-15) |
| 46 | No passwords | Passwords go away on team servers. S1a's accounts keep their ids, names, roles, memberships and current device tokens ([Migration of S1a accounts](#migration-of-s1a-accounts)). **Replaces 2's passwords** | user (2026-10-15) |
| 47 | The cloud's trust boundary | A compromised cloud may forge identity, but never reads project data. Owners can pin or revoke the cloud's key on their server. Rate limits, minimal email and personal data, and codes and tokens never logged ([Security](#security)) | user (2026-10-15) |
| 48 | Cloud for everyone, email and password | Orchester Cloud is run by the person for everyone. Sign-up is a simple form: email and password, with email confirmation and password reset by email. Magic link, Google and GitHub come in a later phase, not v1. It holds identity and invites only, never project data ([Orchester Cloud](#orchester-cloud)). **Changes 1 and 41** | user (2026-10-15) |
| 49 | Server and project addresses | A team server runs at its owner's own address (`orchester.userdomain.com`) and holds several projects, each at its own address (`orchester.userdomain.com/project-a`). The owner creates projects on the server by name and slug. The project address is the server project's identity; the repository remote is kept as a hint, compared with `origin` when someone links a local checkout, with a warning on a mismatch. Everyone clones and sets up the code themselves; the server never touches code ([Servers and project addresses](#servers-and-project-addresses)). **Replaces 10, changes 11, 12 and 16** | user (2026-10-15) |
| 50 | One connection per server | The app connects to a server once, with one device token per server. Each local project is linked to one project on it. Access is per project: an invite to project-a grants project-a only. **Replaces the wording of 40** | user (2026-10-15) |
| 51 | Joining | Invites go by email through the cloud (row 43): B downloads Orchester, signs in to the cloud, accepts the invite to project-a, and links their local checkout through the join review. The code fallback for networks without the internet stays (row 45). Team servers have no passwords and trust the cloud's identity tokens, verified locally (rows 42, 46) | user (2026-10-15) |
| 52 | What a project shares | Shared in a project: tasks (with organization, claims and progress) and all team configuration: pipelines, agent profiles, instructions, skills (stored in the database, files written at step start) and tools of every kind. Local: runs, worktrees, transcripts, connections, CLI accounts, secrets (and, by row 30, environment). **Changes 30. Changed by 72:** only admins (the project's owner or the server's admin) change the team configuration; members read it and run with it. **Changed by 74:** a person's local pipelines are local too, never sent; a shared task that names one keeps only its name and whose it is. **Changed by 76:** a task's and a module's base branch is local too, like runs and connections | user (2026-10-15; changed 2026-10-30) |
| 53 | Live progress both ways | A sees when B's task goes in progress or done, with its step, and B sees A's the same way (row 29) | user (2026-10-15) |
| 54 | The cloud's database | Orchester Cloud's database is PostgreSQL, not SQLite. Team servers stay on SQLite ([Framework and storage](#framework-and-storage)). **Narrows 13 to team servers** | user (2026-10-15) |
| 55 | Skill limits | A skill part up to 1 MiB, a version up to 16 MiB in all, at most 256 parts, for local and shared skills alike; each part uploaded by its hash, no general chunked upload; Import from repository goes down from 4 MiB to 1 MiB per file ([Skills in the database](#skills-in-the-database)) | user (2026-10-15) |
| 56 | Device token expiry | A team server's device token expires after 90 days without use ([Device tokens](#device-tokens)). **Replaces 21's 30 days** | user (2026-10-15) |
| 57 | Join approval | A team server setting, off by default: while on, a cloud join of an existing account waits until one of that account's joined devices or an admin approves it. The new-computer notice on the other devices is always shown ([The cloud's trust boundary](#the-clouds-trust-boundary)) | user (2026-10-15) |
| 58 | Project slugs | A project's slug is fixed once created (its name can change); projects made under row 10 take their repository's name as slug ([Projects made under row 10](#projects-made-under-row-10)) | user (2026-10-15) |
| 59 | Cloud address and configuration | The cloud runs at `orchester.cloud`, its own domain since 2026-10-25 (before, `orchester-cloud.lurq.dev`, which keeps answering while app builds and team servers from before the move call it; `docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`, "Moving the cloud"), with its API at `api.orchester.cloud/v1` (before, `orchester.cloud/api/v1`; [cloud-api.md](../cloud-api.md), "Base"), always taken from the environment: the public base URL, the SMTP relay and the PostgreSQL connection. Nothing is hard-coded; the SMTP relay and the database server are named at deploy time | user (2026-10-15, moved 2026-10-25, API host 2026-10-27) |
| 60 | Cloud backend | Rust with `actix-web` and PostgreSQL, the person's choice. `actix-web` joins the workspace for the cloud only; team servers stay on axum. The JSON API for the desktop stays | user (2026-10-15) |
| 61 | Cloud web front | A small Next.js server-rendered app with simple forms in Orchester's design: sign-up, sign-in, the email confirmation landing, forgot password and reset password; later account and devices. It talks to the backend's API. Its screens are designed in Pencil and approved by the person before it is built; backend work does not wait ([The cloud's web front](#the-clouds-web-front)) | user (2026-10-15) |
| 62 | The server admin | Each team server has a web app of its own at its address, server-rendered Next.js, admin only: first setup (the setup code, then linking to the owner's cloud account), projects (create by name and slug, rename, the remote hint), members and roles per project, invites (by email through the cloud, by code without the internet), devices (revoke), and settings (join approval, the cloud link and key, the certificate check code). Sign-in through the cloud identity, or a code for an offline server. It talks to the team server's own API from its server side only ([The server admin](#the-server-admin)) | user (2026-10-15) |
| 63 | What the desktop keeps | Settings › Orchester (cloud sign-in, connected servers, connecting with the check code), Project settings › Sharing (the link to a server project, the remote mismatch warning, members read-only, *Make local*, and *Manage members and invites*, which opens the server admin in the browser), joining, and the status bar item. Owner setup and member and invite management move to the server admin. **Clarified by the person:** "the desktop app stays the same, all admin actions are done in selfhosted-web". The desktop has no admin action at all (no project creation on a server, no member or role management, no invites, no revoking others' devices, no server settings, no owner setup); where one would be, it shows at most a link that opens the server admin. **Changes 9, 25, 43** | user (2026-10-15) |
| 64 | Shared web components | The cloud's web front and the server admin share one UI package: a `web/` workspace with `web/ui` (components, design tokens from the Pencil variables, i18n helpers), `web/cloud` and `web/server`; each app has its own locale files and shared components take their words from the app. AGENTS.md's code-quality, i18n and design-token rules apply to all of `web/` ([The web workspace](#the-web-workspace)) | user (2026-10-15) |
| 65 | Confirming asks for the password | Confirming an email asks for the password chosen at sign-up (`POST /auth/confirm { link_token, password }`), checked against the hash stored on that link, with the same work whether the link exists or not, rate-limited, and one answer for any failure. A repeated unconfirmed sign-up no longer ends earlier links: each link carries its own pending password hash and display name, and confirming one ends the rest. Reason: someone who signs up first with another person's email must not get their own password set when that person clicks a link ([Orchester Cloud](#orchester-cloud)) | user (2026-10-15, after a security review) |
| 66 | Modules are shared | ORC-48's modules belong to the server project while it is shared, as its tags do: name (unique without regard to case), description, start and target days, open or closed, each under a revision; a task's module is a field of the shared task, changed under the task's revision. The server calls core's `module` rules ([Modules](#modules)). **Extends 4 and 23** | the task's brief (ORC-78, 2026-10-22) |
| 67 | Who changes modules | Any member creates, changes, closes and reopens a module and moves any task in or out of one, as any member edits any task (row 18); there is no delete, as locally ([Modules](#modules)) | architect (ORC-78, 2026-10-22) |
| 68 | Module conflicts | A module change replaces its fields as a whole under the module's revision; a newer revision is `409 revision_conflict` with the current module, which the desktop refuses as core's *changed meanwhile* (a module has no field merge). A task's move merges like organization (row 26) ([Modules](#modules)) | architect (ORC-78, 2026-10-22) |
| 69 | Burndown and progress | Computed on each desktop, not served: core and the workspace already compute them from the tasks, their organization history and the moves, which every member's cache holds; serving them would put a second copy of those rules on the server, and their unit (story points, estimates) and the forecast's throughput are this computer's. The server keeps what they need: every move, with who and when ([Modules](#modules)) | architect (ORC-78, 2026-10-22) |
| 70 | Modules at Share, Join and Make local | Share and Join upload every local module; one of a name the server project has becomes that module, its id replacing the local one in its tasks and moves; the upload records each uploaded task in a module as a move at the upload's time; earlier moves stay local, as organization history does. Make local keeps the modules, each task's module and the moves ([Modules](#modules)) | architect (ORC-78, 2026-10-22) |
| 71 | Modules offline | Until S4 builds the outbox, module writes in a shared project fail at once offline, as task writes do; with S4 a create waits in the outbox with the id made on the desktop, a change with its revision (in `conflict` when met by a newer one), and a move in its task's change ([Modules](#modules)). **Under 27** | user (ORC-78, 2026-10-22) |
| 72 | Who changes the team's configuration | In a shared project, the team's configuration (pipelines, agent profiles, instructions, skills, tools of every kind) is changed only by admins: the project's owner or the team server's admin. Members read it and run with it, on the desktop and in the web app; they have no drafts of shared resources. Admins change it in both places; every write of S5's resource endpoints answers a member `403 not_admin`. Core decides it with `sharing::may_change_configuration` ([Shared configuration](#shared-configuration)). **Changes 30 and 52** | user (2026-10-29) |
| 73 | Snapshot and deltas | The project stream sends a client without a cursor, or with one older than the 30-day retention, a paged snapshot (`snapshot.begin`, `snapshot.tasks` pages of 64, `snapshot.end { cursor }`) with the project, members, tags, modules and tasks, later resources; a cursor inside the retention gets only the deltas after it, and `reset` goes away. Every delta carries `id`, `by`, `actor` (person or Orchester) and `at`; `task.changed` holds only the changed fields with `revision` and `base`, and a client applies it only when its cached revision equals `base`, otherwise it reads the task again; its own changes come back and are skipped by revision. The deltas are the record of who did what; there is no `activity` event, and the server's Activity feed stays. Not included: presence, typing, docs and plugins ([Events](#events)). **Extends 7 and 22; replaces the `task`, `members`, `tags`, `project`, `modules`, `resources` and `reset` kinds** | user (ORC-392, 2026-10-29) |
| 74 | Local pipelines in a shared project | In a shared project a person may keep pipelines of their own that stay on their computer and are never sent to the team server; the team's pipelines stay shared and admin-only (row 72). A local pipeline is a local resource of the project: anyone, member or admin, makes it, publishes its versions and runs it without an admin, and publishing it makes a local version only. Share, Join and sync never upload it. Its steps may use the team's profiles and the person's own local profiles. A shared task that names one keeps on the server only the pipeline's name and kind (local, whose), never its body; other members see *a local pipeline of <person>* and cannot run the task with it until they choose a team pipeline or one of their own. An admin may *Publish for the team*, which makes it a team resource from that version on. Make local keeps local pipelines as they are ([Local pipelines](#local-pipelines)). **Changes 30 and 52** | user (ORC-402, 2026-10-29) |
| 75 | What the web editor can pick | The web app's profile editor picks from what the server serves. **Network:** a profile's network access is `Off`, `Hosts(list)` (only those hosts) or `On`, ordered Off < Hosts < On; a list with a host the previous one lacked, or Hosts → On, widens (row 35); each CLI enforces it where it can, and where it can't the step refuses to start and says so, as for any permission it can't enforce. **References catalog:** the team server keeps, per shared project, the connections and MCP servers its members' desktops report, each as kind, name and role only; the web pickers list it, and a choice there is a reference by name and role that every computer binds as rows 30 and 35 say. **Models per role:** desktops report, per connection role, the model ids their connections list; the server keeps the union, and the web dropdown lists the role's models plus *Other model id…*. Nothing personal leaves a computer: no account, email, path, key, program or setting, and the reads do not say whose computer reported what ([What the web editor can pick](#what-the-web-editor-can-pick)). **Extends 30 and 35** | the task's brief (ORC-444, 2026-10-30) |
| 76 | Base branches are local | A task's and a module's base branch (the branch its runs start from and deliver into; `docs/architecture/storage.md`) is kept on each computer and never shared: in a shared project it is not sent to the team server, is not part of the task's or module's shared snapshot, revision or events, and teammates may choose differently for the same task or module. Setting it leaves a shared task's or module's revision alone; the app's screens and MCP tools set and read this computer's choice. Each run records the base it took, which stays local with the run. `orchester-api`'s shared task and module shapes do not change and the team server needs nothing. **Changes 52** | user (ORC-407, 2026-10-30) |
| 77 | Accepting an invite outside Orchester | **Accepting in the browser** (the person's requirement): one *Accept* on Orchester Cloud's Invites page ends with the membership made on the team server, without the desktop: the server account (and, for a project invite, the membership with the invite's role) and a sign-in of that browser to the server's web app. The cloud never reaches a team server; the person's browser carries a one-time identity token (purpose `accept`, bound to the invite, 5 minutes) to the address the cloud registered for the server, as `/authorize` does for sign-in, and brings the server's signed receipt back. It needs a two-factor session on the cloud, as every acceptance does (row 43). When the browser cannot reach the server either, nothing is used up: the invite stays accepted for that account until it expires, the page says why and offers *Try again* and *Open in Orchester*. **The `orchester://` link**: `orchester://invite/<invite id>` and `orchester://open`, registered per user by the Windows installer; an open window takes the link; a link only opens the invite's existing landing in Settings › Orchester and never accepts, joins or sends anything without the person's click, and nothing in it is ever shown or passed to an agent. The desktop is optional afterwards: the server and project show as joined, and a computer joins later with its own device ([Accepting outside Orchester](#accepting-outside-orchester)). **Extends 43 and 62** | user (requirement 2026-10-31); the flow is the architect's (ORC-576) |
| 78 | Local use and owners | The desktop needs no Orchester Cloud account for local work: the sign-in gate is gone, and sharing, cloud joining, invites and account servers offer the sign-in in place. A project or team server's `owner` NULL means this computer's: added with nobody signed in, it shows to nobody signed in and to every account, and no sign-in claims it. One added signed in is the account's, and waits (runners working) while that account is signed out. A person moves an unshared project between the two on All projects. A project and its server always have the same owner: a first link to the account's server moves a project of this computer's into the account and the page says so, an account's project is refused a server of this computer's, and a shared project doesn't move. Over MCP a project an account holds answers `sign_in_required` while nobody is signed in and `project_not_found` while another account is | user (ORC-132, 2026-10-30) |

## Processes and crates

```
orchester-app (desktop) ── HTTPS, pinned ──> orchester-server (team's network, Linux / Docker)
 │└─ orchester-runner                          └─ orchester-server.db (SQLite, WAL)
 │    └─ agent CLIs                             ╎ optional: invites, key refresh
 ├── HTTPS (identity, invites only) ──> orchester-cloud (run by the person)
 │                                        └─ PostgreSQL (the person's database server)
 └── browser: sign-up, sign-in, confirm, reset ──> Caddy ─┬─ /api/* ──> orchester-cloud (actix-web)
                                                          └─ else ────> web/cloud (Next.js, calls /api server-side)
```

- **Project data goes only between the desktop and its team server.** Orchester Cloud sees identities, devices,
  which servers exist and invites in flight, never a task, a run or a configuration (row 41). A team server needs
  the cloud only to send invites and to refresh the cloud's keys, and the desktop can carry both for it
  ([Orchester Cloud](#orchester-cloud)).
- **Only the desktop talks to the server.** It owns user-authored data already (`storage.md`, "Write ownership"),
  holds the server token, and learns about runs from the runner's IPC events. The runner keeps reading tasks and
  pinned versions from the project database and never sees a server token. Closing the desktop closes the runner
  (`storage.md`, "Lifecycle"), so nothing works on a shared task without the desktop being there to report it.
- **The project database is the cache and the outbox.** A shared project's tasks stay in the `tasks` table of
  `<repo>/.orchester/orchester.db`, and its shared configuration in the resource tables. Each row shows the person's
  own version: what the server confirmed, with the person's changes that wait in the outbox on top. Runs keep their
  foreign key to the task, the runner and every local screen keep working unchanged, online or not.
- **Sync logic lives in `orchester-workspace`**, the application layer both desktop apps share, kept free of UI. The
  apps show its state; they do not decide what is sent or when.

| Crate | Role | Depends on |
| --- | --- | --- |
| `orchester-core` | The shared-task rules (`sharing`), `TaskRevision`, `RepositoryIdentity`, ORC-39's `task::organization`, `ContentMerge` (`task::content`). New with 27–30: the outbox's ordering and holding rules (`sharing::outbox`), `TemporaryTaskKey`, the shared progress of a run (`sharing::progress`), and the references of a shared version (`sharing::references`, reusing `import`'s reference walk). No HTTP or database | as today |
| `orchester-api` | The JSON contract of the team server and of the cloud (`cloud` module): request, response, error and event types. The `client` feature adds the HTTP clients, the event stream reader and the pinned-certificate verifier. The `identity` feature adds identity tokens and signed key sets (Ed25519 through `ring`), signing for the cloud and verifying for the team server. Gains the SSE parser, moved out of `orchester-agent` (see Dependencies) | core, serde; `client`: reqwest, rustls; `identity`: ring |
| `orchester-server` | Binary `orchester-server`: HTTP API, SQLite storage, accounts, devices, codes and invites, its own certificate, the cloud link, event stream, host command line; with `proxy_certificate = "auto"`, reading its proxy's certificate at its own address ([Codes](#codes)) | core, api (`client`, `identity`), axum, rusqlite, rcgen |
| `orchester-cloud` (new, `crates/cloud`) | Binary `orchester-cloud` on `actix-web` (row 60): the JSON API; accounts with sign-up, sign-in by email and password, confirmation and reset by email, devices, the team server registry, invites in flight, the signing keys and identity tokens, email sending | core, api (`identity`), actix-web, tokio-postgres, deadpool-postgres, tokio-postgres-rustls, argon2, lettre; later reqwest |
| `orchester-store` | The cache tables, the outbox, the shared-resource tables and bindings of the project database; the `servers` and `cloud_account` tables of the application database | as today |
| `orchester-workspace` | The cloud session, invites and the device's servers (`sharing::cloud`, new), connecting a project and the connection monitor (`sharing`); shared projects, Share, Join, Make local, claims (`sharing::project`); the sync engine (`sharing::sync`, new): outbox sender, event stream, apply, temporary keys, run progress, shared configuration; the sync state the screens show | api (`client`), store |
| `orchester-app` | The screens of the new app (row 63): Settings › Orchester (the cloud account, invites, connected servers, connecting with the check code), Project settings › Sharing (the link to a server project, members read-only, *Manage members and invites* opening the server admin), Join, the shared task list and task page, the status bar's server item ([server-new-app-plan.md](server-new-app-plan.md)). No S2a Settings › Servers | workspace |
| `orchester-desktop` | Deprecated and frozen (row 31). Keeps its S2a–c screens over the same workspace service, unchanged; they go stale after S4 by design | workspace |

**Why these crates.** The team server is a real boundary (AGENTS.md: "for example the server"). The cloud is
another: a separate program, run by a different party (the person, not the team), on the internet, with its own
data. Merging it into `orchester-server` would put internet-facing sign-in and email into every team's air-gapped
binary. The contract is shared by the three programs, as `orchester-ipc` is between desktop and runner, so a change to
it breaks the build on every side at once. The `client` feature keeps reqwest out of the builds that need no client;
the team server takes it for `proxy_certificate = "auto"`, which reads its own public address with the app's
trust-store verifier. The `identity` feature keeps signing and verifying out of the desktop, which only carries tokens.

**Core stays free of HTTP and databases.** The server reuses core's rules (task completeness, queueing, the edit
lock, `Name`, `TaskLimits`, the organization rules, claim transitions, staleness, resource body validation per kind) by
calling them; storage and HTTP live in `orchester-server`. Core only gains types and pure functions.

### Dependencies (AGENTS.md rule 17)

Each goes into `[workspace.dependencies]` with its reason. Decisions 27–38 add none; 39–54 add `rcgen`, `lettre`,
`tokio-postgres`, `deadpool-postgres`, `tokio-postgres-rustls` and `actix-web`, and move `argon2` from the team server
to the cloud. The web front's Node packages are in `web/cloud/package.json`, outside Cargo.

| Crate | Where | Why it is needed |
| --- | --- | --- |
| `axum` | server | HTTP routing, JSON extraction and SSE responses on tokio; built on `hyper` 1, already in the lock file through lurq |
| `hyper`, `hyper-util` | server | HTTP/1 header/idle deadlines, bounded connections, Tokio timer/IO and the axum service adapter; already in the lock file |
| `argon2` | ~~server~~ cloud | Password hashing (argon2id) moves with passwords from the team server (row 46) to the cloud (row 48); S1a's hashing pool and limiters move with it |
| `tokio-rustls` | server | TLS; same `rustls` the client brings, no OpenSSL on any platform |
| `reqwest` (rustls, OS trust store) | api `client`, cloud, server | The HTTP client already chosen for `orchester-agent`; later, the cloud uses it for Google's and GitHub's token endpoints. The team server uses it through `orchester-api`'s client to read its proxy's certificate at its own address (`proxy_certificate = "auto"`) |
| `rustls` | api `client` | Already a direct dependency for its certificate errors; now also the verifier that accepts exactly a pinned certificate (row 39) |
| `ring` | api `identity` | Already in the lock file (rustls' provider): Ed25519 signing and verifying, HMAC for codes (later, RSA verifying of Google's ID tokens). No JWT crate: the token is a few lines of base64url JSON |
| `rcgen` (new, ring backend) | server | Generates the team server's own certificate on first start (row 39); a certificate is not something to write by hand |
| `actix-web` (new, no TLS feature) | cloud | The cloud's HTTP server, the person's choice (row 60): routing, JSON extraction, request size and time limits, on its own Tokio-based runtime. TLS ends at Caddy, so it needs no TLS feature. The team server stays on axum |
| `lettre` (new, rustls, SMTP only) | cloud | Sends confirmation, password reset and invite emails through the person's SMTP relay; SMTP with TLS and auth is not a few lines |
| `tokio-postgres` (new) | cloud | The PostgreSQL client (row 54): async on the workspace's tokio, plain SQL with typed parameters, no ORM, no macros, no compile-time database. Chosen over `sqlx` (compile-time checked queries need a live database or an offline cache at build time, and bring a large macro and driver stack) and `diesel` (an ORM and a DSL beside plain SQL everywhere else in the workspace). `uuid` and `serde_json` support through its features |
| `deadpool-postgres` (new) | cloud | A small pool of `tokio-postgres` connections, with its connect and wait timeouts; writing a correct async pool is not a few lines |
| `tokio-postgres-rustls` (new) | cloud | TLS to PostgreSQL through the `rustls` already in the workspace, so no OpenSSL on any platform |

Tokens come from `getrandom` and are hashed with `sha2`. The configuration is TOML. The command line is parsed by
hand. The server enables rusqlite's `backup` feature for the host's online backup command.

**The SSE parser.** `orchester-agent` already has one (`crates/agent/src/sse.rs`, bounded events). S2d moves it to
`orchester-api` (module `sse`, no feature, no I/O) and `orchester-agent` uses it from there; nobody writes it twice.

### Framework and storage

- **axum**: tokio-native, on the hyper already built for lurq, with SSE and JSON built in and no macro DSL.
- **SQLite for team servers, not Postgres.** One binary and one data file, nothing else to run or back up, and the team already
  knows rusqlite, WAL and the forward-only migrations of `orchester-store`. Writes are small and rare (a task edit, a
  claim, one heartbeat per person per minute, a published version). SQLite serializes them, which also makes claims,
  numbers and version numbers atomic without extra locking. Postgres becomes worth it only for several server
  instances; there is no trait for the storage now (rule 8).
- One write connection behind a `parking_lot::Mutex`, used only inside `spawn_blocking`, and a few read connections.
  Time comes from one `Clock`: the system clock in the binary, a settable clock in the server's tests.
- **The cloud on actix-web** (row 60). Its handlers call the cloud's own modules, as the team server's call its
  `sharing`; blocking work (argon2) runs on actix's blocking pool behind the same bounded queue as S1a. Request
  limits mirror the team server's: 1 MiB bodies, 10-second header and body deadlines, keep-alive 10 seconds, a cap on
  connections; JSON errors in the team server's `Error` shape.
- **The cloud is on PostgreSQL** (row 54). It serves everyone, so it runs as a service on the person's database
  server, which already brings backups, replication and point-in-time recovery, and it may run as more than one
  instance later. The cloud reads and writes through a `deadpool-postgres` pool of `tokio-postgres` connections (TLS
  through rustls), with plain SQL and typed parameters. Rules that need atomicity use transactions at `SERIALIZABLE`
  (with a bounded retry on `40001`) or a single statement (`INSERT … ON CONFLICT`, `UPDATE … WHERE … RETURNING`):
  marking a link or an identity token used, accepting an invite. Time still comes from one `Clock`, never from the
  database's `now()`, so the settable clock works in tests.
- **The cloud's migrations** are forward-only SQL files compiled into the binary (`crates/cloud/migrations/0001_*.sql`
  and on), applied at start in one transaction each under `pg_advisory_xact_lock`, so two instances starting together
  apply them once, and recorded in `schema_migrations` (version, applied at). A database at a newer version than the
  binary knows is refused at start. No migration crate: the runner is a few dozen lines, as `orchester-store`'s is.
- **Limiters in v1.** The cloud runs as one instance in v1, and its rate limiters stay in memory as S1a's do; a second
  instance would move them into PostgreSQL first.

## The desktop's trait boundary

**`TaskStore` is the boundary.** The desktop's task code is generic over it. A shared project gets
`RemoteTaskStore` (`orchester-workspace`, `sharing::project::store`). Every desktop write goes through `TaskWriter`
(local or shared), in both apps and in the MCP tools.

| Method | `RemoteTaskStore` (after S4) |
| --- | --- |
| `tasks`, `task` | The person's view from the project database: the server's last confirmed task with the person's waiting changes on top |
| `create_tasks` | Writes each task locally with a temporary key and records one outbox entry (an import of tasks that keep their id); when the server can be reached the entry is sent at once and the task returned with its number |
| `update_task`, `save_task` | Records the fields that differ from the person's view, with the revision read, writes the person's view and records an outbox entry. When the server can be reached it is sent at once and the answer returned (`Saved` or `Conflict`, as today); when it cannot, the result is `Saved` with the change waiting |

`TaskStore` stays synchronous, called on blocking workers, never on the UI thread. Offline, the write methods no
longer fail: they record and return. What waits is visible through the sync state ([What the person
sees](#what-the-person-sees)). The `TaskUpdate { Saved, Conflict }` result is unchanged; a conflict found later, when
a waiting change is sent, is a state of the task, not a return value.

**`ResourceStore` for shared configuration** (row 30). A shared project's resources go through a
`RemoteResourceStore` beside `RemoteTaskStore`: `save_draft` and `draft` stay local (drafts are not shared),
`publish_draft` records an outbox entry for a new server version, and the read methods read the cache. No new trait
(rule 8): the existing `ResourceStore` is the boundary, as `TaskStore` is for tasks.

**No new trait for claims, heartbeats and progress.** They exist only for shared projects. On `TaskStore` they would
need a local implementation that does nothing, a silent no-op (rule 6); a trait of their own would have one
implementation (rule 8). The workspace calls the `orchester-api` client for them directly. Their rules
(`Claim::take`, `release`, `take_over`, `heartbeat`, `is_stale`, the run phases that end a claim, the offline start
rule of row 28) live in `orchester_core::sharing`, so the server enforces and the desktop displays the same rules.

**Organization writes.** Organization travels in the same task change as content, under the revision, and takes the
same path through the outbox. Tags go through the tag endpoints ([Tags](#tags)).

**Pipelines are shared now** (row 30). A shared task names its pipeline by slug, and the slug resolves to the shared
pipeline every member has. A task names a pipeline no published version exists for only while it is a draft. The
pinned version number is now meaningful on every computer: it is the server's. Whoever starts a run pins the latest
published version, as Run again does today. Before row 30, a member who lacked the pipeline saw *missing on this
computer*; that state now applies only to what a version names and this computer lacks ([Shared
configuration](#shared-configuration)).

## Claims

A claim says who has taken a task into work. A task has at most one; the server keeps it and its history.

| Event | Effect |
| --- | --- |
| *Take* (manual) | Takes the claim if the task has none: an atomic check-and-set on the server. Held by someone else: refused with the holder, since when and whether the claim is stale. Needs the server |
| Starting work on a shared task: Create & run, Start, Run again, continuing a paused or interrupted run, Send & continue, a follow-up message | Online: the desktop takes the claim first, or confirms that this person holds it, then asks the runner. Held by someone else: nothing starts and the desktop names the holder. Offline: see *Offline* below. Pause and cancel never need the claim |
| Heartbeat | Every 60 s while the desktop runs and is signed in, one request per server names the claimed tasks in the projects this desktop has joined, except those whose latest run here was merged or cancelled (their claim ends). The server refreshes those the caller still holds and returns the ones they lost. The desktop records the server time of the last answered heartbeat per server |
| Stale | No heartbeat for 15 min. Shown as stale, not as free. The server sends a `task.claim` delta (`stale`, `refreshed`) when a claim becomes stale and when a stale claim is refreshed, so watchers never compute staleness from an old heartbeat ([Live updates](#live-updates)) |
| *Take over* | Only a stale claim, after an explicit confirmation in place that names the holder and the last heartbeat. The request names the claim it saw; if that claim changed or was refreshed meanwhile, the request is refused. The takeover is recorded (from, to, when) and the old holder is told on their next connection. Needs the server |
| *Release* | By the holder, while none of their runs of the task has an agent at work. Offline it waits in the outbox; the claim stays the person's until the server took the release |
| Run cancelled | The claim is released (the cancelled progress is reported; waits in the outbox while offline) |
| Run merged | The claim is completed: the task shows *Merged* with who merged it. A merged task cannot be claimed again. A run whose work a person merged without Orchester counts as merged |
| Run succeeded or failed | The claim is **kept**: review and merge, Send & continue and Run again all happen on the holder's computer, where the run's worktree is |
| Member removed or user disabled | Their claims are released, and the release is recorded |

**Ending claims on the desktop (ORC-284).** On every run change and every sync the desktop goes over the claims the
person holds in each shared project (`sharing::project::claim_ends`) and applies core's `claim_end` to the phase of
each task's latest run here, read as the task page reads it (a succeeded run whose work git finds merged into its base
branch is merged): a merged one is reported with `PUT …/run` (`merged`), which completes the claim; a cancelled one
releases it with `DELETE …/claim`; succeeded, failed and every other phase keep it. Only a claim taken before its
run ended is that run's to end: one whose `since` is later than the run's end (the merge, the cancel, or for work merged
without Orchester its last event), such as one the person's other computer took since, is kept, and so is one that
work on this computer asked for after the run ended (Run again, before the runner records the new run; the desktop
remembers when, for the session, and forgets it when that work does not start). Claims that were never ended
before (ORC-281) end this way on the first sync of a fixed desktop. A server without `PUT …/run` answers `404` or
`405`: the report fails as *not supported* and is logged, and the next sync tries it again; meanwhile the heartbeat
no longer names the task, so the claim goes stale instead of staying fresh.

**Offline (row 28).** Without the server, work may start on a task only when the cached claim is the person's and is
not stale by the person's own last answered heartbeat. Core decides it with one function over explicit times:
`held_offline(claim, me, last_answered_heartbeat, offset, now)`, where *fresh* means the later of the claim's
heartbeat and the last answered heartbeat is less than 15 minutes ago, in server time.

| Offline, the task's cached claim is | Starting or continuing work |
| --- | --- |
| The person's, fresh | Starts. The run's progress waits in the outbox |
| The person's, stale by the last answered heartbeat | Refused: *Your claim on ORC-12 went stale at 14:47 while Kontur team could not be reached; another member may have taken it over. Starting needs the server.* A run already working goes on: Orchester never stops work on its own |
| Someone else's, or none | Refused: *Taking a task needs Kontur team.* |
| A task created offline (temporary key) | Refused: it has no claim yet. *Create & run* only saves it (row 32). Once it has its number and the server can give the claim, the task and the temporary-key notice offer *Start*; pressing it takes the claim and starts the run. Nothing starts on its own |

On reconnect the first heartbeat refreshes the person's claims; a stale claim that nobody took over is the person's
again. One that was taken over meanwhile is the lost-claim case below: the run that started offline goes on, its
progress reports are refused and dropped, and the person is told who took it and when.

**Two computers.** A heartbeat from any computer where the person has joined the project keeps all their claims in
it fresh, even while the run is on another computer that is off (row 22). Offline, a computer cannot see the other
computer's heartbeats, so it judges by its own last answered one; this is the safe side.

**Editing.** A claim does not lock the task: any member may edit it, and the revision keeps two edits from
overwriting each other (row 18).

**Lost claim while working.** A person offline for more than 15 minutes can find their claim taken over. Their local
run goes on. Their desktop tells them who took it, and the server refuses their progress reports for that task.

**Time.** Staleness is decided by the server's clock. Responses carry the server time (`Orchester-Server-Time`); the
desktop keeps the offset and computes the offline rule with the same core function.

## Task organization

ORC-39's organization (`task::organization`) is shared with the task (rows 23–24). The server mirrors it field for
field and calls core's rules; it adds no rule of its own.

| Part | Shared as |
| --- | --- |
| Workflow status | `workflow`: Backlog, Todo, In progress, In review, Done, Canceled. Canceled is refused while the task's run has not ended (`stop_run_first`) |
| Priority | `priority`: Urgent, High, Medium, Low, None. For lists only, never for the runner's order |
| Blocking | `blocking`: links to other tasks of the project, each with its gate (Review: the blocking task's run succeeded or merged; Merge: merged). A link that would close a cycle is refused. Only an unfinished link keeps a task from running |
| Blocked reason | `blocked_reason`: the manual flag, a non-empty reason; informational, it never stops a run |
| Tags | `tags`: ids of the project's tags ([below](#tags)) |
| History | One row per change: before and after, the tag names at the time, when, by whom, and `person` or `orchester` |

- **Who changes it.** Any member, as any field of the task (row 18): the change names the revision it was made on.
  Orchester's own changes (the workflow status that follows a run: In progress when it starts, In review when it
  succeeds, Done when it is merged) are sent by the holder's desktop with `actor: "orchester"`. They are ordinary
  outbox entries now; S2b's `shared_automatic_status` queue becomes part of the outbox (S4).
- **Conflicts** (row 26). A change that meets a newer revision gets `409 revision_conflict` with the current task.
  The desktop merges the organization fields with `TaskOrganization::rebase` and sends again; only content fields both
  people changed reach the person. A bulk change is one change per task, each merged the same way.
- **Offline** (row 27). Organization changes are made locally and wait in the outbox like every other change; they
  merge on send without asking.
- **Join.** The review list uploads each ticked task with its organization (tags by name, links inside the upload or to
  tasks already on the server); *Not shared* tasks keep their tag names until *Share task* maps them the same way.
  **Make local** turns the server project's tags into the project's local tags (ids kept) and keeps every task's
  organization as the person last saw it.

### Tags

Tags belong to the server project and replace the project's local tags while it is shared. Members create, rename,
merge and delete them with ORC-39's rules: a name is a core `Name`, unique in the project without regard to case
(`409 tag_exists`); merge moves every assignment of a tag into another and removes it (`409 same_tag` for itself);
delete removes the tag from every task. Merge and delete rewrite each affected task in one transaction, raise its
revision, write its history row and send its `task.changed` delta, then a `tag.merged` or `tag.deleted` delta
(row 73).

**Offline.** Creating a tag works offline: the tag gets a temporary id locally, the create waits in the outbox
(with an idempotency key), and task changes that assign it wait behind it. When the server answers, or answers
`409 tag_exists` with the existing tag of that name, the temporary id is replaced in the cache and in every waiting
entry. Renaming, merging and deleting a tag rewrite other people's tasks, so they need the server, like taking a
task (row 33). Offline, Manage tags offers only *New tag*; the other actions are unavailable with *Needs Kontur
team*.

### Member roles

Owners (and admins) change a member's role with `PATCH …/members/{user}` (row 25), in the server admin (row 63);
the desktop shows members and roles read-only. The last owner can be neither
made a member nor removed (`409 last_owner`). Role changes are audited and send a `member.role_changed` delta
(row 73). Members and roles need the server; they never wait in the outbox.

## Modules

ORC-48's modules (`orchester_core::module`: epics, milestones, sprints that group tasks, with progress and a burndown)
belong to the server project while it is shared, as its tags do (rows 66–71, ORC-78). Every member sees the same
modules and which tasks are in them. The server calls core's module rules and adds none of its own.

| Part | Shared as |
| --- | --- |
| Module | `Module`: id (made by the desktop that creates it, UUIDv7), `name` (a core `Name`, unique in the project without regard to case), `description` (trimmed), `start` and `target` (local days, `YYYY-MM-DD`, each optional; a target before the start is refused by core's `ModuleDetails::new`), `state` (`open`, `closed`), `revision`, created and updated by and at |
| A task's module | `module`: a field of `Task`, `TaskContent` and `Changes`, changed under the task's revision like its organization; at most one module per task. Like organization it changes whatever the task's status (a queued task too) |
| History | Every move of a task into a module, out of one or from one to another: `ModuleChange { sequence, task, from, to, by, actor, at }`, append-only, `actor` `person` or `orchester` as for organization changes. The burndown reads it |
| Progress, burndown | Not served: each desktop computes them from the cached tasks, their organization history and the moves (row 69) |

- **Who changes them** (row 67). Any member creates a module, changes it and closes or reopens it (closing is
  `state: closed`); there is no delete, as locally. Any member moves any task in or out of a module (row 18).
  Admins who are not members cannot, like every task write.
- **Revisions** (row 68). A change names the module's revision it was made on and replaces its fields as a whole;
  a mismatch is `409 revision_conflict` with the current module. The desktop then caches the current module and
  refuses the change as core's `ModuleError::ChangedMeanwhile`, as a local edit over a stale form is refused: a
  module has no field merge. A task's move that meets a newer task revision is put on the task as it is now and
  sent again without asking, as organization is (row 26); only the move is sent.
- **Refusals.** `409 module_exists { module }` (the name is taken, without regard to case), `422 module { reason }`
  (`target_before_start`; `module_not_found` for a task's module the project does not have), `422 invalid` (a name
  or a day that is not one), `409 id_taken` (the id is the project's already). The desktop says each as core's
  `ModuleError` for a local project, so a refusal reads the same in both.
- **Idempotency.** A module create and a module change carry an idempotency key; the server keeps the first answer
  per (project, user, key) for 30 days in `change_keys`, as [Idempotency](#idempotency) says, and answers a repeat
  with it. A move travels in the task change (`PATCH …/tasks/{task}`), whose key comes with S4 (N6a).
- **Events.** ~~A create, a change and an upload that adds modules send a `modules` event with the project's
  modules; a move sends the task's `task` event.~~ **Replaced by 73:** a create sends `module.created` and a change
  `module.changed`, each with the module; an upload sends `module.created` for each module it adds. A move sends the
  task's `task.changed` with `module`; the moves themselves are read with
  `GET …/modules?after=<the last sequence the cache has>`, which S2d's stream does when a `task.changed` changes a
  task's module ([Events](#events)).
- **Share** (S2b, row 70). The first checkout's link uploads every module of the project (`modules` in
  `POST …/link`) and each task's `module` in `UploadTask`. An uploaded module keeps its id, or becomes the project's
  module of its name when the project has one (`mapped` in the answer lists them). The server records each uploaded
  task in a module as a move into it at the upload's time, by the uploader, so every member's burndown counts it; the
  moves made before stay on the uploader's computer only, as organization history does ([Uploads](#endpoints)).
  The desktop takes the server's modules in before it links the project; a local module that became the module of
  its name takes that module's id in its tasks and its moves.
- **Join** (S2c, row 70). The import uploads every local module, whatever is ticked, so no module stays only on this
  computer; a module of a name the server project has becomes that module, as at Share. *Not shared* tasks keep their
  module (with its server id); *Share task* uploads the task with its module.
- **Make local** (row 70). The server project's modules become the project's local modules with their ids, every
  task keeps its module, and the moves the cache holds stay its history; the module revisions are dropped.
- **Offline** (row 71). Until the outbox is built (S4), a module write in a shared project fails at once while the
  server cannot be reached, as every task write does today, and nothing is queued. With S4, a module create waits in
  the outbox (`create_module`, with its id made here, so nothing needs a temporary id and task moves into it wait
  behind it); a module change waits with the revision it was made on (`change_module`) and, met by a newer one, is in
  `conflict` for the person; a move waits in its task's `change_task` entry and merges like organization.
- **What the desktop keeps.** The project database's `modules`, `task_modules` and `module_changes` are the cache:
  the server's modules with their revision (`modules.revision`, empty for a local project), each cached task's module,
  and each move the server kept with its sequence there (`module_changes.server_sequence`). They are written only from
  what the server confirmed; the local writes of modules refuse a shared project.

## Shared run progress

S3's live run status, extended by row 29. The holder's desktop reports the progress of the task's latest run whenever
the runner says that run changed (`Event::RunChanged`, `WorkSummary`) or its pull request changed. Only the holder's
reports are accepted. Core derives the report from the local run (`sharing::progress::of(run, steps, merge, pull
request)`), so every app reports the same thing.

| Field | Content |
| --- | --- |
| `holder` | Who took the task (the claim's holder) |
| `number` | The run's number on the holder's computer (`#3`); shown with the holder's name, since run numbers are per computer |
| `phase` | `queued`, `running`, `waiting`, `stuck`, `paused`, `stopped_at_limit`, `interrupted`, `succeeded`, `merged`, `failed`, `cancelled`: core's `TaskPhase` |
| `step` | The current step: its key and name in the pinned pipeline version, or none |
| `waiting_for` | What the run waits for, a kind only: `person` (a stuck run, a plan decision, a step returned to a person), `limit` (a token or time limit, an account's plan limit), `question` (an agent asked), `approval` (a tool call awaits access approval); none while it does not wait. Never the question, the plan, the approval's action or any text |
| `steps` | Each step of the pinned version in order: key, name, and its result in this run: `passed`, `failed`, `unverified` (it could not check everything it is for), `unchanged` (row 34: not run again because its latest failed or unverified result was for the revision it would run on now and nothing a person asked for changed since), or none while it has not finished |
| `branch` | The run's branch name (`orchester/orc-41-run-2`) |
| `pull_request` | The run's pull request when it has one: number, URL, state (`open`, `draft`, `merged`, `closed`) |
| `started_at`, `phase_since`, `updated_at` | Milliseconds since the Unix epoch, UTC |

Other members see the task's progress from this report (*Running · Review · Alice · since 14:02*, the step rail with
each step's result, *Waits for a person*, the branch, the pull request link). For their own runs, the desktop keeps
deriving everything from the local run, as today.

**Offline.** A report is a state, not a change: the outbox keeps only the latest report per task, and it is sent on
reconnect after the task changes recorded before it. A report the server refuses because the claim was taken over is
dropped and the lost-claim notice shows.

**What the server never stores about runs:** transcripts, prompts, messages, plans, questions and their answers,
approvals' actions, diffs, file names, commits, worktree names, tool calls, token usage, agent or model names,
environment variables or secrets. The fields above are all it gets.

## Shared configuration

Rows 30, 52, 74 and 75. Pipelines, agent profiles, instructions, skills and tools of every kind (command, HTTP,
database) are shared resources of the server project. What a tool needs that belongs to the person (its secrets, the
MCP servers and connections around it) stays on each computer and is bound there. Besides the team's pipelines, each
person may keep [local pipelines](#local-pipelines) that are never sent. The web app's editor picks connections, MCP
servers and models from a catalog the desktops report ([What the web editor can pick](#what-the-web-editor-can-pick)).

| Part | Where it lives |
| --- | --- |
| Resource (id, kind, slug, name, archived) | Server; cached locally. Ids are the sharer's `ResourceId`s, kept on upload as task ids are |
| Published versions (`v1..vN`, immutable) | Server; numbered by the server; cached locally with who published them and when |
| Draft | Local only, per admin, based on a server version. Publishing sends it. A member has none (row 72) |
| A person's local pipeline, its drafts and versions (row 74) | Local only, per person; never sent. A shared task that names one keeps only its name and whose it is |
| Connections, CLI accounts, MCP servers and their credentials, secrets, environment | Local only, per person |
| Catalog: connection and MCP server names and roles, model ids per role (row 75) | Server, the union of the desktops' reports; nothing else about them is sent |

**Who publishes** (row 72). Only admins change the team's configuration: the project's owner or the team server's
admin. They edit drafts, publish versions, create, import, rename and archive resources and upload their files, on
the desktop and in the web app. A member reads every resource and its versions and runs with them, but has no drafts
of shared resources; every write of the resource endpoints answers a member `403 not_admin`. The server and the
desktop decide it with one core function, `sharing::may_change_configuration`, from the person's project role and
whether they are the server's admin.

**Publishing.** *Publish* sends the draft as a new version based on the version it was edited from. The server gives
it the next number when that base is still the latest. If someone published meanwhile, the server refuses with the
latest version (`409 version_conflict`), and the person decides: open their draft on top of the newer version, or
publish theirs as the next version anyway. Bodies are never merged automatically. Offline (row 36), a publish waits
in the outbox. The version is usable by the person's own new runs at once, shown as *v? · published here, not sent*;
the runs keep their snapshot of it, so its later number or a conflict changes nothing for them. After reconnect it is
sent; if someone published from the same base meanwhile, the person decides as above.

**A teammate's version that widens** (row 35). A version published from another computer applies here at once
unless it widens what this computer's agents may do compared with the version this computer uses now. Then runs keep
using that previous version, and the resource shows *v5 widens what agents may do here* with what it adds and
*Use v5 here*. *Use v<N> here* is person-only: no MCP tool and no synthetic input can press it, the rule of *Let
automation change agent permissions*. Core decides *widens* with one pure function over the two versions, reusing the
comparison the MCP person gate already makes:

- an agent profile whose permissions are broader (repository access, commands, network: up the order Off < Hosts <
  On, or a host list that gains a host, row 75), or that references an MCP
  server the previous version did not;
- any version that references a secret the previous version did not;
- a tool whose program, arguments or environment (command), address, method or headers (HTTP), database or
  query, or access policy (allowed pipelines and steps, approval mode, access window) changed;
- a first version received from someone else counts as widening when it grants anything of the above.

A version this computer published itself never waits. A version that waits blocks nothing else: pipelines keep
pinning the version this computer uses.

**The Orchestrator** (row 35) always proposes a card for publishing a shared resource; *Let the Orchestrator publish
drafts* covers only local resources. The card says the version is published for everyone on the server.

**What a version names on each computer.** A body refers to connections, MCP servers and secrets by the
publisher's ids (other resources, tools included, are shared and keep their ids). A published version carries one
**reference** per such id: its kind, its name, and its role, which is the connection's kind (`claude_cli`,
`codex_cli`, `openai_api`, …) or the secret's kind. Each computer keeps a **binding** per reference: the local
connection, MCP server or secret it means here.

| Reference kind | Bound automatically to | Otherwise |
| --- | --- | --- |
| Connection | The local connection of that role with that name (without regard to case); else the only local connection of that role, marked *by role* | *Missing on this computer*; the person picks one of that role |
| MCP server | The local MCP server with that name, only after the person confirms it once (it widens what agents may do) | *Missing on this computer*; the person picks one |
| Secret | The local secret with that name, only after the person confirms it once (a tool would send it) | *Missing on this computer*; the person picks one |

Binding an MCP server or a secret is a person-only action: no MCP tool and no synthetic input can make it, the same
rule as *Let automation change agent permissions*. A binding stays until the person changes it or the local entity is
removed; later versions that keep the reference reuse it.

**Resolving.** The cache holds each version twice: the server's body, unchanged (for its hash and for display), and a
local body with every bound reference replaced by the local id. Runs pin and read the local body, so the runner is
unchanged. A reference with no binding keeps the publisher's id, which matches nothing here, so the existing
preflight reports it as it reports an import copy's unresolved reference today: the resource shows *missing on this
computer: Claude Code (CLI) "Work"*, and pipelines that need it cannot run here until the person binds it. When a
binding changes, the local bodies of the cached versions that use it are rewritten; they are a cache, not authored
data. Core does the walk with `import`'s reference functions; it is not written twice.

**Publishing from this computer** turns local ids back into references: an id that is bound to a reference uses that
reference; any other local connection, MCP server or secret becomes a new reference with its kind, name and
role. Nothing about it is sent beyond those three words: never its settings, account, credentials, values or
program.

### What the web editor can pick

Row 75. The web app's profile editor (Orchester Platform OP-51, frames C10, C11, C17 and C18 of its
`web/design/orchester-web.pen`) has no computer of its own: it lists what the team server serves. Three of its controls
need more than the contract above.

**Network: Off / These hosts / On.** Core's `NetworkAccess` (`permission/mod.rs`) gains a third value between its two:

| Value | Stored form | Agents may reach |
| --- | --- | --- |
| `Off` | `"off"` | nothing |
| `Hosts(list)` | `{ "hosts": ["api.example.com", "*.example.org"] }` | only the listed hosts; `*.` covers the domain's subdomains |
| `On` | `"on"` | any host |

They are ordered Off < Hosts < On. A host is a lowercased name or address with no scheme, port or path; the list is
sorted, without duplicates, not empty, at most 64 hosts (core's constructor refuses anything else, the server with
`422 invalid { reason: "network" }`). For *widens* (row 35, and the MCP person gate's comparison, which is the same
function) a change widens when it moves up the order, or when a `Hosts` list gains a host the previous list did not
cover; narrowing a list, or On → Hosts → Off, never widens. Each CLI enforces the value where it can, through its
connection's own sandbox or network settings for the step. Where a CLI cannot limit the network to hosts (or cannot
turn it off), the step refuses to start and says which permission this connection cannot enforce, as the preflight
does today for every permission a connection cannot enforce; Orchester never runs it with wider access instead.

**References catalog.** The team server keeps, per shared project, the connections and MCP servers its members'
desktops report: for each, its kind (connection or MCP server), its name and, for a connection, its role (the
connection's kind: `claude_cli`, `codex_cli`, `openai_api`, …; an MCP server has none, as in core's `sharing::Reference`).
Never its settings, account, program,
arguments, address, environment or credentials, and secrets are not reported at all. The catalog is the union of the
latest report of each device of each current member, without regard to case of the name: one entry per kind, role
and name, with no count and nothing that says whose computer has it. A device's report replaces its previous one; a
revoked device's or a removed member's entries leave with it. The web's connection and MCP server pickers list the
catalog for the reference kind and, for a connection, the role the step needs. A choice there writes a reference by
kind, name and role into the version, with a fresh reference id the web mints, the same shape a desktop publish sends.
Nothing changes on the computers: each binds the reference as [What a version names on each
computer](#shared-configuration) says (rows 30 and 35): a connection by name, else by role; an MCP server only after
the person confirms it; *missing on this computer* otherwise.

**Models per role.** Models come from each computer's CLI (`connection/catalog.rs`). The desktop reports, per
connection role, the model ids its connections of that role list, ids only: no display names, account tiers or
limits. The server keeps the union per role over the same latest reports. The web's model dropdown lists the models of
the role the step's connection reference has, sorted, plus *Other model id…*, which takes any id. A model id is a
string in the profile as today; a computer whose CLI does not know it fails the step as it does today for an unknown
model.

**Reporting.** The desktop sends `PUT /projects/{id}/catalog` for every shared project it is linked to: after
joining, after each connect, and when a connection, an MCP server or a connection's model list changes (at most once
a minute, the latest state winning). The body is the whole state, so a lost report costs nothing; offline it is not
queued in the outbox, only sent again after reconnect. Members and admins both report and both read
(`GET …/references`, `GET …/models`): the catalog is what the editor offers, not a permission. The references read
also lists the connections and MCP servers the project's versions already name, so a reference a version carries
stays pickable after the desktop that reported it leaves.

**Nothing personal leaves a computer.** A report holds kinds, names, roles and model ids only: no account, email,
user name, path, URL, key, token, program or setting. Names are the person's own labels for their connections and MCP
servers, the same words a published version already carries (row 30). Core builds the report from the local
connections and MCP servers with one function, so no other field can reach it.

### Local pipelines

Row 74 (the person, 2026-10-29). In a shared project a person may keep pipelines of their own beside the team's. The
team's pipelines stay shared and admin-only (row 72); a local pipeline stays on the person's computer and is never
sent to the team server.

- **Kind.** A local pipeline is a local resource of the project, as every resource of a project that is not shared.
  Making it, editing its draft, publishing its versions, archiving and running it need no admin: members and admins
  alike. Publishing it makes a local version only (numbered on this computer), never a server version and never an
  outbox entry. The resource's `sharing` is empty, as for a local project's resources; core's
  `sharing::may_change_configuration` answers only for shared resources.
- **Share and sync.** Share's `POST …/resources/import`, Join's upload and sync never upload a local pipeline: the
  upload takes only shared resources, and nothing else about a local pipeline reaches the server. Share and the Join
  review list it under *Stays on this computer*, and *What sharing sends* says *Pipelines you keep only on this
  computer* among what stays here. At Join, *Keep on this computer* on a pipeline keeps it as a local pipeline,
  runnable as before, instead of *Not shared*; other kinds kept at Join stay *Not shared* as before.
- **Steps.** A local pipeline's steps may use the team's profiles (pinned to the team's versions, resolved with this
  computer's bindings like any run) and the person's own local profiles. The team's profiles stay read-only for a
  member; the pipeline only names them.
- **Tasks.** A shared task that names a local pipeline keeps on the server only its name and kind: `TaskContent`'s
  `pipeline` becomes `slug | { local: { name, of: user id } } | null`. The body, the steps and the versions never
  leave the computer. The person who made it runs the task with it as with any pipeline. Other members see *a local
  pipeline of <person>* and cannot run the task with it: *Run* is unavailable with that reason until they choose a
  team pipeline or one of their own in the task's *Pipeline*, which changes the task as any edit does (row 18). A
  run's progress (row 29) names the pipeline the same way.
- **Promotion.** An admin may *Publish for the team* on their own local pipeline, after a confirmation. It is
  uploaded with its id and slug (a taken slug asks for a free one) and becomes a team resource from that version
  on: the new version is the team's `v1`, and the earlier local versions stay on this computer as its history, kept
  by the runs that pinned them. It is unavailable while a step uses a local profile, with that reason: the profile is
  published for the team first, or the step picks a team profile. Tasks that named it as local name its slug from
  then on. A member has no *Publish for the team*; they ask an admin.
- **Make local.** Make local keeps local pipelines as they are; they were never the server's. Tasks that named
  another person's local pipeline keep its name and show it as missing, as an unresolved pipeline does today.


### Skills in the database

Row 37. A skill is a resource like a profile or a pipeline: one draft, immutable published versions, pinned by runs,
shared through the server. Nothing of a managed skill lives as files Orchester reads from disk.

**Today** (ORC-15, checked 2026-10-15). The skill's version body is already in the project database (description,
`SKILL.md` body, scope, and a list of reference files by path, hash and size), but each reference file's content is a
project blob, a zstd file under `<repo>/.orchester/blobs/`. At each start of a step execution the runner writes the
skills that reach the agent to `<repo>/.orchester/skills/<execution-id>/<slug>/` and keeps them afterwards
(`crates/runner/src/execution/step/skills.rs`). Besides that, the repository's own skill folders are a live source:
while Project settings › *Repository agent files* is on, Codex loads `.agents/skills` and `.codex/skills` from the
worktree, the native agent lists them in its prompt (`core::agent_files::skills::native`), and `list_skills` /
`read_skill` offer them (`core::orchester_tools::skills::repository`). Import from repository
(`crates/workspace/src/skills/repository/`, its bounded no-follow scan of `.claude/skills` and `.agents/skills`)
already only reads them and makes drafts.

**After row 37:**

| Part | Where |
| --- | --- |
| Description, `SKILL.md` body, scope | The version body, as today |
| Reference files and scripts | A new table `skill_files` of the project database: content hash, size, executable flag, zstd content. The version body lists each part by path, hash, size and whether it is executable; the content is no longer a blob file |
| Placed copies for an agent | Created by the runner at the step's start in the run's private CLI folder: `<private root>/<project>/skills/<execution-id>/<slug>/`, under the runner's private files (`crates/runner/src/private_file/`, the user's temporary folder, readable by the user only). Never in the repository, a worktree, `.orchester` or the person's CLI home. Written again at every start of the execution (a resumed session finds the same paths) and removed when the step ends. A runner that ended abnormally leaves them; the next runner of the project removes them when it starts, as it does today for private files |
| Repository skill folders | An import source only. Codex is always kept from loading `.agents/skills` and `.codex/skills` (the fail-closed search and `skills.config` disabling run for every Codex start, not only while the setting is off); the native agent no longer lists them; `list_skills` and `read_skill` offer Orchester's skills only. *Repository agent files* then governs only `AGENTS.md` |

Claude Code reads the placed folder through `--add-dir`, Codex through its sandbox, the native agent through
`read_skill`; `read_outside_file` keeps refusing the private root.

**Size.** Each part travels as its own content-addressed request, so the 1 MiB request limit concerns one part, not
the version: `PUT /projects/{id}/files/{sha256}` with the raw bytes (not base64), idempotent by its hash, before the
version that lists it is published; the publish answers `422 missing_files { hashes }` for any part the server lacks.
Limits, for local and shared skills alike (core `SkillLimits`): a part up to 1 MiB, a version up to 16 MiB in all, up
to 256 parts. The editor and Import from repository refuse a larger part with the limit named (Import's scan allows 4
MiB today and goes down to 1 MiB). No general chunked upload (row 55).

**Migration.** A project database migration copies the content of every reference file of every skill version and
draft from its blob into `skill_files` (by hash, so each content once), and marks nothing executable (the person can
mark scripts). Blob files stay, since other data uses the blob store; skills stop reading it. A migrated part over 1
MiB is kept, so nothing is lost, and its skill says *too large to share* with the part named; Share uploads the rest
and lists it. The first runner after the update removes `<repo>/.orchester/skills/` (copies that only past
executions read; a resumed execution is placed again in the private folder). Projects whose *Repository agent files*
is on keep it on; the setting's text changes to say it covers `AGENTS.md` only, and Import from repository is offered
where the repository has skill folders.

## Offline and the outbox

Row 27. A shared project works the same with or without its server. The person's changes are written to the project
database at once and recorded in its **outbox**; the sync engine sends them in order when the server can be reached,
and applies what the server and the other members changed.

### The sync engine

`orchester-workspace`, `sharing::sync`, one per workspace on the application's runtime. No UI types; the apps read its
state through a watch channel.

| Part | Does |
| --- | --- |
| `outbox` | Records entries in the project database, in the same transaction as the person's view; reads the next sendable entry under core's holding rules |
| `sender` | One loop per shared project: sends the next entry, applies the answer, retries with backoff |
| `stream` | One SSE stream per open shared project, from the cached cursor |
| `apply` | Writes a confirmed task, tag, claim, progress or resource into the cache and advances the cursor in one transaction; recomputes the person's view of a task with waiting changes |
| `temporary` | Temporary task keys and tag ids, and their replacement |
| `progress` | Turns the runner's run changes into progress reports for held tasks |
| `config` | Resource sync, publishing, references and bindings |
| `state` | The sync state per server, project and task, for the status bar, the lists and the MCP tools |

The minute-by-minute poll of S2b (`sharing::project::work`) ends with S2d: the stream replaces it. The stream's
snapshot ([Events](#events)) gives the first open and a cursor past the retention the whole project; a full read
(tasks, tags, members, resources) remains for *Check again*. The heartbeat loop stays.

### Entries

| Field | Content |
| --- | --- |
| `sequence` | Order in the project, from 1 |
| `subject` | What it changes: a task, a tag, a module, a claim, a run's progress, a resource |
| `operation` | `create_tasks` (an import of tasks that keep their ids), `change_task` (the fields that differ, with `actor`; a task's module is one of them), `create_tag`, `create_module`, `change_module` ([Modules](#modules), row 71), `release_claim`, `report_progress`, `publish_version`, `change_resource` |
| `base` | The revision (or version) the change was made on |
| `payload` | The request body without the idempotency key |
| `key` | Idempotency key, a UUIDv7 made when the entry is recorded; every retry sends the same one |
| `state` | `waiting`, `sending`, `held` (behind another entry), `conflict` (needs the person), `refused` (the server will not take it) |
| `attempts`, `next_at`, `refusal` | Retry bookkeeping and the last refusal code |

**What never enters the outbox:** taking or taking over a claim, Share, Join, Make local, members and roles,
renaming, merging or deleting tags, binding references. These need the server and say so when it cannot be reached.

### Ordering

- One request in flight per project; entries go in `sequence` order.
- A change is merged into the last waiting entry of the same task when that entry has not been sent yet, so ten edits
  offline send one change. A progress report replaces the waiting report of its task.
- An entry in `conflict` or `refused` holds the later entries of the same subject, and every entry that depends on
  it: a link to a task whose create waits, an assignment of a tag whose create waits, a text that names a temporary
  key. Other subjects go on.
- Core decides holding (`sharing::outbox::next_sendable(entries)`), tested over explicit lists.

### Idempotency

- Creates already carry an idempotency key (`POST …/tasks/import`, `POST /projects`). Task changes, tag creates,
  resource creates, publishes and resource changes gain one (API 3). The server keeps the first answer per (project,
  user, key) for 30 days, as long as events, and answers a repeat with it.
- A lost answer is therefore harmless: the retry gets the first answer. Past 30 days the key is forgotten, and the
  revision still keeps a repeat from overwriting anything: it becomes an ordinary conflict, mostly a clean one.
- Claims release and progress reports are idempotent by nature: a repeat release meets `not_holder` with the claim the
  task has now, which the sender treats as done; a repeat report replaces itself.

### Retry and backoff

| Answer | Sender |
| --- | --- |
| Success | Applies the answer to the cache, removes the entry, sends the next |
| No connection, timeout, 5xx | Waits: 1 s doubling to 60 s with jitter, the connection monitor's schedule; the project reads *offline* once the monitor does |
| `429` | Waits for `Retry-After` |
| `401` | The server signed this computer out; the outbox keeps everything until the person signs in again |
| `403`/`404` on the project | No longer a member; the outbox keeps everything until the person is added again or chooses Make local |
| `409 revision_conflict` | Merges ([Conflicts](#conflicts)) |
| `409 version_conflict` | `conflict` for the person |
| `422` (`not_editable`, `not_queueable`, `organization`, `invalid`), `409 task_merged` | `refused` for the person |

### Cursor and applying events

- The stream starts from `sharing.event_cursor`, or without a cursor the first time. Each delta is applied and the
  cursor advanced in one transaction, so a crash replays at most the delta it was applying; applying is idempotent (a
  `revision` not newer than the cached one changes nothing). The kinds are those of [Events](#events) (row 73).
- A `task.changed` whose `base` equals the cached revision is applied to the task's confirmed state (its fields, and
  the new `revision`); one whose `base` differs makes the desktop read the task again and take it as the confirmed
  state. `task.created` adds a task, `task.claim` replaces its claim and `task.run` its run's phase and step.
- When the person has waiting changes for a task whose confirmed state moved, their view is recomputed on top: core's
  `ContentMerge` and `TaskOrganization::rebase` of the waiting change over the new state. A clean merge rebases the
  waiting entry to the new revision; a field both changed puts the entry in `conflict` before it is even sent, so the
  person learns it as soon as possible.
- The person's own changes come back as deltas too; the cache has them already and skips them by revision.
- A cursor older than 30 days, or none, gets the snapshot ([Events](#events)): at `snapshot.end` the desktop replaces
  the confirmed cache and continues from its `cursor`; waiting entries are rebased on the snapshot's tasks the same
  way.

### Conflicts

- **Organization** fields merge without asking (row 26).
- **Title, brief, criteria, pipeline, limits** both sides changed go to the person: the entry is in `conflict`, the
  task shows *Needs you · both changed*, and the person resolves it in the task's conflict view (10.41's view, with
  *Not sent · Alice Moreau saved a newer version*), field by field. Resolving replaces the entry with one based on the
  current revision. The person's text is never dropped until they choose.
- **Fields only the other side changed** are kept, marked *added by Alice*, as in 10.41.
- **Refused** changes: the person's view of the task returns to the server's version, and the refused change is
  kept on the task page with the reason (*ORC-12 could not take your change: it started running on Maria Lind's
  computer meanwhile, so its pipeline can no longer change*), *Edit again* (opens the form with the person's
  version) and *Discard* (asks in place). Nothing is lost silently.
- **Published versions** both sides made from one base: see [Shared configuration](#shared-configuration).

### Temporary numbers

- A task created without the server gets a temporary key, `ORC-T1`, `ORC-T2`, … from a local counter.
  `TemporaryTaskKey` in core; it never parses as a `TaskKey` (whose number is digits only), so the two cannot be
  confused. Its id is a UUIDv7 made locally and kept: the create is an import that keeps ids.
- The task is a draft and has no claim, so it cannot run until it has its number ([Claims](#claims)). Blocking
  links name tasks by id, so they need no remapping.
- When the import is answered, the task takes the server's number. In the same transaction the temporary key is
  replaced in every waiting entry's title, brief and criteria (as a whole word), and in the person's view of the tasks
  those entries change. Texts that are never rewritten (messages, memory, conversations: `storage.md` keeps them
  immutable) keep `ORC-T1`; the router, the search and the MCP tools resolve a temporary key through the
  `temporary_keys` table to the task.
- The person sees it: a notice above the task list, *3 tasks created while offline got their numbers: ORC-T1 →
  ORC-57, ORC-T2 → ORC-58, ORC-T3 → ORC-59*, with *Dismiss*; the task page says *was ORC-T1* until dismissed; the
  search finds the temporary key.

### What the person sees

| Where | What |
| --- | --- |
| Status bar server item (10.43, extended) | Connection state as before, plus *3 changes waiting* while the outbox holds entries and *1 needs you* for conflicts and refusals; offline: *Kontur team offline · since 14:31 · 3 changes waiting · next try in 32 s*. The hint lists each project |
| Task list | The offline notice replaces 10.42's read-only one: *Kontur team can't be reached since 14:31. You keep working: your changes wait on this computer and are sent in order when it is back. Taking a task needs the server.* with *Try now*. New task, bulk changes, Manage tags (create) stay available; Take is unavailable |
| Task rows and task page | A sync mark: *Waiting to send* (since when), *Needs you · both changed*, *Not taken by the server* (refused), and the temporary key with *number on reconnect*. Holders read *as of 14:31* |
| Task page | The conflict view, the refused change with its reason, the offline start refusals of [Claims](#claims) |
| Project settings › Sharing | *Waiting to send*: each entry (subject, what changes, since, state), *Discard* in place for the person's own unsent change |
| Resources | *Published here, not sent*, *Needs you · Alice published v5 meanwhile*, and *missing on this computer* with the binding picker |

## Live updates

**Server-Sent Events**, one stream per shared project the desktop has open:
`GET /api/v1/projects/{id}/events`.

- **Why SSE.** The push only goes one way; changes are ordinary requests, which need their status codes and conflict
  bodies anyway. SSE is plain HTTP, so the bearer header, TLS and reverse proxies work unchanged. Resuming is part of
  the protocol (`id:` and `Last-Event-ID`). axum serves it without another crate.
- **Events.** The server appends one row to `events` in the transaction of every change and wakes the streams.
  Each stream then reads the events after its cursor from the database, so a slow client misses nothing. Event `id`
  is the event's sequence number. The kinds are those of [Events](#events) below (row 73).
- **Staleness transitions.** A sweep once a minute marks claims whose last heartbeat is 15 minutes old and sends their
  `task.claim` delta (`stale`); a heartbeat that refreshes a marked claim clears the mark and sends one (`refreshed`).
  Heartbeats that only keep a fresh claim fresh send nothing. This closes S1b's open minor (watchers could not see
  staleness).
- **Keep-alive.** A comment line every 20 s; `X-Accel-Buffering: no` for nginx.
- **Desktop.** On open, the desktop subscribes from the cache's cursor, or without one the first time; the server
  sends the snapshot or replays what it missed. Reconnects back off from 1 s, doubling to 60 s, with jitter. `401`
  means *signed out*; `403` or `revoked` means *no longer a member*. Applying follows
  [Cursor and applying events](#cursor-and-applying-events).
- **Connection state** per server: *connected*, *reconnecting*, *offline*, *signed out*, *not a member* (per
  project). The status bar shows it for the servers of open shared projects.

### Events

The person's decision of 2026-10-29 (row 73): a snapshot on connect, then deltas that say who did what. It replaces
the earlier kinds `task` (a full task on every change), `members`, `tags`, `project`, `modules` (ORC-78), the planned
`resources` (API 3) and `reset`; S2d builds it, S5 adds the resource deltas.

**Connect.** A client without a cursor, or with one older than the 30-day retention, first gets a paged snapshot. A
client with a cursor inside the retention gets only the deltas after it. There is no `reset` event: a cursor too old
to resume from simply gets the snapshot.

| Kind | Payload |
| --- | --- |
| `snapshot.begin` | `{ project: Project, members: [Member], tags: [Tag], modules: [Module] }`, and with S5 `resources: [Resource]` (each with its latest version number) |
| `snapshot.tasks` | `{ tasks: [Task] }`, at most 64 tasks a page, as many pages as the project needs |
| `snapshot.end` | `{ cursor }`: the sequence of the last event the snapshot includes |

- The snapshot is read in one database transaction, so it is one consistent state; `cursor` is the last sequence at
  that moment, and the deltas after it follow on the same stream. Snapshot events carry no SSE `id:`; the desktop's
  cursor moves only at `snapshot.end` and with each delta.
- The desktop keeps the pages aside and, at `snapshot.end`, replaces the project's confirmed cache (project, members,
  tags, modules, tasks, later resources) and sets the cursor in one transaction. A stream that breaks before
  `snapshot.end` leaves the cache as it was, and the next connect sends the snapshot again.
- A connect from a cursor gets no snapshot, so the desktop first reads `GET …/resources` and keeps the versions it
  does not have, as it does for the snapshot's resources: Join sets the cursor from `GET …/tasks` without a snapshot,
  and the team's configuration published before must still reach the cache.

**Deltas.** Every delta carries `id` (its sequence), `by: UserRef` (who did it), `actor: "person"|"orchester"` (a
person's own action, or Orchester acting for them: a run, the staleness sweep, a merge) and `at`, beside the fields
of its kind:

| Kind | Fields |
| --- | --- |
| `task.created` | `task: Task`, the full task (a create, an import, an upload at Share or Join) |
| `task.changed` | `task` (its id), `revision` (after the change), `base` (the revision it was made on), and only the fields that changed: content `title`, `brief`, `criteria`, `pipeline`, `limits`, `status`, `module`; organization `priority`, `workflow`, `tags_added`, `tags_removed`, `blocked_by` (the task's blocking links `[{ task, gate }]` as they are now), `blocked_reason` |
| `task.claim` | `task`, `claim: Claim\|null` (as it is now), `action`: `taken`, `released`, `taken_over`, `completed` (a merge), `stale` or `refreshed` (the sweep and a heartbeat, `actor` `orchester`, `by` the holder) |
| `task.run` | `task`, `number`, `phase`, `step: { key, name }\|null`, `phase_since`, `updated_at`; light: the rest of the task's `RunStatus` (waiting kind, step results, branch, pull request) is read with the task |
| `tag.created` | `tag: Tag` |
| `tag.renamed` | `tag: Tag` (its new name) |
| `tag.deleted` | `tag` (its id); every task it is removed from sends its own `task.changed` |
| `tag.merged` | `from` (the tag that goes away), `into: Tag`; every rewritten task sends its own `task.changed` |
| `module.created` | `module: Module` |
| `module.changed` | `module: Module` (a module change replaces its fields as a whole, row 68); a task's move is its `task.changed` `module`, and the moves' history is still read with `GET …/modules?after=` |
| `member.added` | `member: Member` |
| `member.removed` | `user` (its id) |
| `member.role_changed` | `user`, `role` |
| `project.changed` | `project: { id, key, name }` |
| `resource.published` | `resource` with its metadata and the new version number; the body is read with `GET …/versions` (S5) |
| `resource.renamed` | `resource`, `name` (S5) |
| `resource.archived` | `resource`, `archived`: `true` when it was archived, `false` when it was restored (S5) |
| `revoked` | `{}`: the session or this person's membership ended; the stream closes after it |

**Applying.** A client applies `task.changed` only when its cached revision of that task equals `base`; otherwise it
reads that task again (`GET …/tasks/{task}`) and takes it as the confirmed state. A client's own changes come back as
deltas too; it already has them and skips them by revision (a delta whose `revision` is not newer than the cached one
changes nothing). The other kinds are applied as they come and are idempotent.

**Who did what.** The deltas are the record of who did what: each names the person and whether Orchester acted for
them, so a client can show it next to the change. There is no separate `activity` event; the server's Activity feed
stays as it is.

**Not included:** ephemeral events (presence, typing), docs and plugins.

## API

Base path `<server URL>/api/v1`. JSON bodies, UTF-8. Times are milliseconds since the Unix epoch, UTC. Ids are
UUIDv7 strings. Every call except `GET /server` and the two ways of joining (`POST /auth/cloud`, `POST /auth/code`)
sends `Authorization: Bearer <device token>`. The token never
appears in a URL. Requests are limited to 1 MiB. No CORS headers. **API version 3** (2026-10-15) adds what rows 27–30
need; a desktop that needs 3 says *update the server* to a server that reports 2.

### Shapes

```text
User        { id, username, display_name, admin: bool, disabled: bool, cloud: bool }   // API 3: cloud linked
UserRef     { id, display_name }
Session     { id, device, created_at, last_used_at, current: bool, via: "cloud"|"code"|"before_3",
              mfa: bool }                                                  // a device token; mfa: it may make invites
Invite      { id, kind: "server"|"project"|"account", email|null, project|null, role|null, user|null,
              by: UserRef, delivery: "cloud"|"code", expires_at }                                       // API 3
Project     { id, slug, address, key, name, remote_hint|null, role: "owner"|"member"|null, next_number } // API 3: slug, address, remote_hint (was remote)
Member      { user: UserRef, role, added_at }
TaskContent { title, brief, criteria: [string], pipeline: slug|{ local: { name, of: user id } }|null, // local: row 74
              limits: { budget_tokens, max_duration_ms }|null,
              ...Organization, module: module id|null }                                     // module: ORC-78
Organization { workflow: "backlog"|"todo"|"in_progress"|"in_review"|"done"|"canceled",
              priority: "urgent"|"high"|"medium"|"low"|"none",
              tags: [tag id], blocking: [{ task: id, gate: "review"|"merge" }],
              blocked_reason: string|null }      // core's TaskOrganization, field for field
Tag         { id, name }
Module      { id, ...ModuleFields, revision, created: { by: UserRef, at }, updated: { by: UserRef, at } } // ORC-78
ModuleFields { name, description, start: "YYYY-MM-DD"|null, target: "YYYY-MM-DD"|null,
              state: "open"|"closed" }           // core's ModuleDetails, field for field
ModuleChange { sequence, task: id, from: module id|null, to: module id|null, by: UserRef,
              actor: "person"|"orchester", at }  // ORC-78; core's MembershipChange
UploadModule { id, module: ModuleFields, created_at }                                     // ORC-78
Task        { id, number, key: "ORC-42", former_key: "KON-12"|null, revision, ...TaskContent,
              status: "draft"|"queued", has_run: bool,
              created: { by: UserRef, at }, updated: { by: UserRef, at },
              claim: Claim|null, run: RunStatus|null }
OrganizationChange { at, by: UserRef, actor: "person"|"orchester", from: Organization, to: Organization,
              from_tag_names: [string], to_tag_names: [string] }
Claim       { id, holder: UserRef, kind: "manual"|"run", since, heartbeat_at, stale: bool,
              taken_over_from: UserRef|null }
RunStatus   { holder: UserRef, number, phase, step: { key, name }|null,                       // API 3: step is an object
              waiting_for: "person"|"limit"|"question"|"approval"|null,                       // API 3
              steps: [{ key, name, result: "passed"|"failed"|"unverified"|"unchanged"|null }], // API 3
              branch: string|null,                                                            // API 3
              pull_request: { number, url, state: "open"|"draft"|"merged"|"closed" }|null,   // API 3
              started_at, phase_since, updated_at }
RunReport   { number, phase, step: string|null, started_at, phase_since }  // ORC-284: the body of `PUT …/run`,
              // orchester-api's RunStatus without holder and updated_at (the server sets them); S3 adds row 29's fields
Changes     { title?, brief?, criteria?, pipeline?, limits?, status?: "queued",
              workflow?, priority?, tags?, blocking?, blocked_reason?, module?: id|null,      // module: ORC-78
              actor?: "person"|"orchester" }
UploadTask  { id, number?, former_key?, ...TaskContent, tag_names: [string] instead of tags,
              status, created_at, has_run: bool, claim: bool }   // module: one of the upload's or the project's
Resource    { id, kind: "pipeline"|"agent_profile"|"instruction"|"skill"|"tool", slug, name, archived: bool,
              revision, latest: number|null, created: { by, at }, updated: { by, at } }   // API 3
Version     { resource, number, body: JSON, hash, references: [Reference],
              based_on: number|null, published: { by: UserRef, at } }                     // API 3
Reference   { id, kind: "connection"|"mcp_server"|"secret"|"tool", name, role: string|null } // API 3
UploadResource { id, kind, slug, name, archived, versions: [{ number, body, references, published_at }] } // API 3
Error       { error: code, message, ...details }   // message is English for logs; the desktop shows its own i18n text by code
```

Every response carries `Orchester-Server-Time` (the server's clock, for staleness).

`Project.role` is the caller's membership role. An admin who is not a member can manage the project but receives
`role: null`; admin authority does not grant membership or task visibility.

`not_queueable` always includes `missing`. When the task is already queued, it is an empty array and
`reason: "not_draft"` explains the status refusal; incomplete drafts name their missing details.

### Endpoints

Auth column: **none**; **user** (any signed-in user); **member** (of the project); **owner** (owner of the project
or admin); **admin**; **admin (row 72)** (the project's owner or the server's admin, core's
`sharing::may_change_configuration`; a member gets `403 not_admin`); **holder** (member holding the task's claim).
The *Since* column marks what API 3 adds or changes; the rest is built (S1a, S1b).

| Method and path | Auth | Request | Response | Errors | Since |
| --- | --- | --- | --- | --- | --- |
| `GET /server` | none | | `{ name, api_version: 3, needs_setup, cloud: { root_fingerprint, accepting }\|null }` | | changed in 3 |
| `POST /server/key-proof` | none | `{ nonce }`; header `Orchester-Presented-Certificate` (optional): the SHA-256 hex of the certificate the app's connection presented | `{ public_key, proof }`: the server's key, and its signature of its id, the nonce and its certificate (with the header, also the one before a proxy's renewal) | `422 invalid` (also a header that is not 64 hex characters) | new in 3 ([Codes](#codes)); the header came with `proxy_certificate = "auto"`, and a server from before ignores it |
| ~~`POST /auth/sessions`~~ | | | | | removed in 3 (row 46) |
| `POST /auth/cloud` | none, or user (to link an account) | `{ identity_token, key_set?, device }` | `201 { token, session, user, joined: { project, role }\|null }` | `401 invalid_identity` (also a token whose `name` is blank or over 256 characters), `403 not_invited`, `403 cloud_off`, `409 key_set_stale`, `429 rate_limited` | new in 3 (row 42) |
| `POST /auth/cloud/accept` | none (the web app's server side) | `{ identity_token, key_set? }`: a token of purpose `accept` naming an open invite | `201 { browser_session: { token, session, user }?, membership: { kind, project?, role? }, receipt, approval_required }` | `401 invalid_identity` (also another purpose), `403 mfa_required`, `403 not_invited`, `403 forbidden`, `403 cloud_off`, `409 key_set_stale`, `410 invite_ended { revocation }`, `429 rate_limited` | contract only (row 77, [Accepting outside Orchester](#accepting-outside-orchester)) |
| `POST /auth/code` | none | `{ code, device, username?, display_name? }` | `201 { token, session, user, joined }` | `401 invalid_code` (one answer for unknown, used and expired), `409 username_taken`, `422 invalid`, `429 rate_limited` | new in 3 (row 45) |
| `POST /auth/pairings` | user | | `201 { code, expires_at }` | `409 too_many_codes` (the server's 50 open codes), `429 rate_limited` (3 open per account) | new in 3 (row 44) |
| `GET /auth/me` | user | | `{ user }` | `401` | |
| `GET /auth/sessions` | user | | `{ sessions: [Session] }` | | |
| `DELETE /auth/sessions/current` | user | | `204` | | |
| `DELETE /auth/sessions/{id}` | user (own sessions) | | `204` | `404` | |
| ~~`POST /auth/password`~~ | | | | | removed in 3 (row 46) |
| `POST /invites` | admin (server, account), owner (project) | `{ kind, email?, project?, role?, user?, delivery }` | `201 { invite, code? }` (a code only for `delivery: code`, shown once), `signed?` (the signed invite for the server admin's page to carry through the browser when the server cannot reach the cloud), `cloud_delivery?` (how the server's own delivery went: `delivered`, `declined`, `no_cloud` or `unreachable { cause: unreachable \| tls \| refused \| timeout }`; none for a code) | `403` (also an account invite naming an admin), `403 mfa_required`, `409 invite_open { invite, sent_at }` (the address has an open invite of the same kind to the same project or the server), `409 already_member` (its account, known by the email its cloud identity last verified, has what the invite gives), `409 too_many_codes`, `422 invalid` | new in 3 (row 43) |
| `GET /invites` | admin (all), owner (their projects') | | `{ invites: [Invite] }`, open only, never a code | | new in 3 |
| `DELETE /invites/{id}` | the inviter, owner, admin | | `200 { revocation?, cloud_delivery? }`: for an invite by email the signed revocation and how the server's own delivery of it went, as for `POST /invites`; the server admin's page delivers it through the browser when the server could not | `404` | new in 3 |
| `GET /server/cloud`, `PATCH /server/cloud` | admin | `{ accepting?, link?, unlink?, pin? }` | `{ cloud }` | `422 invalid` | new in 3 (rows 47, 62) |
| `GET /server/settings`, `PATCH /server/settings` | admin | `{ join_approval? }` | `{ settings, check_code }` | `422 invalid` | new in 3 (rows 57, 62) |
| `GET /admin/sessions`, `DELETE /admin/sessions/{id}` | admin | | `{ sessions: [Session with user] }`; `204` | `404` | new in 3 (row 62) |
| `GET /admin/browsers`, `DELETE /admin/browsers/{id}` | admin | | `{ browsers: [{ id, user, via, created_at, last_used_at, expires_at, current }] }`, the live `admin_web` sign-ins, newest first; `204` | `404` (also a computer's id) | new in 3 (security review 1, minor 5) |
| `GET /projects` | user | | `{ projects: [Project] }` the user is a member of | | `?remote=` removed in 3 (row 49) |
| `GET /projects?slug=<slug>` | user | | `{ project }` when the user is a member | `404` when there is no such project or the user is not a member of it: one answer for both, so a lookup tells a non-member nothing of the project (its name and owners included) | new in 3 (row 49) |
| `POST /projects` | admin | `{ idempotency_key, name, slug, key, remote_hint?, tasks: [UploadTask] }` | `201 { project, tasks: [Task], cursor }`; the caller is the project's owner; `tasks` may be empty | `409 slug_taken`, `409 conflict`, `422 invalid` | changed in 3: admins only, by slug; `remote_taken` removed |
| `POST /projects/{id}/link` | member | `{ idempotency_key, tasks: [UploadTask], remote?, modules?: [UploadModule] }` | `201 { tasks: [Task], dropped_links, cursor, modules, mapped }`: the first checkout linked to a project with no tasks keeps its numbers (row 12); later ones are imports with new numbers; sets `remote_hint` when empty | `409 id_taken`, `422 module`, `422 invalid` | new in 3 (row 49); modules ORC-78 |
| `GET /projects/{id}` | member | | `{ project }` | `404` | |
| `PATCH /projects/{id}` | owner | `{ name?, key?, remote_hint? }` | `{ project }`; the slug never changes | `422 invalid` | `remote_hint` in 3 |
| `GET /projects/{id}/members` | member | | `{ members: [Member] }` | | |
| `POST /projects/{id}/members` | owner | `{ username, role }` | `201 { member }` | `404 no_such_user`, `409 already_member` | |
| `PATCH /projects/{id}/members/{user}` | owner | `{ role }` | `{ member }` | `404`, `409 last_owner` | |
| `DELETE /projects/{id}/members/{user}` | owner, or the member themself | | `204`; their claims are released | `409 last_owner` | |
| `GET /projects/{id}/tasks?after={n}` | member | | `{ tasks: [Task], cursor, next?, total? }`: with `after`, the tasks numbered above `n`, lowest first, at most 64, `next` (the `after` of the following page) only when there is one, and `total` (the project's tasks now); without it, every task, with neither. A server that ignores the parameter answers one page | | `after` ORC-291 |
| `GET /projects/{id}/tasks/{task}` | member | | `{ task }` | `404` | |
| `POST /projects/{id}/tasks` | member | `{ idempotency_key, tasks: [TaskContent] }` | `201 { tasks: [Task] }`, consecutive numbers; a repeated key returns the first result | `422 invalid` | |
| `POST /projects/{id}/tasks/import` | member | `{ idempotency_key, tasks: [UploadTask], modules?: [UploadModule] }` | `201 { tasks: [Task], dropped_links, modules, mapped }`, new numbers, ids and former keys kept. Also the create of tasks made offline (`former_key: null`) | `409 id_taken`, `409 conflict`, `422 module`, `422 invalid` | used by the outbox in 3; modules ORC-78 |
| `PATCH /projects/{id}/tasks/{task}` | member | `{ idempotency_key, revision, changes: Changes }` | `{ task }`; a repeated key returns the first answer | `409 revision_conflict { current: Task }`, `422 not_editable { reason }`, `422 not_queueable { missing }`, `422 organization { reason }`, `422 module { reason: "module_not_found" }` (ORC-78), `422 invalid` | key added in 3 |
| `GET /projects/{id}/tasks/{task}/history` | member | | `{ changes: [OrganizationChange] }`, oldest first | `404` | |
| `POST /projects/{id}/tasks/{task}/claim` | member | `{ kind }` | `{ task }`; also when the caller holds it already | `409 claim_held { claim }`, `409 task_merged` | |
| `DELETE /projects/{id}/tasks/{task}/claim` | holder | | `{ task }` | `409 not_holder { claim }`, `422 not_editable { reason: "stop_run_first" }` | |
| `POST /projects/{id}/tasks/{task}/claim/take-over` | member | `{ claim: id }` | `{ task }` | `409 claim_not_stale { claim }`, `409 claim_changed { claim }` | |
| `PUT /projects/{id}/tasks/{task}/run` | holder | `RunReport` | `{ task }`; `merged` completes the claim, `cancelled` releases it, other phases are stored as the task's `run` | `409 not_holder { claim }`, `422 invalid` | new in 3 (S3; contract ORC-284) |
| `POST /heartbeat` | user | `{ tasks: [id] }` (at most 256) | `{ lost: [{ task, claim: Claim\|null }] }` | `422 invalid`, `429 rate_limited` | |
| `GET /projects/{id}/tags` | member | | `{ tags: [Tag] }`, by name | | |
| `POST /projects/{id}/tags` | member | `{ idempotency_key, name }` | `201 { tag }`; a repeated key returns the first answer | `409 tag_exists { tag }`, `422 invalid` | key and `tag` detail in 3 |
| `PATCH /projects/{id}/tags/{tag}` | member | `{ name }` | `{ tag }` | `404`, `409 tag_exists`, `422 invalid` | |
| `POST /projects/{id}/tags/{tag}/merge` | member | `{ into: tag id }` | `{ tags: [Tag], tasks: [Task] }` | `404`, `409 same_tag` | |
| `DELETE /projects/{id}/tags/{tag}` | member | | `{ tags: [Tag], tasks: [Task] }` | `404` | |
| `GET /projects/{id}/modules?after={sequence}` | member | | `{ modules: [Module], changes: [ModuleChange] }`: every module, by when it was made, and the moves after `after` (all without it), oldest first | | ORC-78 |
| `POST /projects/{id}/modules` | member | `{ idempotency_key, id, module: ModuleFields }` | `201 { module }` at revision 1; a repeated key returns the first answer | `409 module_exists { module }`, `409 id_taken`, `422 module { reason: "target_before_start" }`, `422 invalid` | ORC-78 |
| `PATCH /projects/{id}/modules/{module}` | member | `{ idempotency_key, revision, module: ModuleFields }` (the fields as a whole) | `{ module }`; the same fields change nothing; a repeated key returns the first answer | `404`, `409 revision_conflict { current: Module }`, `409 module_exists { module }`, `422 module { reason }`, `422 invalid` | ORC-78 |
| `GET /projects/{id}/resources` | member | | `{ resources: [Resource], cursor }` | | new in 3 (S5) |
| `GET /projects/{id}/resources/{resource}/versions?after={n}` | member | | `{ versions: [Version], next? }`: the versions after `after` (all without it), oldest first, at most 64 or 1 MiB; `next`, the `after` of the following page, only when there is one | `404` | new in 3 (S5) |
| `POST /projects/{id}/resources` | admin (row 72) | `{ idempotency_key, id, kind, slug, name, version: { body, references } }` | `201 { resource, version }` (v1) | `403 not_admin`, `409 slug_taken { resource }`, `409 id_taken`, `422 invalid { reason }` | new in 3 (S5); admin row 72 |
| `POST /projects/{id}/resources/import` | admin (row 72) | `{ idempotency_key, resources: [UploadResource] }` | `201 { resources: [Resource] }`; version numbers kept | `403 not_admin`, `409 slug_taken { resource }`, `409 id_taken`, `422 invalid` | new in 3 (S5); admin row 72 |
| `POST /projects/{id}/resources/{resource}/versions` | admin (row 72) | `{ idempotency_key, based_on, body, references }` | `201 { version }` with the next number | `403 not_admin`, `409 version_conflict { latest: Version }`, `409 archived`, `422 missing_files { hashes }`, `422 invalid { reason }` | new in 3 (S5); admin row 72 |
| `PATCH /projects/{id}/resources/{resource}` | admin (row 72) | `{ idempotency_key, revision, name?, archived? }` | `{ resource }` | `403 not_admin`, `409 revision_conflict { current }`, `422 invalid` | new in 3 (S5); admin row 72 |
| `PUT /projects/{id}/files/{sha256}` | admin (row 72) | raw bytes (`application/octet-stream`), at most 1 MiB | `201`, or `200` when the server has it; idempotent by its hash | `403 not_admin`, `422 hash_mismatch`, `413 payload_too_large` | new in 3 (S5, row 37); admin row 72 |
| `GET /projects/{id}/files/{sha256}` | member | | raw bytes | `404` | new in 3 (S5, row 37) |
| `PUT /projects/{id}/catalog` | member (admins too; not a configuration write, row 72 does not apply); an app's device token only, refused to a browser sign-in (`admin_web`) | `{ references: [{ kind: "connection" \| "mcp_server", name, role }], models: { role: [model id] } }`: this device's whole state, at most 256 references and 512 ids per role, each word at most 200 characters; `role` is `null` (or left out) for an MCP server | `204`; it replaces this device's previous report | `422 invalid { reason: "catalog" }` (an unknown kind or role, a connection without a role, an MCP server with one, too many), `422 invalid` (an unknown field) | S5, row 75 (ORC-451) |
| `GET /projects/{id}/references` | member, also through the web app's browser sign-in (`admin_web`) | | `{ references: [{ kind, name, role }] }`: the union of the current reports with the connection and MCP server references the project's versions name (`version_references`; secrets and tools are not listed), one per kind, role and name without regard to case, sorted by name; no count, no device or person | | S5, row 75 (ORC-451) |
| `GET /projects/{id}/models` | member, also through the web app's browser sign-in (`admin_web`) | | `{ models: { role: [model id] } }`: the union of the current reports per role, each sorted | | S5, row 75 (ORC-451) |
| `GET /projects/{id}/events` | member | `Last-Event-ID` header (none, or older than 30 days: the snapshot first) | `text/event-stream`: `snapshot.begin`, `snapshot.tasks`, `snapshot.end`; `task.created`, `task.changed`, `task.claim`, `task.run`; `tag.created`, `tag.renamed`, `tag.deleted`, `tag.merged`; `module.created`, `module.changed`; `member.added`, `member.removed`, `member.role_changed`; `project.changed`; `resource.published`, `resource.renamed`, `resource.archived` (S5); `revoked` | `401`, `403`, `429 rate_limited` | row 73 (S2d): these kinds replace `task`, `members`, `tags`, `project`, `modules` (ORC-78), the planned `resources` (3) and `reset` |
| `GET /admin/users` | admin | | `{ users: [User] }` | | |
| `DELETE /admin/users/{id}/cloud` | admin | | `{ user }`: the account linked to no cloud identity, audited | `404 not_found` | new in 3 ([Codes](#codes)) |
| ~~`POST /admin/users`~~ | | | | | removed in 3: accounts come from invites |
| `PATCH /admin/users/{id}` | admin | `{ display_name?, admin?, disabled? }` | `{ user }`; disabling revokes sessions and releases claims | `409 last_admin` | |
| ~~`POST /admin/users/{id}/password`~~ | | | | | removed in 3: an account code replaces it |
| `DELETE /admin/users/{id}/sessions` | admin | | `204` | | |

**Uploads.** `UploadTask.number` is only used when a project is created: Share keeps numbers. Import ignores numbers
and assigns new ones, and keeps `former_key` (`KON-12`) for everyone to see; a task created offline is imported with
`former_key: null`, since its temporary key never meant anything to others. `claim: true` claims the task for the
uploader in the same transaction. `has_run` is true when the task has any run on the uploader's computer (coordinator
decision 2026-10-04, S1b review C3). An upload carries each task's organization with tags by name; links to tasks
neither uploaded nor on the server are dropped and listed (`dropped_links`). History before the upload stays local.
A link or an import may carry `modules: [UploadModule]` (ORC-78): each becomes the project's module of its name, or a
new one with its id and `created_at`; a task's `module` names one of them or one the project has (`422 module {
reason: "module_not_found" }` otherwise), and each uploaded task in a module is recorded as a move into it at the
upload's time. The answer adds `modules: [Module]` (the project's, after the upload) and `mapped: [{ from, to }]`
(the uploaded modules that became another module of the project). `POST /projects` takes no modules.

**Resources.** The server checks a body with core's rules for its kind (the same constructors the desktop uses), and
refuses a body or references whose ids do not match (`reason: "references"`: every id the body names that is not a shared resource of the project
must have a reference). `slug` is unique per kind in the project (`409 slug_taken` returns the resource that has it).
A skill's parts are not in its body: the body lists them by path, hash, size and executable flag, and each part is
uploaded first with `PUT …/files/{sha256}`. A create, import or publish that names a part the project lacks answers
`422 missing_files`; one over core's `SkillLimits` answers `422 invalid { reason: "skill_size" }` ([Skills in the
database](#skills-in-the-database)). `hash` is core's content hash of the body. Import keeps ids and version numbers and names the uploader as each
version's publisher, with the original `published_at`. A `Reference` carries its kind beside its id, so the id's type
follows from it: a connection's or an MCP server's id is a `ConnectionId`, a secret's a `SecretId`, a tool's a
`ResourceId`. Every write of these endpoints, file uploads included, answers a member `403 not_admin` with no details.

The contract is `orchester-api`'s `resources.rs` (`SharedResource`, `SharedVersion`, `Reference`, `UploadResource`,
the requests `ResourceCreate`, `ResourcesImport`, `VersionPublish`, `ResourcePatch`, and the answers) and its client's
`client/resources.rs`: `resources`, `versions`, `create_resource`, `import_resources`, `publish_version`,
`update_resource`, `upload_file` (`FileStored::Created` for `201`, `Known` for `200`) and `file`. The error details are
`SharingErrorDetails`' `Resource` (`slug_taken`), `Version` (`version_conflict`), `Files` (`missing_files`),
`ResourceRevision` (`revision_conflict`) and `Reason` (`invalid`).

**Catalog** (row 75). `PUT …/catalog` is keyed by the calling device's session: one report per device and project,
replaced whole, removed with the device's session or the member's membership. The reads compute the union of the
remaining reports, `GET …/references` with the connection and MCP server references of the project's versions too;
they never name a device, a person or how many computers have an entry. A role is one of core's connection kinds
(`ConnectionRole::as_str`: `codex_cli`, `claude_code_cli`, `anthropic_api`, `openai_api`, `github`); an unknown one is
refused, so the catalog follows the kinds this server's core knows. A model id is core's `ModelId`. The web's profile
editor reads both through its server side, with the browser sign-in; the desktop reports and never reads them (it
binds against its own connections).

The contract is `orchester-api`'s `catalog.rs` (`CatalogKind`, `CatalogReference`, `ModelsPerRole`, the request
`CatalogReport` and the answers `CatalogReferences` and `CatalogModels`, each refusing fields it does not know, so no
account, path, setting, key, id or device can ride along) and its client's `client/catalog.rs`: `report_catalog`,
`catalog_references` and `catalog_models`.

**Progress reports.** `PUT …/run` replaces the task's report. The server checks the shape (step keys unique, `step`
among `steps`, `waiting_for` only with `waiting`, `stuck` or `stopped_at_limit`; a URL that is `https`), stores it,
and sends the `task.run` delta (row 73). It derives nothing else from it except the claim ends of row 15. Today's
contract (ORC-284, `orchester-api`'s `RunReport` and `Client::report_run`) carries `number`, `phase`, `step` (its
name), `started_at` and `phase_since`; row 29's other fields come with S3. Only the holder reports (`409 not_holder` with
the claim otherwise); `merged` completes the claim, `cancelled` releases it (as `DELETE …/claim` does), and every other
phase is stored as the task's `run` with the claim kept. A server without the route answers `404 not_found` or
`405 method_not_allowed`, which the desktop takes as *not supported* ([Claims](#claims)).

**Rules kept from S1b.**

`organization` reasons come from core's `OrganizationError`: an empty Blocked reason, a link that would close a cycle
(checked over the project's links in the transaction), `canceled` while the task's reported run phase has not ended
(`stop_run_first`, from the live run status; the holder's desktop also checks its local run before sending), a tag
that is not the project's. Only linked unfinished tasks prevent a run; the manual Blocked reason is informational, as
locally. Whether a link is finished is `DependencyGate::is_completed` of the blocking task's shared phase (its run
status, or `merged` once its claim is completed).

`not_editable` reasons come from core's `EditError`. A queued task with no run is locked; a task with a run must stay
complete. "Has a run" is `has_run` on the server: it was uploaded with a run or someone took a `run` claim on the
task, which starting work always does. Claiming a task one holds already as `run` turns a manual claim into a run
claim; taking any claim one holds already refreshes its heartbeat. Claims, heartbeats and run status do not bump
the task's content revision. Content and organization changes, including tag rewrites, do bump it.

Organization normalizes blocking links to one gate per task (the last supplied gate wins), as it does for tags,
so task snapshots and history describe the same organization. Unknown tag ids return `422 organization` with
`reason: "tag_not_found"`; unknown or foreign task links return `422 invalid` before foreign-key writes.

**Sharing resource limits.** Heartbeats accept at most 256 ids and 12 requests per 60-second window per session.
Excess requests receive `429 rate_limited` with `Retry-After`; a heartbeat that only refreshes freshness emits no
delta. Freshness is authoritative on subsequent task reads and takeover checks. SSE admits
at most 8 simultaneous streams per session and 16 per user, including connections through trusted proxies, with
`429 rate_limited` and `Retry-After: 1` when full. Disconnects and revocations release that capacity.

Event retention is pruned on successful sharing writes at most once per minute, with an index on `(project, at)`;
replay excludes expired rows between prunes. Replay batches up to 64 events and 1 MiB of payload (a single larger
event is sent alone), preserving order while bounding memory and database round trips. A cursor below the project's
expired event cursor floor gets the snapshot of [Events](#events) (row 73).

Event sequence numbers remain globally monotonic in S1b. Gaps can reveal that another project changed, but never
its identity or event data; this metadata tradeoff is accepted for S1b. A task-id collision outside the caller's
project returns only a generic conflict without naming the task or project. The conflict status can still imply
a collision; UUIDv7 identifiers are not treated as authorization, and every data read remains membership checked.

Common errors on every endpoint: `400 bad_request` (malformed body, including unknown request fields),
`413 payload_too_large` (body over 1 MiB), `405 method_not_allowed` (unsupported method),
`408 request_timeout` (body read exceeds 10 seconds), `401 unauthorized` (no, unknown, revoked or
expired device token), `403 forbidden`, `404 not_found`, which is also the answer for a project the caller is not a
member of. (`403 password_change_required` goes with passwords in API 3.)

## Accounts and tokens

Rows 39–47 (2026-10-15). A team server knows its people by accounts, as in S1a, but nobody has a password. A device
joins a team server once, with a cloud identity token (row 42) or, without the internet, a code (row 45), and then
works with the **device token** the team server issued it. Day-to-day work needs only the team server.

### Transport and certificates

Row 39. The team server needs no internet, no public DNS and no public certificate authority.

| Mode (`tls` in `server.toml`) | Certificate | How the desktop trusts it |
| --- | --- | --- |
| `self_signed` (default) | On first start the server generates an ECDSA P-256 key and a self-signed certificate (`rcgen`), valid 20 years, with the names in `tls_names` (default: the host name, `localhost` and every non-loopback address of the machine), and keeps both in `<data_dir>/tls/` (key readable by the server's user only) | **Pinned.** The desktop accepts exactly the certificate whose SHA-256 it pinned for that server, whatever names or dates it carries |
| `pem` | The team's own certificate and key (`tls_cert`, `tls_key`), e.g. from an internal CA | The OS trust store, or pinned like `self_signed` when the team prefers |
| `off` | None: plain HTTP, for a server behind a proxy on its own network (the Docker example with Caddy and a public domain) | The proxy's certificate through the OS trust store |

- **Check code.** The certificate's SHA-256 shown to people as 16 base32 characters in four groups
  (`3F7A-92C1-KD8M-QX4T`, Crockford alphabet). The server prints it to its console at every start and `orchester-server
  tls show` prints it again. A cloud invite carries the full fingerprint, so the person compares nothing; with a code
  the desktop shows the check code it received and continues only after the person confirms it matches the console or
  the inviter's screen. The confirmation (N1) makes the person compare all four groups: it shows the whole code, never
  a shortened one, and does not let a match of the first group stand for the rest.
- **Pinning** is a rustls verifier in `orchester-api`'s client that compares the presented leaf certificate with the
  pinned hash; nothing else is accepted, and there is no "ignore certificate errors" switch.
- **A new certificate** (`orchester-server tls regenerate`, e.g. after a key leak) breaks every pin on purpose. The
  desktop says *Kontur team presents a different certificate* with the new check code, and the person confirms it
  against the console (person-only), or rejoins through an invite.
- **Plain HTTP** stays allowed only through row 3's explicit opt-in.

### Servers and project addresses

Rows 49–50. A team server runs at its owner's own address, `https://orchester.userdomain.com` (or an address on the
team's network, `https://10.0.4.20:8740`). It holds several projects, each at its own address:
`https://orchester.userdomain.com/project-a`.

- **Creating a project.** The server's owner, or an admin, creates a project by name and slug in the server admin
  (row 62); the first local checkout linked to it uploads its tasks with their numbers. The creator is the project's
  owner. The slug is a core `Slug`, unique on the server, fixed once created (row 58; the name can change); `admin`,
  `api`, `auth` and names starting with `_` or `.` are reserved.
- **The address is the identity** (replaces row 10). The app splits a project address into the server's base address
  and the slug, asks the server's `GET /server`, then finds the project by slug. There is no lookup by repository and
  no uniqueness of repositories on the server: two projects may hold work on one repository.
- **The repository is a hint.** A project keeps the normalized `origin` identity (`RepositoryIdentity`) its creator or
  first linked checkout had, as `remote_hint`. When someone links a local checkout, the app compares the checkout's
  `origin` with it and warns on a mismatch (*This checkout's origin is github.com/acme/billing; Kontur on Kontur team
  was set up for github.com/acme/kontur*), and the person may go on. A checkout with no `origin` or a local path can
  still be linked.
- **The server never touches code.** Everyone clones and sets up the repository themselves, from wherever the team
  keeps it.
- **One connection per server** (row 50). The app connects to a team server once and holds one device token for it;
  every local project linked to a project on that server uses it. A token per project would multiply heartbeats,
  streams and devices, and signing a project out would not sign the person out.
- **Access is per project.** Being connected to a server gives nothing by itself: an invite to project-a makes the
  person a member of project-a only. Server-level invites (admins) create an account with no project.
- **A link per project.** Each local project is linked to one server project: its `sharing` row names the server and
  the project's id and address. Opening a second project on a connected server only asks which of its projects to
  link, or accepts an invite.
- **The `servers` table stays** in the application database as this device's record of connected servers: address,
  name, certificate pin (or *system trust*, or *plain HTTP allowed*), the account there. Settings › Orchester lists
  them, each with its linked projects on this computer, *Add a computer* (a pairing code), *Sign in to the admin* (a
  code for the server admin) and *Sign out*, which asks in place and names those projects. Connecting is done by
  accepting an invite or with an invite code and the check code; the new app builds no S2a Settings ›
  Servers, and the old app keeps its own, frozen (row 31).
- **Leave** a project is in that project's Sharing and keeps the device connected.
- **Discovery.** `orchester.toml` names the project's address (`project = "https://…/kontur"`, no secrets); the app
  prefills it when the person links the checkout, and contacts it only after the person confirms (row 11).
- **An address only on the server that reported it.** The app keeps, writes into `orchester.toml` or opens in the
  browser a project address the server reported only when it is on the address this computer connected to
  (`ProjectAddress::reported`); anything else is `SharingError::ForeignAddress`, which names the address the server
  gave itself. **Limitation:** a server configured with its own `address` names its projects there whatever address
  a device reached it by, so a device connected by another one (a LAN address or a port forward while `address` is
  the public name) cannot link, write the file or open the admin for its projects, and *Write orchester.toml* and
  *Manage members and invites* say so with that address in advance: the person connects to the server by the address
  it names itself by. A server without `address` names its projects at the host each request used, so this does not
  arise. A link made before the check keeps syncing and leaves its address unrecorded; the sync notes that at debug
  level, since it runs every minute.

### Orchester Cloud

Rows 41 and 48. One service, `orchester-cloud`, run by the person on the internet for everyone. It is not a team
server and never sees project data.

| It holds | It never holds |
| --- | --- |
| Accounts: email (confirmed), display name, password hash; later, linked providers | Tasks, runs, code, diffs, configuration, project names beyond an invite in flight |
| Two-factor authentication: each account's TOTP secret, sealed with the key-encryption key, and the HMAC of its recovery codes | A two-factor secret or recovery code in the clear, after the answer that showed it |
| Devices: name, platform, token hash, last use | Device tokens of team servers |
| Team servers: id, owner account, name, address, certificate fingerprint, public key, linked / unlinked at | Members' roles, projects' contents |
| Which accounts joined which team server (to list them on a new computer) | Which projects they joined: the team server answers that |
| Invites in flight: server, project address, kind, invitee email, inviter's display name, project name and role, expiry | Anything about an invite after it ended (accepted, expired, revoked): deleted |
| Its root key, the signing keys and the key set, private halves sealed with a key from its host's environment | |

**Sign-up and sign-in (v1, row 48).** A simple form: email and password. The cloud sends a confirmation link (valid
24 hours); the account can sign in once the email is confirmed. *Forgot password* sends a reset link (valid 1 hour,
one-time); a reset signs out every other cloud device of the account. Passwords: argon2id with S1a's parameters and
rules (12 to 256 characters, no composition rules, hashing on a bounded pool, an unknown email verified against a
dummy hash so it answers like a wrong password). The app keeps its **cloud device token** in the OS keychain
(`orchester-cloud` / `device`). The app signs in itself; *Create an account* and *Forgot password* open the web
front ([The cloud's web front](#the-clouds-web-front)), where the confirmation and reset links also land. Magic link,
Google and GitHub are a later phase.

**Two-factor authentication** (2026-10-16; [cloud-api.md](../cloud-api.md#two-factor-authentication)). An account
can turn on a TOTP authenticator app (RFC 6238: SHA-1, 6 digits, 30 seconds, ±1 step, each code once) with ten one-time
recovery codes. With it on, every sign-in (the desktop's and the web front's) answers the right password with a
challenge that a code finishes; a device that signed in, stepped up or turned it on with a code is a *two-factor
session*, and identity tokens say so (`mfa`). An invite to a team server is sent and accepted only with it: the cloud
issues a token that accepts an invite only to a two-factor session (`403 mfa_required`, `403 session_without_mfa`), and
a team server sends an invite by email only for an admin signed in through the cloud with it ([Invites](#invites)). The
recovery codes are the only way back; nothing resets it by email, and the operator of a cloud can turn it off for an
account with one documented statement (`docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`).

**Address and configuration** (row 59). The cloud's web pages are at `https://orchester.cloud` and its API at
`https://api.orchester.cloud/v1` ([cloud-api.md](../cloud-api.md), "Base"); until 2026-10-25 it ran at
`https://orchester-cloud.lurq.dev`, which keeps answering during the move (`docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`,
"Moving the cloud"), and until its API moved to its own host the API was at `https://orchester.cloud/api/v1`, which
answers until the person switches it off. Nothing about where it runs is in the code: the backend and the web front read
`ORCHESTER_CLOUD_PUBLIC_URL` (the base of every link in an email and the issuer of key sets and tokens, which stays
`https://orchester.cloud` whatever host the API answers on),
`ORCHESTER_CLOUD_DATABASE_URL`, `ORCHESTER_CLOUD_SMTP_URL` and `ORCHESTER_CLOUD_SMTP_FROM`,
`ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY`, and the web front its own variables (`ORCHESTER_CLOUD_EXTRA_ORIGINS` for a
second domain during a move). The desktop reads the cloud's web and API addresses from its build configuration
(`ORCHESTER_CLOUD_URL` and `ORCHESTER_CLOUD_API_URL` at build time, defaulting to `https://orchester.cloud` and
`https://api.orchester.cloud`; a build that names only `ORCHESTER_CLOUD_URL` calls `/api/v1` there) and from Settings ›
Orchester Cloud's advanced field, for a test or self-run cloud, where an empty API address means `/api/v1` on the web
address; at its start it moves an address stored with the public cloud's former URL or its web address alone to the
two addresses, keeping the sign-in.

**API** (`/v1` on the API address, or `/api/v1` on the web address of a cloud without one; JSON, bearer cloud device
token unless marked):

| Method and path | Who | Does |
| --- | --- | --- |
| `POST /auth/signup` | none | `{ email, password, display_name }`: records a pending sign-up on a new confirmation link (its own password hash and display name; earlier unconfirmed links of the email stay valid) and sends it; the same answer whether or not the email is registered (a registered one gets a *you already have an account* email) |
| `POST /auth/confirm` | none | `{ link_token, password }` (row 65): checks the password against the hash stored on that link; on success creates the account with that link's password and display name and ends every other link of the email. Always one argon2 verification (against a fixed dummy hash when the link is unknown, used or expired), so the work is the same either way; one answer for any failure, `401 confirm_failed`, saying neither which part was wrong; rate-limited |
| `POST /auth/sessions` | none | `{ email, password, device }`: `201 { token, account }`; with two-factor authentication on, `202 { challenge }`, which `POST /auth/sessions/two-factor { challenge, code }` finishes; `401 invalid_credentials`, `403 email_unconfirmed` |
| `POST /auth/sessions/current/two-factor` | device | `{ code }`: this device becomes a two-factor session (step-up) |
| `POST /me/two-factor`, `…/confirm`, `…/recovery-codes`, `DELETE /me/two-factor` | device | Two-factor authentication: on with the password and a code (the recovery codes once), new recovery codes with a code, off with the password and a code |
| `POST /auth/reset` | none | `{ email }`: sends a reset link; the same answer for any email |
| `POST /auth/reset/finish` | none | `{ link_token, new_password }`: `204` |
| `POST /auth/password` | device | `{ current_password, new_password }`: `204`; other devices signed out |
| `DELETE /auth/sessions/current` | device | Sign this device out |
| `GET /me`, `PATCH /me`, `DELETE /me` | device | The account (display name; a new email is confirmed first) and its two-factor authentication as this device sees it; deleting removes everything the cloud holds about it |
| `GET /devices`, `DELETE /devices/{id}` | device | The account's cloud devices |
| `GET /keys` | none | The current key set, signed by the root key |
| `POST /identity-tokens` | device | `{ audience: server id, invite?: id }`: a token for one team server ([Identity tokens and keys](#identity-tokens-and-keys)); one that accepts an invite only for a two-factor session |
| `GET /servers` | device | The team servers the account owns or joined, with address and fingerprint |
| `POST /servers` | device | Links a team server: `{ link }`, the statement the team server signed at setup; `201 { server }` |
| `PATCH /servers/{id}`, `DELETE /servers/{id}` | owner device, or the server's key | Address or name changed; unlink |
| `POST /invites` | the team server's key | `{ invite }`, signed by the team server (sent by the server, or carried by the inviter's browser from the server admin); `202 { server }`, the server as registered. Limited per client address, 100 new invites a day per owning account, and invite emails in buckets of their own (1 a day per server and address, 3 a day per address; security review 1, M2) |
| `GET /invites` | device | Invites waiting for this account, each saying whether this device needs two-factor authentication to accept it |
| `GET /servers/{id}` | device | A linked server as registered: `{ server: { id, name, address, fingerprint, check_code } }`; the app pins its fingerprint when it accepts an invite to it, with an identity token that names the invite |
| `POST /invites/{id}/decline` | device | The invitee declines: the cloud's copy ends, and no token can accept it any more |
| `DELETE /invites/{id}` | the team server's key | Revoked by the team server; the id stays revoked for that server (`revoked_invites`, keyed by id and server) until the invite would have expired (security review 1, minor 1). A delivery is matched to a delivered or revoked invite by id and server, never by id alone (security review 2) |

**Data** (PostgreSQL, row 54; ids `uuid`, times `timestamptz` written from the cloud's clock, payloads `jsonb`):
`accounts` (id, email stored lowercased with a unique index, confirmed at, display name, password hash, created; a
row exists only once an email is confirmed), `email_links` (hash, email, purpose: confirm or reset, account for a
reset, pending password hash and display name for a confirmation, expires, used), `devices` (account, name, platform, token hash, created / last used / revoked, and why: `revoked`, `password_reset` or `signed_out`), `ended_tokens` (a deleted account's token hashes with `account_deleted`, until they would have ended unused), `servers` (id, owner,
name, address, fingerprint, public key, linked / unlinked), `server_accounts` (server, account, first joined),
`invites` (id, server, project address, kind, email, inviter display name, project name, role, expires),
`two_factor` (account, sealed TOTP secret, last step accepted, confirmed at), `recovery_codes` (HMAC, used at),
`sign_in_challenges` (hash, account, device, a digest of the password hash checked, tries, expires, used),
`devices.two_factor` (the authenticator a two-factor session used),
`revoked_invites` (id, server, until the invite would have expired), `used_link_statements` (a link statement's nonce
until it is too old to be taken), `used_tokens` (identity token id until its expiry), `signing_keys` (kid, public key, private key encrypted with a key
from the host's environment, not before / not after), `audit_events`, `schema_migrations`. Later: `providers`
(account, provider, subject). Expired `email_links`, `used_tokens`, `used_link_statements`, `revoked_invites`, `ended_tokens` and
ended invites are deleted by a sweep every minute.

**How a team server reaches it.** A team server with the internet sends its invites and fetches `GET /keys` itself.
A team server without it relies on its people: the inviter's browser carries the server-signed invite from the server
admin to the cloud, and every app that joins (and every cloud sign-in to the server admin) carries the latest key set
(below). A team server never needs the cloud for work already joined.

### The cloud's web front

Row 61. A small Next.js app, rendered on the server, with simple forms in Orchester's design. It is the cloud's
only browser face; the desktop keeps using the JSON API.

| Page | Path | Does |
| --- | --- | --- |
| Sign up | `/signup` | Email, display name, password; then *Check your email* |
| Sign in | `/signin` | Email and password |
| Two-factor code | `/signin/code` | The code from the authenticator app, or a recovery code, for an account with two-factor authentication; the challenge waits in a sealed cookie for its 5 minutes |
| Email confirmed | `/confirm/{token}` | The landing of the confirmation link: a **Password** field (the one chosen at sign-up, row 65) and *Confirm email*, sent as a `POST` from the page's server side, never on a bare `GET`, so mail scanners that fetch links do not use it up; a failure says only *This link and password don't match. Check the password, or send a new link.* (the approved design `CVRpO`), with the field emptied and not marked, and *Send a new link*; then *Your email is confirmed* and *Sign in on the web*. *Open Orchester* waits for an `orchester://` URL scheme the desktop registers; until then the page does not show it |
| Forgot password | `/forgot` | Email; the same answer for any address |
| Reset password | `/reset/{token}` | New password; then *Signed out of your other devices* |
| Account | `/account` | Get started, invites (saying when accepting needs two-factor authentication), servers, devices, *Security* (two-factor authentication on with a QR code, the recovery codes shown once with copy and download, new codes, off, and *Verify* for a browser that signed in without a code), display name, password change |

The desktop opens these pages on the cloud's web address in the system browser for *Create an account* and *Forgot
password*; it signs in with `POST /v1/auth/sessions` on the API address itself. Emails link to them (`/confirm/…`, `/reset/…`; an invite to an unregistered
address links to `/signup?invite=…`).

- **Who calls the API.** Only the web app's server side, over the compose network to the actix backend; the browser
  never calls the API and never holds an API token. The web app sends the client's address in `X-Forwarded-For`, and
  the backend lists the web app's address in its trusted proxies, so S1a's per-address limits see the real client.
- **Sessions.** Signing in on the web gets a cloud session token of kind `web` (shorter: 12 hours, ended by Sign out,
  a password reset or deleting the account). The web app keeps it in one cookie: `__Host-orchester_session`, `Secure`,
  `HttpOnly`, `SameSite=Lax`, `Path=/`, no `Domain`. Pages read the account through it on the server side.
- **CSRF.** Every form is a `POST` handled on the server side, which checks that `Origin` is the cloud's own base URL
  and that the form's hidden token matches the per-session CSRF token (a signed double submit; before sign-in, a
  short-lived pre-session cookie carries it). `SameSite=Lax` stops cross-site `POST`s too; the two checks do not rely
  on each other.
- **Links and tokens in URLs.** Confirmation and reset tokens are one-time, short-lived and only in the email's link.
  The pages send `Referrer-Policy: strict-origin`, load no third-party resources, and Caddy's access log drops query
  strings and the token segment of `/confirm/` and `/reset/`; the web app logs paths without them. `strict-origin`, not
  `no-referrer` (coordinator's decision): a `Referer` never carries a path or a query, so never a link's token, while
  forms keep sending their `Origin`; under `no-referrer` browsers send `Origin: null` with a form `POST`, which defeats
  the `Origin` check above.
- **Headers.** A strict Content Security Policy (self only, no inline scripts beyond Next.js' nonce), `X-Frame-Options:
  DENY`, HSTS from Caddy.
- **Design and words.** The screens are drawn in Pencil first (`data/orchester2.pen`, a *Cloud web* layer using the
  file's design variables) and approved by the person before the web app is built; backend work does not wait for
  it. Colors, type and spacing come from those variables, exported to one CSS variables file; every string is in
  `web/cloud/locales/en.json` (rule 13 applies to the web app too).
- **Where it lives.** `web/cloud/` (TypeScript, Next.js App Router, no client-side data fetching beyond form posts),
  with its own `package.json`, lint, type check and tests; `tools/check.ps1` runs them when Node is present and says
  so when it is not. Configuration comes only from the environment: `ORCHESTER_CLOUD_PUBLIC_URL`,
  `ORCHESTER_CLOUD_API_URL` (the backend on the compose network), `ORCHESTER_CLOUD_WEB_SECRET` (cookie and CSRF
  signing).
- **Deployed beside the backend** (`deploy/cloud/compose.yaml`): `caddy` (TLS for the public host name, from
  `ORCHESTER_CLOUD_PUBLIC_URL`), `cloud` (actix backend), `web` (Next.js server), and optionally `postgres`. Caddy
  routes `/api/*` to `cloud` and everything else to `web`; only Caddy is published.

### The server admin

Row 62. **Every admin action is here and only here** (row 63): the desktop does what a member needs (sign in to the
cloud, connect to a server, accept an invite and join by linking a checkout, work with shared tasks and configuration
and sync, *Make local*, show the connection state) and at most links to this admin. Each team server has a web app of
its own for administration, at the server's address
(`https://orchester.userdomain.com/admin`). It is admin only; task work stays in Orchester. Server-rendered Next.js,
like the cloud's web front, on the shared `web/ui` package ([The web workspace](#the-web-workspace)).

| Area | Who | Does | Team server API |
| --- | --- | --- | --- |
| First setup | whoever holds the setup code | The setup code from the server's console, the first admin's username and display name, then *Link to Orchester Cloud* | `POST /auth/code`, then the cloud link below |
| Projects | admins; owners for their projects | Create by name and slug (the slug fixed once created), rename, the repository remote hint | `POST /projects`, `PATCH /projects/{id}` |
| Members and roles | owners, admins | Per project: members, roles, the last-owner refusal, removing | `…/members` |
| Invites | owners (their projects), admins | By email through the cloud, or with a code without the internet; open invites, revoke | `/invites` |
| People | admins | Accounts, disable or enable, account codes, admin flag, unlinking an Orchester Cloud identity | `/admin/users`, `/admin/users/{id}/cloud`, `/invites { kind: "account" }` |
| Devices | admins | Every device joined to the server, with account, how it joined and last use; revoke. Under them, the browsers signed in to the admin; revoke (not drawn: the Joined section's frame) | `GET /admin/sessions`, `DELETE /admin/sessions/{id}`, `GET /admin/browsers`, `DELETE /admin/browsers/{id}` |
| Settings | admins | Join approval (row 57), the cloud link and its root key fingerprint (on, off, re-pin, unlink), the certificate's check code | `/server/settings`, `/server/cloud` |

A project owner who is not an admin sees only Projects (their own, rename and hint), Members and Invites for those.

**Signing in to the admin.**

- **Through Orchester Cloud.** *Sign in with Orchester Cloud* sends the browser to the cloud's web front with a
  form `POST` to `/authorize/start` (`purpose=admin`, `server=<id>`, `state`, `return=<admin URL>/admin/auth/callback`;
  never in a URL, security review 1). The cloud shows *Sign in to Kontur team's
  admin as alice@acme.dev?*; on confirmation it returns an identity token for that server (purpose `admin`, 5
  minutes, one use) by a form `POST` to the admin's callback, checked against `state`. The admin's server side sends
  it to the team server (`POST /auth/cloud { identity_token, device: { kind: "admin_web" } }`), which answers with a
  short-lived admin web token if the account is an admin or a project owner. Only the browser talks to the cloud; the
  team server needs no internet. **While join approval is on** (row 57) the team server refuses a cloud sign-in to
  the admin with `403 approval_required`, and the sign-in page says to make a code in Orchester on one of the
  person's computers (Settings › Orchester, the server's entry, *Sign in to the admin*): a cloud identity alone opens
  no more than it could join with. A browser signed in to the admin never answers its own account's approvals; one of
  the account's own computers, or another admin, does (security review 1, B2). The refusal is not drawn: it is
  `j8WujU`'s *code* path with its own sentence under the sign-in title.
- **With a code, offline.** *Sign in with a code*: a pairing code made in the desktop (Settings › Orchester, the
  server's entry, *Sign in to the admin*) or an account code from the host (`orchester-server account-code <username>`),
  redeemed with `POST /auth/code` as an `admin_web` device.
- **Linking the server to the cloud** at setup or later goes the same way: the admin's server side gets the server's
  signed link statement (`PATCH /server/cloud { link: start }`), the browser carries it in a form's body to the cloud's
  `/authorize/start` (purpose `link`), the cloud registers the server for the signed-in owner and returns its key set
  with an identity token, and the admin hands both to the team server, which pins the cloud's root.
- **A server's id is its own key's** (security review 1, B1): the team server's id is a hash of its Ed25519 public key
  (`PublicKey::server_id`), the server replaces a stored id that is not its key's with its key's at every start
  (logged; an earlier admin build's database, security review 2), and the cloud registers an id only from a statement
  that key signed, and drops migration 3 registrations whose id is not their key's. A statement works once (its `nonce`), and the newest one
  owns the server, so the server's admins can always take it back. Squatting a server's id with another key, and with
  it getting identity tokens whose audience is that server, is not possible.

**Sessions and CSRF**, the cloud web front's model: the admin web token (kind `admin_web`, 12 hours, revocable like a
device) lives only in the `__Host-orchester_admin` cookie, `Secure`, `HttpOnly`, `SameSite=Lax`, no `Domain`; every
call to the team server is made from the admin's server side; every form is a server-side `POST` checked for `Origin`
and a signed per-session CSRF token; `Referrer-Policy: strict-origin` (not `no-referrer`, which makes form `POST`s send
`Origin: null` and defeats the `Origin` check), a strict CSP, `X-Frame-Options: DENY`. Codes are
shown once and never logged.

**How it is served.** The Rust team server stays the front door, so a server has one address, one port and one
certificate in every setup:

- `orchester-server` answers `/api/*` itself and reverse-proxies every other path to the admin app on a private
  address (`admin_url`, default `http://127.0.0.1:3100`), with `hyper-util`'s client (already a dependency). `/`
  redirects to `/admin`; a project's own address (`/<slug>`) shows a page that names nothing and says to open it in
  Orchester. `admin`, `api`, `auth` and names starting with `_` or `.` are reserved slugs; a project an older server
  let take one gets the next free slug when the server opens (`admin` → `admin-2`, the S1c rule), logged as a warning
  (security review 1, minor 6). The server sets
  `X-Forwarded-For`, `-Proto` and `-Host` on what it forwards, replacing any the client sent; the admin listens only
  on the private address and trusts them (its own origin for the `Origin` check is their proto and host). The proto
  is `https` when the server presents its own certificate; behind a proxy that holds it (`tls = "off"`, Caddy) it is
  what a trusted proxy said in `X-Forwarded-Proto` (one value, `http` or `https`), else the scheme of `address`, else
  `http` (security review 1, M1). The listener settles that scheme once per request, and without `address` a project's
  address is that scheme and the host the request named: on the public listener for apps, and on the private one from
  what the admin passes on (found behind Caddy in the review's end-to-end run). Hop-by-hop headers and every header `Connection` names stay on their connection,
  both ways, and a client's `Forwarded`, `X-Forwarded-*`, `X-Real-IP` and `X-Middleware-Subrequest` (which would let
  a request skip the admin's own Next.js middleware) never reach the admin (minor 4).
- **The admin's server side calls the API** on a second, private plain-HTTP listener of the server
  (`admin_api_listen`, default `127.0.0.1:8741`, never published): the public port presents the server's own
  certificate, which the admin's Node would otherwise have to pin. That listener takes the browser's address and host
  from the forwarded headers the admin passes on, so the per-address limits count the browser and project addresses
  carry the host it used. The server warns at start when that address is not loopback or private (minor 6); the
  Caddy compose file binds it to the server's own address on its network.
- **Browser sign-ins** are device tokens of the kind `admin_web`: 12 hours from sign-in, refused on what only the app
  does (tasks, claims, events, linking, pairing), not listed among the account's computers, and issued only to admins
  and project owners (`403 nothing_to_manage` otherwise). Admins see them apart, in Devices › *Signed in to this
  admin* (`GET /admin/browsers`), and sign any one out (security review 1, minor 5). Identity tokens carry a `purpose` (`admin`, `link`, or none
  to join), and a team server takes each only for its own use.
- **Invites are made only with two-factor authentication** (2026-10-16, the person's decision: sending or accepting an
  invite, or entering a code, needs it). The team server makes one, by email or with a code, only for a device token
  that came with the `mfa` claim: a browser signed in to the admin through Orchester Cloud with two-factor
  authentication, an app that joined with a code or a cloud identity with it, or a browser signed in with a pairing code
  such an app made (the code keeps `codes.mfa`). Any other device token (a setup or account code's sign-in, or a cloud
  sign-in without it) gets `403 mfa_required`, which the admin shows as *Turn on two-factor authentication in your
  Orchester Cloud account to send invites* with a link to the cloud's `/account#security`, and for a code as *Making
  invites, codes too, needs two-factor authentication*. Invites made before by an admin without it are still delivered
  (only new ones are held to it); accepting any of them needs it.
- **Invites by email** are delivered to the cloud by the admin's own server side when the host has the internet, or
  posted from the inviter's browser to the cloud's web front (`/invites/deliver`) when it has not; the team server
  itself never reaches the cloud. The browser's form carries only the signed invite: the cloud's page leads back to
  the admin at the address it registered for the server, never to one the form or a link named (security review 1,
  minor 2).
- **The HTTPS page** (*This page needs HTTPS*) names the address the browser used, which `proxy.ts` passes to the page
  in a request header it signs with the admin's secret (Next reaches itself again for the rewrite, so the forwarded
  headers are its own by then); no `?origin=` or unsigned header puts words on it (security review 1, minor 2).
- **The cloud sign-in's state** cookie (`__Host-orchester_admin_state`) ends only with the answer that carries its
  `state`: a post to the callback with another state, which any site can send, leaves the trip as it was (security
  review 1, minor 3).
- **Docker:** two containers in `deploy/server/compose.yaml`, `server` (Rust) and `admin` (the Next.js standalone
  build on Node 22), on a private network; only `server` is published, or Caddy in front of it for a public domain
  (`tls = "off"`, as before). Caddy routes everything to `server`.
- **Without Docker:** the admin is a Node standalone folder started beside the binary (a service unit example ships
  with it); a server without it serves the API and answers admin paths with *the admin app is not running*, and the
  host commands remain for recovery. That page is in the design's face, Geist, carried inside the page under a CSP of
  `default-src 'none'; style-src 'unsafe-inline'; font-src data:` (the fonts and their OFL licence are in
  `crates/server/assets/fonts`).
- **Air-gapped:** both images (or the binary and the admin folder) are built elsewhere and copied in; nothing is
  fetched at runtime (no npm, no fonts or scripts from the internet: the fonts are bundled). The browser meets the
  server's own certificate: the admin's first page and the console both show its check code, and the team trusts the
  certificate in the browser after comparing it, or uses `tls = "pem"` with its internal CA. Plain HTTP is not
  supported for the admin (its cookies are `Secure`), except on loopback. Signing in uses codes when browsers cannot
  reach the cloud.

### The web workspace

Row 64. Both web apps share one component package:

```
web/
  package.json      npm workspaces: ["ui", "cloud", "server"]
  ui/               @orchester/ui: components, design tokens, i18n helpers
  cloud/            the cloud's web front (C1b)
  server/           the server admin (SA)
```

- **npm workspaces**, not pnpm, Yarn or Turborepo: npm ships with Node, so every build host (Windows, macOS, Linux
  and an air-gapped one) needs nothing extra; one `package-lock.json` at `web/` pins all three packages; three packages
  need no task runner. pnpm's stricter linking would be the reason to switch, and is not needed at this size.
- **`web/ui`** holds the React components (form, field, button, notice, table, page frame), the design tokens
  generated from the Pencil variables of `data/orchester2.pen` into one CSS variables file (a script, checked in with
  its output, re-run when the variables change), and the i18n helpers (a typed `t(key, params)` over a messages object
  and parity tests between locales). It has no words of its own: shared components take every string from the app,
  as props or through the app's messages.
- **Each app** has its own locale files (`web/cloud/locales/en.json`, `web/server/locales/en.json`), with key and
  placeholder parity between shipped locales checked by a test.
- **Rules.** AGENTS.md's code-quality, i18n and design-token rules apply to all of `web/`: no literal user-facing
  strings, no color, size or spacing literal outside the tokens, small files and functions, typed errors, no fake
  success. `tools/check.ps1` runs `npm --prefix web run check` (lint, type check, tests for all three) when Node is
  present and says plainly when it is not.

### Identity tokens and keys

Row 42; the rules here are the architect's.

- **Keys.** A **root key** (Ed25519) signs **key sets**. The cloud makes it at its first start and keeps it in its
  database, sealed with the key-encryption key from its host's environment as the signing keys are; nobody runs a
  key ceremony (changed 2026-10-16: the root was first kept on the person's offline machine, and deploying needed
  manual signing steps the person rejected). A key set lists the current **signing keys** (Ed25519, `kid`, valid
  from / until) and expires 30 days after it was issued; the cloud signs and publishes a new one itself at least
  weekly and after every rotation, and rotates a signing key a week before it stops signing
  (`docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`, "Keys"). A key-encryption key that does not open the stored root stops
  the cloud's start; it never makes a new root silently. A signing key signs identity tokens for 30 days and stays
  listed 60 more, so a token signed just before a rotation still verifies. The root does not roll over: a new root
  (after a lost key-encryption key or a compromise) is pinned again on each team server by its owner.
- **At link** (team server setup, below), the team server pins the root key's fingerprint; the owner sees it in
  Sharing and on the console (`orchester-server cloud show`).
- **Caching.** The team server keeps the newest key set it verified with the pinned root. It replaces it from `GET
  /keys` every 24 hours when it can reach the cloud, and from any key set an app sends with a sign-in that is newer.
  It accepts a token only while its cached key set is unexpired and lists the token's `kid` as valid at the token's
  time. Without the internet and without joining apps for 30 days, cloud sign-ins stop with *the server's copy of
  Orchester Cloud's keys is out of date*; devices already joined are unaffected, and codes still work.
- **Two-factor claim.** An identity token carries `mfa: true` when the cloud device it was issued to is a two-factor
  session. The team server keeps it on the device token it issues (`sessions.mfa`, schema 5): only such a token sends
  invites by email. A join token that accepts an invite without it is refused (`403 mfa_required`) before anything is
  used: the cloud issues none.
- **Identity token.** Compact JWS (`EdDSA`): `iss` the cloud, `aud` one team server id, `sub` the cloud account,
  `email` (verified), `name`, `device` (name and platform), `invite` (optional), `jti`, `iat`, `exp` = `iat` + 5
  minutes. Accepted once (`jti` remembered until `exp`), with 2 minutes of clock skew.
- **Joining** (`POST /auth/cloud` on the team server, `{ identity_token, key_set? }`): the server verifies, then finds
  the account linked to `sub`, or the open invite named by `invite` for that verified email, or refuses. It issues a
  device token as S1a issued session tokens. The email is not stored; only `sub` and the display name. With join
  approval on (row 57), a cloud join of an existing account answers `202 { approval }` instead, the account's
  joined devices and the admins see *Approve this computer?* (person-only), and the joining app finishes with the same
  request once it is approved, or learns it was refused. An account and cloud identity have one waiting approval at a
  time: asking again returns it. Each new approval counts against the client address like a failed join (asking
  costs the approvers' attention), the approval records that address, and approvals that ended (expired, refused,
  used) are removed.
- **Owner control.** `PATCH /server/cloud` (admin) or `orchester-server cloud off|on|pin <fingerprint>|unlink` on the
  host: *off* stops accepting identity tokens at once (joined devices keep working); *pin* replaces the pinned root
  (after a root compromise); *unlink* forgets the cloud and tells it when it can.

**Linking a team server at setup.** The server starts with an empty data directory and prints its check code and a
one-time **setup code** to its console (row 45). The owner opens the server admin (row 62), enters the setup code and
chooses a username; the server makes that account its first admin. *Link to Orchester Cloud* then carries the
server's signed link statement (server id, which is its public key's, a nonce, public key, address, `fingerprint`:
the certificate's SHA-256 in hex, the value apps pin, never the shorter check code) through the browser, in a form's
body, to the cloud's `/authorize/start`, where the owner is signed in; the cloud registers the server for that account (`POST /servers`) and the
browser brings back the cloud's key set, whose root the server pins ([The server admin](#the-server-admin)). Without
the internet the setup stops after the first step: the server is simply not linked, and can be linked later. The owner
then creates projects by name and slug and invites people, all in the server admin, and connects their own desktop
with an invite or pairing code.

### Invites

Row 43.

- **Two-factor authentication.** An invite is made, and accepted, only by a person with two-factor authentication on at
  Orchester Cloud, signed in with it: the sender's admin sign-in (by email or with a code), the invitee's device that
  asks for the token that accepts an invite by email, and the app that enters a code ([Codes](#codes)). The cloud still
  lists an invite to a device without it, saying so (`needs_two_factor`).
- **Who invites.** A project's owner or an admin invites to a project with a role; an admin invites to the server
  (an account with no project yet, optionally admin). Both in the server admin (row 62); the desktop's *Manage
  members and invites* opens it.
- **Flow.** The team server records the invite (`POST /invites`, kind, email, project and role, expiry 7 days) and
  sends it, signed with its key, to the cloud; when it cannot reach the cloud, the server admin's page posts the signed
  invite from the inviter's browser to the cloud. The cloud routes it to the invitee's app if the email is registered and sends one email
  either way (an unregistered address is asked to register first, and the invite waits for it until expiry).
- **Accepting.** The invitee's app lists the invite (Settings › Orchester, and a notice in All projects). Accept
  returns the team server's address, its fingerprint and a one-time identity token bound to the invite. The app pins
  the fingerprint, connects to the team server directly (the server's id, address and fingerprint go together: the
  app asks the cloud for a token only after the server at that address presented that certificate, so a token for one
  server reaches no other), joins (`POST /auth/cloud`), and for a project invite opens
  that project's join review: in the local project whose repository identity matches, or after the person adds the
  checkout. A project invite for someone already on the server only adds the membership; a server invite with
  *admin* makes the account that accepts it an admin. An account invite names one account: an identity already linked
  to another account is refused (`403 not_invited`) and the invite stays open. Acceptance checks again what making
  the invite checked: an account invite or code for an account that has become an admin, and an admin invite whose
  inviter is no longer an enabled admin, are refused (`403 forbidden`) and stay open.
- **Revoking.** The inviter or an admin revokes an open invite; the team server tells the cloud, which deletes it.
- **Outside Orchester.** The invite is also accepted on the cloud's Invites page, in the browser, or opened from there
  in Orchester through an `orchester://` link ([Accepting outside Orchester](#accepting-outside-orchester), row 77).

#### Accepting outside Orchester

Row 77 (ORC-576, 2026-10-31). Not built: the cloud's Invites page shows *Accept in Orchester* disabled, and someone
who works only in the team server's web app (members work on tasks in the browser too, 2026-10-27) cannot accept at
all. The desktop's side of the `orchester://` link below is built on Windows (ORC-578: the installer's registration,
the hand-over pipe, `crates/desktop-app/src/open_link`); macOS takes no links yet.

**What the cloud's Invites page offers.** Each waiting invite has three actions, in this order:

| Action | Kind | Does |
| --- | --- | --- |
| *Accept* | primary | Accepting in the browser (below): ends with the person on the team server, without the desktop |
| *Open in Orchester* | secondary | `orchester://invite/<id>`: opens the invite in the app, where its own *Accept* joins this computer |
| *Decline* | quiet | `POST /invites/{id}/decline`, as in the app, after *Decline the invite to …?* |

Two buttons rather than one with a choice: *Accept* is complete by itself, and *Open in Orchester* is only a shortcut
to the app's own Accept, which also connects a computer. Without a two-factor session the row shows, in place of
*Accept*, *Turn on two-factor authentication* (an account without it: `/account#security`) or *Confirm with your
code* (a browser session that signed in without one: stepping up, `POST /auth/sessions/current/two-factor`), and
*Open in Orchester* stays. The page's states, for the design: a waiting invite (to a project, to the server, for an
account); needs two-factor; *Continuing to <server>…* (the hand-off below); *<server> couldn't be reached* with *Try
again* and *Open in Orchester*; *Waiting for <server>* (accepted, no receipt yet); *You joined <project> on <server>*
with *Open in the browser* and *Open in Orchester*; ended (expired or revoked); refused (an identity the server links
to another account, an admin invite whose inviter is no longer an admin). They are drawn in the platform's
`web/design/orchester-web.pen`, on the cloud's *Account · Invites and servers* page beside its present row with the
disabled button, which they replace, and are approved before the page is built (row 61).

**Accepting in the browser.**

1. *Accept* is a form `POST` to the cloud's web front (`/invites/<id>/accept`, `Origin` and CSRF checked as every form
   there). Its server side asks the backend `POST /identity-tokens { audience, invite, purpose: "accept" }` with the
   browser's session. The backend checks what it checks for the app (a two-factor session, the invite unexpired, to
   the account's email, from that server, open or accepted by this same account), marks the invite accepted by the
   account (`accepted_at`), and issues the token: `purpose: "accept"`, `invite`, `mfa`, 5 minutes, one `jti`. Unlike
   the app's acceptance it records no join yet: the cloud records it when the receipt comes back (step 5).
2. The web front answers a page that hands the token on: a form whose `action` is the address the cloud registered for
   that server (`<address>/invite/accept`, never one the request, the invite or a link named), with `identity_token`
   and the newest `key_set` in its body, sent at once where the page's script runs and by its *Continue to <server>*
   button where it does not, as `/authorize` returns its tokens. The token is never in a URL, a history or a log.
3. The team server's web app takes the post only when its `Origin` is the cloud's public URL (the origin of the `iss`
   the server pinned at link), so another site cannot sign a browser in to someone else's account there. Its server
   side sends `POST /auth/cloud/accept { identity_token, key_set? }` to the team server's private API. The team server
   verifies the token as any other (its own `aud`, the pinned root's key set, `jti` once, 2 minutes of skew), takes a
   token of purpose `accept` here and nowhere else (`POST /auth/cloud` and `POST /auth/code` refuse it as `401
   invalid_identity`, and this endpoint refuses every other purpose so), requires `mfa` (`403 mfa_required`) and the
   open invite named by `invite` for the verified email. It then does what an app's join with that invite does, in one
   transaction: makes the account linked to `sub` (or finds the one linked to it, or for an account invite links the
   account it names), adds the membership with the invite's role for a project invite, sets admin for a server invite
   with *admin*, ends the invite as accepted, and audits it (`invite_accepted`, `via: browser`). The checks of
   [Invites](#invites) apply unchanged: an identity linked to another account is `403 not_invited`, an account invite
   for an admin or an admin invite whose inviter is no longer an enabled admin is `403 forbidden`; each leaves the
   invite open.
4. The answer: a browser sign-in to the web app (the browser kind of device token, 12 hours, as an admin's browser
   sign-in, issued to members too since they work in the browser) and a **receipt**, a statement of type
   `orchester-invite-accepted+jws` signed with the server's key: `{ id, server, subject, issued_at }`. No device token
   for an app: an app's device comes only from an app's own join. With join approval on (row 57) and an account that
   already existed on the server, the membership is still made (the invite grants it, and a forged identity gains
   nothing by it) but no browser is signed in: the page says to sign in with a code from one of the person's
   computers, as the admin's sign-in does.
5. The receipt goes back to the cloud by two ways, whichever comes first: the web app's server side sends it (`POST
   /invites/{id}/accepted { receipt }` on the cloud's API) when the host has the internet, and the page the browser
   lands on carries it in a form's body to the cloud's `/invites/joined` (*Back to Orchester Cloud*). The cloud checks
   it with the key the server linked with, records the join (the server is then in `GET /servers`) and drops the
   invite's row. A receipt that never comes leaves the invite *Waiting for <server>* until it expires.
6. The landing page on the team server: a project invite opens that project in the web app (*You're a member of
   Kontur*); a server invite opens the server's home with no project yet (*An owner adds you to projects*), or the
   admin for an admin invite; an account invite says the account is now linked. Each also offers *Open in Orchester*
   (`orchester://open`).

**When the browser cannot reach the server either** (a server on the team's own network, a VPN that is off, a
certificate the browser does not trust yet). The cloud cannot see that the post failed, so the hand-off page says it
before it goes: *Your browser now goes to <address> to finish. If it can't reach it, connect to <server>'s network and
choose Try again*, with *Try again* and *Open in Orchester* on the cloud's page the person comes back to. Nothing is
used up: the token is unused and ends in 5 minutes, the server's invite stays open, and the cloud keeps the invite
accepted for this account until it expires, so *Try again* asks a new token (an accepted invite issues tokens to the
account that accepted it, [cloud-api.md](../cloud-api.md), "Accepting an invite") and *Open in Orchester* finishes
it from a computer that reaches the server. For a server with its own certificate the page names its check code and
says to trust the certificate in the browser after comparing it, as for the admin ([The server
admin](#the-server-admin), *Air-gapped*).

**Ended invites.** An expired invite, or one the server revoked and told the cloud about, is not listed; a click on a
page loaded before answers *This invite has ended: it expired or was revoked* (the backend's `404`), and nothing is
issued. One the server revoked, or that expired there, while the cloud did not know yet gets a token, which the team
server refuses with `410 invite_ended`; its page says so and carries the server's signed revocation
(`statements::InviteRevocation`, for an expired one too) back to the cloud the way a receipt goes, and the cloud
deletes the invite. Revoking after acceptance changes nothing: the membership is then the server's to remove
(Members).

**What the desktop sees afterwards.** Nothing has to happen on a computer. Once the cloud has the receipt, Settings ›
Orchester lists the server under Team servers as *joined · this computer isn't connected*, with *Connect*: the [New
computers](#new-computers) flow (an identity token without `invite`; the account is linked to `sub`), which gives
this computer its own device token; while join approval is on that waits for an admin, since the account has no
computer yet. A project invite's project shows under the server, and linking a checkout opens its join review (the
invite landing `xeFRB`, then `AiCwO`), as for an invite accepted in the app. An invite accepted in the browser whose
receipt has not come back stays in the app's invites (`accepted_at` set) and its Accept works as today: the cloud
issues the token, and the team server admits the computer either way, finding the account linked to `sub` when the
browser's join did happen.

**The `orchester://` link.**

- **Forms.** `orchester://invite/<invite id>` opens that invite. `orchester://open` brings Orchester to the front (the
  cloud's email-confirmed page, the team server's landing). Nothing else is defined; a later use (a server, a
  project) gets a row here first. A link carries ids only, never a name, address, token, code or text.
- **What a link may do.** Bring the window to the front and open a screen the person could open themself. For an
  invite the app reads its own `GET /invites` and opens Settings › Orchester at that invite's row, with its existing
  Accept and Decline; the words on screen come from the cloud's listing, never from the link. An id that is not a
  UUID, an invite not listed for the account signed in here (another account's, ended, declined), any other path or
  any query string: the window comes to the front with *This link isn't for anything here* and nothing else happens.
  Signed out, the sign-in gate comes first and the invite opens after it. A link never accepts, declines, joins,
  connects, uploads or sends anything without the person's click, never reaches an agent, a chat, a prompt, memory or
  the app's MCP tools, and is logged by its form only (`orchester://invite/…`). A page can start the app, nothing more.
- **Windows registration.** The installer writes, per user, `HKCU\Software\Classes\orchester` (`URL Protocol`,
  `DefaultIcon`, and `shell\open\command` = `"{app}\orchester.exe" --open-link "%1"`), removed by the uninstaller.
  The app never registers it itself, so a development build, a QA copy or a zip install never takes links; installing
  into another folder takes them over. macOS (later): `CFBundleURLTypes` in the bundle's `Info.plist`, the link
  arriving as an Apple event to the running app.
- **One window takes it.** A process started with `--open-link` that finds the data folder's lock held
  (`orchester.lock.holder`, [stable-build.md](../stable-build.md), "Data") does not show *Orchester is already open*:
  it hands the link to the holder over a named pipe of that data folder (`\\.\pipe\orchester-link-<hash of the
  folder>`, the current user only, one line of at most 256 bytes) and exits. The holder parses it (the forwarder never
  does), brings its window to the front and opens it. With no app running, the started process opens as usual and
  opens the link once its window is up. A holder that does not answer within 2 seconds (an older build, one that is
  closing) gets the present *Orchester is already open* page.

**Contract changes** (`orchester-api`, [cloud-api.md](../cloud-api.md)):

- `TokenPurpose::Accept`; `IdentityTokenRequest` takes `invite` with purpose `accept` as well as with none.
- `CloudInvite.accepted_at: Option<i64>`: `GET /invites` also lists an invite accepted in the browser whose receipt has
  not come back.
- `statements::InviteAcceptance` (`orchester-invite-accepted+jws`, `{ id, server, subject, issued_at }`), signed by the
  team server, verified by the cloud.
- The cloud's `POST /invites/{id}/accepted { receipt }` (`InviteJoined`), `204`; the web front's `/invites/<id>/accept`
  and `/invites/joined` forms.
- The team server's `POST /auth/cloud/accept { identity_token, key_set? }` answering `{ browser_session?, membership:
  { kind, project?, role? }, receipt, approval_required }` and the new error code `invite_ended` (`410`, carrying the
  signed revocation in its details).

### New computers

Row 44. Signing in to the cloud on a new computer lists the team servers the account owns or joined (`GET
/servers`). For each, the app asks the cloud for an identity token, joins the team server, and lists the person's
projects there from the team server itself; each project is connected when the person opens or adds its checkout.
Each team server issues the new device its own token. Accounts that exist only on a team server (codes, row 45) add
a computer with a **pairing code** made on a device already joined (Settings › Orchester, the server's entry, *Add a
computer*).

### Codes

Row 45: address plus an 8-character code, for networks without the internet.

| Code | Made by | Valid | Gives |
| --- | --- | --- | --- |
| Setup | The server, on first start and at each start until it has an admin; printed to its console (stdout), never to the log | 24 hours | The first admin account and a device token |
| Invite | A project owner or an admin, in the server admin (*Invite with a code*) | 72 hours | A server account, or a project membership with a role, and a device token; a project invite opens the join review |
| Account | An admin, for an existing account that lost every device and is not an admin (in the server admin), or anyone with the host for any account (`orchester-server account-code <username>`). An admin's own next device comes from their own devices (a pairing code) or the host, never from another admin | 24 hours | A device token for that account, which it links to the cloud identity it is entered with |
| Pairing | A joined device, for the same account (Settings › Orchester, the server's entry) | 10 minutes | A device token for that account, or a sign-in to the server admin |

- **Shape.** 8 characters of the Crockford base32 alphabet (40 bits), shown `ABCD-EFGH`, read without regard to case
  or the hyphen. One-time: redeeming ends it.
- **Storage and comparison.** Only an HMAC-SHA-256 of the code, keyed by a random key in the server's data directory,
  is stored; a redemption computes it and compares with `ring::hmac::verify` (constant time). Codes are shown once,
  to the person who made them, and never logged, returned again or put in a URL.
- **Redeeming.** `POST /auth/code { code, device, username?, display_name?, identity_token?, key_set? }` (username and
  display name only when the code creates an account). After the TLS check-code confirmation described above.
- **Two-factor authentication** (2026-10-16). Every code but the setup code, entered in an app, needs the app's
  Orchester Cloud identity token for codes to this server (purpose `code`) whose `mfa` claim says its device signed in
  with two-factor authentication, with the key set the app carries. The server verifies it offline against the root it
  pinned, whether or not it takes cloud identities to join with (*cloud off*), uses its `jti` once, and the device token
  keeps the claim. No token, or one without the claim: `403 mfa_required`; a token the root does not vouch for, or one
  for another use (a join or admin token): `401 invalid_identity` (counted as a failed redemption); nothing is used up.
  A `code` token is taken nowhere else (`POST /auth/cloud` refuses it as `401 invalid_identity`).
- **Whose identity.** Redeeming a code links no cloud identity to an account, with one exception. A pairing code, made
  by a signed-in device of the account, takes only the identity the account is linked to (`401 invalid_identity` for
  another, and the code stays open), and with an account linked to none it links none: whoever redeems a leaked one
  gets a device, never the account's identity, and the owner keeps linking their own. An invite code makes an account
  linked to nothing; shown with an identity that an account here is linked to, it is refused with `409
  account_exists` and stays open: that person has an account here, and joins with it (a pairing code from one of its
  computers, their cloud identity, or an account code), not with a second one. The exception is the account code,
  the host's or an admin's way back for a person who lost every device or whose cloud account was made anew: it links
  the account to the identity shown, in place of the one before (audited `cloud_relinked`, or `cloud_linked` for an
  account linked to none), unless another account here is linked to that identity (`401 invalid_identity`). Otherwise
  an account is linked only from where its person is already signed in: a cloud join, or `POST /auth/cloud` from one
  of its devices.
- **Seeing and ending a link.** The server admin's People page shows how each account signs in and offers *Unlink
  Orchester Cloud* (`DELETE /admin/users/{id}/cloud`, audited `cloud_unlinked`); on the host, `orchester-server
  account-cloud <username>` prints the cloud account an account is linked to and `account-unlink <username>` ends the
  link. Neither an unlink nor an account code's relink revokes a session: the account's computers stay joined, and a
  computer that joined with the old identity keeps its device token until an admin signs it out (*Devices*) or the
  account is disabled. The unlinked identity signs in to nothing here until it is linked again.
- **Which server.** The app never asks the cloud for a token for an id on the server's word alone, since a server
  could name another one's and pass the token on. A server's id is its public key's (`PublicKey::server_id`). Before
  it asks, the app sends the server a fresh nonce (`POST /server/key-proof { nonce }`, 16 to 64 base64url characters);
  the server answers its public key and a statement of type `orchester-key-proof+jws` naming its id, that nonce and
  the SHA-256 of the certificate its connections present, signed with that key: its own certificate, or with `tls =
  "off"` the one its proxy presents, which the host configures or the server reads itself (`proxy_certificate`,
  `docs/server-hosting.md`; see *A proxy's certificate* below). The app checks that the key's id is the id the server gives itself, that the signature holds, and that the certificate
  is the one of the very connection that carried the proof (the pinned one, or the leaf the trust store verified, chain and name, on that connection); otherwise it sends nothing (*the server didn't prove it is the
  server it says it is*). A server that names another's id cannot sign for it, and one that passes the nonce on to the
  other and returns that one's proof is caught by the certificate. The one case the check cannot tell apart is two
  servers presenting the same certificate (one certificate shared by several servers, or a proxy in front of several
  with one certificate): either could pass on the other's proof, so give each server a certificate of its own. A
  server with TLS off and no `proxy_certificate`, or one reached over plain HTTP, names no certificate the app can
  compare, so the app asks for no token and the server takes no code but its setup code. An app that named its
  connection's certificate also accepts a proof whose `previous_certificate` names it (below). A `GET /server`
  that names no `codes_need_two_factor` is a server from before two-factor authentication: the app sends it the code
  alone, since its `CodeJoin` refuses unknown fields; this server keeps refusing them too, so an app and a server that
  disagree on the contract fail at once rather than drop a field.
- **A proxy's certificate.** Behind a proxy whose certificate renews on its own (Caddy or Traefik with Let's Encrypt,
  about every 60 days), a fixed `proxy_certificate` stops matching at each renewal, and codes fail until the host sets
  the new one. With `proxy_certificate = "auto"` (only with `tls = "off"` and an `https://` `address`) the server reads
  the certificate itself: it connects to its own configured `address`, as an app does, through the API client's
  verifier over the operating system's trust store, which checks the chain and the name, asks `GET /server`, and keeps
  the leaf that connection presented. It reads it at start and every hour (30 seconds after a read that failed), before a proof when the last read is older
  than ten minutes, and when an app names, in the header `Orchester-Presented-Certificate`, a certificate it has not
  seen (at most twice a minute, so requests cannot make it connect out at will). After it sees a renewal, it still
  names the certificate before it for an hour, as `previous_certificate`, but only to an app that sent the header, which
  then accepts a proof that names its connection's certificate as either: an app's connection made just before the
  proxy switched still presents the old one. An app from before sends no header and gets the statement it knows (its
  `KeyProof` refuses unknown fields); a server from before ignores the header and names one certificate. A read that
  fails (no route, no answer within ten seconds, a certificate the trust store does not accept for that name) forgets
  both: proofs name none, so no app asks for a code's token, until a read succeeds. The server logs why, at most every
  ten minutes; the reason names no secret. Security: a server that passes proofs on (A, relaying to B) cannot make B
  read A's certificate, since B connects only to its own configured address, never to one a request names; the header
  only makes B read that same address again sooner, and B names only certificates it read there itself. What remains
  is a hijack of B's own name (DNS, or a route to B's address) by someone holding a certificate the trust store accepts
  for that name: the same exposure every app connecting to B has. A wrong `address` (another server's name) would name
  that server's certificate, as a wrong fixed fingerprint would. The server needs a route to its own public name: a
  container behind a proxy on the same host may need hairpin NAT, or the proxy's address for the name (the Caddy
  compose file maps the name to Caddy's address on its network).
- **What it lasts.** A device token keeps `mfa` for as long as it lives: turning two-factor authentication off at the
  cloud is not seen here, and the device keeps making invites until an admin signs it out or it joins again. A server with no root pinned cannot check it
  and refuses those codes with `403 mfa_unverifiable`: it says so rather than let a code in without it. An air-gapped
  server pins the cloud's root on the host without linking (`orchester-server cloud pin <root public key>`, the key the
  cloud's log names); its people's apps then need the internet once per code, for the token and the key set, and the
  server none. A team with no internet anywhere cannot add people. The setup code needs no token (the server has no root
  yet, and the person at the host holds it); a browser's admin sign-in with a code carries none either, and its session
  has the claim only from a pairing code an app with it made. The desktop asks the cloud before it sends a code: with
  two-factor authentication off, or this computer signed in without it, it says so and sends nothing.
- **Rate limits.** 10 failed redemptions or cloud joins per client network (an IPv4 address, an IPv6 /48) in 15
  minutes block that network for 15 minutes. 200 failed redemptions on the whole server in any hour (a sliding hour)
  pause redemption until fewer than 200 remain in the last hour, at least 15 minutes; failed cloud joins have a
  server-wide budget of their own of the same size, so neither pauses the other. Redemptions and cloud joins from the
  host itself (loopback) are exempt from the server-wide budgets, so the person at the host can always redeem the
  setup code; their own address limit still holds. The pause is per server, not per person: over IPv4 about 5
  addresses (10 failures each per 15 minutes) can keep code redemption paused for everyone but loopback, which stops
  invite and pairing codes from other computers while it lasts; the loopback exemption only keeps the setup code
  usable on the host. At most 3 open pairing codes per account and 50 open codes per server. A guess succeeds with odds of 50 in 2^40 with 50 codes open, so the 200 failures an hour the server allows
  succeed with odds of about 1 in 650,000 a week and 1 in 12,500 a year (every hour spent guessing, 50 codes open
  all the time).
- **Linking later.** A code account links to a cloud identity from Settings › Orchester, the server's entry (*Link to
  Orchester Cloud*):
  the app joins with an identity token while signed in with the device token, and the server records `sub`.

### Device tokens

- **Format.** As S1a's session tokens: 32 random bytes, sent as `orc_` + 64 hex characters, stored as SHA-256. One per
  device per team server, never shown to anyone, kept in the OS keychain under `orchester-server:<server id>` /
  `token` through `orchester-vault`, a `SecretValue` (redacted in `Debug`, no `Display`).
- **Expiry.** 90 days without use (S1a: 30 days; with no password, expiry means rejoining through the cloud or a
  code, so it is longer; row 56). Last use is persisted only when it moved by more than a
  minute.
- **Revoking.** Signing the device out, an admin in the server admin's Devices, or disabling the
  account. Open event streams check their device at every wake-up and keep-alive and close when it ends. The outbox
  keeps every waiting change while a device is signed out.

### Migration of S1a accounts

Row 46. One forward migration of `orchester-server.db`:

- `users` keeps id, username, display name, admin and disabled; it drops the password hash and *must change
  password*, and gains `cloud_subject` (none yet). Memberships, claims, history and audit are unchanged.
- `sessions` become device tokens with their device names and times; a desktop signed in before the update stays
  signed in. Sessions still limited to a password change (signed in with an initial password) are revoked.
- Removed: `POST /auth/sessions` (sign-in), `POST /auth/password`, `POST /admin/users`, `POST
  /admin/users/{id}/password`, the host commands `user create` and `user reset-password`, and the password limiters.
  `GET /server` answers `needs_setup` instead of `needs_bootstrap`.
- A server that already has an admin prints no setup code. A person whose devices were all signed out gets an account
  code from an admin, or links their cloud identity through an invite naming their existing account
  (`POST /invites { kind: "account", user }`), which the cloud delivers like any invite.
- The old app's password sign-in stops working against the updated server; it is frozen (row 31).

### Kept from S1a

- **Client address and proxies.** The client address is the connection's, or the `X-Forwarded-For` hop added by a
  proxy listed in `trusted_proxies`; otherwise the header is ignored. Entries accept exact IPs or CIDRs. All header
  lines form an ordered comma list; malformed lists fall back to the connection peer. Walk from the right, skipping
  trusted proxies, to the first untrusted hop. IPv4-mapped IPv6 is canonicalized; hops may include ports or IPv6
  brackets. Failed-join counters group IPv6 clients by /48, the usual allocation of one site. A restart clears the
  counters, which is acceptable for v1.
  The redemption and cloud sign-in limits use this address.
- **Audit.** The server records joins (by code kind or cloud `kid` and `sub`, and address, never the code or the
  token), device revocations, invites made, redeemed and revoked, cloud link changes, user and membership
  changes, and claim takeovers, in the append-only `audit_events` table. User changes that leave every stored value
  unchanged create no audit event. Failed joins are counted, not audited one by one, so guessing cannot grow the
  append-only table without bound: `failed_joins` keeps a count per kind (code, cloud), client address and hour, for
  90 days.
- **HTTP deadlines.** Request headers and keep-alive idle connections have a 10-second deadline, as do body reads.
  Connections must send their first byte within 2 seconds (also the first HTTP byte after TLS). At most 256 HTTP
  connections (64 per untrusted peer) and 64 concurrent direct-TLS handshakes (32 per untrusted peer) are served;
  excess connections close immediately. Peers in `trusted_proxies` are exempt from per-peer caps, since a proxy
  multiplexes clients, but still count toward the global caps. Trust is checked on the actual connection address
  before /64 grouping. Peer caps normalize mapped IPv4 and group IPv6 by /64. TLS handshakes expire at 10 seconds.

## Security

| Concern | Rule |
| --- | --- |
| Tokens and codes | Device tokens, cloud device tokens, identity tokens, sign-in links and codes never appear in logs, errors, URLs or `Debug` output, on any side. The request log has method, route template, status, user id and duration, never headers or bodies. A test on each server signs in, works and asserts that no token or code appears in the captured log |
| TLS | The team server's own certificate, pinned by the desktop (row 39); or the team's PEM certificate; or a proxy's public certificate through the OS trust store. There is no "ignore certificate errors" switch. The cloud uses a public certificate |
| Plain HTTP | Accepted for loopback hosts; any other `http://` server needs the person's explicit opt-in for that server. Redirects are never followed |
| Membership | A project is visible only to its members: everything else answers `404`. Members read and change tasks, organization, tags and shared configuration, claim, and report progress for tasks they hold. Owners also manage members, roles, project invites and the project's name and key. Admins manage accounts, server invites, members and the cloud link, but see a project's data only as members |
| Discovery file | `orchester.toml` is untrusted. The desktop never contacts its address until the person confirms it |
| Published configuration | A version from another member can reach this computer's agents. MCP server and secret references are bound only after the person confirms them; a version that widens what agents may do waits for the person's *Use v<N> here* (row 35); neither can be done by a tool |
| At rest | The team servers' SQLite files are not encrypted; securing the hosts and backing up their data belong to whoever runs them. The cloud's PostgreSQL is reached over TLS with its own credentials from the host's environment; the person secures and backs up its database server. The cloud's root key and signing keys are encrypted with a key from its host's environment. The outbox is in the project database, beside the cache |

### The cloud's trust boundary

Row 47. Orchester Cloud vouches for who a person is; it is never on the path of project data.

| A compromised cloud can | It cannot |
| --- | --- |
| Forge identity tokens: join, as any cloud-linked account, a team server it can reach over the network that accepts the cloud | Read or change project data: none passes through it, and it holds no device token of any team server |
| Accept invites in flight as the invitee | Use devices that already joined, or act on a server that turned the cloud *off*, unlinked or re-pinned it |
| Send convincing invites that point to another server | Join a server with code-only accounts as them |
| See who is invited to which server, and which servers exist | Reach a team server on a closed network without a path to it |
| Refuse sign-ins (lock people out of new joins) | Stop joined devices from working |

**Limiting the damage.** Every cloud join is audited on the team server with its `kid`, `sub` and device, and the
account's other devices show *A new computer joined Kontur team as you* with *Sign it out*. A forged invite leads to
the join review, which shows the server's name, its owner and the repository identity before anything is uploaded.
An owner who distrusts the cloud turns it off or re-pins its root key on their server at once ([Identity tokens and
keys](#identity-tokens-and-keys)); joined devices keep working and codes remain. An owner who wants a second factor
against a forged join turns on *Join approval* (row 57, off by default): a cloud join of an existing account then
waits for one of that account's devices or an admin.

**Rate limits of the cloud.** Sign-in: S1a's limiters, moved (5 failures for an email in 15 minutes block it for 15
minutes, 20 failures from one address block that address, reserved before the hashing pool). Confirming (row 65):
5 failures for one link in 15 minutes end that link, and 20 failures from one address in 15 minutes block that
address, reserved before the hashing pool like sign-in. Confirmation and reset emails: 5 an hour per email, 20 an
hour per client address. Identity tokens: 60 an hour per device. Invites: 100 a day
per team server, 30 a day per inviter, 10 a day per invitee email. Failed link finishes: 20 per address in 15
minutes. All answer `429 rate_limited` with `Retry-After`.

**Confirming an email** (row 65). Whoever signs up first with someone else's email cannot take that account: a
link confirms only with the password that link was created with, so the email's owner who clicks the attacker's link
cannot complete it, and their own sign-up's link sets their own password. Every confirmation attempt costs the same
(one argon2 verification), and the answer never says whether the link or the password was wrong.

**Email and personal data.** The cloud keeps a confirmed email, a display name and a password hash per account,
and nothing else about the person (later, a provider's id when one is linked). An invitee's email is kept only while the invite is open; an
unregistered invitee gets one email and is not added anywhere. Logs carry account ids, never emails. Emails hold no
project content beyond the project's name and the inviter's display name. Team servers never store an email: they
keep the cloud account id and the display name. Deleting a cloud account deletes everything the cloud holds about it,
except the hashes of its tokens still in use then, with the time of the deletion and no account id or email: for 90
days, until they would have ended unused, they answer that the account was deleted (`docs/cloud-api.md`, "Why a token
ended").

**What never leaves the computer.** Transcripts, prompts, messages, conversations, memory, plans, questions and
answers, approvals' actions and decisions, diffs, file names, commits, worktree names, tool calls and their output,
token usage, agent and model names, environment variables, secret values, connection settings, accounts and their
credentials, MCP servers' programs, addresses and credentials, drafts, task and project sources, limits settings,
and the code: the server never touches a repository (row 49). What does leave it, to the team server only: task content and organization, claims, the
progress fields of row 29 (including step names, the branch name and the pull request's number, URL and state), and
published configuration: bodies and parts of pipelines, profiles, instructions, skills and tools, with the
**names and roles** of the connections, MCP servers and secrets they need, and the repository remote as a hint. To the cloud: the person's
email, display name and device names, and the invites they send. A test on each server asserts its database holds
nothing beyond these.

## Server data model

`orchester-server.db`, SQLite (WAL), with `meta.schema_version` and forward-only migrations. UUIDv7 ids, UTC
milliseconds. Rows marked *3* are new or changed for API 3.

| Table | Purpose |
| --- | --- |
| `meta` | schema version |
| `users` | id, username (unique, case-insensitive), display name, admin, disabled at, created; *3*: password hash and *must change password* dropped, `cloud_subject` (unique when set) added |
| `sessions` | device tokens: id, user, token hash (unique), device, created / last used / revoked; *3*: `via` (cloud, code, before 3) and the cloud `kid` it joined with |
| `codes` (*3*) | HMAC of the code, kind (setup, invite, account, pairing), the invite or account it serves, made by, expires, used at |
| `invites` (*3*) | id, kind, email (until the invite ends), project, role, account, made by, delivery, expires, accepted / revoked at |
| `cloud_link` (*3*) | at most one row: cloud id, pinned root key, the cached key set, accepting, linked / unlinked at |
| `used_identity_tokens` (*3*) | `jti` until its expiry |
| `server_keys` (*3*) | the server's Ed25519 key for invites and the link statement, and the HMAC key for codes (also kept in the data directory beside `tls/`) |
| `projects` | id, key, name, next task number, created by / at, expired event cursor floor; *3*: `slug` (unique, fixed), `remote_hint` (was the unique `remote`) |
| `project_batches` | user, idempotency key, first response; unique (user, key) |
| `memberships` | project, user, role, added by / at, membership lifetime id; unique (project, user) |
| `tasks` | id (kept on upload), project, number (unique per project), former key, content, status, organization fields, revision, has run, created / updated by and at; schema 7: `module` |
| `task_batches` | project, idempotency key, user, first response; unique (project, key) |
| `change_keys` (*3*) | project, user, idempotency key, first answer, at; unique (project, user, key); pruned after 30 days with the events. For task changes, tag creates, resource creates, publishes and resource changes; built in schema 7 for module creates and changes |
| `tags`, `task_tags`, `task_links` | as S1b |
| `organization_changes` | append-only history, as S1b |
| `modules` (schema 7) | id, project, name, normalized name, description, start and target days, state, revision, created / updated by and at; unique (project, normalized name) |
| `module_changes` (schema 7) | append-only: sequence, project, task, from module, to module, by, actor, at |
| `claims` | the current claim per task: id, task (unique), holder, kind, since, last heartbeat, taken over from; *3*: `stale_sent` (the sweep sent its stale event) |
| `claim_events` | append-only: task, claim, action, by, from, at |
| `run_status` | per task: holder, run number, phase, step, started at, phase since, updated at; *3*: step key, waiting for, steps (JSON: key, name, result), branch, pull request (number, URL, state) |
| `resources` (*3*) | id, project, kind, slug, name, archived, revision, latest version, created / updated by and at; unique (project, kind, slug) |
| `resource_versions` (*3*) | resource, number (unique per resource), body (JSON), hash, based on, published by / at |
| `files` (*3*) | project, sha256, size, content (zstd); unique (project, sha256): the parts of skill versions |
| `version_references` (*3*) | version, reference id, kind, name, role |
| `catalog_reports` (S5, row 75) | project, device session, reported at; unique (project, device session); removed with the session or the membership |
| `catalog_references`, `catalog_models` (S5, row 75) | per report: kind, name, role; role, model id. Nothing else about a connection or MCP server is stored |
| `events` | append-only: sequence, project, kind (*3*: adds `resources`; schema 7: adds `modules`, the table rebuilt with its sequence carried over; S2d: the delta kinds of [Live updates](#live-updates) replace them, row 73), by, actor, payload, at; older than 30 days removed |
| `audit_events` | append-only: actor, action, target, address, at |

**Numbers.** Task numbers come from `projects.next_task_number`, read and raised in the inserting transaction; version
numbers from `resources.latest` the same way. Numbers are for people; every reference is an id.

**Repository hint.** `RepositoryIdentity::from_remote_url` in core normalizes an `origin` fetch URL (credentials,
scheme and port dropped; scp-like `git@host:owner/name` becomes `host/owner/name`; host lowercased; trailing `.git`
and `/` removed; path compared without regard to case). Since row 49 it is a project's `remote_hint`, compared when a
checkout is linked; it no longer identifies the project and is not unique.

## Desktop data

**Application database.**

| Table | Purpose |
| --- | --- |
| `servers` (S2a; changed by rows 40, 50 and 74) | This device's connected team servers, listed in Settings › Orchester: id, base URL, the server's name, how it is trusted (`pinned` with the certificate's SHA-256, `system`, or `plain` with the opt-in), the account there (user id, username, display name, admin, cloud-linked), joined at. One device token per server, in the keychain (`orchester-server:<server id>` / `token`), used by every project linked to a project on it. A row is removed when the device signs out of that server. Each row is owned by the Orchester Cloud account signed in when it was joined (`owner`, 2026-10-16; a cloud join, and a code join entered while signed in, take the account of the sign-in that asked the cloud for its token, even when another account signs in before the join finishes): the signed-in account sees and uses its own and this computer's, an address is listed once per owner, and a second account joining the same server gets its own row, device session and token there (the server issues one session per join and keys nothing on the device's name). A row without an owner (from before, or joined while signed out) is this computer's: it shows to everyone and no sign-in claims it (row 74); signing out of the cloud keeps the rows and tokens. Background sync goes on for every owner's projects through that owner's own servers |
| `cloud_account` (new, row 41) | At most one row: the cloud account id, email, display name, signed in at, the cloud's address. The cloud device token is in the keychain (`orchester-cloud` / `device`) |

A project reaches its server through its `sharing` row's `server`, as in S2b.

**Project database** (desktop-written; the runner never writes these):

| Table | Purpose |
| --- | --- |
| `tasks` (S2b, S2c; changed in S4) | `revision`; for a shared project the pipeline slug, former key, created / updated by, `sharing` (`shared`, `not_shared`), `kept_key`, `kept_tag_names`. S4: the row is the person's view; `number` is empty and `temporary_number` set while the task waits for its number |
| `sharing` (S2b) | at most one row: server, server project, key, role, joined at, event cursor, last synced at, removed at; *3*: the project's address and slug. S4 adds the server time of the last answered heartbeat and the clock offset |
| `task_claims`, `task_run_status` (S2b) | the cached claim and progress per task; S3 adds the fields of row 29 |
| `task_organization`, `project_tags`, `organization_changes` (ORC-39) | for a shared project, the cache of the server's organization, tags and history; S4: tags made offline carry a temporary id until confirmed |
| `modules`, `task_modules`, `module_changes` (ORC-48; ORC-78) | for a shared project, the cache of the server's modules (`modules.revision`, empty for a local project), each task's module and the server's moves (`module_changes.server_sequence`, unique when set; moves made before Share stay without one). A local module that became the server's of its name takes its id, in its moves too: the moves' module columns may be rewritten for that, nothing else of them |
| `joined_keys` (S2c) | every local task's key before the join, until Make local |
| `shared_tasks_confirmed` (S4) | per shared task: the revision, content and organization the server last confirmed, the base every waiting change merges on |
| `outbox` (S4) | the entries of [Entries](#entries) |
| `temporary_keys` (S4) | task, temporary key, assigned key, assigned at, dismissed |
| `shared_automatic_status` (S2b) | folded into `outbox` by S4's migration; removed |
| `resources`, `resource_versions` (changed in S5) | `resources` gains `sharing` (`shared`, `not_shared`, empty for local) and the server revision; `resource_versions` gains the server's number, publisher, whether it is sent (`published here, not sent`) and whether this computer uses it yet (row 35); its `body` is the local body |
| `shared_version_bodies` (S5) | per cached version: the server's body, hash and references |
| `skill_files` (SK, row 37) | sha256, size, executable, content (zstd): every part of every skill version and draft, local or shared; replaces the blob files skills used |
| `reference_bindings` (S5) | reference id, kind, name, role, the local connection, MCP server or secret (or none), how (`name`, `role`, `person`), when |
| `joined_resources` (S5) | every local resource adopted or kept *Not shared* at the join, with its local id before, until Make local |

`storage.md` gets these rows when each phase lands.

## Migration

**Share (a local project → a new server project).** An admin creates the project by name and slug in the server admin
(row 62). The first checkout linked to it in the desktop (Sharing › *Link to a project*) uploads every task with
`POST …/link` in one transaction. Task ids and numbers are kept, and the server's counter continues after
the highest. Tasks with a run that has not ended are claimed for the uploader. The checkout's `origin` becomes the
project's `remote_hint`. Then (S5) `POST …/resources/import` uploads every pipeline, agent profile, instruction, skill
and tool with its id, slug, name, archived flag and every published version, numbers and times kept, in batches under
1 MiB (skill parts first, by hash), each with its own idempotency key. References are made from the local
connections, MCP servers and secrets the versions name, and this computer's bindings are those same entities, so
nothing changes for its runs. Drafts stay local, based on the same versions. The desktop writes the `sharing` row with
the project's address and offers to write `orchester.toml`; it never commits it. A project created empty on the
server keeps the first linked checkout's numbers (row 12); later checkouts join with new numbers.

**Join (a colleague).** See [Two people share one repository](#two-people-share-one-repository). The local project id
does not change, since the keychain names project secrets by it.

**Make local (any member, on their own computer).** The person's claims are released first when the server can be
reached; otherwise they go stale on their own. Waiting outbox entries are not sent: they are already in the person's
view, which becomes the local data, and the desktop says how many changes never reached the server and which tasks
they touched. The `sharing` row is removed. The tasks as the person sees them become local tasks with their ids and
numbers; tasks with temporary keys get numbers after the highest. Claims and other people's progress are dropped.
*Not shared* tasks return with new numbers after the highest. Shared resources become local resources with every
cached version, ids and numbers kept; their local bodies already name local entities, and references left unbound
stay visible as missing. Resources kept *Not shared* at the join return as they were. The server project is unchanged
for everyone else.

### Projects made under row 10

Projects created before API 3 are identified by their repository remote. The server's migration gives each a slug
from the last segment of its remote identity (`github.com/acme/kontur` → `kontur`; `kontur-2` and on when taken),
keeps the remote as its `remote_hint`, and keeps its id, so every desktop's `sharing` row still points at it. A desktop
that opens such a link reads the project's address from `GET /projects/{id}` and stores it. The owner can see the
given slug in Sharing; slugs are fixed afterwards (row 58).

### Two people share one repository

Two people each have the repository checked out, each with different local tasks and configuration, and both work
in the same server project.

**Identity and creation** (row 49). The project's address is its identity. An admin creates it, empty or from their
local project. There is no race on the repository: two projects may even hold work on the same repository.

**Finding the project.** An invite brings it: accepting a project invite (rows 43 and 51, or an invite code, row 45)
connects to the server if this device is not connected yet and opens the project's join review in the local project
whose `origin` matches the project's hint, or asks which local project or checkout to link (with the mismatch warning
of [Servers and project addresses](#servers-and-project-addresses)). A person can also enter a project address in
Sharing; `orchester.toml` prefills it.

**Membership.** Joining needs membership (row 17), per project (row 50). A connected person who is not a member of the
address they entered learns nothing of the project (the server answers as if there were none) and sees *Ask for an
invite* from whoever shared it. Nothing is uploaded.

**Review before joining: tasks.** If the joiner's local project has tasks, Join opens a review list of them, all
ticked, each with the server's similar titles as a hint (`sharing::similar_titles`). The person unticks duplicates. A
task with a run that has not ended cannot be unticked and is uploaded claimed for the joiner.

**Review before joining: configuration (S5).** The same page lists the joiner's pipelines, agent profiles,
instructions, skills and tools. Each is matched to the server's by kind and slug, then by name:

| Local resource | Choice (default first) | Result |
| --- | --- | --- |
| Same slug, latest versions equal by hash | *Use the team's* | The team's resource replaces it; the local one is kept *Not shared* with its history |
| Same slug, different content | *Use the team's, keep mine as a draft* / *Upload as a new one* (a free slug, typed) | The team's resource, with the local latest body as the person's draft on top of the team's latest version; or a new shared resource |
| No match | *Upload* (ticked) / *Keep on this computer* | Uploaded with its versions, numbers kept; or kept *Not shared* |

- References between the joiner's resources are rewritten before upload (a local pipeline that names a local profile
  adopted as the team's names the team's profile), with core's `import` reference rewrite.
- The team's versions' references are bound on this computer by the rules of [Shared
  configuration](#shared-configuration); what cannot be bound is listed on the review page as *missing on this
  computer* and can be bound there or later.
- Local runs keep the version snapshots they started with; nothing about a past run changes.
- A *Not shared* resource is read-only, used by no new run, and can be uploaded later with *Share*, like a *Not
  shared* task.

**Upload and numbers.** Confirming links the checkout and uploads the ticked tasks in one `POST …/link` (an import,
or, into a project with no tasks yet, an upload that keeps numbers), then the ticked resources with `POST
…/resources/import`. Each task gets a new number, keeps its id and records its former key. The
local project takes the server project's key. After the join the desktop shows the old-to-new mapping, which stays in
the sharing settings. Runs, branches, worktrees and texts are handled as before:

| Carries the old key | What happens |
| --- | --- |
| Local runs, their checkpoints, conversation and memory | Attached by task id, so unchanged |
| Run branches (`orchester/kon-12-run-1`) | Keep their names; the next run is `orchester/orc-41-run-2` |
| Worktrees | Named by run id, so unaffected |
| References in briefs, criteria and messages (`see KON-12`) | Not rewritten: *was KON-12* and the mapping explain them, and searching a former key finds the task. (Temporary keys of row 27 are different: they are rewritten in unsent text, see [Temporary numbers](#temporary-numbers)) |

**Unticked tasks: archived as *Not shared*.** They stay in the project database with their runs, conversation and
memory, read-only and out of the task list, behind a *Not shared* filter, with their old key. *Share task* uploads one
later through the same import. Keeping them as live local-only tasks beside the shared list was rejected: the list
would have two sources, and the rules for them have no good answers.

**Not shared:** daily token budgets and usage, conversations and memory, connections, CLI accounts, secrets,
environment, task and project sources, limits settings, the person's local pipelines (row 74), and the code itself.

## Platforms

- **Desktop, Windows and macOS.** The client is reqwest with rustls and the OS trust store, or the pinned-certificate
  verifier, on both. The tokens go through `orchester-vault`. Cloud sign-in opens the system browser and listens on a
  loopback port for the redirect, which works the same on both. The outbox, the stream and the bindings are plain Rust over SQLite and HTTP; nothing
  platform-specific is new. The macOS side is checked with `cargo check`/`clippy --target aarch64-apple-darwin`.
- **Server.** Supported hosting is Linux (x86_64 and aarch64) and Docker. The binary also builds and runs on Windows
  and macOS (it is a workspace member, AGENTS.md rule 0), and the tests start it on loopback there.
- **Server configuration** (`server.toml`, or the same keys from the environment): `listen`, `data_dir`, `name`,
  `tls` (`self_signed` by default, `pem`, `off`), `tls_names`, `tls_cert` and `tls_key` (for `pem`),
  `trusted_proxies`, `cloud` (the cloud's address, or `none` for a server that never links). Nothing in it needs the
  internet.
- **Host commands** (API 3): `serve`, `backup`, `tls show`, `tls regenerate`, `setup-code` (a fresh setup code while
  the server has no admin), `account-code <username>`, `account-cloud <username>`, `account-unlink <username>`,
  `cloud show|on|off|pin <fingerprint>|unlink`. Codes are printed
  only to the command's own output.
- **Cloud.** `orchester-cloud` and `web/cloud` run on Linux and Docker behind Caddy at the public URL from the
  environment (`orchester.cloud`; `orchester-cloud.lurq.dev` too during the move), with the PostgreSQL connection, the SMTP relay and the
  key-encryption key from the environment (later, the Google and GitHub client credentials);
  `deploy/cloud/compose.yaml` has `caddy`, `cloud`, `web` and an optional `postgres`. It builds on Windows and macOS like
  every workspace member. Its database tests need a PostgreSQL named by `ORCHESTER_CLOUD_TEST_DATABASE_URL` (a local
  Docker `postgres` on every platform); each test creates and drops its own database, and without the variable the
  database tests fail with that message rather than pass. The pure parts (tokens, key sets, limiters, codes) are
  tested without a database.

## Phasing

Each phase ends with `tools/check.ps1` passing and a clean `cargo check --target aarch64-apple-darwin`. Phases with
UI pass design verification (rule 15). Server tests live in `crates/server/tests/` and start a real server on loopback
with a temporary data directory and a settable clock; the workspace's sharing tests start one too. The desktop phases
are built in the new app only; their order, files, frames and tests are in
[server-new-app-plan.md](server-new-app-plan.md).

**S1a · Server and accounts.** Done (merged). Its passwords are removed by S1c.

**S1b · Projects, tasks, claims and events on the server.** Done (merged 2026-10-04; evidence
[server-s1b-2026-10-04.md](../evidence/server-s1b-2026-10-04.md)). Open minor, closed by S2d: heartbeats emit no
events.

**S2a · Servers and sign-in; S2b · Share, members, Make local; S2c · Join.** Done in the deprecated app (merged
2026-10-04/05); the service is in `crates/workspace`. The new app builds no S2a Settings › Servers; it connects to
each server once and links each project (rows 49–50, plan phase N1) and gets S2b's and S2c's screens in N2 and N3.

**C1a · Orchester Cloud backend** (rows 41–42, 47–48, 54, 59–60). Before the team server's sign-in rework.
- [ ] `crates/cloud` (`orchester-cloud`) builds on Windows, macOS and Linux; Docker image and compose example with
  PostgreSQL; the migrations apply once with two instances starting together, and a newer database is refused.
- [ ] Sign-up with email and password, confirmation and reset by email (SMTP to a test relay in the tests), sign-in
  with S1a's moved limiters, devices, account deletion. Magic link, Google and GitHub are later.
- [ ] The root key (made by the cloud at its first start, sealed in its database), signing keys, signed key sets,
  rotation with the settable clock, all kept by the cloud itself; identity tokens with audience, `jti` and 5-minute expiry; `api` `identity` tests
  over explicit keys and times.
- [ ] Team server registry from a server-signed link statement; invites in flight, routed and emailed, deleted when
  they end; the rate limits; a log test finds no email, link, code or token. Every address and credential from the
  environment.

**C1b · Orchester Cloud web front** (rows 61, 64). After the person approves its Pencil screens; C1a does not wait.
- [ ] The `web/` npm workspace with `web/ui` (components, tokens from the Pencil variables, i18n helpers) and
  `web/cloud`; shared components take every word from the app.
- [ ] `web/cloud/`: sign-up, sign-in, the confirmation landing, forgot and reset password against C1a's API, called
  from the server side only; the session cookie and CSRF checks; no token in logs or referrers.
- [ ] Lint, type check and tests; the compose example with Caddy routing `/api/*` to the backend and the rest to the
  web app; design verification of each page against its Pencil frame at the same size.

**S1c · Team server: certificate, devices, codes, invites, cloud link, project addresses** (rows 39–40, 42–47,
49–51).
- [ ] The self-signed certificate on first start, its check code on the console, `tls` modes, `tls regenerate`; the
  client's pinned-certificate verifier refuses any other certificate.
- [ ] Setup, invite, account and pairing codes: HMAC storage, constant-time comparison, lifetimes with the settable
  clock, one-time use, the rate limits; printed only to the console or shown once.
- [ ] `POST /auth/cloud` against identity tokens from a test cloud: audience, replay, expiry, key set caching and
  replacement by a newer one an app carries, an expired key set refused, *off* and re-pinning.
- [ ] Invites by email (to the test cloud, or returned signed for the server admin to carry) and by code; a project invite adds
  the membership; revoking tells the cloud.
- [ ] Projects by slug and address: admins create them, `?slug=` finds them, `POST …/link` keeps numbers for the
  first checkout into an empty project; `remote_hint` instead of the unique remote.
- [ ] The S1a migration: a populated schema keeps users, memberships and device tokens; initial-password sessions
  revoked; the removed endpoints answer `404`; `argon2` gone from the server. Projects made under row 10 get slugs
  ([Projects made under row 10](#projects-made-under-row-10)).

**SA · Server admin** (rows 62–64). After S1c and C1b's `web/ui`; its Pencil screens approved first.
- [ ] `web/server`: setup with the setup code, the cloud link, projects, members and roles, invites by email and by
  code, people, devices, settings, sign-in through the cloud's `/authorize` (added to `web/cloud`) or a code; the
  session cookie and CSRF checks.
- [ ] The team server proxies non-API paths to the admin; reserved slugs; the compose example with `server` and
  `admin`; an air-gapped run with no outside request.
- [ ] Design verification of each page in a browser against its frame.

**SK · Skills in the database** (row 37). Local; before S5.
- [ ] Core `SkillLimits` and the executable flag; store `skill_files` and the migration from blob files (a populated
  project keeps every part); the editor and Import from repository at the new limits.
- [ ] The runner places a step's skills in the run's private folder at each start of the execution and removes them
  when the step ends; leftovers are removed when a runner starts; `<repo>/.orchester/skills/` is removed once.
- [ ] Repository skill folders are no longer loaded: Codex always disables them, the native agent and `list_skills`
  offer Orchester's skills only; *Repository agent files* covers `AGENTS.md` only.

**S2d · Live task list and claims** (row 73).
- [ ] Server: the event kinds of [Live updates](#live-updates) replace `task`, `members`, `tags`, `project`, `modules`
  and `reset`; every delta carries `id`, `by`, `actor` and `at`, and `events` rows keep them. Tests: each change
  endpoint appends its delta with the right fields (a `task.changed` holds only the fields that changed, with
  `revision` and `base`); a tag delete or merge appends `tag.deleted`/`tag.merged` and a `task.changed` per rewritten
  task.
- [ ] Server: the connect snapshot. A stream without a cursor, or with one older than the 30-day retention, gets
  `snapshot.begin`, `snapshot.tasks` pages of 64 and `snapshot.end { cursor }`, then the deltas after that cursor; a
  cursor inside the retention gets only the deltas after it. A test with 130 tasks sees three pages, and a change made
  while the snapshot is sent arrives as a delta after `snapshot.end`.
- [ ] Server: the staleness sweep and its `task.claim` deltas; a test with the settable clock sees `stale` when a claim
  becomes stale and `refreshed` when it is refreshed, and none for a refresh of a fresh claim. Take, release, takeover
  and a merge's completion send `taken`, `released`, `taken_over` and `completed`.
- [ ] Server: `task.run` on every accepted progress report, with `number`, `phase`, `step`, `phase_since` and
  `updated_at` only.
- [ ] The SSE parser moved to `orchester-api`; the client reads the event stream and its kinds.
- [ ] One stream per open shared project, resumed from the cached cursor, with backoff; the snapshot replaces the cache
  in one transaction at `snapshot.end`; a `task.changed` whose `base` is not the cached revision makes the desktop
  read that task again; its own changes are skipped by revision. Two workspaces on one project see each other's
  changes within a second on loopback. The minute poll is gone.
- [ ] Take, Release and Take over on the task row and page; a lost claim is announced.

**S3 · Shared run progress** (extended by row 29).
- [ ] `PUT …/run` on the server with the fields of row 29, merged completing and cancelled releasing the claim, a
  non-holder refused.
- [ ] Core derives the report from a local run (`sharing::progress`); tests over explicit runs for every phase,
  each waiting kind and each step result.
- [ ] The holder's desktop reports on every run and pull request change; other members see phase, step, waiting
  kind, step results, branch and pull request on the task row and page.
- [ ] A takeover while the old holder's run works leaves that run going, tells its owner, and their reports stop
  being accepted.
- [ ] A test asserts that the server's database holds no run data beyond these fields.

**S4 · Offline and the outbox** (rows 27–28, 32–33).
- [ ] Server: idempotency keys on task changes and tag creates (`change_keys`), `409 tag_exists { tag }`, API 3.
- [ ] Core: `sharing::outbox` holding rules, `TemporaryTaskKey`, the offline start rule; tests over explicit lists
  and times.
- [ ] Store: `outbox`, `shared_tasks_confirmed`, `temporary_keys`; `shared_automatic_status` folded in; migration of a
  populated database.
- [ ] Workspace: `RemoteTaskStore` records and returns offline; the sender with ordering, coalescing, keys, backoff,
  conflict merge and refusals; temporary keys replaced and texts rewritten.
- [ ] Loopback tests: edits made with the server stopped are sent in order after it starts; a lost answer (a proxy
  drops it) is applied once; two people editing one task offline end with organization merged and content both changed
  in `conflict`; a task created offline gets its number and the waiting texts naming it are rewritten, and nothing
  starts until the person presses *Start*; offline starts follow row 28 with the settable clock.
- [ ] Screens: the status bar's waiting counts, the offline notice, the per-task sync marks, the conflict and refused
  views, the temporary-key notice, *Waiting to send* in Sharing.

**S5 · Shared configuration** (rows 30, 35–37, 75).
- [ ] Server: `resources`, `resource_versions`, `version_references`, `files`, the endpoints of API 3 with core's body
  checks; the `resource.published`, `resource.renamed` and `resource.archived` deltas (each with `by`, `actor` and
  `at`) and the project's resources in the connect snapshot's `snapshot.begin` (row 73; they replace the planned
  `resources` kind).
- [ ] The desktop applies the resource deltas to its resource cache and reads a published version's body with
  `GET …/versions`; a stream test sees each of the three deltas name who made the change.
- [ ] Core: references of a body and their resolution through bindings, reusing `import`'s walk; *widens* over two
  versions.
- [ ] Store and workspace: the resource cache with local bodies, bindings, `RemoteResourceStore`, skill parts uploaded
  by hash, publishing through the outbox with `version_conflict` for the person, *Use v<N> here*.
- [ ] Share uploads resources with their history; Join's review adopts, uploads or keeps them; Make local keeps them.
- [ ] Screens: shared marks in the libraries, *Publish for the team*, the version publisher, *missing on this
  computer* and the binding picker, *Use v<N> here*, the join review's configuration part.
- [ ] Loopback tests: a profile published on one computer runs on another with its connection bound by role; an MCP
  server reference stays unbound until the person confirms; a widening version waits for *Use v<N> here* while runs
  keep the previous one; two publishes from one base give the second a `version_conflict`; a skill with parts
  round-trips by hash.
- [x] Row 75 contract: `orchester-api`'s `catalog.rs` and `client/catalog.rs` (ORC-451); `crates/api/tests/resources`
  filter `catalog`.
- [x] Row 75 desktop report (ORC-452): core's `sharing::catalog` builds it from the local connections and MCP servers
  (agent connections only, at most 256 references and 512 ids per role); the workspace's `sharing/sync/catalog.rs`
  sends it while the project's stream is open, on each connect and then every minute when it changed;
  `crates/workspace/tests/live_updates` filter `catalog`. *What linking sends* and *What joining sends* name it.
- [ ] Row 75: core's `NetworkAccess::Hosts` with its order and *widens*, each connection's enforcement or refusal at
  preflight; the server's `catalog_*` tables, `PUT …/catalog` and the two reads (`GET …/references`,
  `GET …/models`). A loopback test: two desktops report, the reads list the union with no device or person, and a
  revoked device's entries leave.

**ORC-78 · Modules in shared projects** (rows 66–71). Built 2026-10-22.
- [x] Server: schema 7 (`modules`, `module_changes`, `tasks.module`, `change_keys`, the `modules` event kind with the
  event sequence carried over), `GET`/`POST`/`PATCH …/modules` with core's rules, revision conflicts, `422 module`,
  idempotency keys, and modules in link and import uploads; `crates/server/tests/sharing/modules.rs`.
- [x] Store and workspace: the module cache, `RemoteTaskStore`'s module writes through `TaskWriter` (the Modules page,
  the task form's and task page's module picker, the domain tools), sync of modules and moves, Share and Join
  uploads, Make local; `crates/workspace/tests/shared_modules.rs` on loopback.
- [ ] With S4: module creates and changes in the outbox (row 71); with S2d: `module.created` and `module.changed`,
  and a `task.changed` delta's `module`, applied from the stream (row 73).

**Later.** Cloud sign-in by magic link, Google and GitHub (row 48); comments on tasks; notifications; Postgres for
several server instances.

## Open questions

None. Rows 31–61 settled every question of the 2026-10-15 revision.
