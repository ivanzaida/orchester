# Orchester Cloud API

The JSON API of `orchester-cloud` (`crates/cloud`): accounts, devices, identity tokens, the team server registry,
invites in flight (C1a, with SA's linking and admin sign-in) and app releases ([App releases](#app-releases)).
The design is in [architecture/server.md](architecture/server.md), "Orchester Cloud" and "Identity tokens and keys";
running it is in `docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-cloud`. The wire types are in `orchester-api`'s `cloud` module, and
the token and key set checks team servers use are in its `identity` feature. The release types are in its `releases`
module, and the check of a release file's signature in its `release-signature` feature.

Two kinds of client call it:

- **The desktop app.** It signs in with the person's email and password and keeps the device token it gets back in
  the OS keychain (`orchester-cloud` / `device`).
- **The web front** (Next.js, `web/cloud`). It shows sign-up, sign-in, the confirmation landing and forgot/reset
  password pages. Its **server** calls this API; browsers never call it directly, so the API sends no CORS headers.
  It signs in with platform `web` and keeps the token in the `__Host-orchester_session` cookie (`HttpOnly`, `Secure`,
  `SameSite=Lax`), never in script-readable storage. The web front's server must forward the browser's address in `X-Forwarded-For` and be
  listed in `ORCHESTER_CLOUD_TRUSTED_PROXIES`. Otherwise every browser counts as the web front's one address in the
  rate limits.

## Team server reviewer resource

This is the team server's shared configuration contract, exported by `orchester-api`, rather than a cloud
inference endpoint. The resource kind is `team_reviewer` (`SharedResourceKind::TeamReviewer`). Its body is core's
`TeamReviewer`, with the same immutable, numbered versions as the other shared resource kinds:

```text
language { kind: "language",
           connection: { kind: "connection", name, role: AgentProvider },
           model, effort: string | null }
decision { kind: "decision",
           connection: { kind: "decision_connection", name, role: "decision" },
           model, lowest_threshold: number,
           destinations: { kind: "members_computers" } | { kind: "hosts", hosts: [exact host] } }
```

Connection references name a kind, name and role for each computer to bind locally. They carry no computer-local
connection id, URL or credential. These named references are in the body; the version's separate `references`
array is empty for a reviewer. Models are nonempty model identifiers; `lowest_threshold` is a finite probability
from 0 through 1. A member may raise the effective threshold locally, never lower it. Decision destinations always
allow servers on the member's computer; `hosts` additionally allows exactly the listed remote DNS names or IP
addresses, case insensitive, without wildcards, schemes, ports or paths. An empty list allows no remote server.
Language reviewers carry effort instead of a threshold or decision destinations.

All paths below are under the team server's `/api/v1/projects/{project}`, with its normal project authentication:

| Method and path | Access | Shape |
| --- | --- | --- |
| `GET /resources` | member | `Resources { resources: [SharedResource], cursor }`, including reviewer metadata and latest version number |
| `POST /resources` | project owner or server admin | `ResourceCreate { idempotency_key, id, kind: "team_reviewer", slug, name, version: { body, references: [] } }`; `201 ResourceCreated { resource, version }` |
| `GET /resources/{resource}/versions?after=` | member | `Versions { versions: [SharedVersion], next? }`, oldest first, at most 64 versions or 1 MiB per page |
| `POST /resources/{resource}/versions` | project owner or server admin | `VersionPublish { idempotency_key, based_on, body, references: [] }`; `201 { version: SharedVersion }` |
| `PATCH /resources/{resource}` | project owner or server admin | existing rename/archive `ResourcePatch` |
| `GET /resources/{resource}/used-by` | member, project owner or server admin (whoever may archive it reads it first) | `UsedBy { resources: [UsedByResource], runs: [UsedByRun] }`; `404 not_found` for a resource the project does not have, a malformed resource id, or a caller who can neither read nor archive; `200` also for an archived resource |

A member's write returns `403 not_admin`. Publishing from a stale `based_on` returns `409 version_conflict`
with the latest version. Uploading history at `POST /resources/import` uses the existing `UploadResource` and
`UploadVersion` shapes and the same write access; the server computes each imported version's flag in number order.

`GET /resources/{resource}/used-by` (decision E of `docs/design/team-configuration.md`) is what the archive
confirmation reads before an admin archives a resource; any member may read it, and so may everyone who may archive
it (`PATCH …/resources/{resource}`: the project owner or a server admin, a member or not). It only reads: archiving still
cancels the runs it lists (row 79 of `docs/architecture/server.md`). The answer is
`UsedBy { resources: [UsedByResource { kind, id, name, version }], runs: [UsedByRun { task_key, run, holder? }] }`,
and both lists are empty when nothing uses the resource.

- **`resources`**: the other resources that are not archived and whose latest published version names this one, with
  the version that names it. The resource asked about is never in its own list, even when its latest version names
  itself. Ordered by `name` compared case-insensitively, then by `id`. A version *names* a resource by typed data, not
  by a text scan: the ids in a free-text field, a prompt or a description name nothing. It names resource `R` when
  - one of its `references` is a `TeamResource` with `R`'s server id, of any kind (a profile's MCP servers, tools,
    skills and instructions are among them; a `CliKind` or a `SecretName` names no resource), or
  - its body, read as the body type of its kind, holds `R`'s server resource id in one of these fields:
    - pipeline: a step's executor, `steps[].kind.agent.profile` or `steps[].kind.tool.tool`, and the Deliver step's
      `deliver.integrator`;
    - instruction: `targets.profiles[]`, and `scope.pipeline.pipeline` or `scope.step.pipeline`;
    - skill: `scope.pipeline.pipeline` or `scope.step.pipeline`;
    - tool: the pipelines its access policy lists, the keys of `access.callers`;
    - agent profile, team reviewer and MCP server: none in the body today; a profile names what it uses in its
      `references`.

  A body that is no body of its kind names nothing. The reference's `kind` is not compared with `R`'s: a reference of
  the wrong kind is refused when published, not read here.
- **`runs`**: the runs that have not ended and whose execution bundle pins any version of the resource, as the
  team server knows it from the bundle it built at `work.start` (`execution-bundles.md` of the platform): every
  pipeline, agent profile, tool, instruction and skill version the bundle pins, and no other. A run uses an MCP
  server only through the profile versions it pins. The execution bundle does not carry MCP servers yet (ORC-880; an
  `ExecutionProfile` leaves them out), so today the team server lists no runs for an MCP server; once the bundle carries the MCP server versions its profiles name,
  a run pins an MCP server when its bundle does. The desktop's own archive already stops a run whose pinned profile
  version chooses the server (`pins` in `crates/workspace/src/sharing/sync/archive.rs`), and its local list follows
  that rule. A team reviewer has no runs.
  - **When a run is listed**: the server learns a run id only from `work.report` (`WorkReport` carries `run_id`, no run
    number), so a run is listed from the first work report the server accepted for it, and its last accepted report
    decides: `progress` lists it; `succeeded`, `failed` (whatever its reason, `limit_reached` included) or `cancelled`
    ends it, and a run that resumes with `progress` after a `limit_reached` is listed again. Known limit: work that
    was started and has no accepted report yet is not listed. For a team project the list can therefore be narrower
    than what a member's desktop stops on archive (a run with no report yet, a run paused at a limit, a run that uses
    an MCP server). A run whose last accepted report is `progress` has not ended, nor has one that was interrupted or waits for
    a person on the desktop; a run whose runner stopped without a terminal report stays listed too: a stale claim
    (`stale_task_threshold`) does not end its run.
  - **Superseded**: a run whose work has a newer run (a later run id first reported for the same work) has ended,
    even when its own last report was `progress`.
  - **Shape and order**: each is `{ task_key, run, holder? }`: the task's key (`KON-12`), the run's id, and the
    `UserRef` who holds the task's claim now. After a take-over that is the new holder; `holder` is absent when the
    claim was released. There is no run number: the server has none. Ordered by the number part of the task key,
    numerically (`KON-3` before `KON-12`), then by run id (a stable tie-break).
- **Paging**: none. Both lists are returned whole: they are bounded by the project's resources and by its runs that
  have not ended.
- **Access and errors**: a member of the project reads it, and so does whoever may archive the resource (the project
  owner, or a server admin without membership, as `PATCH …/resources/{resource}` allows). Access is checked first, and
  one answer, `404 not_found`, serves a project the caller may not read, a resource the project does not have and an id
  that is no resource id, so it tells an outsider nothing. (`GET …/versions` answers `400` for a malformed id; used-by
  answers `404` for it, because the access check comes first.) An archived resource answers `200` like any other, so
  an admin can check what still uses it before a restore.

The client call is `used_by`. The stand-in team server answers it from its own resources, applying the names-it rule
above: the `TeamResource` references of every kind (a profile's MCP servers, tools, skills and instructions among them)
and the body fields read with core's body types, and from the runs a test registers with `TeamServer::pin_run`: it builds no execution
bundle, so the pins a test names stand in for the bundle's (an MCP server's id among them), a pinned run is one whose
first work report was accepted, and `last_report`, `work` (the later run of a work supersedes the earlier ones) and
`claim_run_task` stand in for the accepted work reports and the task's claim. It lists whatever the pins name, an
MCP server's id included, which a real bundle does not carry yet. It has no non-member, so that `404` is not
exercised there.

`SharedVersion` is `{ resource, number, body, hash, references, based_on, published, widens }`, with `references` as the
next section gives them.
The server computes `widens: boolean` with ORC-570's `TeamReviewer::widens_over` and keeps it on that immutable
version; it is never accepted from a publisher, including on import. Compare against the preceding published
version, or no previous body for v1. Adding a remote host or lowering an existing decision threshold widens;
removing hosts, raising the threshold, and language reviewers do not. A first decision version, or a switch from
language, compares destinations with members' computers and has no previous threshold to lower. Host order and case
do not widen. Older servers may omit the flag; API readers default it to false and still compare permissions locally
before applying a version.

The existing stream carries reviewer metadata in `snapshot.begin.resources` and the existing `resource.published`,
`resource.renamed` and `resource.archived` deltas. A publication carries the resource with its new `latest`;
members read the body and stored flag through the versions endpoint. No additional event kind is introduced.
The platform server and web tasks must update their repository pin to the commit containing this contract.

## Team resource references

A team resource refers only to other team resources, by their server id, and to secrets, by name; it never carries an
id or a path from somebody's computer (`docs/design/team-configuration.md`). A version's `references` (in
`SharedVersion`, `VersionContent`, `UploadVersion` and `VersionPublish`) is a list of `ReferenceTarget`, each tagged by
`type` and holding nothing else (the server refuses a reference with another field):

```text
{ type: "cli_kind",      cli: AgentProvider }                // "codex_cli" | "claude_code_cli" | "anthropic_api" | "openai_api"
{ type: "team_resource", kind: SharedResourceKind, id }      // an MCP server, tool, skill, instruction or agent profile
{ type: "secret_name",   name }                              // a capital letter, then capital letters, digits and "_"
```

- **`cli_kind`** replaces the reference to a connection. A profile names the CLI it runs with; each computer runs it on
  one of its own connections of that kind and the person picks when there are several. A version without one names no
  CLI.
- **`team_resource`** replaces the references to an MCP server (by a computer's connection id) and to a tool. `id` is
  the resource's id on the server, and `kind` its kind (`mcp_server`, `tool`, `skill`, `instruction`, `agent_profile`,
  `pipeline`, `team_reviewer`; never agent testing's kinds). The server accepts it only for a resource of that kind in
  the same project that is not archived.
- **`secret_name`** replaces the reference to a secret by a computer's secret id. The server holds the name, never a
  value; each computer fills in the value.

A body holds none of these ids either: it has no connection, local MCP server or secret id, which the server refuses
(`422 invalid { reason: "references" }` for a reference, `{ reason: "body" }` for a body). The kinds' bodies that still
name a connection, an MCP server or a secret by a computer's id (the agent profile and the tool) change in ORC-820 and
later tasks; until then a desktop sends no such body, and a server migration rewrites the versions that exist
(`team-configuration-handover.md`, OP-200). There is no compatibility window: a server and a desktop on different
versions of this contract do not read each other's `references`.

The desktop's local projects keep their local ids; this shape is the team body's. The execution bundle carries none
of these ids either: its `bindings` hold `{resource_id, role, cli, model}` of each pinned profile (below).

## Team server MCP server resource

An MCP server is a team resource of kind `mcp_server` (`SharedResourceKind::McpServer`), with the same immutable,
numbered versions and the same create, publish, rename and archive calls, access and errors as the reviewer above. Its
body is core's `TeamMcpServer`; `references` is empty. The team holds only what is the same on every computer:

```text
{ name,                       // lowercase letters, digits, "-" and "_", starting with a letter
  launch: { kind: "program",
            program,          // a name looked up on the PATH ("npx"), never a path
            args?: [string], working_directory?: string /* relative to the worktree */,
            variables?: [name] }
        | { kind: "remote", url, secret_headers?: [header name] },
  exempt_tools?: [tool name] } // tools that run without asking
```

The body has no other fields and the server refuses a body with one (`deny_unknown_fields`): no program path, no
variable or secret value, no connection, secret or MCP server id of any computer. Each computer sets the path of a
program, the values of the variables and of the secret headers, and whether the server is on, itself. Names within
`variables` and `secret_headers` are distinct, ignoring case. A working directory that is absolute is refused, as is a
program that is not a bare name.

The server computes `widens` with core's `SharedBody` rules for this kind: a version widens when
its program or arguments differ from the previous version's, its URL differs, it switches between a program and a
remote server, or it adds a tool that runs without asking. The first version does not widen, because a person turns a
team MCP server on at each computer. The stream carries it as it carries the other kinds: the metadata in
`snapshot.begin.resources` and the `resource.published`, `resource.renamed` and `resource.archived` deltas, with
`kind: "mcp_server"`; no new event kind. Libraries and pickers (`ResourceLibraryRequest`, `ResourcePickerRequest`)
accept `"mcp_server"` as `kind`.

## Base

The cloud has two addresses: its API address, which every API call goes to, and its web address (the public URL),
whose pages the app opens in the browser.

- **API:** `https://api.orchester.cloud/v1`, for example `https://api.orchester.cloud/v1/auth/sessions`. The paths
  below are under it.
- **Web pages** stay on `https://orchester.cloud`: `/signup`, `/signin`, `/account`, `/forgot`, `/invites/…` and the
  links in emails.
- **The old base**, `https://orchester.cloud/api/v1`, is the same service. It answers while app versions from before
  the API moved to its own host are in use, and is switched off after that. The desktop no longer calls it: what an
  earlier version stored with the public cloud (also its former address `https://orchester-cloud.lurq.dev`,
  `docs/cloud-hosting.md` in the private repository `ivanzaida/orchester-platform`, "Moving the cloud") moves to the
  two addresses at the start, without signing out.
- **Identity tokens and key sets** name the public URL, `https://orchester.cloud`, as their issuer, wherever the API
  answers, so team servers keep verifying them as before.
- **A test or self-run cloud** is set in the desktop's Settings › Orchester Cloud › Advanced, with its web address
  and, when its API has an address of its own, its API address (`/v1` there). Without an API address the desktop calls
  `/api/v1` on the web address, so a cloud with no separate API host keeps working. A build names another cloud with
  `ORCHESTER_CLOUD_URL` and, for its API address, `ORCHESTER_CLOUD_API_URL` at build time.
- Team servers keep `<server URL>/api/v1` ([architecture/server.md](architecture/server.md), "API").

## Team authority S2 shared contracts

The public prerequisite for platform OP-145 is in core and `orchester-api`; it does not add cloud endpoints.
Team server admins alone change `server_admin::ServerSettings.claim_staleness_seconds` through
`SettingsChanges.claim_staleness_seconds` (optional in changes); every reader of a workspace, member or Admin, reads it
on the v2 `WorkspaceMetadata` (S10, below). Core's `ClaimStalenessThreshold` is a JSON
integer in whole seconds, defaults to 900, and accepts 300..=86400 inclusive. Missing read fields from older
servers use the default; invalid supplied values are refused. Unknown settings-change fields remain refused.

The separate `orchester_api::v2` surface adds `WorkspacePolicyResponse`:
`{policy:{revision:1,automatic_merge:false},capabilities:{read_policy:true,change_policy:false}}`.
New workspaces default to automatic merge off at revision 1. Members read policy; workspace Admins change it;
server admins inherit both capabilities even without membership. Capabilities are computed from authoritative
records for this workspace and are response metadata, never authorization supplied by a caller.

`WorkspacePolicyChanges` is `{revision,automatic_merge}`; both fields are required, unknown fields refused.
Platform must authorize, compare the revision, persist the change and increment the revision atomically.
A stale revision returns HTTP 409 with typed `WorkspacePolicyError::PolicyRevisionConflict`:
`{error:"policy_revision_conflict",current:{revision:2,automatic_merge:true}}`.
`PolicyRevision` accepts positive integers through i64::MAX and refuses overflow on increment.
The effective policy returned is the persisted workspace policy, with no member override.

A policy change is announced by the v2 `WorkspacePolicyEvent::Changed` delta, `kind` `policy.changed`:
`{kind:"policy.changed",data:{id:7,by:{...},actor:"person",at:1,policy:{revision:2,automatic_merge:true}}}`.
`policy` is the new effective policy; clients replace their cached policy and ignore a revision that is not
newer than theirs. The server-wide claim staleness threshold has no change event (undecided).

Thresholds belong to the server, never a workspace policy. A member's compatible local Claude/Codex account
choice stays local execution metadata. Other team-owned defaults (pipeline, delivery, effort), admin drafts,
module heartbeat source and human-wait renewal are **not covered**; no decisions for those open items are
introduced here. The referenced team-authority-plan.md §12 is not present in this public checkout.
Existing v1 workspace contracts, events and the /api/v1 client are unchanged. Routes, persistence and server
enforcement belong to OP-145 in `ivanzaida/orchester-platform`; publishing these types does not enable routes.

## Team authority S4 command envelopes

The public prerequisite for platform OP-147 (idempotent action receipts) is in core and `orchester_api::v2`; it adds
no endpoints. Ids are UUID strings; `OperationId` (made by the client once per intent, repeated on every retry) and
`WorkId` (made by the server) are distinct core types, never a task id or a client-local run number. Unknown fields
are refused in every shape below.

- `Subject`: `{kind:"task",id}` or `{kind:"module",id}`.
- `ExpectedRevision`: `{kind:"project"|"task"|"module"|"tag"|"resource"|"work",id,revision}`, the id of that kind;
  `revision` is 1..=i64::MAX. A set naming one kind and id twice is refused.
- `CommandRequest`: `{operation_id,expected:[ExpectedRevision],command}`; `command` is internally tagged by `kind`
  (e.g. `{kind:"claim.acquire",subject}`).
- `CommandResult`: `{operation_id,state:"committed",result,cursor}`; `result` is the command's value, which carries
  `effects:[LocalEffect]` (empty when no local effect is authorized). `cursor` is the `EventCursor` just after the
  command's events.
- `EventCursor`: the server's committed event sequence position in one project's event stream. Its wire form is a
  string holding the canonical decimal of a positive integer that fits in 64 bits: digits only, no sign, no leading
  zeros (`"1"`, `"10"`; never `""`, `"0"`, `"-1"`, `"+1"`, `"007"` or `"c1"`), and anything else is refused
  (`EventCursorError`). Cursors order by that integer, never by string order (`"9"` < `"10"`; `EventCursor` is `Ord`).
  Clients may compare two cursors of the same project; cursors of different projects are not comparable.
- `LocalEffect`: `{kind:"start",work_id,bundle:{sha256,schema_version}}`, `{kind:"pause",work_id}`,
  `{kind:"stop",work_id}`, `{kind:"deliver",work_id}`, `{kind:"deliver_module",module_id,branch,source_commit,base_commit}`
  (S7, below) or `{kind:"none"}`; `sha256` is a 64-digit lowercase hex `ContentHash`.
- `OperationRead`: `{operation_id,state:"committed",response:CommandResult}` or
  `{operation_id,state:"rejected",error:AuthorityError}`.
- `AuthorityError`: `{operation_id?,code,details}`; `operation_id` is left out when the refusal came before the
  operation was read. `details` is `{}` for every code but `decision_required`, whose details are
  `{decisions:["D1".."D9"]}` with at least one decision, and `unsupported_contract`, whose details are
  `{required_version:"0.4.0"}` (ORC-733): the oldest desktop version the server accepts, a `ReleaseVersion`
  (`MAJOR.MINOR.PATCH` with an optional `-pre` suffix, no build metadata), so the desktop can say which Orchester the
  server needs. A code with another code's details, or details of two codes, is refused. Details never echo the
  request or carry credentials.

| Code | Status |
| --- | --- |
| `unauthenticated` | 401 |
| `forbidden`, `delivery_not_authorized`, `membership_revoked` | 403 |
| `not_found` | 404 |
| `revision_conflict`, `operation_mismatch`, `claim_conflict`, `claim_lost`, `module_required`, `module_claim_conflict`, `transition_denied`, `decision_required`, `bundle_missing`, `session_mismatch`, `sequence_regressed` | 409 |
| `cursor_expired` | 410 |
| `invalid` | 422 |
| `unsupported_contract` | 426 |
| `rate_limited` | 429 |
| `unavailable` | 503 |

The server keys a receipt by project, authenticated account and `operation_id`; the same id with another request is
`operation_mismatch`. No command or command value has a public payload type yet: the envelopes are generic over them
(`CommandRequest<C>`, `CommandResult<V>`, `OperationRead<V>`), and each later stage adds its own. Read models
(pages, task and module reads, capabilities) are the S10 contract's. Existing v1 contracts, the /api/v1 client and
earlier v2 types are unchanged.

## Team authority S10 read contract

The public prerequisite for platform OP-153 (bounded direct reads and server-side summaries) is in core and
`orchester_api::v2`; it adds no endpoints. It reuses S4's `EventCursor`, `Subject`, `DecisionId` and
`cursor_expired`. In the read models every field is sent: `?` below means `null`, never left out, and a missing or
unknown field is refused. Times are milliseconds since the Unix epoch; days are `YYYY-MM-DD`.

- `PageRequest`: `{after:PageCursor?,limit}`; `limit` may be left out for 50 and is 1..=200 (`PageRequestError`).
  `PageCursor` is an opaque, non-empty string, bound to the workspace, the filters, the sort and the snapshot; unlike
  `EventCursor` it is not ordered, and clients never read or compare it.
- `Page<T>`: `{items:[T],next:PageCursor?,cursor:EventCursor}`; `next` is `null` on the last page, and every page of
  one listing carries the same snapshot `cursor`. Entities come in ascending (creation sequence, id) order, histories
  in ascending sequence. A snapshot is kept at least 15 minutes (`PAGE_CURSOR_RETENTION_SECONDS`, 900) after the last
  page served from it; a later cursor may answer `cursor_expired` (410), and the client reads from the first page again.
- `Capabilities`: `{project_id,revision,role:"member"|"admin"|"server_admin",actions:[ActionCapability],unresolved:[DecisionId]}`.
  `ActionCapability`: `{action,subject:Subject?,allowed,reason:DenialReason?}`; `reason` is `null` exactly when
  `allowed`. `action` is a command kind (dotted lowercase words, e.g. `claim.acquire`; `CommandKind`), `read`,
  `export.shared` or `server.admin`. `DenialReason`: `role_required`, `claim_required`, `claim_lost`,
  `module_required`, `decision_required`, `configuration_invalid`. Capabilities are advisory; commands check again.
- `ClaimRead`: `{subject,id,holder:UserRef,acquired_at,last_execution_heartbeat_at,stale_at,stale,parent_module_claim_id?}`.
- `WorkRead`: `{id,subject,claim_id,revision,state,runs:[RunId],bundle:BundleRef,created_at,updated_at}`;
  `ExecutionState` is `running`, `paused`, `awaiting_human`, `succeeded`, `failed` or `cancelled` (proposed).
- `TaskRead`: every `SharedTask` field (the content's fields inline) and `assignee:UserRef?`, `work:WorkRead?`,
  `effective_claim:ClaimRead?`, `capabilities`. `workflow` stays apart from `work.state`.
- `ModuleRead`: every `SharedModule` field (its fields inline) and `branch`, `assignee?`, `work?`, `effective_claim?`,
  `capabilities`, `pending_delivery:PendingDeliveryRead?`. A module's tasks are read with the task list's `modules`
  filter.
- `PendingDeliveryRead` (ORC-733): the module's open delivery to base, from its `delivery.request` until the runner
  reports the merge or the delivery fails, `null` when there is none:
  `{requested_by:UserRef,requested_at,source_commit,base_commit,authorization}`. `requested_at` is when it was asked
  for; `source_commit` (the module branch head) and `base_commit` are S7's `CommitId`s and differ, or it is refused
  (`PendingDeliveryError::NothingToDeliver`). `authorization` (`PendingAuthorization`) is `awaiting_admin` (it waits
  for an Admin's `delivery.authorize`), `admin` (an Admin authorized it) or `automatic` (the workspace's
  `automatic_merge` authorized it, D7). An Admin's screen reads it as "Delivery to main waits for your authorization ·
  olha asked at 15:38". The full record stays `ModuleDeliveryRead` (S7).
- Task list: `TaskListRequest` `{filters,sort,page}`, each defaulted. `TaskFilters`
  `{statuses,priorities,tags,all_tags,pipelines,modules,search}`: each facet keeps tasks with any of its values, `null`
  among `pipelines` (team pipelines' resource ids, as a team task names its pipeline; `docs/architecture/server.md`
  row 80) or `modules` is a task without one, `all_tags` keeps tasks with every tag. `search`
  (`SearchText`, trimmed, 1..=200 characters) matches title, key, former key, id or brief, ignoring case. `TaskSort`:
  `created` (default), `newest`, `title`, `status`, `priority`, each ending on creation sequence and id.
  `TaskBoardCounts`: `{backlog,todo,in_progress,in_review,done,canceled,cursor}` for a `TaskFilters`.
- Modules: `ModuleListRequest` `{state?:"open"|"closed",page}` answers `Page<ModuleSummary>`, a summary being
  `{module:ModuleRead,progress:{unit:"tasks",scope,done},tasks,forecast:ForecastRead}`. `ForecastRead`:
  `{remaining,history_days,history_delivered,outlook}`, `outlook` `{kind:"done"}`, `{kind:"not_enough_history"}` or
  `{kind:"dates",likely,safe,past_horizon}`. `ModuleBurndownRead`: `{module,unit,start_scope,days,forecast,cursor}`,
  a day `{date,scope?,remaining?,ideal?,added,removed}`. `ModuleHistoryPage` is `Page<SharedModuleChange>`. All are
  counted by core's rules (`orchester_core::module::{progress, burndown}`, `orchester_core::forecast`): Canceled tasks
  are out of scope, Done counts as done, and every move into or out of the module counts on its day.
- Configuration: `ResourceLibraryRequest` `{kind?,archived,search?,page}` answers `ResourceLibraryPage`
  (`Page<SharedResource>`); a resource's versions are `ResourceVersionsPage` (`Page<SharedVersion>`, oldest first);
  `ResourcePickerRequest` `{kind,search?,page}` answers `ResourcePickerPage` of `ResourceChoice`
  `{id,kind,slug,name,latest}`, resources not archived with a published version.
- `WorkspaceMetadata`: `{project:SharedProject,policy:WorkspacePolicy,capabilities,cursor,claim_staleness_seconds}`.
  `claim_staleness_seconds` (ORC-733) is the server's `ClaimStalenessThreshold` in whole seconds (300..=86400), sent
  to every reader of the workspace, so a member's desktop can say when a claim goes stale; it is required, and a value
  out of range is refused. It stays the server's setting, not `WorkspacePolicy`'s, and only a server admin changes it
  (S2, above). A change to it makes every workspace's `{kind:"workspace"}` scope stale (S11).

Left open, not invented here (they rest on the open decisions D1–D9 or on later stages): human waits beyond
`awaiting_human` (questions, plans, approvals), the *Needs you* and *Active* list views and their counts that rest on
them, the full set of command kinds behind `action`, which time zone a server counts days in, and what `revision` in
`Capabilities` counts. Existing v1 contracts, the /api/v1 client and earlier v2 types are unchanged.

## Team authority S8 execution bundle

The public prerequisite for platform OP-151 (consistent immutable execution bundles and pinned skill assets) is in
core (`orchester_core::run::bundle`) and `orchester_api::v2`; it adds no endpoints. The server builds a bundle when it
starts work under a claim and never accepts one a client wrote. Unknown fields are refused in every shape below.

- `ExecutionBundle`, in this field order:
  `{schema_version,project_id,subject,claim_id,module_id,branch,base_commit,definition_revision,task_number,task,pipeline,profiles,permissions,limits,delivery,resources,files,skill_files,bindings,inputs}`.
  `schema_version` is `EXECUTION_BUNDLE_SCHEMA` (2), as `BundleRef.schema_version` names it; a bundle of another
  version is refused, with no compatibility shim (2 added `task_number` and the pinned versions' `label`, so a
  desktop and a server on different versions refuse each other's bundles). `module_id`, `branch`
  and `base_commit` are S7's (below): the module the work is under, its module branch (`BranchName`) and the base
  branch's commit the work starts from (`CommitId`). `definition_revision` is the task's revision when the work
  started; `task_number` is the task's number in its project from 1 (the `7` of `ORC-7`, which names the run and its
  branch with the project's key); `task` (`TaskAtStart`) is `{title,brief,criteria}` at that revision. `limits` is a
  `SharedLimits` (`{}` for none) and `delivery` what the run delivers (`changes`, `report`, `outside` or
  `changes_or_report`: the task's own choice, else its pipeline's).
- Derived from `inputs`, and refused on read when they differ: `pipeline` `{resource_id,version}`; `profiles`
  `[{role,resource_id,version}]`; `permissions` `[{step,permissions}]`, each agent step's profile permissions
  narrowed by the step, in pipeline step order; `resources` `[{resource_id,version}]`, the pinned instructions, tools
  and skills; `files` `[{sha256,size_bytes}]`, every file once; `skill_files`
  `[{resource_id,files:[{path,sha256,size_bytes}]}]`. `version` is the published version number.
- `bindings` `[{resource_id,role,cli,model}]`: one per pinned profile, with its role and model; `cli` is the agent
  provider (`codex_cli`, `claude_code_cli`, …) the executing computer runs it with. `resource_id` is the profile's server
  resource id; a binding holds no id of a computer's connection, MCP server or secret. Which account or connection runs
  it, and every path, stays on that computer.
- `inputs` (`CanonicalExecutionInputs`): `{pipeline,steps,profiles,instructions,tools,skills}`, each pinned version
  `{resource,version,number,label,body}` with its published body. `label` (`ResourceLabel`) is `{slug,name}`, the
  resource's slug and name as the pinned version had them, which prompts and skill folders name it by: the slug is
  1–64 lowercase letters, digits and single inner hyphens, the name is not blank and at most 200 characters; a
  malformed label is refused. `steps` are the run snapshot's `[{key,executor}]` in
  pipeline order. The pipeline body lists no sources, a profile body is
  `{role,responsibilities,instructions,model,settings,permissions}` without its connection, MCP servers or writable
  folders, and a command tool never starts a program by an absolute path; such inputs are refused. Bodies name
  secrets by id today; the tool body names them by name (`secret_name`, above) once ORC-820 and the tasks after it change it.
- Order: `steps` and `permissions` keep pipeline order; `profiles`, `bindings`, `resources`, `skill_files` and the
  pinned versions are read in any order and kept in resource id order, `files` in hash order, a skill's files in path
  order.
- Hash: the lowercase hex SHA-256 of the bundle's canonical bytes, its compact JSON as `serde_json` writes it with the
  fields in the order above and collections in the order just given (`run::bundle::canonical`). The same bundle always
  hashes the same, whichever order its collections were read in; any field change changes it.
- Reads: the bundle read answers `ExecutionBundle`. A bundle file read answers one file's raw bytes by its `sha256`,
  at most `MAX_BUNDLE_FILE_BYTES` (8 MiB; a run whose skills have a larger file makes no bundle), which the desktop
  checks against the bundle (`ExecutionBundle::check_file`). There is no confirmation command.

The work is not in the bundle: a bundle binds to its claim, and S7's `work.start` attaches the work. Existing v1
contracts, the /api/v1 client and earlier v2 types are unchanged.

## Team authority S7 delivery contract

The public prerequisite for platform OP-150 (mandatory modules, module branches and authorized delivery) is in core
and `orchester_api::v2`; it adds no endpoints. Unknown fields are refused in every shape below.

- `CommitId` (`orchester_core::run`): a full Git object id, 40 lowercase hex digits (SHA-1) or 64 (SHA-256), sent as
  a string. Abbreviated ids, uppercase and ref names are refused (`CommitIdError`). Branches are `BranchName`.
- The execution bundle gains `module_id:ModuleId`, `branch:BranchName` and `base_commit:CommitId` after `claim_id`
  (above). This changes the canonical bytes, so a bundle with these fields hashes differently from the same bundle
  in ORC-651's shape; nothing stored that shape yet, and `schema_version` stays 1.
- `DeliveryRead`:
  `{work_id,claim_id,authorization_operation_id:OperationId?,source_commit,base_commit,bundle:BundleRef,state,revision}`.
  `authorization_operation_id` is always sent, `null` when no `delivery.authorize` allowed the delivery; leaving it
  out is refused. `source_commit` and `base_commit` are `CommitId`s, `revision` is 1..=i64::MAX.
- `DeliveryState`: `authorized`, `requested`, `delivered` or `failed`. The server reports `delivered` only after the
  runner reported the merge commit.
- `LocalEffect` gains `{kind:"deliver",work_id}`: the runner may merge the work's module branch into base on its
  computer.
- Commands, instantiated by the server on the S4 envelopes rather than defined here: `delivery.authorize`
  `{work_id,claim_id,source_commit,base_commit,bundle}` (Admin) and `delivery.request`
  `{authority,authorization_operation_id,source_commit,base_commit,bundle}`, each answering
  `{kind:"delivery",delivery:DeliveryRead,effects}`.
- Module delivery (S7c, ORC-656): base delivery is a module's, and task works reach base only through their module
  (the run branch merges into the module branch locally; work reports tell the server). `ModuleDeliveryRead`:
  `{subject:{kind:"module",id},claim_id,authorization_operation_id:OperationId?,branch,source_commit,base_commit,state,revision}`,
  with the module's claim and branch and no `work_id` or `bundle`; a task `subject` is refused. It is a separate type,
  not a tagged `DeliveryRead`, so the task form above keeps its wire; the field sets are disjoint, so a body reads as
  one form only. `LocalEffect` gains `{kind:"deliver_module",module_id,branch,source_commit,base_commit}`: the runner
  may merge that module branch head into base at `base_commit` on its computer; no local path is sent. Authorized by
  an Admin's `delivery.authorize`, or by the workspace's `automatic_merge` for a request from the module's current
  effective claim (D7).

Decided for S7 (the orchestrator's D3 and D7, 2026-11-03, revisable by the person): the automatic module branch is
`module/<name-slug>` (with `-<8 hex of the module id>` on collision), fixed at first assignment and created by the
runner from its checkout's base at the first task start; task run branches merge into it, and only delivery takes it
to base. A manually assigned branch is unique in the workspace, may be an existing Git branch, and cannot change while
work under the module runs. With the workspace's `automatic_merge` on, a delivery request from the current effective
claim on current unfenced work counts as Admin-authorized; off, it needs an explicit `delivery.authorize`. Report and
outside deliverables have no base merge (D4 open). Existing v1 contracts, the /api/v1 client and earlier v2 types are
unchanged, apart from the bundle fields above.

## Team authority S9 work reports

The public prerequisite for platform OP-152's S9b (`work.report`) is in core (`orchester_core::run`) and
`orchester_api::v2`; it adds no endpoints. Unknown fields are refused in every shape below.

- `ExecutionOutcome` (core): `{kind:"progress"}`, `{kind:"succeeded"}`, `{kind:"failed",reason}` or
  `{kind:"cancelled"}`. `reason` is an `ExecutionFailure`, a closed set without text, paths or local ids, named as the
  desktop's run states name them: `step_failed`, `retries_exhausted`, `worktree_not_created`, `limit_reached` or
  `runner_failed`. An execution success is not permission to mark the task Done; human-wait entry and exit and the
  task-completion effects have no outcome until D1, D4 and D8 are decided.
- `ArtifactRef`: `{sha256:ContentHash,kind,size_bytes}`, a secret-free shared artifact by the hash of its bytes, never
  a local path. `ArtifactKind` is `report` (a completed run's report), `patch` (the run's changes) or `log` (a step's
  captured output). The set is closed, so a reader refuses a kind it does not know: a new kind is a new variant in
  `v2::ArtifactKind` and in this list, published before any runner reports it.
- `AuthorityRef`: `{subject,claim_id,work_id:WorkId?}`; `work_id` is always sent, `null` before the claim has work.
- `WorkReport`, the payload of the command `work.report`:
  `{authority:AuthorityRef,run_id:RunId,event_id:WorkEventId,sequence:u64,outcome:ExecutionOutcome,artifacts:[ArtifactRef]}`.
  `run_id` is the reporting computer's run, `event_id` (a UUID) is made once per event and repeated on every retry,
  and `sequence` is monotonic per run. It answers `{kind:"work",work:WorkRead,bundle:BundleRef,effects}`, instantiated
  by the server on the S4 envelopes.
- Order (`run::order_report`): a report is accepted only with a sequence greater than the last one accepted for its
  run. The same sequence with the same report is a duplicate, answered with the original result, not an error; any
  other equal or lower sequence is refused with `sequence_regressed`.
- Fencing, as `AuthorityError` codes: a stale claim generation or fenced work is `claim_lost`; the claim held by
  another session of the same account is `session_mismatch` (409); a regressed sequence `sequence_regressed` (409); a
  caller no longer a member `membership_revoked` (403). A delayed report never overwrites the state of the work's new
  holder.
- `WorkEvent::Changed`, `kind` `work.changed`, a delta with `{work:WorkRead}`:
  `{kind:"work.changed",data:{id,by,actor,at,work}}`. A client replaces its cached work and ignores one whose
  `work.revision` is not newer than its own.

No D4/D8 completion or human-wait variant is added. Existing v1 contracts, the /api/v1 client and earlier v2 types are
unchanged, apart from the new error codes.

## Team authority S11 subscriptions

The public prerequisite for platform OP-154 (scoped invalidation subscriptions without replica snapshots) is in
`orchester_api::v2::subscription`, re-exported from `orchester_api::v2`; it defines no route. Unknown fields are refused
in every shape below.

- Route, the server's: `GET /api/v2/projects/{project}/subscription`, a server-sent event stream, with an optional
  `Last-Event-ID` header carrying the last cursor the client saw. Each event's SSE `event:` name is its `kind`, its
  `id:` its `cursor` (`revoked` has none), and its `data:` the JSON below.
- `QueryScope`, a family of cached reads, tagged by `kind`: `{kind:"workspace"}` (project, policy, claim staleness
  threshold, capabilities, members), `{kind:"task_list"}` (task list and task counts), `{kind:"task",task:TaskId}`,
  `{kind:"task_comments",task:TaskId}`, `{kind:"module_list"}`, `{kind:"module",module:ModuleId}` (summary, progress,
  burndown, history), `{kind:"library"}` (library and pickers) and `{kind:"resource",resource:ResourceId}` (its
  version list). Scopes compare by value and are totally ordered, so a set of them is stable.
- `SubscriptionEvent`, tagged by `kind`:
  - `{kind:"ready",cursor:EventCursor}`: sent first to a stream opened without `Last-Event-ID`; the stream's
    position, with no snapshot.
  - `{kind:"invalidated",cursor,scopes:[QueryScope]}`: one per committed batch; `cursor` is the batch's last entry and
    `scopes` name each scope once.
  - `{kind:"reset",cursor}`: the resume point is below the server's replay floor; the client drops every cached query
    and reads again at or after `cursor`.
  - `{kind:"revoked"}`: the caller lost access; the stream then closes.
- Client race rule: a cached read taken at cursor `R` (a page's or command's `EventCursor`) is stale only when an
  `invalidated` event with a cursor greater than `R` names its scope, "greater" in the `EventCursor` ordering above
  (by integer, never by string). Events with a cursor at or below `R` are already in the read, so a client may
  subscribe before or after reading without missing a change. The rule compares cursors of the same project only. A
  `reset` drops everything.

Existing v1 contracts, the /api/v1 client and earlier v2 types are unchanged.

## Team authority S10/S11 routes

The desktop's client of the S10 reads and the S11 subscription (`orchester_api::client::Authority`, through
`Client::authority(token, project)`; ORC-713, milestone P5, routes corrected in ORC-718) calls the team server's own
routes (platform OP-153 and OP-154, its `docs/workspace-reads.md` and `docs/workspace-subscriptions.md`) under
`/api/v2/projects/{project}`, each with the device token as `Authorization: Bearer`. A listing that takes filters or
a page is a `POST` of its request body, so filters never travel in a URL. The server answers these routes for whoever
may read the workspace (a member, a workspace Admin, or a server admin without membership); anyone else, a removed
member too, gets `404`.

Every `/api/v2` request, the commands and the work routes below too, names the desktop's own version in the
`Orchester-Client-Version` header (`orchester_api::client::CLIENT_VERSION_HEADER`, ORC-735), a `ReleaseVersion` such
as `0.5.0`; the desktop's client sends it once made with `Client::with_client_version`. A server refuses a desktop older
than it accepts, or one that names no version or an unreadable one, `426 unsupported_contract` with
`details:{required_version}`, the oldest version it accepts (the team server's minimum is `0.5.0`), before the session
or the operation is read: nothing was kept, and the same request after the update is safe. The desktop says which
Orchester the server needs.

| Method and path | Request | Answer |
| --- | --- | --- |
| `GET …/workspace` | | `WorkspaceMetadata` |
| `GET …/capabilities` | | `Capabilities` |
| `POST …/task-list` | `TaskListRequest` | `Page<TaskRead>` |
| `POST …/task-counts` | `TaskFilters` | `TaskBoardCounts` |
| `GET …/tasks/{task}` | | `TaskRead` |
| `POST …/module-list` | `ModuleListRequest` | `Page<ModuleSummary>`, oldest module first |
| `GET …/modules/{module}` | | `ModuleSummary` |
| `GET …/modules/{module}/progress` | | `ModuleProgressRead` |
| `GET …/modules/{module}/burndown` | | `ModuleBurndownRead`, with the forecast |
| `POST …/modules/{module}/history` | `PageRequest` | `ModuleHistoryPage` |
| `POST …/library` | `ResourceLibraryRequest` | `ResourceLibraryPage`, without versions |
| `POST …/resource-picker` | `ResourcePickerRequest` | `ResourcePickerPage` |
| `POST …/resources/{resource}/version-list` | `PageRequest` | `ResourceVersionsPage`, oldest first, each version with its body |
| `GET …/subscription` | optional `Last-Event-ID` | the S11 event stream |

There is no route for one version on its own: a version's body is read in its page of the version list, which pages
by count and by at most 1 MiB of JSON (a larger version fills a page of its own).

Refusals come in two forms. What is new to these reads is an `AuthorityError` (`AuthorityCallError::Refused`): `422
invalid` for a page cursor that is not one or belongs to another request, `410 cursor_expired` for one whose listing
is gone. The failures common to every route keep their `/api/v1` `{error,message}` envelope (`ClientError::Api` inside
`AuthorityCallError::Client`): `401 unauthorized`, `404 not_found`, and `400 bad_request` for a malformed body, an
unknown field or a malformed `Last-Event-ID`. A body in neither form, or an answer not of the route's shape, reads as
not an Orchester server.

The subscription sends a `: connected` comment first and `: keep-alive` every 20 seconds while nothing else comes.
Opened without `Last-Event-ID` it sends `ready` at the log's cursor; resumed with one it sends no `ready`, only the
`invalidated` batches after it, or `reset` when it cannot replay from it. `revoked` (data `{kind:"revoked"}`, no `id:`)
ends it.

The workspace's reads (`orchester_workspace::team_reads`) follow the race rule above: they wait for the
subscription's first `ready`, are stamped with the cursor they answer at (a page's or count's snapshot, or the
position the read was issued at for a model without one), read again when an `invalidated` event of their scope
arrives while they are in flight, and treat every reconnect (a subscription resumed from the last cursor seen) like a
`reset`. A `401`, a `404`, `revoked`, and a changed session end them for good.

## Team authority P2 commands

The team server's commands on the S4 envelopes, as it accepts them (its `docs/operation-receipts.md`,
`docs/claim-authority.md` and `docs/module-branches.md`): `orchester_api::v2::TeamCommand` in `CommandRequest`,
answered with `CommandResult<TeamCommandValue>`, through `Authority::command` and `Authority::operation`. The two
enums are field for field the server's own `Command` and `CommandValue`, so the server can take them without a wire
change. Unknown kinds and fields are refused; the server answers a kind it does not have `400`.

| Method and path | Request | Answer |
| --- | --- | --- |
| `POST /api/v2/projects/{p}/commands` | `CommandRequest<TeamCommand>` | `201` `CommandResult<TeamCommandValue>`, `200` the same result for a repeat, or an `AuthorityError` |
| `GET /api/v2/projects/{p}/operations/{operation}` | | `OperationRead<TeamCommandValue>`; `404 not_found` while the server has no outcome of it |

- Commands, tagged by `kind`: `task.create {content:TaskContent}` (Admin), `claim.acquire {subject}`,
  `claim.take_over {subject,observed_claim_id}`, `claim.release {authority:AuthorityRef}`,
  `assignment.set {subject,user_id}` (`user_id` sent, `null` for nobody; Admin), `module.branch.set {module_id,branch}`
  (Admin; `branch` is required), `work.start {authority:AuthorityRef,base_commit}`,
  `delivery.authorize {subject,claim_id,branch,source_commit,base_commit}` (Admin; a module subject), since P4
  `work.report` with `WorkReport`'s fields (below), and since P4c `delivery.request` and `delivery.report` ("Team
  authority P4 module delivery").
- Expected revisions: the module's for `module.branch.set`, the task's and its module's for `work.start`, the
  subject's for `assignment.set` (the desktop sends it), none for the others, whose claims fence them by id.
- Values, tagged by `kind`, each with `effects`: `task {task:SharedTask}` (until `TaskRead` is published there),
  `work {work:WorkRead,bundle:BundleRef}`, `claim {subject,claim:ClaimRead,children:[ClaimRead]}`,
  `released {subject,released_claim_id}`, `assignment {subject,assignee:UserRef?,revision}`,
  `module {module:ModuleRead}` and `delivery {delivery:ModuleDeliveryRead}`. Only `work.start` and
  `delivery.request` answer an effect, `start {work_id,bundle}` and `deliver_module`; every other list is empty.

Definitions are no commands. The desktop's dispatcher (`orchester_workspace::actions`) sends these actions to the
server's own Admin-checked routes (Authority S3, its `docs/definition-authority.md` and `docs/workspace-policy.md`),
which refuse a member with `403` (`not_admin` on the `/api/v1` routes):

| Dispatcher action | Route | Decided on |
| --- | --- | --- |
| Task edit, organization, move | `PATCH /api/v1/projects/{p}/tasks/{t}` `TaskPatch {revision,changes}` | the task's revision; `409 revision_conflict` carries the current task |
| Module create | `POST /api/v1/projects/{p}/modules` `ModuleCreate` | its `idempotency_key` and `id`, both the intent's operation id |
| Module edit | `PATCH /api/v1/projects/{p}/modules/{m}` `ModulePatch` | the module's revision and the intent's key; `409 revision_conflict` carries the current module |
| Tag create, rename, merge, delete | `POST …/tags`, `PATCH …/tags/{tag}`, `POST …/tags/{tag}/merge`, `DELETE …/tags/{tag}` | nothing: tags have no revision |
| Workspace policy | `PATCH /api/v2/projects/{p}/policy` `WorkspacePolicyChanges {revision,automatic_merge}` | the policy's revision; `409 {error:"policy_revision_conflict",current}` |

The dispatcher records each command's id and request in the project database (`team_operations`) before sending it,
bound to the SHA-256 of its expected revisions and command: another request under a recorded id is refused before
anything is sent. A lost or unreadable response leaves the operation open as *acceptance unknown*; carrying it on
reads `GET …/operations/{operation}` first and sends the same request again only on `not_found`. `unauthenticated`,
`rate_limited`, `unavailable` and `unsupported_contract` leave it open too; every other refusal, including a lookup's
final one, is kept as final. After `revision_conflict` the dispatcher reads the subject as it is now (`GET
/api/v2/projects/{p}/tasks/{task}` or `…/modules/{module}`) and returns it with the refusal. A committed receipt is
kept before its local effects: a task it answers with is recorded as a team task identity, and `start` stays pending
on the open operation until the team start (P4, below) carries it out and completes the operation.

The definition routes keep no receipt, so a definition edit is not recorded. When its response is lost, the
dispatcher reads the subject again (`GET /api/v1/projects/{p}/tasks/{t}`, `…/tags`, `GET /api/v2/projects/{p}/policy`)
and compares it with the edit: holding the edit, it was made; still at the revision read (a tag without the edit),
it is sent again, which the revision or the module routes' key makes safe; otherwise it changed meanwhile and the
edit is refused as superseded, with the current value. A revision conflict whose current value holds the edit is
taken as made, so the same intent sent again after a lost response finds its edit rather than a refusal.

## Team authority P4 work

The routes of authorized work, as the team server serves them (its `http/workspace/mod.rs`, `bundles.rs`,
`heartbeat.rs`, `docs/claim-authority.md`, `docs/module-branches.md` and `docs/work-reports.md`), through
`orchester_api::client::Authority`, each with the device token of the session the claim executes under. Refusals are
`AuthorityError`s.

| Method and path | Request | Answer |
| --- | --- | --- |
| `GET /api/v2/projects/{p}/bundles/{sha256}` | | `ExecutionBundle`; `409 claim_lost` once no claim it was built for is the caller's in this session, and for a hash the workspace does not have |
| `GET /api/v2/projects/{p}/bundles/{sha256}/files/{file}` | | the file's raw bytes (`application/octet-stream`, at most `MAX_BUNDLE_FILE_BYTES`); refused as the bundle is, `404 not_found` for a file it does not pin |
| `POST /api/v2/projects/{p}/heartbeat` | `ExactHeartbeat {authorities:[AuthorityRef]}`, at most 256 | `Renewed {renewed:[ClaimRead],lost:[AuthorityRef]}`; `409 decision_required [D1]` while a named claim's work waits for a person or for review, `409 transition_denied` while it is not running; either renews nothing |
| `POST /api/v2/projects/{p}/commands` | `work.report {authority,run_id,event_id,sequence,outcome,artifacts}` | `{kind:"work",work:WorkRead,bundle:BundleRef,effects:[]}`, decided as the server's `docs/work-reports.md` orders it |

The desktop's team work (`orchester_workspace::team_work`) uses them so:

- **Start.** From a held task claim, the start is recorded in the project database (`team_starts`) with what its caller
  read and chose, then `work.start` goes through the dispatcher under the start's operation. The receipt's work and
  bundle reference are kept first; then the bundle and every file it pins are read by hash, each checked
  (`ExecutionBundle::check_file`, and the bundle's own hash, claim, task and base commit), and kept as the run's
  accepted inputs, the files as blobs, in one transaction with the start's record; only then is the runner asked to
  start the run (protocol 42's `StartAcceptedRun`). A crash or an outage at any boundary is carried on from it, once;
  the operation and the runner's start are idempotent, so a retry never makes a second work or run. A refusal of the
  start or of its bundle (`claim_lost`) is kept and starts nothing. A bundle that can never become the run's inputs
  here (it is not the one the receipt names, or the caller's bindings do not fit it) settles the start `failed`, kept
  with the reason; recovery carries each open start on its own, so one such start holds back no other.
- **Inputs.** The task's number and each pinned version's label (`{slug,name}`) are the bundle's (schema 2); the
  start's caller supplies only the local bindings and consent, and each pinned profile must be bound to a connection
  of the CLI the bundle binds it to.
- **Reports.** Each change of a team run's outcome (core's `ExecutionOutcome::of_status`) is queued once in
  `work_report_outbox` with its event id, the run's next sequence, an operation id and its request, before it is sent;
  queued reports go in sequence order, each again with the same bytes until the server acknowledges it. A transport
  failure or a retryable refusal keeps the rest waiting in order. `claim_lost`, `membership_revoked`,
  `session_mismatch`, and an answer whose work's newest run is another, fence the run: it reports and renews nothing
  more, and its run, worktree and unsent reports stay on this computer. Other refusals are kept with their report.
  A run's local status and its server acceptance (the work as the server last answered its reports, by revision) are
  kept apart (`team_runs`).
- **Heartbeat.** Only runs at work here (running, no step waiting for a person) are named, with their work's
  authority; when none is, nothing is sent. `decision_required` is returned as such, not retried as a failure; a lost
  authority fences its run. Nothing calls the team work yet: the screens adopt it in P6, automation in P7.

## Team authority P4 module delivery

A team task's run merges into its module's branch (D3: the accepted run's base branch is the module's); only the
module's delivery takes that branch to the base. The commands, on `POST /api/v2/projects/{p}/commands`, as the team
server's `docs/module-branches.md` (S7c) defines them; each answers `{kind:"delivery",delivery:ModuleDeliveryRead,
effects}`, and refusals are `AuthorityError`s.

| `command.kind` | Payload | Effects | Who |
| --- | --- | --- | --- |
| `delivery.authorize` | `{subject,claim_id,branch,source_commit,base_commit}` | `[]` | Workspace Admin |
| `delivery.request` | `{authority:AuthorityRef,authorization_operation_id:OperationId?,branch,source_commit,base_commit}` | `[{kind:"deliver_module",module_id,branch,source_commit,base_commit}]`, or `[]` while it waits for the Admin | The module's holder, in its session |
| `delivery.report` | `{authority:AuthorityRef,source_commit,base_commit,outcome}` | `[]` | The module's holder, in its session |

`authority` is the module's claim with `work_id` `null`. `authorization_operation_id` is always sent: the Admin's
`delivery.authorize` operation, or `null` for the workspace's automatic merge (D7). With `null` and the automatic
merge off, the request is no longer refused (the server's S12, ORC-735): it is kept `requested` as the module's
`pending_delivery` with `authorization:"awaiting_admin"` and answered with `effects:[]`. The Admin's
`delivery.authorize` of exactly that binding turns it to `admin`, and the holder's next `delivery.request` of it, under
an operation of its own, gets the `deliver_module` effect; an Admin may still authorize first, for a request to cite.
A cited authorization that is not of exactly this delivery is `403 delivery_not_authorized`. `outcome`
(`v2::DeliveryOutcome`) is `{kind:"delivered",merge_commit}` or `{kind:"failed"}`; only a `delivered` report makes the
delivery delivered.

The desktop's module delivery (`orchester_workspace::team_work::ModuleDeliveries`) uses them so:

- **Request.** The delivery is recorded in the project database (`team_deliveries`) with the module, its claim and
  branch, both commits, the cited authorization and the branch of this repository the base commit was read from,
  then `delivery.request` goes through the dispatcher under the delivery's operation (a lost answer is looked up, not
  sent again). It is not sent while a team run of the module still works on this computer. A refusal is kept; a
  receipt without an effect is kept as `waiting`, awaiting an Admin's authorization, and goes no further under its
  operation. The Module page shows the server's `pending_delivery` with who asked and when, offers an Admin
  *Authorize* of exactly its commits, and offers its holder on this computer *Ask again* once an Admin authorized it.
- **Merge.** Only a receipt whose effects carry `deliver_module` for exactly the requested branch and commits lets this
  computer merge, by the merge a run takes into its base branch (core's `merge_module`, under the merge hold that
  keeps local integration and other merges out), always as a merge commit and only while the module branch and the
  base branch are still at those commits. The merge is marked begun before git is asked; on recovery a base branch
  whose tip is a merge commit of exactly those two commits is taken as the merge made, so it is never made twice. A
  busy checkout leaves the merge to try again; any other refusal of git's is reported `failed`.
- **Report.** The merge's outcome is kept with the `delivery.report` made of it, with its own operation id, in one
  transaction, and the request's operation is then completed. The report is sent again with the same bytes until the
  server answers it, oldest delivery first; a report that waits for the server holds back the later ones.
- **States.** Each delivery keeps what the server last answered (`ModuleDeliveryRead`) and reads as requested,
  awaiting authorization, authorized by an Admin, authorized automatically, delivered, failed or refused, with its
  merge and whether its report waits (`TeamDeliveryState`), for the screens (P6).
- **MCP `merge_run` (P7b).** On a team project it requests the run's module's delivery: the module's claim
  (`claim.acquire`, taken only once the request is about to be sent), the module branch's tip as `source_commit` and
  the tip of the checkout's checked-out branch (never the module's own) as `base_commit`, citing as
  `authorization_operation_id` the Admin's `delivery.authorize` operation its caller names in `authorization`, else
  none (the automatic merge); it never authorizes. An open delivery of the module is carried on instead. Only a
  delivered module is its result; every other state is a refusal naming it.

## Team authority releases

A shared project's releases, each task's release and each module's horizon belong to its team server
(`docs/design/roadmap.md`, decision 7). The contract the server's *Roadmap and releases · team authority* module
(platform OP-177…OP-195) builds against is in `orchester_api` (the definitions and the history at the crate root, the
reads in `orchester_api::v2`) and core (`orchester_core::release`, `id::ReleaseId`, `module::Horizon`); architecture
in [server.md](architecture/server.md#releases-team-authority). It supersedes ORC-468, which described the old replica
model. It is a coordinated breaking upgrade: a task read now requires `release` and a module read `horizon` and
`place`, with no compatibility window, so the desktop and the team server move to it together. Unknown fields are
refused in every shape below; in the reads every field is sent, `?` meaning `null`.

- `SharedRelease`:
  `{id,name,description,planned?,state,released_on?,released_by:UserRef?,released_commit:CommitId?,revision,created,updated}`.
  `id` is a `ReleaseId` the desktop makes (UUIDv7); `name` is a core `Name`, unique in the project without regard to
  case; `description` is Markdown, trimmed; `planned` and `released_on` are `YYYY-MM-DD` days; `state` is `planned`
  or `released`. A released release names `released_on` and `released_by`, and `released_commit` when it was given; a
  planned one names none of the three, and any other combination is refused (`ReleaseStateError`). `revision` counts
  the release's changes and its changelog's. `ReleaseFields` `{name,description,planned?}` is what a person sets
  (`details()` gives core's `ReleaseDetails`, `422 invalid` when the name or the day is not one).
- A task's release: `release:ReleaseId?` in `TaskContent` (so `SharedTask` and `TaskRead`) and in `TaskChanges`
  (absent leaves it, `null` takes the task out), moved under the task's revision as `module` is. At most one per task.
- `ReleaseChange` `{sequence,task,from?,to?,by,actor,at}`: every move of a task into a release, out of one or from one
  to another, append-only, `actor` `person` or `orchester` (*Mark released*'s moves are Orchester's).
- The changelog, `SharedChangelog` `{release,revision,document,hidden,kept_aside}`: `revision` is the release's;
  `document` (`ChangelogDocument`) is `{markdown,blocks}`, the Markdown as core's `docs::blocks` writes it and the id of
  each of its blocks in order (as many as the Markdown parses into, each once, or refused with `ChangelogError`);
  `hidden` are the release's tasks whose entry a person deleted; `kept_aside` (`KeptEntry` `{task,markdown}`) the
  edited entries of tasks that left the release or are no longer Done.
- A module's horizon: `ModuleFields` (so `SharedModule`, `ModuleRead` and the module create and patch) gains
  `horizon` (`now`, `next`, `later`; core's `module::Horizon`) and `place` (`u32`): the Now / Next / Later board shows a
  horizon's modules in ascending `place`, then in creation order. A module that names neither, a new one too, is in
  `later` at place 0. Dates never change a horizon.

Definitions, on the server's Admin-checked routes as a module's are (Authority S3); only a workspace Admin or a server
admin writes them, and anyone else gets `403 not_admin`. Each write carries an `idempotency_key`, and the server
answers a repeat with its first answer, as for modules.

| Method and path | Request | Answer |
| --- | --- | --- |
| `POST /api/v1/projects/{p}/releases` | `ReleaseCreate {idempotency_key,id,release:ReleaseFields}` | `201 ReleaseResponse {release}`, planned, at revision 1 |
| `PATCH /api/v1/projects/{p}/releases/{r}` | `ReleasePatch {idempotency_key,revision,release}`, the fields as a whole | `ReleaseResponse` |
| `POST /api/v1/projects/{p}/releases/{r}/released` | `MarkReleased {idempotency_key,revision,commit:CommitId?}` (`commit` always sent) | `ReleaseMarked {release,moves:[ReleaseChange]}` |
| `DELETE /api/v1/projects/{p}/releases/{r}` | `ReleaseDelete {idempotency_key,revision}` | `ReleaseDeleted {moves}`; a planned release only |
| `PATCH /api/v1/projects/{p}/releases/{r}/changelog` | `ChangelogPatch {idempotency_key,revision,document,hidden,kept_aside}`, the changelog as a whole | `ChangelogResponse {changelog}` |
| `PATCH /api/v1/projects/{p}/tasks/{t}` | `TaskPatch` with `changes.release` | the task, as every task edit |

*Mark released* records the server's day, the caller and the commit, keeps the release's Done tasks and moves every
other one but the Canceled to the next planned release (planned releases with a day by day, then those without one in
creation order; the first after this one), or out of any release when there is none; each move is a `ReleaseChange`
with `actor:"orchester"`, all in the answer. A released release is final: it takes no change, no changelog edit, no
task in or out and no delete. Deleting a planned release takes each of its tasks out (`actor:"person"`) and removes
its changelog.

Refusals, in the `/api/v1` envelope (`SharingError`):

| Status, `error` | When | Details |
| --- | --- | --- |
| `409 release_exists` | another release of the project has the name, without regard to case | `release`: that release (`SharingErrorDetails::Release`) |
| `422 release` | `reason` (`ReleaseRefusal`): `released` for any write of a released release, a move into or out of one; `release_not_found` for a task's release the project does not have | `reason` |
| `409 revision_conflict` | a release write or changelog patch on another revision | `current`: the release as it is now (`SharingErrorDetails::ReleaseRevision`) |
| `422 invalid` | a name that is not a name, or a day that is not `YYYY-MM-DD` | |
| `409 id_taken` | the release id is the project's already | |
| `403 not_admin` | a member's definition write | |
| `404 not_found` | no such release | |

Reads, on the S10/S11 routes under `/api/v2/projects/{project}` for whoever may read the workspace:

| Method and path | Request | Answer |
| --- | --- | --- |
| `POST …/release-list` | `ReleaseListRequest {state?:"planned"\|"released",page}` | `Page<ReleaseSummary>`, oldest release first |
| `GET …/releases/{release}` | | `ReleaseSummary` |
| `GET …/releases/{release}/progress` | | `ModuleProgressRead` |
| `GET …/releases/{release}/burndown` | | `ReleaseBurndownRead`, with the forecast |
| `POST …/releases/{release}/history` | `PageRequest` | `ReleaseHistoryPage` (`Page<ReleaseChange>`, ascending sequence) |
| `GET …/releases/{release}/changelog` | | `SharedChangelog` |

- `ReleaseSummary` `{release:ReleaseRead,progress,tasks,forecast:ForecastRead,after_planned,overdue_days?}`;
  `ReleaseRead` is `{release:SharedRelease,capabilities}`. Progress, burndown and forecast are a module's, counted by
  core's rules over the release's tasks (Canceled out of scope, Done done, each move on its day). `after_planned` is
  `true` when the forecast's 85% (`safe`) day is after the planned day; `overdue_days` is how many days a planned
  release is past its planned day, `null` when it is not. A released release is neither.
- `ReleaseBurndownRead` `{release,unit,start_scope,days,forecast,cursor}`, its days from the release's first task to its
  planned day (no ideal line without one), each as a module's burndown day.
- The task list's `TaskFilters` gains `releases` (`null` among them is a task without one), as `modules`.
- `QueryScope` gains `{kind:"release_list"}` and `{kind:"release",release}` (its summary, progress, burndown, history
  and changelog). A release write invalidates both; a task's move the task's scopes and each release it left or
  joined; *Mark released* also the release its tasks moved to.

Open, not decided by this contract: the source of *Mark released*'s commit picker (the base branch's recent commits
read from the desktop's checkout or from the server), and the time zone a server counts days in (the released day,
`overdue_days`, the burndown's days). Not here either: a `releases` event on the `/api/v1` stream and Share, Join and
Make local carrying releases, which come with the desktop's half.

## Conventions

- JSON bodies in UTF-8. Requests are limited to 1 MiB and must arrive within 10 seconds.
- Unknown request fields are refused (`400 bad_request`).
- Endpoints marked *device* need `Authorization: Bearer <device token>`. A device token is `orcc_` followed by 64
  hex characters. It ends when the device signs out or is revoked, and when the password is reset or changed on
  another device. A desktop's token also ends after 90 days without use; a browser's (platform `web`) ends 12
  hours after its sign-in.
- Times are milliseconds since the Unix epoch, UTC, except in identity tokens and key sets, which use JWT seconds.
- Ids are UUIDv7 strings.
- Every response carries `Cache-Control: no-store`.
- A request with a body must say `Content-Type: application/json` (`415 unsupported_media_type` otherwise).
- Errors have the form `{ "error": "<code>", "message": "<English, for logs>" }`. Clients show their own words by
  `error`. A `429` and a `503 busy` carry `Retry-After` in seconds. A `401 unauthorized` for a token the cloud ended
  adds `reason` and `ended_at` ([Why a token ended](#why-a-token-ended)).

| Code | Status | Meaning |
| --- | --- | --- |
| `bad_request` | 400 | Malformed JSON, a wrong field type or an unknown field |
| `unauthorized` | 401 | No device token, or one that is unknown, revoked or expired; one the cloud ended says why ([Why a token ended](#why-a-token-ended)) |
| `invalid_credentials` | 401 | Wrong email or password. One answer for both, and for an email with no account |
| `invalid_code` | 401 | A two-factor code that is wrong or was used, or a recovery code that is unknown or used ([Two-factor authentication](#two-factor-authentication)) |
| `challenge_expired` | 401 | A sign-in's two-factor challenge that is unknown, used, past its 5 minutes or 5 tries, or whose password changed since: sign in again |
| `email_unconfirmed` | 403 | The password was right, but the email is not confirmed yet |
| `mfa_required` | 403 | Accepting an invite (or stepping up) needs two-factor authentication, and the account has it off |
| `session_without_mfa` | 403 | The account has two-factor authentication, but this device signed in without a code: step up with one, or sign in again |
| `forbidden` | 403 | The token is valid but may not do this: a release token without the `releases:write` scope, or a release admin call from an account that is not Orchester staff ([App releases](#app-releases)) |
| `not_found` | 404 | No such path, or no such device of the account |
| `method_not_allowed` | 405 | |
| `request_timeout` | 408 | The body did not arrive within 10 seconds |
| `payload_too_large` | 413 | The body is over 1 MiB (a release file's part: over its `part_size`) |
| `unsupported_media_type` | 415 | A body without `Content-Type: application/json` (a release file's part: without `application/octet-stream`) |
| `range_not_satisfiable` | 416 | A download's `Range` starts at or past the end of the file, or is not one `bytes=<start>-[<end>]` range |
| `invalid` | 422 | An input breaks a rule: an email that cannot be one, a password outside 12–256 characters, an empty or over-long name |
| `invalid_link` | 422 | A confirmation or reset link that is unknown, used or expired. One answer for all three |
| `conflict` | 409 | The team server is linked by a statement as new as this one, or this statement was used; or two-factor authentication is on already (`POST /me/two-factor`); or a release's stage does not allow the change ([App releases](#app-releases)) |
| `too_many_servers` | 409 | The person owns 5 linked team servers, ownerless ones they linked included |
| `rate_limited` | 429 | Too many attempts; see Rate limits below |
| `keys_unavailable` | 503 | No published key set lets the cloud sign now. The cloud renews its key set itself at every start and every minute while it serves (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Keys"), so this means it could not, for example its database failed for weeks |
| `busy` | 503 | No password hashing slot freed within 10 seconds |
| `internal` | 500 | |

## Shapes

```text
DeviceInfo { name: string (1–256), platform: "windows"|"macos"|"linux"|"web" }
Account    { id, email, display_name, created_at }
Device     { id, name, platform, created_at, last_used_at, current: bool }
TwoFactor  { enabled: bool, enabled_at: ms | null, recovery_codes_left: 0–10, this_session: bool }
```

`this_session` says whether the asking device is a *two-factor session*: it signed in with a code, stepped up with one,
or turned two-factor authentication on itself.

## Endpoints

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `POST /auth/signup` | none | `{ email, password, display_name }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/confirm` | none | `{ link_token, password }` | `204` | `422 invalid_link`, `429` |
| `POST /auth/confirm/resend` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/links/inspect` | none | `{ link_token, purpose: "confirm"\|"reset" }` | `200 { email }` | `422 invalid_link`, `429` |
| `POST /auth/sessions` | none | `{ email, password, device: DeviceInfo }` | `201 { token, device: Device, account: Account }`; with two-factor authentication on, `202 { challenge, expires_at }` | `401 invalid_credentials`, `403 email_unconfirmed`, `422 invalid` (device), `429` |
| `POST /auth/sessions/two-factor` | none | `{ challenge, code }`: a code from the app or a recovery code | `201 { token, device, account, recovery_codes_left }` | `401 invalid_code`, `401 challenge_expired`, `429` |
| `POST /auth/sessions/current/two-factor` | device | `{ code }` | `200 { recovery_codes_left }`: this device is a two-factor session now, with the same token | `401 invalid_code`, `403 mfa_required`, `429` |
| `POST /auth/reset` | none | `{ email }` | `202`, empty | `422 invalid`, `429` |
| `POST /auth/reset/finish` | none | `{ link_token, new_password }` | `204` | `422 invalid`, `422 invalid_link`, `429` |
| `POST /auth/password` | device | `{ current_password, new_password }` | `204` | `401 invalid_credentials`, `422 invalid`, `429` |
| `DELETE /auth/sessions/current` | device | | `204` | |
| `GET /me` | device | | `{ account: Account, two_factor: TwoFactor }` | |
| `PATCH /me` | device | `{ display_name? }` | `{ account: Account, two_factor: TwoFactor }` | `422 invalid` |
| `POST /me/two-factor` | device | `{ password }` | `201 { secret, uri }`: a new setup, replacing one not confirmed | `401 invalid_credentials`, `409 conflict` (on already), `429` |
| `POST /me/two-factor/confirm` | device | `{ code }`: a code from the app | `200 { recovery_codes: [10] }`, shown once | `401 invalid_code`, `404` (no setup in its 15 minutes), `429` |
| `DELETE /me/two-factor` | device | `{ password, code }`: a code from the app or a recovery code | `204` | `401 invalid_credentials`, `401 invalid_code`, `404` (off), `429` |
| `POST /me/two-factor/recovery-codes` | device | `{ code }`: either kind | `200 { recovery_codes: [10] }`, shown once; every earlier one ends | `401 invalid_code`, `404` (off), `429` |
| `DELETE /me` | device | `{ password }` | `204` | `401 invalid_credentials`, `429` |
| `GET /devices` | device | | `{ devices: [Device] }`, oldest first | |
| `DELETE /devices/{id}` | device | | `204` | `404` |
| `GET /me/sign-ins` | device | | `{ sign_ins: [{ at, device_name, platform, succeeded, two_factor }] }`: the account's own sign-ins of the last 90 days, newest first, at most 50; `two_factor`: a code was asked for. A wrong password counts only for an account that exists | |
| `GET /keys` | none | | `{ key_set }`: the newest key set, a compact JWS the cloud's root key signed. The cloud makes its root at its first start and publishes a key set before it listens, so a running cloud always has one | `503 keys_unavailable` |
| `POST /identity-tokens` | device | `{ audience, purpose?, invite? }`: the team server's id (a UUID); `purpose: "admin"` for a sign-in to that server's admin, `"code"` to show with a code entered in the app, `"accept"` (with `invite`) to accept the invite in the browser (*Accepting in the browser* below); `invite`: an invite from that server to the account's email, open or accepted by this account, which the token accepts | `201 { identity_token, expires_at }` (seconds) | `403 mfa_required`, `403 session_without_mfa` (an `invite` from a device that is not a two-factor session; the invite stays open), `404 not_found` (purpose `admin` for a server not linked here; an `invite` that is expired, declined, accepted by another account, not to the account's email or not from `audience`), `422 invalid` (also purpose `link`, `invite` with a purpose other than `accept`, or `accept` without `invite`), `429`, `503 keys_unavailable` |
| `POST /servers` | device | `{ link }`: the link statement the team server signed | `201 { server, root_key, key_set, identity_token, expires_at }` | `409 conflict` (a replayed or older statement), `409 too_many_servers`, `422 invalid` (also a `server` that is not its `public_key`'s id), `429`, `503 keys_unavailable` |
| `GET /servers` | device | | `{ servers: [{ id, name, address, fingerprint, owner, joined_at }] }` | |
| `GET /servers/{id}` | device | | `{ server: { id, name, address, fingerprint, check_code } }` | `404` (unknown or not linked) |
| `DELETE /servers/{id}` | device | | `204`: the owner unlinks the server. The cloud forgets its link, its members' joins and its invites, so invites in flight are refused; nothing on the server changes, and it links again with a new statement | `403 session_without_mfa` (two-factor authentication on, and this device is not a two-factor session), `404` (not linked, or not this account's), `409 conflict` (an administrator of the cloud blocked it) |
| `POST /invites` | the server's signature | `{ invite }`: an invite the team server signed | `202 { server: { id, name, address } }`: the inviting server as the cloud registered it | `422 invalid` (also an invite that was revoked, until it would have expired), `429` |
| `GET /invites` | device | | `{ invites: [{ id, kind, role, server: { id, name, address }, project: { address, name } \| null, inviter_display_name, expires_at, needs_two_factor, accepted_at }] }`; `needs_two_factor`: this device could not accept it now (it is not a two-factor session); `accepted_at` (ms, absent or `null` for an open invite and from a cloud before browser acceptance): accepted in the browser, the team server's receipt not back yet | |
| `DELETE /invites/{id}` | the server's signature | `{ revocation }`: the team server's signed revocation | `204`; the id stays revoked until the invite would have expired | `422 invalid` |
| `POST /invites/{id}/accepted` | the server's signature | `{ receipt }`: the team server's signed receipt (`orchester-invite-accepted+jws`) for this invite, accepted in the browser | `204`: the join is recorded and the invite's row dropped | `422 invalid` (a receipt the server's linked key did not sign, or one for another invite or server), `429` |
| `POST /invites/{id}/decline` | device | | `204`; the invite stays declined until it expires: no token accepts it, and delivering it again changes nothing | `404` (also one already answered) |

**Sign-up.** `202` means the request was taken. The answer is the same whether or not the email has an account, so
a sign-up form shows one message: *Check your email*. What goes out, in the background:

- a new email gets a confirmation link, valid for 24 hours;
- an email whose account is not confirmed yet gets one more link, and the earlier links stay valid. Each link
  carries the password and display name of the sign-up that sent it. A link is confirmed only with that sign-up's
  password, and confirming it sets that password and name on the account and ends the account's other links. So an
  account's password is always one chosen by a person who also reads the email. Someone who signed up with another
  person's email cannot use the owner's link, and the owner's link cannot be confirmed with their password;
- a confirmed email gets a *you already have an account* note.

An account that is never confirmed is deleted once its last link has expired.

**Confirmation page** (`<public URL>/confirm/<token>`). The page asks for the password chosen at sign-up, and
*Confirm my email* posts it. The confirmation happens only on that form's `POST`, from the web front's server
side, so mail scanners that fetch the link do not use it up. The server side sends
`POST /auth/confirm { link_token, password }`. An unknown, used or expired link and a wrong password give the same
`422 invalid_link`, after the same password check. On `204` the page says the email is confirmed and offers
sign-in. On `invalid_link` it offers to try the password again or to sign up again. After 5 wrong passwords a link
answers `429` for an hour; signing up again sends a new one. Confirming does not sign anyone in.

**Resend.** `POST /auth/confirm/resend { email }` answers `202` for any email, before it touches the database, and
counts against the email limits like a sign-up. In the background an unconfirmed account gets one more confirmation
link. It carries the password and display name of the account's newest open link, so it is confirmed with that
sign-up's password, like any link. A confirmed account gets the *you already have an account* note. An unknown email,
or an account whose links have all expired, gets nothing.

**Inspecting a link.** `POST /auth/links/inspect { link_token, purpose }` gives the email a live link was sent to,
without using the link, so the confirmation and reset pages can show the address before anything is pressed. An
unknown, used or expired link, or one for the other purpose, answers `422 invalid_link` after the same one query, and
counts as a failed link finish against the client's address.

**Sign-in.** A wrong password and an unknown email answer alike (`401 invalid_credentials`). `403 email_unconfirmed`
comes only after the right password. The token in `201` is shown once; the cloud keeps only its SHA-256. `device`
names the computer (the desktop) or the browser (the web front, platform `web`). An account with two-factor
authentication answers the right password with `202 { challenge, expires_at }` instead, and a code finishes the sign-in
([Two-factor authentication](#two-factor-authentication)).

**Why a token ended** (the desktop's sign-in notice, 2026-10-16). A `401 unauthorized` for a device's or a web
session's token that the cloud ended carries why and when (milliseconds):
`{ "error": "unauthorized", "message": "…", "reason": "<reason>", "ended_at": 1792147957042 }`.

| `reason` | The token ended because |
| --- | --- |
| `revoked` | another device of the account signed it out (`DELETE /devices/{id}`, also from the account page's Devices) |
| `password_reset` | the password was reset (`/auth/reset/finish`), or changed on another device (`/auth/password`) |
| `account_deleted` | the account was deleted (`DELETE /me`) |
| `signed_out` | the device signed itself out (`DELETE /auth/sessions/current`) |
| `account_disabled` | an administrator of the cloud disabled the account (since 2026-10-27). The cloud ends every token with it, refuses sign-in and issues no identity tokens for team servers until the account is enabled again; a sign-in with the right password answers `401 unauthorized` with this reason too, before any two-factor challenge |

A client that does not know `account_disabled` reads it as no reason, as below. Any other `401` has no `reason`: no token, one the cloud never issued, a desktop's token that ended after 90 days
without use, a browser's after its 12 hours, or one revoked before migration 9. A client reads a reason it does not
know as none and shows its general words. A revoked device keeps its reason in `devices.revoked_reason`. A deleted
account's devices go with it, so the hashes of their tokens are kept in `ended_tokens` (the hash, the reason, when,
and nothing that names the account) until they would have ended unused, 90 days later; the minute's sweep deletes
them then. Telling a token's holder why it ended says nothing they could not see in the app, and an unknown token
learns nothing.

**Forgot password.** `POST /auth/reset { email }` answers `202` for any email, before it touches the database. The
lookup, the link (valid for 1 hour) and the email happen in the background, and only an account gets one.

**Reset page** (`<public URL>/reset/<token>`). It asks for the new password and sends
`POST /auth/reset/finish { link_token, new_password }`. A password outside 12–256 characters is refused
(`invalid`) before the link is used, so the person can try again with the same link. On `204`: the password is
set, the email counts as confirmed (the link proves it), every device of the account is signed out, and every
other reset link ends.

**Password change** (signed in). It needs the current password. It signs out the account's other devices and ends
open reset links; the device that asked stays signed in. With two-factor authentication on, only a two-factor session
changes it, deletes the account (`DELETE /me`) or signs another device out (`DELETE /devices/{id}`); any other device
gets `403 session_without_mfa` before its password is looked at
([Two-factor authentication](#two-factor-authentication)).

**Identity tokens.** `POST /identity-tokens { audience, purpose? }` returns a token for one team server, valid for 5
minutes.
The desktop hands it to that team server, which checks it with `orchester_api::identity::VerifiedKeySet` against
the key set from `GET /keys`, itself checked with the root key the team server pinned, and accepts each `jti` once.
Claims: `iss` (the public URL), `aud`, `sub` (the account id), `email`, `name`, `device { name, platform }`,
`purpose` (absent to join with a device; `admin` for a sign-in to the server admin; `link` for linking the server;
`code` to show with a code; `accept` to accept an invite in the browser), `invite` (when the token accepts an invite),
`mfa` (`true` when the device it was issued to is a two-factor session; absent otherwise, and in every token from a
cloud before two-factor authentication), `jti`, `iat`, `exp`. A team server
accepts a token only for the use it was issued for: a `code` token is taken with a code and nowhere else, so a server
that passes on a token an app got for a code there cannot join or sign in with it anywhere; an `accept` token is taken
only at the team server's `POST /auth/cloud/accept`, which takes no other.

**Accepting an invite** (the desktop, N1). The app reads the invite's server from the registry (`GET /servers/{id}`:
the address and the certificate fingerprint it pins), then asks `POST /identity-tokens { audience, invite }`. The
cloud first checks that the device is a two-factor session (`403 mfa_required` while the account has it off,
`403 session_without_mfa` for a device that signed in without a code; either way the invite stays open). It then
checks that the invite is unexpired, to the account's email and from that server, and either open or accepted
by this same account. It marks the invite accepted by the account, issues a join token whose `invite` claim names it,
and records the join. The team server admits the verified email to that invite with the token; without the claim it
admits only an account it already knows.

**Accepting in the browser** (decided 2026-10-31, ORC-576; the contract is in `orchester-api` since ORC-577, the cloud,
the team server and the web front are not built). The web front's *Accept* asks `POST /identity-tokens { audience,
invite, purpose: "accept" }` (`TokenPurpose::Accept`) under the same checks, marks the invite accepted (`accepted_at`)
and records no join until the team server's signed receipt comes back; until then `GET /invites` keeps listing it with
`accepted_at` (`CloudInvite::accepted_at`). The flow, the `orchester://` link and the ended and unreachable cases are
in [server.md](architecture/server.md), "Accepting outside Orchester" (row 77).

- **The team server's side.** The web front hands the token and the newest key set to the team server's web app,
  whose server side sends `POST /auth/cloud/accept { identity_token, key_set? }` (`CloudAccept`) to the team server.
  It takes only an `accept` token with `mfa` that names an open invite for the verified email, makes the account and
  the membership in one transaction and answers `201 { browser_session?, membership: { kind, project?, role? },
  receipt, approval_required }` (`CloudAccepted`): `browser_session` (`{ token, session, user }`, the browser kind of
  device token, 12 hours) is absent when `approval_required`, join approval being on for an account that existed
  already. Errors: `401 invalid_identity` (also a token of another purpose), `403 mfa_required`, `403 not_invited`
  (also an identity linked to another account), `403 forbidden` (an account invite for an admin, or an admin invite
  whose inviter is no longer an enabled admin), `403 cloud_off`, `409 key_set_stale`, `410 invite_ended`, `429
  rate_limited`; each but `invite_ended` leaves the invite open.
- **The receipt** (`statements::InviteAcceptance`, type `orchester-invite-accepted+jws`): `{ id, server, subject,
  issued_at }`, the invite, the server, the cloud account (`sub`) that joined and when (milliseconds), signed with the
  server's own key (`SigningKey::sign_invite_acceptance`). It reaches the cloud by `POST /invites/{id}/accepted {
  receipt }` (`InviteJoined`) from the server's web app, or by the browser's `/invites/joined` form, whichever comes
  first. The cloud checks it with the key the server linked with and that it names this invite and that server
  (`identity::verify_invite_acceptance`), records the join (the server is then in `GET /servers`), drops the invite's
  row and answers `204`.
- **`410 invite_ended`.** An invite the server revoked, or that expired there, while the cloud still listed it: the
  error carries the server's signed revocation in its details (`{ "error": "invite_ended", "message": "…",
  "revocation": "<orchester-invite-revocation+jws>" }`, an expired invite's too), which the browser takes back to the
  cloud as `DELETE /invites/{id} { revocation }` does, and the cloud deletes the invite.

The rule for an answered invite (N1 security review 1, M2 and minor 1): the cloud keeps the row with its answer
(`accepted` and by whom, or `declined`) until the invite expires. An answered invite is not in `GET /invites` (once
browser acceptance is built, except one accepted in the browser whose receipt has not come back, above). An
accepted one still issues tokens that accept it, to the account that accepted it only, until it expires or the server
revokes it: a join that failed after its token (the server unreachable, a token lost) is simply asked again, and the
team server, which accepts each token once and ends its own invite at the first join, admits it once. A declined one
issues none, and cannot then be accepted; an accepted one cannot then be declined. Because the row stays, delivering
the same signed invite again changes nothing. The cloud has no word from the server that the join happened; the row
goes with the invite's expiry (the minute's sweep) or a revocation.

**Linking a team server** (`POST /servers { link }`, from the cloud's `/authorize` page, purpose `link`). The link
statement is a compact JWS of type `orchester-link+jws` that the server signed with its own Ed25519 key:
`{ server, nonce, name, address, public_key, fingerprint, issued_at }` (`orchester_api::statements::LinkStatement`).

- **A server's id is its key's** (security review 1, B1): `server` must be
  `orchester_api::identity::PublicKey::server_id(public_key)`, a version-8 UUID of the first 16 bytes of
  SHA-256(`"orchester-team-server-id\0"` ‖ the 32 key bytes). A team server makes its id that way and, at every
  start, replaces a stored id that is not its key's with its key's, so nobody can register another server's id with a
  key of their own, and every identity token the cloud issues for an id (the token's `aud`) is for the holder of that
  key. Migration 4 deletes the servers linked under migration 3 whose id is not their key's (with their accounts and
  invites, logged); such a server links again under its own id (security review 2).
- The cloud verifies the statement with the `public_key` inside, refuses one issued more than 10 minutes ago, and
  uses each `nonce` once: a replay is `409 conflict`.
- **The newest statement owns the server.** A statement newer than the one that linked it takes the server over for
  the signed-in account (its admins moved it); an older or equally old one is `409 conflict`.
- One person owns at most 5 linked servers (`409 too_many_servers`): the account's, and those the same person linked
  from an account they deleted (see *Deleting the account*). Linking one it owns already does not count.

The answer carries what the server pins: the cloud's root key (`root_key`, base64url; the cloud makes it at its first
start and keeps it, `docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Keys") and the newest key set it signed, and an identity token with purpose `link` that links the admin's account on the server. A token with purpose
`admin` is issued only for a linked server.

**The web front's `/authorize`** takes a team server admin's request only in a form's body, `POST /authorize/start`
(`purpose`, `server` or `statement`, `state`, `return`), keeps it sealed in this browser's `__Host-orchester_authorize`
cookie for 10 minutes and shows `/authorize?return=<callback>`, whose address names only the callback: the statement
never travels in a URL, a history or a log. Confirming uses the request up and binds it to the account signed in then.
`/authorize/start` keeps a request only when the post's `Origin` is the callback's origin (the admin that asked);
anything else leaves this browser's request as it was. A browser sends `Origin: null` when an https page posts to plain
http; a cloud on plain http (loopback only, a local or test setup) therefore takes a null origin with an https
callback, and an https cloud never does. The confirm form carries a digest of the request the page
showed, and confirming refuses with *This page is out of date, open the link again* when the cookie holds another
one by then, so a request another site posts after the page loaded is never the one confirmed (security review 2).

**Invites in flight.** A team server signs each invite by email (`orchester-invite+jws`:
`{ id, server, kind, email, project?, role?, inviter, expires_at, issued_at }`, times in milliseconds) and its server
admin delivers it, from the host or from the inviter's browser through the web front's `/invites/deliver`. The cloud
checks the signature with the key the server linked with, keeps the invite once (a second delivery changes nothing),
and emails the invitee in its own words only (nothing the server chose: an account gets *open Orchester*, an address
with no account `/signup?invite=<id>`). The answer names the inviting server as registered (`202 { server }`).
`GET /invites` lists them to the account with that email; a revocation (`orchester-invite-revocation+jws`,
`{ id, server, issued_at }`) deletes one and keeps its id and server in `revoked_invites` until the invite would have expired
(at most 8 days), so the same signed invite cannot be delivered again (security review 1, minor 1); expired invites
and ended tombstones are deleted by the minute's sweep.

**Invite limits** (security review 1, M2): deliveries are counted per client address before the signature is
checked; the servers one person owns keep at most 100 new invites a day between them, those linked from an account the
person deleted included, and revoking gives none back;
an invite's email has buckets of its own, apart from confirmation and reset emails: one a day per server and address
and three a day per address. An invite over an email limit is still kept and listed in the invitee's app, without an
email. Numbers in [Rate limits](#rate-limits).

**The web front's `/invites/deliver`** takes the signed invite in a form's body only, delivers it, and keeps the
answer in a signed cookie for `/invites/delivered`, which leads back only to the admin at the address the cloud
registered for that server (`<address>/admin`): nothing in the post or the page's address names where to go
(security review 1, minor 2).

**Declining an invite.** `POST /invites/{id}/decline` answers an open invite to the account's email with no (`404`
for any other, also one already answered). It leaves the list and issues no token. The row stays, as declined, until
the invite expires, so delivering it again (from the server or the inviter's browser) changes nothing and sends no
email. `revoked_invites` holds only the team server's revocations. The team server keeps its own copy until it
expires or is revoked; nothing joins with it, since a join needs an identity token that names it.

**Deleting the account.** `DELETE /me { password }` asks for the password again (failures count as a password
change's do). It deletes the account, its devices, links and joins, and the invites to its email, answered or not. The team
servers it linked stay registered with no owner, with their members' joins and invites (migration 5), until an admin of
the server links it again: the newest statement the server's key signs owns it. Meanwhile the server keeps the
person who linked it as `last_owner` (migration 7, N1 security re-review 2), and counts toward that person's caps: the
5 linked servers and the 100 invites a day. A re-signup gets a new account id, so the person is keyed by the account's
email, which the cloud does not keep: `last_owner` is an HMAC-SHA256 of the normalized email under a key derived (HKDF,
info `orchester-cloud people v1`) from the key-encryption key, so the database alone can neither name the email nor
check one against it, and signing up again with the same email gives no caps back. A new link statement clears it with
the new owner. A server ownerless since before migration 7 has no `last_owner` and counts its invites on its own. So
after a deletion the cloud still holds, on each server the account linked, that keyed hash of its email, until the
server is linked again; the web's delete confirmation says so when the account linked servers ("A keyed hash of your
email stays with the team servers you linked, for abuse limits, until they are linked again."). The audit keeps the
account's id with the actions it took.

## Two-factor authentication

Added 2026-10-16 with migration 10. An invite to a team server is sent and accepted only by a person with two-factor
authentication on.

**Method.** TOTP (RFC 6238): HMAC-SHA-1, 6 digits, a 30-second step, accepted for the step before, the current one and
the one after. A code works once: the cloud keeps the last step it accepted, and refuses that step and every earlier
one (`401 invalid_code`). The secret is 20 random bytes, shown in base32. It is sealed with
`ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY` (AES-256-GCM, as the root and signing keys are) with the account's id as associated
data, so it opens only for its account. The secret and the recovery codes appear only in the answers that make them,
never in a log, an error, `Debug` output or the audit.

**Recovery codes.** Ten, each `xxxxx-xxxxx`: ten characters of Crockford's base32 in lower case (`0-9a-z` without `i`,
`l`, `o`, `u`; 50 random bits) with a dash in the middle. Each works once in place of a code from the app, anywhere a
code is asked. Case, spaces and dashes do not matter, and `o`, `i` and `l` read as `0`, `1` and `1`. The cloud keeps
only an HMAC-SHA256 of each under a key derived from the key-encryption key (HKDF, info
`orchester-cloud recovery codes v1`). Signing in or stepping up with one answers `recovery_codes_left`, so a client can
warn when few remain; `GET /me` says it too.

**Turning it on.** `POST /me/two-factor { password }` answers the secret and an
`otpauth://totp/<host>:<email>?secret=…&issuer=<host>&algorithm=SHA1&digits=6&period=30` link for a QR code, where
`<host>` is the host of `ORCHESTER_CLOUD_PUBLIC_URL`. A setup not confirmed within 15 minutes ends. A code from the app
confirms it: `POST /me/two-factor/confirm { code }` turns it on, answers the recovery codes once, and makes the device
that confirmed it a two-factor session. Other devices stay signed in as they were, without it.

**Turning it off** takes the password and a code from the app or a recovery code (`DELETE /me/two-factor`). It deletes
the authenticator with its recovery codes, and every device stops being a two-factor session. No token ends with it,
so it records no `reason` ([Why a token ended](#why-a-token-ended)): the devices stay signed in, without two-factor. **New recovery codes**
take a code (`POST /me/two-factor/recovery-codes`) and end every earlier one. The account's email gets a notice when
two-factor authentication is turned on or off, when a recovery code is used, and at the tenth wrong code of a day
(purposes `two_factor_on`, `two_factor_off`, `recovery_code_used`, `two_factor_failures`, each with a link to
`<public URL>/account#security`).

**Signing in.** With it on, the right password answers `202 { challenge, expires_at }`, for the desktop and the web
front alike. `challenge` is `orcm_` and 64 hex characters; the cloud keeps only its SHA-256. It works once, for 5 minutes
and 5 codes, and only while the password is the one checked (a change or a reset ends it). `POST /auth/sessions/two-factor
{ challenge, code }` with a code from the app or a recovery code answers what a password-only sign-in answers, with
`recovery_codes_left`. The device is a two-factor session. A wrong code answers `401 invalid_code` and the challenge may
be tried again; an ended one answers `401 challenge_expired`, and the client signs in again with the password.

**Stepping up.** A device that signed in without a code (before two-factor authentication was turned on, or turned on
on another device) becomes a two-factor session with `POST /auth/sessions/current/two-factor { code }`: no password,
no new token. The web front's Account page uses it.

**Code limits.** Every code check of an account counts against two limits, whichever endpoint and challenge it came
to: 5 wrong codes in 15 minutes per account, in memory (`429` with `Retry-After` until the oldest leaves the window),
and 20 wrong codes a day, counted from the first of them and kept in the database, so a restart does not reset it
(`429` with `Retry-After` until that day ends). The day's count is the authenticator's for a sign-in's second step and
for a device that is a two-factor session; a device signed in without a code (before two-factor authentication was
on) counts its own, so its wrong step-ups, or any other code it enters, spend only its own day. Each check reserves
its place in the first before it runs, so parallel guesses cannot pass it, and a right code takes its place back. At
most 20 guesses a day at 3 codes in a million keep a guesser's chance under one in ten thousand a year. The tenth wrong
code of a day mails the account's email (purpose `two_factor_failures`): whoever entered it had the password or a
signed-in device.

**The cap locks out.** Whoever has the password can spend the account's day with wrong codes at the second step of a
sign-in, and then its owner cannot finish a sign-in with a code either, a recovery code included, until that day ends:
the cap trades a day of lockout for a bound on guessing. Devices already signed in stay signed in. A password reset
does not clear the count: a reset proves only the mailbox, and clearing on it would give whoever reads the mail twenty
guesses more each time; the owner changes the password (the intruder had it) and waits out the day, or the operator
clears the count (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Two-factor authentication for an account that lost its phone"). These limits are
apart from the password limits: wrong codes do not block passwords, and a right password with two-factor
authentication counts as a successful password check. A challenge also ends after 5 codes, right or wrong. Failed
codes are audited (`two_factor_failed`). A sign-in takes its challenge in the same transaction that spends the code,
before the code is looked at: a second request with the challenge waits for the first, then finds it used and spends
no code of its own.

**What a two-factor session may do.** Only a two-factor session accepts an invite (`POST /identity-tokens` with
`invite`: `403 mfa_required` or `403 session_without_mfa` otherwise, and the invite stays open), and `GET /invites`
says for each whether this device would be refused (`needs_two_factor`). Every identity token says whether its device
is a two-factor session (claim `mfa`). Team servers take an invite's join token only with `mfa`, make invites (by email
or with a code) only from a device token issued for an identity token with it, and take every code but the setup code
from an app only with a token for codes (purpose `code`, the server's id as `aud`) that says `mfa`
([architecture/server.md](architecture/server.md), "Invites", "Codes"). So the desktop asks for a token for the server it
enters a code for, from a two-factor session, and carries the key set with it. It asks only after the server proved
it holds the key whose id it gives itself, on a connection presenting the certificate the proof names (a team
server's id is its public key's; `POST /server/key-proof` on the team server), so a server cannot get a token for
another's id, nor pass on another's proof, unless the two present the same certificate. Invites made before and delivered later are not held to it: delivering
(`POST /invites`) checks the server's signature only. A team server keeps whether a device joined with `mfa` for as
long as that device token lives: turning two-factor authentication off at the cloud does not reach team servers, and
a device keeps sending invites there until an admin signs it out or the person joins it again.

**A lost phone.** The recovery codes are the only way back; two-factor authentication is not reset by email, and a
password reset leaves it on. On a cloud the person runs, an operator can turn it off for an account with one
statement, which deletes the authenticator, its recovery codes and its challenges and ends every two-factor session
with it (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`, "Two-factor authentication for an account that lost its phone").

**Someone else turned it on.** Turning it on needs only the password from a signed-in device. Whoever has both while
it is off (a stolen password and an unlocked computer) can turn it on with their own phone and lock the person out of
new sign-ins. The person learns of it from the `two_factor_on` email. Their devices stay signed in, but none of them is
a two-factor session, so none can turn it off, make recovery codes, change the password, delete the account or sign
the intruder's device out. The way back is the operator's: on proof of the account (the mailbox), the statement above
turns it off, after which the person changes the password and signs the other devices out. Orchester Cloud adds no
self-service path for this, since any path that skips the code is one an intruder could take as well.

## App releases

Added 2026-10-30 (ORC-411): how the desktop app learns of and fetches a new version, how CI uploads one, and how
Orchester staff steer it. The types are in `orchester_api::releases`, the calls on `orchester_api::client::CloudClient`.
Three kinds of caller:

- **anonymous**: the update check, the public Stable release and downloads need no sign-in, so an app used only
  locally updates too and the website's download page reads the cloud directly;
- **`releases:write`**: CI, with `Authorization: Bearer <release token>`, a cloud API token whose scopes include
  `releases:write`. The operator makes it (`docs/cloud-hosting.md` in `ivanzaida/orchester-platform`); it is not a
  device token and signs nobody in. A device token, or a token without the scope, is `403 forbidden`;
- **admin**: the device token of an Orchester staff account (the web front's session) that is a two-factor session.
  An account that is not staff is `403 forbidden`; staff without two-factor authentication `403 mfa_required`, and a
  device that signed in without a code `403 session_without_mfa`.

### Versions, channels and stages

- **Version**: `MAJOR.MINOR.PATCH` with an optional pre-release suffix (`0.5.0-beta.1`), ordered as Semantic Versioning
  2.0.0 orders them. Numbers have no leading zeros and fit in 32 bits, pre-release parts are `[0-9A-Za-z-]+` joined by
  `.`, build metadata (`+…`) is refused, at most 64 characters: every version has one spelling, which is also the one
  in paths and in the signed message.
- **Platforms** `windows`, `macos`; **architectures** `x86_64`, `aarch64` (Rust's `std::env::consts::ARCH`).
- **File names**: 1–128 characters of ASCII letters, digits, `.`, `_`, `+` and `-`, not starting with `.` or `-`, so
  they go into URL paths as they are.
- **Channels** `stable` and `beta`. A Beta install is offered every published release; a Stable install only releases
  staff promoted to Stable, at their rollout percentage. A version with a pre-release suffix is Beta only: it cannot be
  promoted (`422 invalid`).
- **Stages** (`stage` in the admin's shapes): `draft` while CI uploads; `beta` with `paused` once finalized; `stable`
  with `percent` (0–100) and `paused` once promoted (it stays on Beta for every Beta install); `withdrawn`, offered to
  nobody, its files no longer served (`404`). Withdrawing is final; a fixed build is a new version. `paused` is about
  the channel the stage names: a paused `beta` release is offered to no Beta install, a paused `stable` one to no
  Stable install (Beta installs still get it). A cloud before ORC-575 leaves `paused` out of `beta`; read it as
  `false`.

### The signed message

CI signs each file with an Ed25519 release key. The signature is over these exact bytes (`SignedFile::message`):
seven lines of ASCII, each ended by one `\n` (also the last), no other whitespace, the size in decimal without leading
zeros, the SHA-256 of the file's bytes in lowercase hex:

```text
orchester-release-file-v1
version=0.4.0
platform=windows
arch=x86_64
name=Orchester-0.4.0-windows-x86_64.msi
size=48213504
sha256=1055256c88ce265e44b7f378877ffe1c8e85c7670c4fa552f69f2e691a682490
```

So a signature vouches for one file of one version for one platform and architecture: a valid file moved to another
release, name or platform no longer verifies. Signatures travel as 128 lowercase hex characters, SHA-256 digests as
64. The app checks the downloaded file's size and SHA-256 and then the signature with the release keys built into it,
before it runs anything; the cloud checks the same at finalizing, with the same public keys
(`AcceptedReleaseKeys::verify`). Fixed vectors (RFC 8032's first test key) are in `crates/api/tests/releases/`.

**Key ids and rotation.** A release key's id is the first 8 bytes of SHA-256(`"orchester-release-key\0"` ‖ the public
key's 32 bytes), as 16 lowercase hex characters. Every declared and offered file names the `key_id` its signature was
made with. The app holds a short list of accepted public keys (the current one and, while it replaces another, the one
before); a signature naming a key not on the list is refused (`unknown_key`), whatever it says. To rotate: ship an app
version that accepts both keys, then sign with the new key; once installs have moved past the versions that know only
the old key, drop it from the list. The private key lives only in CI's secret store.

### Rollout

An install is in a Stable release's rollout when `bucket < percent`, where `bucket` is the first 8 bytes, big-endian,
of SHA-256(`"orchester-rollout-v1\0"` ‖ the install id as 36 lowercase hyphenated characters ‖ `"\0"` ‖ the version as
written), modulo 100 (`releases::rollout_bucket`). Raising the percentage only adds installs; each version spreads its
buckets anew, so the same installs are not always first. At 0 % nobody gets it, at 100 % everybody.

**The install id** is the app's `InstallationId` (`orchester_core::id`), the UUID of its application database
(`orchester.db`), which already names its secrets in the OS keychain. It names no person or computer. No other
anonymous id existed before this contract: the active-installs count of Launch readiness (ORC-133, in the platform
repository) counts the distinct install ids of update checks, so the two use this one id. A development build and a QA
copy have databases, and so ids, of their own.

### Endpoints

| Method and path | Auth | Request | Response | Errors |
| --- | --- | --- | --- | --- |
| `GET /updates/check?version&platform&arch&channel&install_id` | anonymous | the query, every field required: the app's version, platform, architecture, channel and install id | `200 { update: null }` when nothing newer is offered; `200 { update: { version, channel, notes, critical, published_at, files: [{ name, platform, arch, url, size, sha256, key_id, signature }] } }` | `400 bad_request` (a field missing, unknown or repeated), `422 invalid` (a version, platform, architecture, channel or id that is not one), `429` |
| `GET /releases/stable?platform&arch` | anonymous | the query, both fields required: a platform and an architecture | `200 { release: null }` when Stable offers everyone no release for them; `200 { release: { version, published_at, files: [{ name, platform, arch, size, sha256 }] } }`; both with `Cache-Control: public, max-age=60` | `400 bad_request` (a field missing, unknown or repeated, an `install_id` too), `422 invalid` (a platform or architecture that is not one), `429` |
| `GET /releases/{version}/files/{name}` | anonymous | `Range: bytes=<start>-[<end>]` to resume, optional | `200` the whole file, or `206` with `Content-Range: bytes <start>-<end>/<size>`; both with `Content-Type: application/octet-stream`, `Content-Length`, `Accept-Ranges: bytes` and `ETag: "<sha256>"` | `404 not_found` (no such release or file, a draft, a withdrawn release, deleted files), `416 range_not_satisfiable`, `429` |
| `POST /releases` | `releases:write` | `{ version, notes, critical?, ci_run? }`: notes in Markdown, at most 64 KiB; `ci_run` the CI run that uploads it, `{ number, url }` | `201 { release: ReleaseSummary }`, a draft | `403 forbidden`, `409 conflict` (the version exists, in any stage), `422 invalid` (also a `url` that is not `https://…`, over 2048 characters or with spaces or control characters) |
| `POST /releases/{version}/files` | `releases:write` | `{ name, platform, arch, size, sha256, key_id, signature }` | `201 FileUpload` for a new file, `200 FileUpload` for the same declaration again (resume) | `403 forbidden`, `404` (no such draft), `409 conflict` (the release is not a draft, or another declaration under this name, or a second file for the same platform, architecture and name), `422 invalid` (size 0 or over 2 GiB) |
| `PUT /releases/{version}/files/{name}/parts/{part}` | `releases:write` | the part's bytes, `Content-Type: application/octet-stream`: `part_size` bytes, the last part the rest | `200 FileUpload`; a part sent again replaces it | `403 forbidden`, `404` (no such draft or declared file), `409 conflict` (not a draft), `413 payload_too_large`, `415`, `422 invalid` (a part number past `parts`, or a part of the wrong length) |
| `POST /releases/{version}/finalize` | `releases:write` | | `200 { version, files: [{ name, result: "accepted" }` or `{ name, result: "rejected", reason }] }` | `403 forbidden`, `404` (no such release), `409 conflict` (not a draft), `422 invalid` (no file declared) |
| `GET /admin/releases` | admin | | `{ releases: [ReleaseSummary], installs_by_channel: ChannelInstalls \| null }`, newest version first | `403 forbidden`, `403 mfa_required`, `403 session_without_mfa` |
| `GET /admin/releases/{version}` | admin | | `{ release: ReleaseDetails }` | as above, `404` |
| `POST /admin/releases/{version}/promote` | admin | `{ percent }` | `200 { release: ReleaseSummary }`, stage `stable`, not paused | as above, `404`, `409 conflict` (not `beta`, or a paused `beta`: resume it first), `422 invalid` (a pre-release version; a percent over 100) |
| `PUT /admin/releases/{version}/rollout` | admin | `{ percent }` | `200 { release }` | as above, `404`, `409 conflict` (not `stable`), `422 invalid` |
| `POST /admin/releases/{version}/pause` | admin | | `200 { release }`, `paused: true`: a `beta` release stops being offered on Beta, a `stable` one on Stable | as above, `404`, `409 conflict` (a draft or withdrawn release, or paused already) |
| `POST /admin/releases/{version}/resume` | admin | | `200 { release }`, `paused: false`: offered again on the channel it was paused on | as above, `404`, `409 conflict` (a draft or withdrawn release, or not paused) |
| `POST /admin/releases/{version}/withdraw` | admin | | `200 { release }`, stage `withdrawn` | as above, `404`, `409 conflict` (a draft, or withdrawn already) |
| `PATCH /admin/releases/{version}` | admin | `{ notes?, critical? }` | `200 { release }` | as above, `404`, `409 conflict` (withdrawn), `422 invalid` (notes over 64 KiB) |
| `DELETE /admin/releases/{version}/files` | admin | | `204`; the release stays in the list with `files_deleted: true` | as above, `404`, `409 conflict` (the release is still offered: see below) |

Shapes of the admin:

```text
ReleaseSummary  { version, stage: "draft"|"beta"|"stable"|"withdrawn", percent? (with stable),
                  paused? (with beta and stable), critical, created_at, published_at: ms | null, installs,
                  installs_by_channel: ChannelInstalls | null, files_deleted, ci_run: CiRun | null,
                  withdrawn: { at: ms, by: string } | null, uploads: [FileProgress] }
ReleaseDetails  { …ReleaseSummary, notes, files: [AdminFile],
                  adoption: [{ day: "YYYY-MM-DD", installs, installs_by_channel: ChannelInstalls | null }],
                  activity: [{ at, actor: string | null, action, percent? | critical? | channel? }] }
ChannelInstalls { stable, beta }
CiRun           { number, url: "https://…" }
FileProgress    { name, platform, arch, size, sha256, parts, received, state: "receiving"|"verifying"|"verified"
                  | "rejected", reason? (with rejected), received_sha256? (with rejected) }
AdminFile       { name, platform, arch, size, sha256, key_id, downloads }
FileUpload      { name, size, part_size, parts, received: [part numbers, ascending] }
```

`installs` counts the distinct install ids whose newest update check in the last 30 days named this version;
`adoption` counts, for each of the last 30 days (UTC, oldest first), the install ids whose checks that day named it;
`downloads` counts downloads that started at byte 0, so resumed ranges do not count twice. `activity` is newest first;
`action` is one of `created`, `rejected` (a finalize that rejected a file), `published`, `promoted` and `percent_set`
(with `percent`), `paused` and `resumed` (with `channel`, `stable` or `beta`), `withdrawn`, `critical_set` (with
`critical`), `notes_edited`, `files_deleted`; `actor` is the staff member's display name, `null` for CI.

The fields ORC-575 added, each optional for a reader, so a cloud or app on the shapes before it still reads:

- **`installs_by_channel`** splits a count by the channel each install follows, as the update check named it: on a
  release, its `installs`; on an adoption day, that day's `installs`; on the list, every active install (an install id
  with an update check in the last 30 days), whatever version it runs. `stable + beta` is the total. `null` (or absent)
  from a cloud that does not count them apart.
- **`withdrawn`** on a summary: when the release was withdrawn and the display name of the staff member who did it.
  Set with stage `withdrawn`, `null` otherwise.
- **`ci_run`**: the CI run that uploaded the release, as CI named it at `POST /releases`: its number as the CI shows it
  and an `https` link the cloud stores and shows as it is, without following it. `null` when CI named none. The body
  of `POST /releases` takes no unknown field, so a cloud before ORC-575 refuses one with `ci_run`: CI sends it only
  once the cloud reads this contract, and leaves the field out when it has no run.
- **`uploads`**: a draft's declared files and how far each is. `parts` is the file's part count, `received` how many
  of them the cloud has (`received` of `parts`), `sha256` the declared digest. `state` is `receiving` while parts
  arrive and until CI finalizes, `verifying` while finalizing checks it, `verified` once finalizing accepted it, and
  `rejected` with the `reason` finalizing gave (`incomplete`, `sha256_mismatch`, `unknown_key`, `signature_mismatch`)
  and `received_sha256`, the digest of the bytes that arrived (absent with `incomplete`). A rejected file stays in
  `uploads` until CI declares that name again, though its parts and declaration are dropped. Empty once the release
  is published, and from a cloud before ORC-575.
- **`paused` on `beta`**, and `channel` on `paused` and `resumed` activity: absent from a cloud before ORC-575, which
  paused only Stable; read them as `false` and `stable`.

**What the check offers.** Among releases newer than the asked version, for the asked platform and architecture
(a release without a file for them is skipped): on Beta, the newest in stage `beta` that is not paused or in stage
`stable`; on Stable, the newest in stage `stable` that is not paused and whose rollout includes the install.
`critical` is true when the offered release or any release between the asked version and it that this install could
have been offered is marked critical, so skipping a critical release still installs promptly. `files` are the offered
release's files for that platform and architecture; `url` is the absolute URL of its download on the cloud's API
address. An asked version the cloud does not know is still compared by order. Each check counts the install id toward
`installs`, `adoption` and their `installs_by_channel` under the channel it names.

**What the public Stable release answers.** Added 2026-11-01 (ORC-609) for the website's download page
(`StableReleaseQuery`, `StableReleaseAnswer`). Among releases with a file for the asked platform and architecture, the
newest in stage `stable` at 100 % that is not paused and whose files are not deleted; `null` when none is. So a
newer Stable release still rolling out (under 100 %) or paused does not count, and the page keeps the previous one
that passes: the release every Stable install outside the rollout runs, never one only some installs are offered, and
never one staff stopped. A withdrawn release is in stage `withdrawn`, not `stable`, so it never counts. `files` are
that release's files for the asked platform and architecture, each served at `GET /releases/{version}/files/{name}`;
the answer carries no URL, notes, key id or signature, since the page links the file and the app checks the signature
itself. The read takes no install id and counts nothing: no `installs`, `adoption` or `installs_by_channel`, no
download. It reveals no draft, Beta-only (stage `beta`), withdrawn or deleted-files release, and no percentage or
pause. `Cache-Control: public, max-age=60` lets the website and any cache in between keep an answer for a minute, so
promoting to 100 %, pausing or withdrawing shows on the page within a minute. `stable` is not a version, so the path
does not clash with `/releases/{version}/…`.

**Uploading a release.** CI creates the draft, then declares each file with its size, SHA-256, key id and signature.
The answer gives the cloud's `part_size` (8 MiB) and the number of parts (`size / part_size`, rounded up). CI sends
each part with `PUT …/parts/{part}`, numbered from 0, part `i` holding bytes `i * part_size` on. These bodies are exempt
from the 1 MiB request limit up to `part_size`, and from the 10 seconds a body must arrive in (5 minutes). An
interrupted upload resumes: declaring the same file again answers `received`, and CI sends only the parts missing.
Then `POST …/finalize` checks every declared file: all its parts arrived (`incomplete` otherwise), the bytes' SHA-256 is
the declared one (`sha256_mismatch`), the key id is one the cloud accepts (`unknown_key`) and that key verifies the
signature over the signed message built from the version in the path and the declaration (`signature_mismatch`). When
every file is accepted the release is published: stage `beta`, `published_at` set. Otherwise it stays a draft, each
rejected file's parts and declaration are dropped, and CI declares and sends those again. A draft not finalized
within 7 days is deleted with its parts.

**Deleting an old release's files** frees their storage. It is allowed for a draft, a withdrawn release, and a release
older than the newest `stable` release that is at 100 % and not paused; any other answers `409 conflict`. The
release's row, counts and activity stay.

## Rate limits

| What | Limit |
| --- | --- |
| Sign-in failures | 5 per email in 15 minutes block that email for 15 minutes; 20 per client address and 100 per IPv6 /48 block that address or network |
| Failed current passwords (`/auth/password`) | 5 per account in 15 minutes |
| Emails (sign-up, resend, reset) | 5 an hour per email, 20 an hour per client address, 100 an hour per IPv6 /48 |
| Failed `confirm`, `reset/finish` and `links/inspect` | 20 per client address and 100 per IPv6 /48 in 15 minutes; for `confirm` and `reset/finish`, 5 an hour per link, each try counted before its password check |
| Wrong passwords for `DELETE /me` | Counted with the failed current passwords of `/auth/password` |
| Identity tokens | 60 an hour per device |
| Two-factor codes (sign-in, step-up, setup, turning off, new recovery codes) | 5 wrong an account in 15 minutes, and 20 wrong a day kept in the database, apart from the password limits; a sign-in's challenge takes 5 codes |
| Invite deliveries (`POST /invites`) | 60 an hour per client address and 300 per IPv6 /48, counted before the signature is checked |
| New invites | 100 a day for all the servers one person owns, ownerless ones they linked included |
| Invite emails | 1 a day per server and invitee address, 3 a day per invitee address; separate from the emails above |
| Linked servers | 5 per person (`409 too_many_servers`), ownerless ones they linked included; not a rate |
| Update checks (`GET /updates/check`) | 12 an hour per install id, 600 an hour per client address and 3,000 per IPv6 /48 |
| The public Stable release (`GET /releases/stable`) | 600 an hour per client address and 3,000 per IPv6 /48, apart from the update checks |
| Downloads (`GET /releases/…/files/…`) | 60 an hour per client address and 300 per IPv6 /48, resumed ranges included |

The counters are in memory, but for the daily cap on wrong two-factor codes. A restart clears them; the cloud runs as
one instance in v1. A client address is its
IPv4 address or its IPv6 /64. Each counter tracks at most 100,000 keys. A new key beyond that evicts the oldest key that holds no count, block or
attempt in flight, rather than being refused, so a flood of new emails cannot lock everyone out. When every key
still counts, the counter grows past 100,000 until those counts expire; it never lifts a block or resets a count
early. The address and network limits bound how fast a flood can add counts.

## Not built yet

- A new email in `PATCH /me` is refused as an unknown field (`400`); the Account page shows *Changing your email is
  not available yet*, as designed.
- Magic link, Google and GitHub sign-in come later.
- `audience` is not checked against registered team servers for joins; a server not linked here may still be joined
  with a token.
