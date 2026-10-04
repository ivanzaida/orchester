# Work plan

The backlog of Orchester work that is known but not started, written so an agent (Codex or Claude, inside or outside
Orchester) can pick up one task without extra context. It lists the goal of developing Orchester inside Orchester first,
then the backlog by area.

## How to take a task

- Read `AGENTS.md` and follow it. It links the skills to read before Rust code (`rust-code-quality`) and before any
  visual change (`lurq-ui`, `orchester-ui`).
- Take one task. Its **Done when** list is the acceptance criteria; do not widen the scope. Report unrelated findings
  instead of fixing them.
- **Non-UI** tasks need `powershell -File tools/check.ps1` and the named filtered tests.
- **UI** tasks also need an independent design verification (`AGENTS.md` rule 15): a second agent that did not write
  the change compares Pencil and app screenshots. Inside an Orchester run that means a separate *Verify* step with its
  own agent profile, exact-PID app capture and host-side focus check. D4 proved that verification path for an
  already-authored UI candidate; authoring a UI change inside Orchester has not yet been demonstrated.
- When a task is done, move it to *Done* at the end of this file in the same change, with the commit or branch.
- Tasks marked **needs decision** wait for the user; do not start them.

## D · Developing Orchester inside Orchester

What is already in place: runs in their own worktrees, pipelines with agent and command steps, limits, questions and
the task conversation, Ready for review, *Merge into main* into the user's checkout, worktree cleanup, project
Environment variables, project MCP servers, tabs, and the installed stable build (`docs/stable-build.md`).

### D1 · Set up the Orchester project in the stable build (user, manual)

1. In the stable build, add `H:/projects/orchester` as a project. First make `.orchester/` ignored by Git for this
   checkout (for example in its local `.git/info/exclude`); the runner refuses to create a run worktree otherwise.
2. Project instruction: *Read AGENTS.md at the repository root and follow it. Before writing Rust, read
   `.claude/skills/rust-code-quality/SKILL.md`. Take tasks from `docs/plan.md`.* Claude Code is started with
   `--setting-sources ""`, so it does not load `CLAUDE.md` on its own; Codex reads `AGENTS.md` itself.
3. Agent profiles: **Implement** (Codex, workspace write, network per the capability table) and **Review** (a different
   profile, read-only), so the reviewer is never the author.
4. Pipeline *Orchester change*:
   Implement → command `powershell -File tools/check.ps1` → command with the task's filtered `cargo test` lines →
   Review → Ready for review.
   Set the Implement **step** to *Can write* as well as the profile: a new agent step starts *Read-only* and can narrow a
   write profile. Keep Review read-only.
5. Environment: nothing for builds. Cargo builds into each worktree's own git-ignored `target/`, which goes away
   with the worktree. A new worktree starts cold (several GB, several minutes).
6. Limits: start at about 3M tokens and 45 minutes of active agent time per run.
7. *Merge into main* merges into the user's checkout, which must be on `master` with no uncommitted changes. Do not
   merge from Orchester while another agent is merging branches into `master` by hand.

## T · Tests and runner

### T4 · A listing survives a row whose id cannot be parsed (non-UI)

A run row whose id or task id cannot be parsed still fails the whole listing
(`crates/store/src/project_db/runs/load.rs`), because it cannot say which run it is. Decide with the store owner
whether such a row is reported as a project-level "unreadable rows" error that still lists the others. **needs
decision**.

### T5 · Re-measure the code index after ORC-48 (non-UI)

ORC-48 changed what agents get of the code index: `symbols.tsv` lists only the code's own definitions
(`symbols-consts.tsv` holds constants and statics, `symbols-tests.tsv` the definitions in test folders; index format
6), the prompt's *Codebase index* part carries an excerpt of `map.md` of at most 8 KiB, and the internal skill
(`crates/runner/assets/codebase-index/SKILL.md`) gives column-anchored `rg` commands for PowerShell and bash. The
measurement that should show whether this helps was deferred by the user: the stable install does not have the
change, and a dev build on this repository would share `<repo>/.orchester` with it.

1. **Done 2026-10-01.** Skill commands verified as written in bash, pwsh 7 and Windows PowerShell 5.1 against a
   format-6 index by the ORC-48 review.
2. Measure index on vs off on 2–3 tasks of different kinds, at least one that needs navigating this repository (e.g.
   a core/runner change), plus ORC-32 again for comparison with
   `docs/evidence/code-index-effect-2026-10-01.md`. Alternate the order of the arms; each run builds in its own
   worktree. Record tokens, time to the first edit and command counts (all, and before the first edit),
   and which index files the agent used.

**Done when** a new `docs/evidence` file has the table and a conclusion.

## M · MCP servers

### M1 · Confirm MCP behaviour in a real run (non-UI)

Confirm with model turns whether Codex under `-a never` lets the agent call an MCP tool, whether
`shell_environment_policy.exclude` keeps secret variables out of the agent's own commands, how Codex reports a server
that failed to start in a signed-in run, and whether Claude's `mcp__<name>` allow rule is enough. **Done when** each is
confirmed or fixed, with the observed CLI output saved as a fixture and parser tests updated
(`crates/core/src/connection/mcp/`).

### M7 · Tools that run without asking, built on `claude/mcp-tool-exempt`

An MCP server lists tools that run without asking when a profile asks before MCP actions (first use: Pencil's
`get_editor_state`, `get_guidelines`, `batch_get`, `snapshot_layout`, `get_screenshot`, `get_variables`; not
`export_nodes`, which writes files). Claude Code: `mcp__<server>__<tool>` allow rules; Codex App Server: the tool's
`approval_mode = "approve"` in the thread configuration (`docs/interactive-tool-approvals.md`). **Done when** an
independent design verifier (no frame exists; closest patterns) reports no blocking or major mismatch for the MCP
server dialog's *Tools that run without asking* area, the inspector's list, the profile enforcement row, the step
inspector line and the activity mark.

### M6 · Environment names in secret references (needs decision)

Environments display in lower case (`@staging`, *(staging)*) where the design writes *Staging*, and typing `@Staging`
is refused (`Environment::as_str` in core). Decide whether names are case-insensitive with a display form; ties in with
the Secrets decision under P.

### M4 · Not built yet (needs decision)

Tool discovery and OAuth sign-in for MCP servers, narrowing servers per pipeline step, changing a running run's servers,
and MCP tools in the Tools library.

## C · Claude Code

### C0 · Claude Code parity, in progress on `claude/claude-support`

Built and tested; **not done until C2's design verification passes**, and not merged (merge order with Codex's
pause-at-approval work in `step/worker.rs` and with `claude/integration` is open).

- Claude Code connections offer `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-fable-5-1` and
  `claude-haiku-4-5-20251001`, then the models the CLI lists through the stream-json `initialize` request, then the
  `opus` and `sonnet` aliases, with the effort levels the CLI accepts. The model reaches `--model` on new and resumed
  sessions and is recorded per invocation.
- *Ask before running commands* and *Ask before MCP actions* work on Claude Code through
  `--permission-prompt-tool mcp__Orchester__approve`, served by `orchester-runner --claude-permission-prompt`, with
  the same approval records and modal as Codex (`docs/interactive-tool-approvals.md`). Every command of a writing step
  asks (whole-tool ask rule); a read-only step gets no commands.
- C1: the step result comes from `--json-schema`'s `structured_output`, or strictly from the JSON that closes the final
  message; a turn without one fails.
- C3 (user decision): an API refusal of a model to an old CLI is reported as `CliTooOld` and recorded on the
  connection; the Connections inspector, the profile's model field and a step's model show a notice. No documented
  source lists these versions, so nothing is known before the first refusal.
- Real Opus 5.5 runs on Claude Code 2.1.284: Implement and Review with one approval round trip, and a `question`
  result. See [evidence](evidence/claude-code-e2e-2026-09-30.md).

### C2 · Verify the Claude Code approval controls against the design (UI)

`claude/claude-support` enables *Ask before MCP actions* and *Ask before running commands* for Claude Code profiles,
pipeline steps and runs, and shows a "CLI too old for this model" notice (C3) in the Connections inspector, the
profile's model field and the step inspector's model. It shows the mechanisms `--permission-prompt-tool · asks each
command` and `read-only · no commands` (with why under it), the run popover's Claude transport note, the MCP
enforcement row "Each tool call waits for your decision", an MCP approval's labelled *Arguments* row and a command
approval's *Agent's note* in the approval modal, and the activity problems "Orchester approvals were unavailable to Claude Code" and
"Claude Code … can't run …". It also changed the model source notes in the Connections inspector and the profile's
model field. A separate agent must compare these states
with Pencil (`AGENTS.md` rule 15): the profile Permissions tab on a Claude connection (`HnBBr`, `ossOP`), the run
permission popover (`W29Od2`), the pipeline step switch (`wujuh`), the command approval modal for a Claude request
(`GbNSA`) and the Connections inspector model note (`xK9v9`). **Done when** the verifier reports no blocking or major
mismatch. Round 1 (at `6422a6d`) found two major mismatches (the transport note's role, the MCP arguments without a
label) and five minor ones; all are fixed and wait for a re-check.

### C5 · Follow-ups from the Claude Code review (non-UI)

- The runner records a model's required CLI version (C3) by reading, changing and saving the whole connection. A
  person who edits the same connection in the desktop at that moment wins or loses as the last writer. Record the
  requirement with a targeted update, or merge it on save.

### C6 · Tool approval layout issues found during C2 (UI)

Seen by C2's verifier, older than the Claude Code work and left alone there:

- The run permission popover opens at the top right of the task screen instead of at its design anchor (`W29Od2`).
- The command approval modal (`GbNSA`) explains the request with the worktree's absolute path embedded in the
  sentence, and cuts the working directory row short. Name the worktree the way the design does and let the row show
  the path's end.

## U · Screens

### U10 · Pull requests screen jumps while it loads (UI)

Opening the Pull requests tab shifts its content two or three times
(`crates/desktop/src/screens/project/pull_requests/mod.rs`):

1. The stored inbox is read from the project database asynchronously (`actions::load` → `ctx.future`,
   `actions.rs`). Until it arrives, `inbox` is `None`: the body is an empty column and the Open tab has no count.
2. When the stored inbox arrives, the Open tab gains its count (`synced.pull_requests.len()`), so it widens and pushes
   the other tabs right, and the list appears below. For an unlinked repository the whole tab row disappears
   (`tabs` is skipped for `Inbox::Unlinked`), which moves the body up.
3. `create` also starts a GitHub sync through `gh` at once. While it runs the topbar sync status text changes width,
   and afterwards the list and count may change again.

**Done when** the screen keeps a stable layout from the first frame: the heading, tab row and body keep their place
until the stored inbox is known (no empty-body frame followed by a list; no tab row that appears and then vanishes),
the Open tab reserves room for its count so a count arriving does not move the other tabs, and the topbar sync status
does not shift the Refresh button when its text changes. A sync that changes the list may still add or remove rows.
Tests: a headless `form_keyboard` test that records the tab and body bounds before and after the stored inbox and a
sync arrive (fake `gh`), plus the existing `pull_request` tests. UI change: independent design verification against the
Pull requests frames (`DF8II` tab row, 08.D `dmib5`) is required.

### ORC-63 · Remove worktree for every ended run · branch `claude/orc63` (done 2026-10-04: three code reviews and three design verifications passed)

Design: `docs/design/orchester-workspace.md` › *ORC-63 · Remove worktree for every ended run* (group `j0VU3B`,
frames 63.01 `O0AMKY`, 63.02 `H9Z7D4`, 63.03 `e4QBB`, 63.04 `rkAU3`, 63.05 `LmTnc`, 63.06 `VBR27`; decisions approved
2026-10-04). Implemented 2026-10-04:

- Core: `RunStatus::has_live_work` (an interrupted run has none); `clean_up` removes the worktrees of an interrupted
  run (`CleanupError::LiveWork` otherwise); `leftovers_of` reads several runs with one worktree listing; Run again
  (`run_again`, `holds_task`, `can_run_again_after`) follows an interrupted run whose worktree was removed, which no
  longer holds its task. `WorktreeStore` records removals.
- Store: migration 40, `run_worktree_removals` (once per run); `interrupted_runs` leaves such runs out.
- Runner: `Prepared::continuing` refuses a run whose removal is recorded (`Rejection::WorktreeRemoved`, the run
  unchanged); Run again reads the removals and records the removal it does.
- Review fixes (2026-10-04): a removal is recorded before git runs, in the write transaction that re-reads the run
  and refuses it once it has live work (`WorktreeStore::begin_worktree_removal`, core `clean_up_recorded`); recording
  a continuation of a run whose removal is recorded is refused in its own write transaction
  (`RunStoreError::WorktreeRemoved`), so the two never both go ahead. The record is taken back when nothing was
  touched (changes appeared since the dialog), and stays once anything was removed or git refused (a refusal can
  come part way). A worktree folder git forgot after a failed removal is listed (`LeftWorktree::forgotten`, IPC
  protocol **15**), shown as left behind with Remove worktree again, and deleted as a folder, only ever
  `.orchester/worktrees/<run or step execution id>`. An interrupted run whose folder is gone is recorded as removed
  when the task page or Run again finds it. The task's active run (Canceled) and a group's first-run statuses leave
  an interrupted run without its worktree out. Rows keep their lines while what was left is read again.
  Second review round: a clean-up fails only before it removes anything (git not runnable, the listing); once
  removing started, a git failure or timeout keeps that worktree or branch with why, so the record stays. A
  forgotten folder lists its files like uncommitted changes and goes only as they were seen. Git's refusal names
  the worktree once, as `.orchester/worktrees/<id>`, with its reason on one line.
- Desktop: the removal is recorded by *Remove worktree* and by a merge's clean-up; *Runs of this task* on Summary
  (after the steps; on Ready for review after Verification; on the unreadable run's page in place of *Other runs of
  this task*), rows with columns from a 1340 window and stacked below, *Remove worktree* on every run with worktrees
  and no live work (the latest keeps its topbar button, which now also shows for an interrupted run, left of
  Continue run), the 02.Q dialog for any run with the interrupted-run notice and the *keeps working* line, one
  removal at a time per task, *Removing…* and git's refusal under the row. An interrupted run without its worktree:
  notice *Run #N can’t continue*, Continue run unavailable with its lock line, step *Interrupted · can’t continue*,
  heading *· worktree removed HH:MM*, *Worktree · Removed · HH:MM* in This run, *Run again* offered.

Not done: *Runs of this task* is not on the plan review (02.11) or the retry-limit page (02.B), which lay out their
own Summary; Cancel run stays beside Continue run for an interrupted run (63.04/63.05 draw none). Design verification
(separate agent) is outstanding.


### ORC-17 · Plan limits in the status bar · branch `claude/orc17` (done 2026-10-02)

Independent review and design verification passed (all states, widths 960–1600). Known: a truncated activity label
could stay cut after the window widened; fixed by lurq 0.33.0 (text layout cache). Implemented on `claude/orc17`, with master merged into the branch on 2026-10-02 (merge `c290682`) and the review and
design follow-ups on top. Codex and Claude Code report their account's limits live during agent turns: Codex App
Server's `account/rateLimits/updated` notification and Claude Code's `stream-json` `rate_limit_event`. The runner hands
each reading to the desktop without storing it; the status bar shows the newest reading per CLI across open projects,
with hover details, near/reached/reset states, block reasons, extra usage and narrow-window priorities (Pencil `poPso`,
item `XCXvw`). Tests: `cargo test -p orchester-core --test plan_limits plan_limits`, `cargo test -p orchester-runner
--features test-agent --test plan_limits plan_limits`, `cargo test -p orchester-desktop --test status_bar status_bar`.
Notes: [implementation handoff](evidence/orc-17-plan-limits-implementation.md).

**Done when** an independent design verifier reports no blocking or major mismatches against `poPso` (states M–R);
then move this entry to *Done*. Not covered: limits are read only while a run's agent works (App Server's
`account/rateLimits/read` outside a run is not designed or used); Codex buckets other than `codex` and Claude Code
windows of one model (`seven_day_opus`, `seven_day_sonnet`, …) are not shown.

### Useful automatic retries · branch `claude/retry-feedback` (done 2026-10-02)

Independent review (three rounds) and design verification of 02.B3–B7 passed. Noted, not fixed: retries are counted
over the whole run, so after a person returns the work a later verdict failure of that step waits at once; a member
of a task group that waits for a person keeps Return to drafts unavailable for the group until it is stopped or
retried; the Conversation tab's progress column at 960 loses step names (ORC-61 rewrites that tab).

Built: a retry attempt's prompt carries the failure report (the route's target reads why the work came back with the
failed step's whole result or a failing command's output tail; the failed step reads its own earlier result); only a
verdict (`reported_failed`, `command_failed`) follows `on_failure`, a `failed` result wins over a turn that failed
after it, any other failure restarts its step in place until it broke twice since it last passed (at most 10 automatic
attempts in a row, `MAX_AUTOMATIC_ATTEMPTS`); a restart keeps the report and note of the attempt it restarts;
exhausted retries and a second breakage make the run wait for a person (`RunStatus::WaitingForPerson`, task phase *Stuck*, counted in *Needs you*), who allows one more
retry, returns the work to an agent step with a note, or stops the run (`Command::DecideStuckRun`). Migration 36
rebuilds `runs` (status `waiting`) and `attempts` (reasons `restart`, `returned`, column `note`); runs that failed
with `retries_exhausted` stay failed. See `docs/architecture/storage.md`, Step graphs, Failures. Tests: `cargo test
-p orchester-core --test run stuck retry_prompt`, `cargo test -p orchester-store --test stuck_runs stuck_runs`,
`cargo test -p orchester-runner --features test-agent --test retries retries`, `cargo test -p orchester-desktop
--test form_keyboard task_graph_retry_limit`, `cargo test -p orchester-desktop --test overview overview_needs_you`,
`cargo test -p orchester-desktop --test activity activity_run_waiting`.

**Done when** an independent design verifier compares the app with the run-waits-for-a-person frames in `O4vgKt`
and reports no blocking or major mismatches; then move this entry to *Done*:
- 02.B3 `FjCWv`: retries used up, panel `Bi6pU`;
- 02.B4 `K7hbj`: a step chosen (Implement, the step a retry resumes, by default) and a note typed;
- 02.B5 `m3pDW`: a step broke twice;
- 02.B6 `NM5My`: *Needs you* in Overview (card *ORC-128 · Review still fails after 2 retries*) and the status bar
  (*Review needs your decision · ORC-128*, hand, until opened or dismissed);
- 02.B7 `zyusO`: the three Activity transcript marks (automatic return, automatic restart, your return with note) and
  the runner's refusal above *Stop and fail the run*.
`DecideStuckRun`, `RunNotWaiting` and `RunWaitsForPerson` raise the ipc `PROTOCOL_VERSION` to 11.
### ORC-44 · Command approvals and current-run policy changes · branch `claude/orc44` (done 2026-10-02)

Independent review (three rounds) and design verification of the permission rows, popovers and notices passed.

Run #12 (`orchester/orc-44-run-2`, base `37716df`) passed Implement, Check and Review; its Verify was interrupted.
Master was merged into the branch (merge `a05ba75`) with master's approval architecture as the base. Almost all of
ORC-44 was already on master: enabling *Ask before running commands* and *Ask before MCP actions* for a profile, a step
or the current run, one-action command and MCP decisions, resuming an `exec` session through App Server, Apply now at
a checkpoint, immutable invocation launch permissions, Windows sandbox selection and scoped build directories (see the
agent runtime architecture entry under *Done*, since reworked by Claude Code support, App Server for every Codex step,
`tool_approvals`, MCP exemptions and ORC-57/60). Dropped as covered or superseded: those runtime, core, store and
runner changes; the command caption that denied sandbox exceptions (master asks about them on purpose); the
`exec`-path invocation tests; the wrapping Target/working directory row in the approval dialog (C6 and the approval
choices design's long values, `bjrgu`, own it).

Ported (still missing on master), then built to the designer's frames approved 2026-10-02 (design doc, "What the
row shows"): the task's Permissions row shows what the step's current or last invocation launched with, never the
queued settings. The label carries the qualifier (*Running with* / *Last ran with* / *Next run uses*; Activity
*Running with* / *Ran with*), the value is bare (*… · MCP asks* / *MCP auto*, *Commands ask*), breaks only at "·"
and stacks above its never-clipped link; *starting…*, *not recorded* and *no permissions · it did not start* cover
the launches without a record. Notices: *Saved for the next invocation · <change>*, *Applying <change> at the next
checkpoint…*, *Restarting with the new permissions…* (no button) and *New permissions not active: <agent> did not
restart*, then applied once the restarted invocation recorded its launch. The popover has one *Running with* line,
ends 6 px above its row and scrolls when the window is short; an ended run's popover is read-only with only
*Close* (`docs/interactive-tool-approvals.md`).
Tests: `cargo test -p orchester-desktop --test run_permission_policy run_policy`, `--test form_keyboard
run_permissions` (layout at 1440 and 960, popover above its row at 1020/960/700).

**Done when** an independent review passes and an independent design verifier reports no blocking or major mismatch
for the rows (02.10 `H1wCPK`, 02.12 `YOcfT`, 02.A `d6xleF`, 02.S `Lr8Dg`, 02.J anchor `rnGN7`, `iPBOy`), the board
`iPHGE` (panel states, inline row at 1440 and 960, Activity panel), the popovers (`UPDBO` in `NNRH8`, `zCqp2` in
`ecCTw`, ended `YgLEg`/`vXtTX`), Apply now (`D8qjLz`: `v0wd4R`, `R5mCao`) and `LJmTH` (`A3h7B4`, `ztEdT`, `shNob`).
Known differences: at 960 the inline link wraps under the value with its "·" (lurq cannot drop a separator at a
wrap); notice details say "the new permissions" where the frames name the change ("pnpm build allowed"), and the
pending detail does not name the running command; the ended popover's network switch fades to the locked 0.45.
Then move this entry to *Done*.

### ORC-30 · Tasks list: select tasks, Set pipeline, Run selected · branch `claude/orc30` (UI)

**Done 2026-10-02.** Independent review (four rounds) and design verification (1440 and 960) passed.
Built in Orchester run #10 (`orchester/orc-30-run-2`, on the old base `37716df`) and merged with master on
`claude/orc30` (merges `162206c`, `69677c9` for lurq 0.33.0). Row and select-all selection with the ready / incomplete /
already queued or started split, readiness reasons, Set pipeline (published pipelines, drafts only, skipped tasks
explained), search by title/id/brief, the pipeline filter and sort combined with the tabs (an empty result says so and
can be cleared), and Run selected opening the parallel/sequential scheduling review (editable order, shared task
limits, aggregate token cap) that queues each task exactly once: groups are stored atomically and first runs use
stable first-run keys, so a retry never starts a task twice; the runner keeps the reviewed concurrency and its agent
slots. A group held for good (failed task, spent cap, a start to fix) leaves the runner idle; its unstarted tasks go
back to the drafts with *Return to drafts* in the selection bar. Finished groups are no longer polled, and failing
starts back off. The row of the task New task's Create just made stays highlighted next to the selection checkboxes.
Narrow windows: the review stacks its columns below 1260, the table drops its Pipeline column below 1152.
Undrawn states are listed in the design doc as implemented, not drawn yet. The drawn copy that names merge
dependencies is deliberately not used: dependencies are outside ORC-30 and not built.
History: the run passed Implement, Check and Review; its Verify failed (order 12px high, settings too dense), fixed
in `9971dbf`. An independent review of `4b507a3` failed on runner idleness, group polling, errors and dead code,
fixed in the review follow-ups; an independent verification of `4b507a3` passed at 1440 and failed at 960, fixed in
the design follow-ups.
Tests: `cargo test -p orchester-core --test task_groups task_group`, `cargo test -p orchester-store --test
task_groups task_group`, `cargo test -p orchester-runner --features test-agent --test task_groups task_group`,
`cargo test -p orchester-desktop --test form_keyboard task_bulk`, `cargo test -p orchester-desktop --test tasks task`.

**Done when** an independent re-review and design verifier report no blocking or major findings against Tasks
`J9467` and the scheduling reviews `W1Dq6` (parallel) and `IyJ5a` (sequential), at 1440 and 960; then move this entry
to *Done*. Not verified yet after the follow-ups.

## N · Native agent (P1 in progress)

Orchester runs the coding-agent loop itself against model APIs (Anthropic Messages, OpenAI Responses, compatible
endpoints such as DeepSeek), next to the Codex and Claude Code CLIs. The design is
`docs/architecture/native-agent.md`: crate layout, model clients, tools, permissions, context, connections, tests,
and phases P1–P5 (with P3b, Chat Completions) with acceptance lists. The user's decisions of 2026-10-01 are recorded
there; turn the phases into tasks here when work starts, beginning with P1.

P1 is split into five tasks. P1a (ORC-56) and P1b (ORC-57) are done (see *Done*). The rest:

- **P1c · ORC-58:** the `orchester-agent` crate: Anthropic client (§3.1), file and search tools, fake model server
  (§8).
- **P1d · ORC-59:** runner integration: `Driver::Native`, approvals, persistence, resume (§2.2, §6). Native steps
  start here (`AgentProvider::runs_agents`). Also left here by P1e, because only the runner's model client produces
  what they show: *Test connection* and *Fetch models* through the runner (`Command::CheckConnection`, §7; it records
  `CheckResult::Api`/`KeyRejected`/`BillingProblem`/`EndpointUnreachable` and fetched models with
  `ModelSource::Endpoint`). Further notes for this task:
  - A native command's approval is recorded with `PendingToolApproval::native` (the `CommandShell` and the time
    limit), which 04.H9 shows. Its `action` (the full shell string) and `target` are redacted as the CLI path does
    with `self.redact` in `step/approval.rs` before they are stored.
  - The connection's models and the loop's types meet here: `connection::Effort` (`as_str`) becomes the
    `effort: Option<String>` of `native::model::ModelRequest` and `native::session::LoopSettings`, from the profile's
    effort or the connection's `NativeModels::default_effort`; a fetched `native::model::RemoteModel` (`efforts:
    Vec<String>`, `context_window: Option<u64>`) becomes a `NativeModel` with `ModelSource::Endpoint`, dropping
    effort names `Effort` does not know and limits that do not fit `NonZeroU32`; `NativeModel::context_window`
    becomes `LoopSettings::context_window` (`DEFAULT_CONTEXT_WINDOW` when unknown).
  - The invocation records the endpoint, the engine's vendor (`AgentConnection::vendor`) and the effort it used,
    so 04.H10's panel and header and 04.H9's agent row can show what the run used instead of the connection as it
    is now (*Endpoint now*), and the panel's *Effort* row appears once an invocation records one.
  - The transcript rows of 04.H10 `IcGvI`, moved here from P1e by the user's decision of 2026-10-02, with the
    `AgentEvent` data they need (§2.2): a native tool call's kind (read_file, list_files, search, edit_file,
    write_file, run_command), the lines read and the size, the search scope and its match and file counts, an edit's
    added and removed lines, the shell a command ran in (*shell · pwsh*), an approval (*Approved by you · approve once
    · waited 48 s*), a denial by a person (*Denied by a person · the agent continues*, told apart from a refusal, so
    the panel can say *denied by you*) and the result the agent was given for it.
  - The prompt must honour `AgentFilesMechanism::OrchesterLoads`/`OrchesterLeftOut`, which the enforcement panel
    already shows.
- **P1e · ORC-60 (done 2026-10-02, branch `claude/orc60-fix`; independent review and design verification passed):**
  connections UI, 04.H1–H11 (group `PNDQm`). Built: Add connection's four kinds
  (OpenAI API locked until P2), the native form with the Official preset (DeepSeek and Custom locked until P3),
  Advanced base URL, the key secret picked or created with the connection in the environment chosen (Development by
  default, as in Secrets), models added by hand with default model and effort, provider fallback (04.H1–H3); the
  native inspector with its statuses, key missing derived from the project's secrets (04.H4, H5); the models dialog
  (04.H6), where a listed model in use keeps its id and is never dropped: core `ensure_used_kept` compares the list
  before and after against every published profile version and draft, the profile versions unfinished runs pinned,
  and every step of a published pipeline or draft that overrides the model; the §5.4 capability rows with
  *Enforced by Orchester* (04.H8); the approval of a native command with its shell, full string, working directory
  and time limit (04.H9); in Activity, the step header and the invocation panel of a native invocation (04.H10: runs
  on this computer, the connection's endpoint now, network, tool calls); and the note of what Orchester could not
  enforce under the readiness card (04.H11). Test connection and Fetch models are visibly unavailable. Not built:
  04.H7; the *Effort* row of 04.H10's panel (the invocation records no effort yet, P1d). The transcript rows of
  04.H10 moved to P1d by the user's decision of 2026-10-02 (see P1d). Tests: `form_keyboard native_`
  (`native_connections`, `native_dialog`, `native_models`, `native_model_uses`, `native_run`), core `native_connection`/`native_setup`,
  store `connections native`, `tool_approvals native_`.

## L · After lurq 0.30.0

The tab and text-input fixes shipped in lurq 0.30.0, the outside-press fix for window chrome in 0.30.1, the
select-menu width option in 0.30.2, the verifier window focus option in 0.30.3, and text selection after a drag on
scaled displays in 0.30.4. This workspace uses 0.37.0: a shrinking child of a single-line row gives way in its `shrink_order`, down to its `shrink_limit` (`MinSize`, `Content`, or `Drop`: natural size or dropped whole), in whole pixels (0.35.0), or trims to `shrink_drop_below(size)` and then drops, and a drop gives its width back to the line (0.36.0); what gives way whole only accumulates as a line narrows, and an item holding droppable children keeps its natural size until its line collapses it (0.37.0); the status bar's one line uses them. Its layout cache restores text output (ellipsis, wrap, caret geometry) on a cache hit (0.33.0); a scrollbar takes the pointer around its thumb, or its whole lane when the lane is reserved or painted, so pressing there never selects text (0.33.1); and a window-sized layer (a `Modal` a component declares itself, a popup) is laid out at the current window size after a resize back (0.34.0, which also adds profiling sessions and the `lurq_profile_*` MCP tools). A component's `ctx.future`/`ctx.stream` tasks are aborted when it unmounts (0.34.2), so a page's watchers stop with the page. Since 0.31.0 the MCP server also offers `lurq_inspect` and
`lurq_act`, and `lurq_read_tree` quotes app-provided text that is not a plain word (`{tab="All projects"}`); since
0.32.0 percentage-width children of a wrapping row resolve against the row's width. A later patch release needs only `cargo update -p lurq` (the requirement in `[workspace.dependencies]` already
admits it) and, to keep naming the version in use, this line.

## S · Shared server (self-hosted)

The design is `docs/architecture/server.md`: read it whole before starting, it is the specification. The goal: several
people share a project's task list on a server their team runs, and see who took a task into work. Runs, worktrees,
transcripts and secrets stay on each person's computer. Phases S1a and S1b are server-only (no desktop UI) and are
handed to Codex; the screens of S2a–S3 are drawn and approved (2026-10-03).

Rules for both phases, on top of `AGENTS.md`:

- Work in `.tmp/worktrees/codex-server-<phase>` on a branch `codex/server-<phase>`, created from master (S1a is merged).
- New crates exactly as the document's *Processes and crates* says: `crates/api` (`orchester-api`, types only in S1a
  and S1b; the `client` feature with reqwest comes with S2a) and `crates/server` (`orchester-server`, binary
  `orchester-server`). New dependencies only those in its *Dependencies* table, each in `[workspace.dependencies]`
  with the reason in the commit message.
- Core gains only types and pure functions (`sharing`, `TaskRevision`, `RepositoryIdentity`); no HTTP, no database.
  The server calls core's rules; it does not re-implement them.
- The server must build and its tests pass on Windows (this host), and `cargo check -p orchester-server
  --target aarch64-apple-darwin` must pass. Linux and Docker are the supported hosts: ship the Dockerfile and the
  compose example with Caddy; if Docker is not available on this host, say that the image was not built.
- Server tests live in `crates/server/tests/`, start a real server on loopback with a temporary data directory and a
  settable `Clock`, and carry name filters (`cargo test -p orchester-server --test <binary> <filter>`).
- Tokens and passwords never reach logs, errors, `Debug` output or test snapshots (rule 11); a test captures the log
  of a sign-in and work session and asserts neither appears.
- Token idle expiry is 30 days (user, 2026-10-03); it is a constant, not a setting.
- When a phase is done: `powershell -File tools/check.ps1`, the server tests, the macOS check; then move the phase to
  *Done* with its branch, and report what was not run. Do not merge into master yourself unless the user asks; an
  independent review by another agent comes first.

### S1b · Projects, tasks, claims and events on the server (non-UI)

Starts after S1a is merged. Done when every S1b box in § *Phasing* holds:

- Core: `sharing` (claim take, release, take-over, heartbeat, staleness at 15 minutes, the run phases that end a
  claim; tests over explicit times) and `RepositoryIdentity::from_remote_url` (HTTPS, SSH and scp-like forms of one
  repository match; local paths and no `origin` refused).
- Projects: create with upload (numbers kept, counter after the highest), lookup by remote (`403 not_member` with the
  owners), members added and removed by owners and admins only, `404` for non-members; of two simultaneous creates for
  one repository exactly one succeeds and the other gets `409 remote_taken` and changes nothing.
- Tasks: 5 clients creating 50 tasks each get numbers 1–250 with no gap or duplicate; a repeated idempotency key
  returns the first result; a stale revision gets `409 revision_conflict` with the current task; the edit lock and
  queueing follow core's rules; import gives new numbers and keeps ids and former keys.
- Claims: of two simultaneous claims exactly one wins; a heartbeat refreshes and reports lost claims; stale after 15
  minutes of test clock; take-over refused while fresh or changed and recorded when done; removing a member or
  disabling a user releases their claims.
- Events (SSE): a stream resumed with `Last-Event-ID` receives exactly the missed events; a cursor older than the
  30-day retention gets `reset`; a revoked session or membership closes the stream; keep-alive every 20 s.
- Organization (user, 2026-10-03; `docs/architecture/server.md` decisions 23–24, *Task organization*): workflow,
  priority, tags, blocking links and the Blocked reason are fields of `Task` and `Changes` under the revision; core's
  ORC-39 rules refuse an empty reason, a cycle, an unknown tag and Canceled while the reported run has not ended
  (`422 organization`); every change writes a history row (person, actor, before and after, tag names) served by
  `GET …/tasks/{task}/history`; upload and import take tags by name and drop (and list) links outside the upload.
  **Needs ORC-39's `task::organization` on master** (`claude/orc39` is not merged yet); S1b waits for it or merges it
  first.
- Tags (decision 24): create, rename, merge and delete by members; names unique without regard to case (`409
  tag_exists`, `409 same_tag`); merge and delete rewrite every affected task in one transaction with a new revision,
  a history row and a `task` event, then a `tags` event.
- Member roles (decision 25): `PATCH …/members/{user}` by owners and admins; the last owner can be neither demoted
  nor removed (`409 last_owner`); audited; a `members` event.
- `PUT …/run` (live run status, S3's server half) is not part of S1b.

### S2a–S3 · Desktop: servers, sharing, join, live list, run status (UI, design approved; after S1b)

Drawn 2026-10-03 in Pencil section `yYqBE` ("10 / Shared server", column F at x 26080); the design doc's
[Shared server](design/orchester-workspace.md#shared-server) describes every frame and its decisions, all approved by the user on 2026-10-03 (member roles
and shared organization included, also in S1b). Start S2a once S1b is merged. Frames per phase:

- **S2a** (servers and sign-in): 10.01 `sXIz1`, 10.02 `e9DDI`, 10.03 `BWyKt`, 10.04 `X0cARN`, 10.05 `mcI7H`,
  10.06 `YMwe5`, 10.07 `BWEZ9`, 10.08 `kT0ui`; the status bar item 10.43 `hYres`.
- **S2b** (share, members, make local, offline, conflict): 10.11 `xnhV2`, 10.12 `IDfLs`, 10.13 `V4ZxQG`, 10.14
  `NaUDV` (with roles), 10.17 `g5r5VU` (discovery), 10.42 `Cc4WH`, 10.44 `Y6H8A`, 10.41 `RmDpQ` (the revision; its live use is S2d).
- **S2c** (join): 10.15 `y8uP2`, 10.16 `AZwJE`, 10.21 `UlHPB`, 10.22 `m9omas`, 10.23 `l9jYZS`, 10.24 `FAKNF`,
  10.25 `RuBhQ`, 10.26 `m7jOn9`.
- **S2d** (live list and claims): 10.31 `jakf4`, 10.32 `ByIkx`, 10.34 `omHGh`, 10.35 `U4GzjB`, 10.41 `RmDpQ`.
- **S3** (live run status): the run status in 10.31 / 10.32, 10.33 `NikWo`, and the shared status on 10.25.

## P · Product decisions waiting for the user

- **Secrets:** fixed Development / Staging / Production environments; archive a secret instead of deleting it.
- **Permissions:** on the agent profile page the 22px reason indent lines up with neither the radio nor the label.
- **All projects:** the RECENT list can show a project whose data cannot be read.
- **Pipeline history:** show who published a version (the store records no author yet).
- **App log:** an *Open app log* action (no design yet).
- **Import:** where a copy's provenance is shown (recorded, not shown).
- **Tabs:** reopening a closed tab (not designed).
- **Project bar Search and notifications:** Pencil `CsB3h` shows Search and a bell, while the app omits them.
  Define working behavior and data sources before adding controls; do not add inert placeholders.
  Evidence: `.tmp/verify/u9/report.md`.

## Later

- **macOS:** build, install and smoke-test on a Mac (`docs/stable-build.md`, macOS section). All macOS code is only
  cross-compiled so far.
- **Project settings:** disk use of run worktrees.

## Done

ORC-61 polish · Conversation tab design-verification minors · branch `claude/conv-polish` (from master `b96dbf27`)

- **Implemented 2026-10-04, waiting for an independent design verification** against `fTfl8` (`RrScB`, `CxrSA`,
  `ZxYGh`, `haIEI`, `AzKYQ`, `hep0l`, `OlLGD`, `zxIMy`) of the minors ORC-61 left open:
  - Step headers: the state reads *Needs input* (message-circle) while the agent asks, *Waiting for approval* (hand)
    while a plan or tool request waits, *Approved* once the person approved its plan, with *· approved by you* after
    the meta; a folded finished step reads *Codex / Sol · 12:01–12:02* (no invocation count unless it had more than
    one), an open one *· invocation N ·*, as the frames draw; one minute reads as one time; a tool step reads its
    recorded command and exit code (*pnpm test · exit 0 · 12:22*, no exit code on a failed one, whose command row
    shows it) and *no tokens*.
  - Plan card: one text a plan line, 13/1.6, 6 apart (27 a line); *Approves plan v1 and starts Builder on Implement.
    To change it, describe what it needs below.* beside *Approve plan & continue*, from the pipeline's next step; a
    plan step's result card no longer repeats the plan; the waiting label's glyph is the hand.
  - Question card: *Implement · 12:09* in its meta. The composer's *Answering …* / *Changes to plan …* line (with
    corner-down-right / pencil) and *Waiting for your decision* stand above the box; the field takes the focus once
    per question that waits (by its message), when the tab opens on it or when it arrives, but never while the tab's
    search field, its only other field, holds the focus (code review).
  - Composer placeholders per state: *Message Builder…* (working, or queued for a named agent), *Message an agent of
    this run…* (tool step, paused, deciding), *Describe the changes the plan needs…*, *Ask a question or request a
    follow-up…*, *Write to the first agent of the run…*; not drawn: *Answer Builder…*, *Tell Builder how to continue
    the run…*.
  - The working line is the timeline's last row (indented to a message's text, no hairline), so it scrolls with the
    timeline; with nothing else to show it stays under the empty state.
  - A denied command is a titled problem row: *Command denied* (or *Outside write denied* by its category) with
    *permission_denied* / *denied by you* and the time, and *… did not run: …* naming the command, with no command
    box or Copy.
  - Result cards: *Review passed* for a step that judges the work (one with a failure route), *… done* otherwise,
    *… failed*; *Builder reported passed / failed* from the step's structured result; circle-check when it passed.
  - At 960 Attach follows the recipient 10 apart (the select fills its wrapper); the queued run's step names wrap
    inside the tab, each with the chevron before it.
- **Scrollbar drag:** lurq (since 0.33.1) runs a container's `on_scroll` handlers for a scrollbar drag, which the
  timeline already takes as the person scrolling; a headless probe of a `VirtualizedList` with the gutter scrollbar
  confirms it. A headless drag of the Conversation timeline's thumb could not be made to hit the scrollbar, so no
  end-to-end test covers it: check in the running app that dragging the thumb stops following.
- `conversation/mod.rs` was split first (following and *Jump to latest* moved to `conversation/follow.rs`), the
  composer's context line moved to `composer_text.rs`, and `tests/timeline.rs` became `tests/timeline/` (tool events
  in `tool_events.rs`).
- Not built (no data or out of scope): *started 12:04 · waiting since 12:09* and *1 message from you* in step headers
  (data exists, not asked); the question card's answer hint; *Show the whole plan* chevron; the tool step result's
  *exit 1 · attempt 1*; the Details pane wording (ORC-44's panel).
- Tests: `cargo test -p orchester-desktop --test timeline timeline_`, `--test conversation conversation`, `--test i18n
  locale` and `every_key`, `--test form_keyboard` with `conversation` (incl. the new `conversation_queued`,
  `conversation_a_step_that_judges_the_work_…` and `conversation_a_question_that_arrives_while_searching_…`),
  `composer_`, `activity_`, `task_graph`, `run_permissions`, `tool_approval`, `tool_run`, `native_activity`.

U11 · Pull requests: no "synced" without a link, columns at 960 · branch `claude/pr-fixes` (done 2026-10-04, design
verified)

A sync or refresh of a project whose repository is not linked asks GitHub nothing, so it leaves no status bar event
(the list's sync and the pull request screen's refresh both dropped their operation as "synced"); the screen's 08.D
panel says why. No frame draws the list in a narrow window, so it follows the Tasks list (02.03E): below
`theme::PULL_REQUEST_OPEN_IN_ROW_MIN_WINDOW` (1076, derived from the column tokens so a title keeps 200) the rows and
headings drop Open in GitHub (the pull request's own topbar has it); a title keeps 228 at 960 (84 before, cut to
*Add GitH…*). Each heading sits at its own width in a cell as wide as its column, so a heading that does not fit is
measurable. The search field gives way by the few pixels the controls row is too wide at 960, so the sort box keeps the
list's right edge. Tests: `cargo test -p orchester-desktop --test form_keyboard pull_request` (the unavailable test
waits until the activity log holds exactly one operation that ended without an event, then checks the status bar;
`pull_requests_layout_columns_fit_the_narrowest_and_the_design_window` checks at 960 and 1440 that headings do not run
into each other and start their columns, that the sort box ends at the list's edge, that a title keeps 200, and where
Open in GitHub shows), `--test theme pull_request`, `--test i18n locale` / `every_key`.

U10 · Pull requests screen jumps while it loads · branch `claude/u10` (done 2026-10-04, design verification pending)

Until the stored inbox is read the screen shows its topbar and heading only; the tab row (linked repositories only,
08.D has none) and the list or unavailable panel then appear together in their final place, so nothing shows that
later moves or vanishes. The Open tab keeps room for a count of up to `SYNC_LIMIT` (two Geist Mono digits,
`Tab::count_room`, `theme::COUNT_DIGIT_WIDTH`), so a first count or one that grows to 50 moves no tab; a single-digit
count leaves the Open tab 6px wider than in `DF8II`. The topbar's spacer already kept Refresh in place while the sync
status changed (measured at 1440 and at the 960 minimum width). Tests: `cargo test -p orchester-desktop --test
form_keyboard pull_request` (the new `pull_requests_layout` tests hold the sync's fake `gh` behind a gate and record the
heading, tab, list and Refresh bounds before and after the stored inbox and the sync arrive), `cargo test -p
orchester-desktop --test theme pull_request`. **Done when** an independent design verification against `DF8II`
(08.01 `wGtxT`) and 08.D `dmib5` reports no blocking or major mismatches.

S1a · Server and accounts · branch `codex/server-s1a` (done 2026-10-03)

Implemented for review on `codex/server-s1a` (2026-10-03), in `.tmp/worktrees/codex-server-s1a`.
The workspace gate passed; the 43 filtered server tests passed on Windows and Linux (Debian WSL), including TLS,
host commands, idle expiry, rate limits and credential redaction. The macOS check was attempted but cannot complete
without Apple SDK headers for bundled SQLite and ring. The Docker release image and Compose stack built and started;
Caddy HTTPS with a local CA, host bootstrap/reset and the container session flow passed. The disposable test stack
was removed. Independent review's M1 and m1–m8 findings were fixed, with regression tests and an online backup CLI;
the documented Docker backup/copy procedure passed too. The second review's concurrent password-change major and
four minors are fixed with atomic pre-hash reservations, per-peer connection caps, a two-second first-byte deadline,
proxy-trust guidance and a single-step backup. The release image and container smoke checks passed again.
The third review's proxy-cap regression and minors are fixed: trusted proxies bypass peer caps while retaining
global caps, direct TLS admits 32 handshakes per untrusted peer, CLI backup opens read-only without migrations,
and hosting docs describe lock/retry and username-lockout behaviour. With 64 slow HTTPS bodies through Caddy,
the 65th request succeeds within one second; the rebuilt release image and account/backup smoke checks passed.
**Done 2026-10-03:** merged into master after three independent security reviews (the third PASS, "ready to
merge"). Known gap: the `aarch64-apple-darwin` check cannot complete on this Windows host without the Apple SDK
(bundled SQLite and ring need its headers), as for the rest of the workspace; run it on a Mac. S1b starts from master. Evidence:
`docs/evidence/server-s1a-2026-10-03.md`.

Done when every S1a box in `docs/architecture/server.md` § *Phasing* holds:

- `orchester-api` (types only) and `orchester-server` build on Windows, macOS (check) and Linux; the Docker image and
  compose example build and start on Linux.
- Empty data directory: `GET /api/v1/server` reports `needs_bootstrap`; `orchester-server user create --admin <name>`
  creates the first admin and prints the initial password only to its own output; `user reset-password` works.
- Sign-in, `password_change_required`, password change (other sessions revoked), session list and revoke, sign-out,
  the 30-day idle expiry with the test clock; argon2id with the document's parameters; tokens `orc_` + 64 hex, stored
  as SHA-256 only.
- Admin user endpoints; `403` for non-admins; the last admin cannot be removed or disabled.
- Rate limits answer `429` with `Retry-After`; an unknown username answers like a wrong password, in about the same
  time; `X-Forwarded-For` counts only from a `trusted_proxies` hop.
- With a PEM certificate configured the server speaks TLS (tested with a test certificate); the log test finds neither
  token nor password; the `audit_events` table records the events the document lists.


L1 · lurq give-way order for the status bar · branch `claude/statusbar-give-way` (2026-10-03)

- lurq 0.35.0 ships `shrink_order` and `shrink_limit` (handoff `H:/projects/lurq/bugreports/lurq-flex-shrink-give-way-order.md`).
  The status bar's left group now gives way in order: what the oldest waiting request asks trims its end down to
  100px, then which run auto-approves and the times drop out whole, then the activity, then the words after the
  approval count (*2 approvals waiting* → *2*), and last the runner's words; the hand, the count, *Review*,
  *Auto-approving* and *Stop* never give way. The workaround is gone: the `ApprovalsWidths` breakpoint table and its
  plan limits classes, the short *N waiting* count, the power-of-two weights and the `yielding` slack. The bar is one
  line, so the right group gives way in the same order (version, Tokens today, count labels, plan limits' details,
  after the left group's optional details) instead of the `STATUS_WIDTHS` breakpoints, and it never shows more at a
  narrower width (sweep test 900–1700).
- Tests: `-p orchester-desktop --test status_bar status_bar`, `--test form_keyboard ended_approval` (930 to 1600 with
  auto-approval, 960 without), `approval_choices_`, a broad `form_keyboard _` pass for the lurq bump, `--test i18n`.

A tool request whose agent is gone no longer waits forever · branch `claude/stale-approval` (2026-10-03)

- Bug seen by two design verifiers and the user: a request whose agent's invocation ended, whose runner restarted
  or whose run stopped stayed *waiting*; answering it only said it no longer waits and its dialog stayed. The runner
  now ends such a request without an answer (existing `interrupted` state; no migration) at the run task's end, when
  a runner attaches (each request read, by id), and when an answer reaches a request no live agent holds; an answer
  to a request answered already is refused as `ToolApprovalAnswered` (IPC protocol **14**, after ORC-39's 13); the
  desktop drops a request the runner refuses that way from the dialog (next request with a notice, or closed) and
  the inline card (*Ended without an answer*), and the status bar says it ended. The step row's *Waiting for you ·
  <tool> · since* now follows the step's pending request whatever the step's state (a tool request never set the
  Waiting state, so real runs never showed it). Built, not drawn (design doc, "Approval choices and waiting
  requests"); code review and design verification passed.
- Tests: `-p orchester-runner --features test-agent --test stale_approvals stale_approvals`; `-p orchester-store
  --test tool_approvals tool`; `-p orchester-desktop --test form_keyboard ended_approval`.

ORC-39 · Task statuses, priorities and tags · branch `claude/orc39` (done 2026-10-03)

Branch `claude/orc39` (worktree `.tmp/worktrees/claude-orc39`): run #11's work merged with master twice, last at
`51fc02f` (ORC-30, ORC-44, retry-feedback with protocol 11 and stuck runs, approval-long, lurq 0.33.1). The Review
findings are fixed, each with a `form_keyboard` test in `organization_review.rs`: R1 a failed save no longer passes for
the last success; R2 the header, tag manager, tag picker, bulk, Run and tag-mode observers are registered once; R3 the
queued Retry form locks the organization fields; R4 an unavailable tag picker creates no tag. Design fixes from the
earlier Verify are in (header pickers as buttons, bulk Add tag in place, blocked notice and readiness copy, `Ncjvt`).

Integration with ORC-30 (decisions to draw in Pencil, not drawn yet):
- **Tabs:** ORC-39's workflow tabs replace Drafts / Queue / Needs review; every tab shows its count.
- **Query:** line 1 is ORC-30's search, pipeline filter and sort (Manual order, Newest first, Title, Status,
  Priority); line 2 is Status, Priority, Tags and Clear filters, with Manage tags at the right.
- **Selection bar** (`OjUxo` + `EpDpS`), 12 horizontal padding, raised fill: from 1348 wide, line 1 (44 high) is
  select-all, "N selected", the readiness (ellipsis), then Run selected, Set pipeline and, for held tasks, Return to
  drafts at the right; line 2 is Set status, Set priority, Add tag, Remove tag at the left, then each task's outcome.
  Narrower, line 1 is the summary, line 2 the run actions right-aligned, line 3 the organization group. ORC-30's
  problem lines stay under the bar.
- **Table:** checkbox, ID, Task / tags, Pipeline, Priority, Workflow / run, Dependency / readiness. The title keeps
  120. Pipeline goes below a 1348 window, the tag chips below 1196, Priority below 1116, where Workflow / run narrows
  from 212 to 164; the title still has 124 at 960.
- **Overview:** the run tabs stay behind Insights; the Needs you panel shows without Insights while anything needs you.

Review follow-ups (review of `7ddd610`): a draft whose blocking task has not finished is a Run selected problem
("KON-3 is blocked by KON-1") and the store refuses to queue it; a group member blocked after queueing is passed over
(the members after it start), the group holds `GroupHold::Blocked { by }`, which ends on its own and keeps the runner
busy only while a blocking task runs; rows keep the blocked text over the hold. The ipc `PROTOCOL_VERSION` is 13 for
`RunAgainBlock::TaskBlocked` and `GroupHold::Blocked` (12 went to the approval choices, merged first); the
organization migration is 39, after steering (37) and approval choices (38). A refused continue reads "Run is unavailable…"; Start, Resume, Continue, Continue with a raised limit and
another attempt are unavailable while the task is blocked, and the Blocked notice names the blocking tasks.

The combined list follows the approved frames 02.03C `DWqjc`, D `IYlGK`, E `t7ETG` and F `lRbit`: the selection bar
is hidden without a selection, padded 12 with 32-high lines 8 apart, Return to drafts, Set pipeline and Run selected
last; select-all shows square-minus for a partial selection; a bulk change reads as one summary line with a line per
failed task; held tasks read "2 tasks held after a task failed · Return to drafts"; rows are 60 high once the tag
chips are dropped. Re-review follow-ups: a run waiting for a decision on a
blocked task shows the Blocked notice naming the blocking tasks, and the header says Blocked for link-only blocking;
an answer or follow-up to a blocked run is kept ("Your message is kept…") and the scheduler's pass delivers it once
the blocking tasks finish; a blocked Pending first run holds no group slot; a group held as Blocked can be returned
to drafts.

Design verification of `329ea66` (`.tmp/verify-orc39/REPORT.md`) failed; its majors are fixed: the Canceled
confirmations follow `N1vwaM` and `hFH4D` (one shared panel: the Work is kept or Active run notice, Keep and Stop at
the right with icons, bulk plurals, the Set status menu closes); held group members read Queued; the footer counts
the tab's rows, or "N tasks match" while filtered; renaming a project tag onto an existing name offers Merge into
it; the Blocked flag sits beside the run label in 60-high rows; the blocking-task search lists results with a check,
Add selected task and the current task apart (`tUwII`); an empty filtered list is the `rVJoA` notice with Clear
filters.
Re-verification follow-ups: bulk Remove tag counts the tasks that had the tag and names the others ("2 selected
tasks did not have Desktop."), with the panel hint; asking to confirm Canceled in Edit task is no failed save (no
status-bar failure, no inline error); the task form shows "Run unavailable" with the unfinished blocking tasks above
its run action (`ZLhiM`); "Canceled" is the one spelling in the app's text; the bulk Add tag picker has one frame;
the blocking search offers no linked task and adds only a task it shows.
Merged with master `0678ce0d` (ORC-61, approval choices): the task organization history moved from the removed
Activity tab to the end of the Summary; the header's Canceled asks the store whether a run must stop, so a task page
that has not read the run's end yet neither asks nor skips the confirmation.
Code review follow-ups (2026-10-03, after `ee7bab6c`; master `fee1ff89` merged): Edit task saves only the
organization fields the person changed (`TaskOrganization::rebase`), so a status set meanwhile is kept; a confirmed
Canceled stops only the named run (`tasks::cancel_run`), in the header, Edit task and bulk; a Canceled task's other
fields stay editable; bulk outcomes count real changes and name failed tasks; the store's per-task
`update_task_organization` replaces the batch shape; history icons follow the change kind; `TaskSort::Updated` is gone.
`DependencyGate::Review` stays: the scheduler tests exercise it as a rule, though the app only creates Merge links.
User decisions (2026-10-03, recorded in the design doc): history stays at the end of Summary; a Canceled task is
skipped by its group (`TaskGroup::skipping`, `GroupHold::Canceled`); migration 39 backfills statuses from runs.

**Done 2026-10-03:** merged into master after three independent code reviews (PASS) and app/Pencil design
verification of the ORC-39 states, the combined list at 1440 and 960, the Overview (attention column beside the list
from 1340, under it below) and the Summary history, with no blocking or major mismatches left. Protocol 13, migration 39.
Open minors (follow-ups): D1DC6T icon tiles and meta context; the Overview attention column comes after the list below
1340; the header confirmation can open before the page read the run (no run number shown); app tests for the header/Edit
"started since" paths.


Approval choices, waiting requests and auto-approval · branch `claude/approval-choices` (done 2026-10-03)

- **Done 2026-10-03: merged after the security reviews, code reviews and design verification passed.** Built
  2026-10-02 (user decisions of that day). The security review made the command classifier fail closed (per-program
  option allow-lists; unknown programs, options, variables set before a command and unread quoting are Unknown;
  Approve similar never widens and never covers destructive or credential-reading parts; Always allow offers only
  prefixes that stay alike) and, by the user's decision of 2026-10-03, auto-approval asks for network commands. The
  design rounds added the dialog's subject line (category or server chip, the asking profile's real setting, waiting
  since), the classifier's reason, the waiting step row, and the Conversation card's four answers through the
  dialog's own path. Four answers for commands and MCP tools alike, and auto-approval:
  `docs/interactive-tool-approvals.md`, "Answers and waiting requests"; design doc, "Approval choices and waiting
  requests" (with the auto-approval and network decisions and the states built without frames).
- **Accepted deviation (user, 2026-10-03):** the command's Always allow step leaves out `NV6xJ`'s *A denied Network
  still wins* card until a category can be set to Deny.
- Core: `permission::category` (commands split into simple commands, bash and PowerShell quoting, shells followed;
  categories by program, subcommand, options and written paths; credential reads and installs noted),
  `run::answer` (`ApprovalAnswer`, `DecidedBy`, run rules and their matching, Always allow targets, `auto_verdict`,
  `denial_message`), `AgentPermissions::auto_approve` (profile version). Store migration `approval_choices`
  (`tool_approvals.decided_by`/`rule_id`, `approval_rules`, `runs.auto_approve`; rules survive a runner restart),
  `publish_allowed_command` (migration 38, after ORC-61's steering). IPC protocol **12** (`DecideToolApproval { answer }`; master is at 11).
  Runner answers itself what a rule, Always allow or auto-approval covers, and a
  person's Deny tells the agent and lets the turn go on (Claude Code: deny with the message; Codex: `decline`).
- Desktop: the 720 dialog with *1 of N* and ‹ ›, the wider answers as tiles plus *Approve everything in this run*,
  the scope of Approve similar on hover or focus, the Always allow step, Decide later (Esc), Deny's text; Needs you
  counts waiting requests; status bar approvals with Review and *Auto-approving* with Stop; the Summary's Review
  request card, the run's Approvals section (switch, counts, rules with Revoke, always allowed), *Auto-approving*
  under the task heading; the profile's *Approve automatically (don't ask me)*; activity marks.
- Not built: the per-category Allow / Ask me / Deny policy of "Command categories and approval policy" (so no
  category can be Deny, and the Always allow step leaves out *A denied Network still wins*); Run temp (temporary
  files count as outside the worktree); Undo of a revoked rule; answering requests already waiting when auto-approval is switched on (the dialog
  says such a request came before it was on). A Codex rule prompt that offers
  only `cancel` interrupts the turn on Deny, as Codex does.
- Tests: `cargo test -p orchester-core --test command_categories command_categories`, `--test approval_answers
  approval_answers`, `--test claude_approvals claude_approvals`; `-p orchester-store --test approval_rules
  approval_rules`; `-p orchester-runner --features test-agent --test approval_choices approval_choices` (and the
  updated `claude_approvals`, `claude_unlisted`, `codex_escalations`, `command_approvals`); `-p orchester-desktop
  --test form_keyboard approval_choices_` and `tool_approval`; `-p orchester-core --test command_fail_closed command_fail_closed`.

ORC-61 · Conversation merged with Activity, Send now and following live (absorbs ORC-42 and ORC-36) · branch
`claude/orc61` (from run #21's `orchester/orc-61-run-1`)

- **Implemented 2026-10-02; merged with master and Review fixed 2026-10-03; merged into master 2026-10-03** after two
  independent code reviews (PASS) and the independent design verification against `fTfl8` (with `q4fCXK`, `a5hF4`,
  `gLVQQ`, `zyusO`, `iPHGE`) with no blocking or major mismatches left. Open minors (follow-ups): Attach sits apart
  from the recipient at 960; step header wording (*Approved*, *Waiting for approval*, invocation count); plan card line
  spacing, helper text and the plan repeated in *Plan done*; question card meta and focus; composer placeholder per
  state; the working line as the last timeline item; titled problem rows; Details pane wording (master's invocation
  panel); the timeline column 6 px left; a scrollbar drag does not count as the person scrolling, so a newly waiting
  card can still pull the view; queued step names can overflow at 960; two profiles with the same name. The task
  page's Conversation tab is one timeline per run (`tasks/timeline/`: step headers, agent and person messages, tool
  groups, question, plan and tool-approval cards, problem rows, returns and restarts, result cards, the run's end),
  with the step filter, *Show* (All, Messages, Tools), search, the Details pane (closed by default, remembered),
  following live with *Jump to latest · N new*, and the composer in every state (steer, queue, answer, plan changes,
  follow-up, continue with instructions, waiting for a decision, not started, closed). The Activity tab is gone.
  *Send now* reaches the agent at work after its running tool call: Codex through `turn/steer`, Claude Code through
  stream-json input (Claude Code now always runs with `--input-format stream-json --replay-user-messages`); messages
  are queued, delivered mid-turn, delivered at the end of the turn, or not delivered and kept
  (`docs/architecture/storage.md`, "Send now"). Live checks on both CLIs: `docs/evidence/orc-61-send-now-2026-10-02.md`.
- **Merge with master:** master's Activity and old Conversation behaviour now lives in the timeline: the marks of
  02.B7 as rows of their own (`tasks/timeline/mark.rs`, drawn by `activity/mark.rs`), the waiting run's toolbar state
  and composer line with *Open the decision →*, the toolbar's second line when narrow, ORC-44's permission row in
  Details (*Running with* … *change ↗*, *Ran with*), ORC-45 and approval-long through the shared `ui::command_row`
  and `ui::value_box`. The steering migration is now 37 (after `stuck_runs`, 36); ORC-61 changes no IPC message, so
  `PROTOCOL_VERSION` stays 11.
- **Review fixes:** messages and cards are merged into a step's transcript before grouping, so anything that is not a
  tool event splits a group (also in *Tools*); a declined MCP call, a memory one too, is a problem row; *Messages*
  counts only the matches it shows; a queued message for an agent the pipeline no longer offers is not named with an
  empty name; a Codex message accepted by `turn/steer` but not taken before `turn/completed` is delivered at the turn's
  end, never sent again (Codex records it into the thread as the turn finishes). Second review (2026-10-03): a message
  on its way to one agent goes into no other agent's prompt until that agent ends; an acceptance Codex sends after
  `turn/completed` still delivers at the turn's end; *Send after this turn* and the runner's late delivery change a
  message only while it stands as read (`ConversationStore::change_delivery`), so a race never fails the step;
  master merged again for the Changes tab count. Design verification fixes (2026-10-03): the toolbar's state follows the run's state; the
  step the run ended on stays open; at 960 with or without Details the toolbar and composer keep every control; a tool
  step's failing command shows open; the queued empty state and unavailable toolbar of `zxIMy`; read-back links, result
  card layout, approval card title, default recipient and the Messages meta.
- Tests: `cargo test -p orchester-runner --features test-agent --test send_now send_`, `--test conversation
  conversation`; `cargo test -p orchester-desktop --test timeline timeline_`, `--test conversation conversation`,
  `--test form_keyboard` with `conversation`, `composer_`, `activity_`, `multiline_command`, `tool_approval`,
  `run_permissions`, `task_graph`, `changes`, `integration`; `--test changes c`; `-p orchester-core --test app_server
  turn`; `-p orchester-runner --features test-agent --test send_now_delivery send_now_`; `-p orchester-store --test
  conversation conversation`.
- **Differs from the design:** no *Memory* tab yet (its screens come with Memory 3/3), so result cards link Changes and
  Summary only; a failed step starts open; matched words are not highlighted. An approval card sits after the tool
  call that asked for it when the CLI reported that call first (they are matched by time, not by call).

Tool approval dialog: long values shown in full · branch `claude/approval-long`

- **Done 2026-10-02.** Independent review and design verification passed. A value that does not fit beside its label (Action, Command or
  Target/Working directory when longer than 60 mono columns or multiline, and MCP Arguments always) moves under its
  label into the transcript command box (`ui::value_box`: lines as recorded, no wrapping, the full width, scrolling
  both ways in a box no taller than `theme::APPROVAL_VALUE_MAX_HEIGHT`, 232 = 12 lines (`E5NBo` in `bjrgu`), shared
  with 04.H9's shell string; each thumb in a 6px lane of its own, never over a glyph; a quiet Copy at the right of
  the label's line, 12 above the box, takes the value as recorded). A CLI command comes after its working directory,
  labelled *Command · N lines, as recorded* (`Grcu7`); a CLI's request names no shell, so no Shell row. Arguments that parse as JSON read one member a line (2-space indent, only
  whitespace between tokens changes); otherwise as recorded. The dialog is no taller than the screen: its details
  scroll between the header and the buttons, so Approve and Deny stay in view at 960 x 700. Redaction is unchanged
  (the runner records redacted values; the dialog only lays them out). This also fixes C6's cut working directory row.
- Design: long values are drawn in `bjrgu` ("Approval choices", approved); the cap follows its 12 lines.
- lurq bug worked around: a component that renders its own `Modal` and re-renders after two window resizes lays the
  layer out at the earlier size. The dialog renders only the layer (`ToolApprovalDialog`); everything that changes lives
  in `ToolApprovalPanel`. The fix goes upstream as lurq 0.33.2; other dialogs that render their own `Modal` and
  re-render while open are exposed until then.
- Arguments are laid out once per approval (`ArgumentsCache`, keyed by approval id and stored value), checked with
  `serde::de::IgnoredAny`, and laid out only while the result stays within 4 x their size (at least 64 KB, at most
  512 KB); past that they are shown as recorded with a note (`runs.toolApproval.argumentsAsRecorded`), since the
  agent writes them and deep nesting multiplies the indentation. A number holding a secret's digits is redacted too.
- MCP arguments are recorded whole (they were cut at 600 characters): Claude Code's permission prompt removes secret
  values from every text in them first, then `run::recorded_arguments` keeps them whole up to `ARGUMENTS_CEILING`
  (256 KB). Past it they are cut at a character boundary and end with a visible `… cut at 256 KB` line; the dialog lays
  out the part kept and ends it with that line (`runs.toolApproval.argumentsCut`). Activity never showed MCP
  arguments (it names a tool by its file path), and the approval row's `scope` column has no size limit, so neither
  changes. Codex App Server has no cap: its MCP approvals record no arguments at all, by design of that parser.
- The footer keeps Deny and Approve once whole: why a decision failed (e.g. the runner is not connected) takes the
  room left of them and wraps there.
- Copy is the ghost button of `feLwP` (22 high, 6 at each side and between icon and label, `muted`, `text` and the
  hover fill under the pointer, `oriyY`); each value box keeps its thumbs in lanes of their own inside its 6px padding,
  the vertical one from the viewport's top (`e0vyng`). With lurq 0.33.1 a press or drag anywhere in a lane scrolls
  and never selects text (checked in the running app on both lanes).
- Left: a command's *Agent's note* still cuts with an ellipsis. The sideways lane is always reserved (lurq reserves
  always or never), where the design adds it only while a line overflows; lurq has one end inset for both bars, so the
  sideways thumb also starts at the viewport's left rather than 12 in.
- Tests: `cargo test -p orchester-desktop --test form_keyboard tool_approval` (Copy: `tool_approval_copy -- --ignored`,
  writes the clipboard and restores it); `-p orchester-core --test claude_approvals claude_approvals`; `--test theme
  approval`.

Cargo-specific build folder handling removed · branch `claude/no-cargo-special`

- **Done 2026-10-02.** Orchester orchestrates agents for any kind of project, so nothing in its behaviour singles
  out Cargo any more. Removed: Project settings › Environment's suggestion to add
  `CARGO_TARGET_DIR=<repo>/.orchester/target` for a repository with a root `Cargo.toml` and its notice that agents
  need permission to write there; the Agents › Permissions notice offering to add that folder to *Extra writable
  folders*; the runner's per-worktree rewrite of an absolute `CARGO_TARGET_DIR` to `<root>/worktrees/<worktree>`;
  and D5's host-side free-space preflight for that folder (`BuildSpaceLow`/`BuildSpaceUnavailable`, the `fs4`
  dependency). Why: every run got its own 13–17 GB build folder outside its worktree that nothing removed; it grew
  past 200 GB. Without the variable Cargo builds into `<worktree>/target`, which is git-ignored and goes with the
  worktree, as any toolchain's output does. The free-space check is not replaced by a generic one: its 10 GiB / 1 GiB
  floors were sized for a cold Cargo target, and no threshold fits every toolchain. Stored data is not migrated: a
  project that still sets `CARGO_TARGET_DIR` passes it to its agents as written, like any variable, and an agent
  that should write to a folder outside its worktree needs that folder among its profile's extra writable folders.
  Pencil still draws the removed states (Environment `CMSyB` in `tXahd`, D5 `WmW1G`, and the free-space error in
  `WOdDe`); a designer removes them. D5's open observation of intermittent verifier-window foreground activation
  stays recorded in `docs/evidence/d5-final-verification-2026-09-29.md`.

ORC-57 · Native agent P1b: core native module

- **Done 2026-10-02.** `orchester_core::native`: conversation items (`item`), the `ModelClient` boundary (`model`),
  the tool catalog with strict schemas and typed inputs (`tools`), file confinement (`scope`: canonicalised roots,
  `.git` at any depth, `.orchester/` in the worktree, index and skills read-only; Windows device names, ADS,
  verbatim, device and drive-relative paths refused; network paths refused before they are resolved unless inside a
  writable folder), command splitting per platform with hardened git and the policy (`command`), the known-safe table
  (`permission::known_safe`), and the I/O-free loop (`session`: §4.10 order, approvals, denials returned to the
  agent, the finish reminder, every stop mode, resume with unanswered calls, compaction at 80 % of the window, at
  most 3 per invocation). Nothing runs it yet (P1c/P1d). Tests: `cargo test -p orchester-core --test native native_`.
- **Differs from the design:** `cargo metadata`/`cargo tree` are not known-safe, because a `.cargo/config.toml` or
  `rust-toolchain.toml` the agent writes in the worktree can replace `rustc`/the toolchain with any program; they
  ask. Read stamps are left to the file tool executor (P1c), which must drop them when a `Compaction` item is stored.
  `RunnerProblem::ContextCompacted` (§6.3) is not emitted yet: it needs Activity text, so it comes with P1d.

ORC-56 · Native agent P1a: native connections stored and shown, nothing run · branch `claude/orc56-fixes`

- **Done 2026-10-02.** `step/command.rs` is split into `step/start/{mod, cli, shared, prompt}.rs` and `step/worker.rs`
  into `step/worker/{mod, observe, process}.rs`, with no behaviour change. `Connection` holds an `AgentEndpoint`
  (program or `ApiEndpoint`: preset, base URL, key secret), and `AgentProvider` has `AnthropicApi`/`OpenAiApi` (stored
  as `anthropic_api`/`openai_api`). A base URL is `https://` (or `http://` for this computer), with a valid host and
  port and no credentials, query, fragment or backslash; MCP server URLs get the same host and backslash checks.
- A native connection is never checked (`connection::check` has no answer for it, and it refuses an agent check)
  and never runs: preflight refuses its steps and the runner refuses to start one. It reads *Not available yet* in
  the connections list (type *Native*, cpu glyph, provider · preset · host) and inspector; a profile on it shows
  one notice (*This profile can’t run yet: its connection is a native agent*), its permissions panel reads *Not
  enforced yet*, network keeps the chosen access, and the MCP choices are locked with the reason. The pipeline step
  and run preflight say the same.
- Follow-ups: P1b–P1e (ORC-57…ORC-60) under *N · Native agent*.

C6 · Commands outside the permissions ask a person ("List + ask") · branch `claude/ask-outside`

- **Done 2026-10-01.** User decision after ORC-48, where Claude Code steps with allowed commands ran in `dontAsk` and
  13 commands (`$VAR`, compound commands, `sed`, `git grep`, `powershell -Command`, `codex exec`) were denied without
  reaching "Needs you". Claude Code steps with allowed commands now run in `default` with the permission prompt: the
  listed and known-safe read-only commands run, any other asks (also in read-only steps, whose approval says an
  approved command can change files). Writing steps with allowed commands always edit by `Edit` rules. Every Codex
  step runs as App Server with `sandbox_approval`: a command that needs more than the sandbox asks; `codex exec` is
  gone. Launch permissions record what happened to other commands. `docs/interactive-tool-approvals.md`.
- Agents ask everywhere: a Claude Code read-only step that asks before commands, and an empty allowed list, ask about
  every command. App Server agents get none of the user's MCP servers, plugins, apps or notify program; the user's
  execpolicy `.rules` still apply (the user's choice, user decision 2026-10-01), which the enforcement panel and
  the run popover say. The repository's own `.codex` folder never loads: the worktree and its repository are marked
  untrusted, a project layer Codex loaded anyway refuses the step, and a branch with `.codex/` gets a note in its
  activity. Live turns: `docs/evidence/ask-outside-e2e-2026-10-01.md`.
- Open (macOS): check whether Codex canonicalises its working folder (`/var` → `/private/var`, symlinks) before it
  looks trust up, against the keys Orchester writes (the worktree and repository paths as the runner names them). If
  it does, a trusted repository's `.codex` would load; until checked on a Mac, the runner's `config/read` layer check
  fails such a step closed ("Codex loaded the repository's own Codex settings").

ORC-48 · Make the code index useful to agents · branch `orchester/orc-48-run-2`

- **Done 2026-10-01, without the re-measurement.** The index writes `symbols.tsv` with the code's own types,
  functions, methods, impls, modules and macros, `symbols-consts.tsv` with constants and statics and
  `symbols-tests.tsv` with every definition in a `tests/`, `test/`, `__tests__/` or `spec/` folder (format 6, so
  older indexes are rebuilt). The prompt's *Codebase index* part names the three files and carries the package and
  module map from `map.md` (at most 8 KiB: module folders lose their file lists first, then the map is cut at a line
  and points to `map.md`). The internal skill was rewritten with column-anchored `rg` commands for PowerShell and bash.
- Open: the skill's regexes were run in bash against the a89fe98 index (same columns), not through `$IDX`, not in
  PowerShell and not against a format-6 index; the measurement index on vs off is T5 (user decision). Test files
  named by convention outside test folders (`*_test.go`, `*.test.ts`) stay in `symbols.tsv`.

C4 · Claude Code runs known-safe read-only commands outside the allowed list · branch `claude/allowed-commands`

- **Done 2026-10-01.** User decision: they stay allowed, as Codex's known-safe read-only commands are. Said
  wherever the list is explained for Claude Code: the *Allowed commands* option (*These commands run, as do known-safe
  read-only commands such as ls, cat or git log. …*), the empty list's hint (*… so none runs, not even a read-only
  one.*), a detail under the enforcement panel's Commands row, a hint under the run permissions popover's commands and
  a note under the pipeline step's commands (*Claude Code also runs known-safe read-only commands, such as ls, cat or
  git log, without listing them.*); also `docs/interactive-tool-approvals.md`, the capability table's notes and the
  design document's permission rows. Other connections (Codex, where the option is unavailable) keep *Only these
  commands run. …*
- Design verification of these texts passed (at `23f988e`). The Pencil design was not changed; where it shows the
  old option text on a Claude Code profile, the design needs the new text.

C5 (rule hole) · Claude Code runs the commands `acceptEdits` allows only in their listed forms · branch
`claude/allowed-commands`

- **Done 2026-10-01.** Both CLI versions check prefix deny rules before allow rules (2.1.89 `W__`, 2.1.284 `hBt`),
  so a `Bash(sed:*)` deny also denies a listed `sed -n`; dropping it let `acceptEdits` run any `sed`. A writing
  step whose allowed commands contain `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` or `sed` now runs in `dontAsk`
  (`default` when it asks before MCP actions) with `Edit(//<folder>/**)` allow rules for the worktree and the extra
  writable folders (`Mechanism::ClaudeEditRules`, enforcement panel *allowed tools · edit rules per folder*); other
  writing steps keep `acceptEdits` with all seven deny rules. `orchester-fake-claude` models the real precedence;
  runner tests: `sed -n` runs, `sed -i` is denied, `rm -f build/x` runs, `rm -rf src` is denied.
- A listed entry whose first word is one of the seven gets a hint under the command entries in the profile, the run
  permissions popover and the step inspector: "*rm -f build/x* also allows any arguments after it, including ones
  that change files." Prefix matching is unchanged.
- No `Edit` rule can name a network share. A step that edits by rule refuses to start when its worktree or an
  extra writable folder is on one (`InvocationError::NoEditRule`, recorded as the runner problem *could not
  start*), and the enforcement panel shows that row as *not enforceable* with the reason.
- Open: the `Edit` allow rules were read in both binaries (same matcher as the code index's deny rule: `//c/x` is
  rooted at `C:\`) but not exercised in a live model turn; a folder whose path holds `[` or `]` is matched by
  Claude Code as a gitignore pattern.

ORC-38 · Codebase index 2/2 (delivery, settings, activity) · branch `claude/codebase-index`

- **Done 2026-09-30.** Project setting *Use code index* (default on). New and resumed Codex and Claude Code sessions
  get the base revision's index: a prompt part, the internal `orchester-codebase-index` skill and, for Claude, the
  folder as `--add-dir` with an `Edit` deny rule; published index files are read-only and checked against their
  digests before reuse, handing out or building on them. ripgrep is found on the agent's `PATH`, where winget installs
  it or bundled with the Codex CLI or Claude Code, copied into `.orchester/tools/rg-<hash>` and put first on the
  agent's `PATH`; without it the index is still given (Codex is told to search with `git grep`) and the activity says
  why (`RipgrepNotFound`, `RipgrepUnavailable`). Settings › Code index lists the kept indexes (runs, size, counts,
  build time, building phase, failure reason) with Rebuild/Retry and installs ripgrep with winget or brew only on
  click. Activity shows the code index per agent launch and the tokens before the first edit (panel block, step
  header meta, step filter).
- Timings (release, Windows, 32 threads): this repository full 0.6 s, incremental (41 files) 0.3 s, reuse 0.1 s;
  60k-file synthetic repository with a committed 1 MB JS bundle full 44 s (peak 811 MB), incremental of one source
  file 20 s and of the bundle 13 s (peak 708 MB), reuse 1.2 s (digest check), 0.2 s once the process found it intact.
- Independent design verification passed after three rounds (Settings › Code index `i220W`, `LC2V0`, `RkMap`,
  `h6yMk`, `NvR7K`; Activity `rd1Vo`, `F3XhQ`; the activity problem texts).
- Open: before/after measurement on a real task (index on vs off) — scheduled after merge and stable reinstall; the
  Claude `Edit` deny rule was not checked in a live Claude Code session; Codex `exec` reports usage per whole turn,
  so its count before the first edit is an upper bound, marked `≤`; the incremental build on very large repositories is dominated by per-file work (hard links,
  digests); a code index on a network share gets no Claude `Edit` deny rule (its files are read-only); on macOS a
  ripgrep install goes on after Orchester closes; macOS is only cross-checked for core and exec (C build scripts
  need a target `cc`).

ORC-37 · Codebase index 1/2 (backend) · branch `claude/codebase-index`

- **Done 2026-09-30.** New crate `orchester-index` builds a per-revision index in `<repo>/.orchester/index/<revision>/`
  from git objects with tree-sitter (Rust, TypeScript/JavaScript, Python, Go): `tree.txt`, `symbols.tsv`,
  `outline/<path>.md`, `docs.md`, `map.md` (packages from Cargo/npm/Python/Go manifests) and `manifest.json`. Full
  first, then incremental from the closest existing index via `git diff-tree` (equal to a full build); atomic rename
  under a cross-process lock. The runner builds or reuses it before a run's first step; failure is a
  `CodebaseIndexUnavailable` runner problem in each agent's activity and the run continues. Indexes go when the last
  worktree using them is removed (desktop *Remove worktree*, merge, Run again).
- Open: delivery, settings and activity are ORC-38 (its design verification, which covered this crate's activity
  problem text, passed); macOS cross-check stops at C build scripts (no target `cc`).

Agent runtime architecture · effective approvals, Windows sandbox and Cargo isolation · root `master`

- **Done 2026-09-30 as a Windows release candidate, uncommitted.** Effective profile, pipeline and run
  permissions choose the interactive Codex transport. Commands and MCP actions can receive one-action
  decisions, and a waiting exec session can resume through App Server with its worktree and progress preserved.
  Windows read-only invocations select the native sandbox; absolute Cargo targets are scoped per worktree
  for agents and command tools. The expanded run popup scrolls inside the workspace.
- Named-filter regressions, native signed-in gpt-6.1-sol MCP approve/deny runs, `tools/check.ps1`, and the
  paired release build passed. Independent Pencil/app comparison found no blocking or major mismatch in
  the changed profile, pipeline, popup and approval-dialog regions. Command approval and transport migration
  used protocol fixtures, followed by a native command approve/deny probe and a resumed-session local/extra
  folder write probe. Windows cwd/root validation accepts normalized path separators; Apply now waits for
  active MCP actions and pauses uncertain permission restarts. Six affected runner regressions and the final
  quality gate pass. Core/exec macOS checks passed; runner/desktop cross-check lacks a target C compiler,
  and no physical Mac test was run. The authorized stable install was updated and blocked tasks resumed;
  ORC-44's fresh independent Review/Verify and final task-specific verification remain in progress.
  See [evidence](evidence/agent-runtime-architecture-2026-09-30.md).

ORC-45 · Multiline command rendering · branch `claude/orc45`

- **Done 2026-10-02.** Independent review passed; independent design verification passed at 1440 and 960. Command
  headers show a one-line preview: a line break and the indentation around it read as one space. Every agent/tool
  command can expand; its details keep the original command in a box with Copy (exact bytes). In that box the
  command never wraps: each line stays as recorded and long lines scroll sideways in the box's own viewport, while
  Copy keeps its column at the top right. Dialog and page command rows are unchanged. Ids are per entry
  (`activity-command-detail-{sequence}`, `activity-command-copy-{sequence}`).
- Design: the command-details states were added to `data/orchester.pen` by the designer (`iBzOj` 02.I2, `BQgqg`
  02.I3, `tWO8E` 02.I4, hidden `uA3Sh` "Entry command" in `N4SlQq`), saved on master as b65abef. Known
  difference: the design adds a 6px lane for the horizontal thumb only while a line overflows; lurq reserves a lane
  always or never, so the thumb overlays the box's bottom padding and the box keeps its height.
- Minor, left: the design's header preview keeps indentation spacing while the app collapses it to one space; the
  box is 3px shorter at 1440.
- Filtered regressions: `cargo test -p orchester-desktop --test form_keyboard multiline_command`,
  `--test command_presentation multiline_command`, `--test transcript transcript`, `--test form_keyboard command`,
  `--test i18n locale`, `--test theme mono_text`, `--test theme transcript_command`. Opt-in, because they write the
  OS clipboard and need restorable clipboard text: `--test command_presentation multiline_command_copy_action --
  --ignored` and `--test form_keyboard multiline_command_copy_pressed -- --ignored`.

Continue failed runs through task conversation · branch `codex/continue-failed-run`

- **Done 2026-09-30.** **Send & continue** resumes the same failed run in a new attempt, preserving its branch,
  saved/uncommitted work, available provider session and failure history. Dependent and unfinished steps run again.
  Durable failure markers prevent old queued messages and duplicate notifications from triggering retry loops.
- Independent verification: 46 filtered tests, final `tools/check.ps1`, live UI/Pencil comparison and removed-worktree
  closed state passed. Release binaries built. macOS core cross-check passed; store/runner cross-check requires a
  target C compiler and physical Mac verification remains unavailable.
- Main Pencil frame `vUPqr` is live; its manual Save remains pending. Verified design/app exports and the independent
  report are committed in [evidence](evidence/failed-run-conversation-recovery-2026-09-30.md). Stable installation is
  unchanged; the candidate does not silently resume ORC-44 or mutate its database.


ORC-44 follow-up · Outcome instructions distinguish user decisions from failures · branch `codex/attention-outcomes`

- **Done 2026-09-29.** New and resumed turns receive the same explicit JSON contract: needed permission, access and
  scope decisions use `question` with concrete options; genuine failures still use pipeline failure handling.
  Schema descriptions reinforce the distinction without changing fields. Independent prompt 13/13, conversation
  14/14, plans 10/10 and four filtered runner checks passed; final `tools/check.ps1` passed. No UI changes.
- This improves instructions; fixture tests do not guarantee a live model's choice. It does not reopen the stored
  ORC-44 failed run, grant permissions, complete ORC-44 command approvals, or update the stable install. Its saved
  implementation remains at checkpoint `9b3484f1f85cdcefcdcd237984a9c0bcbbd07453`.
  See `docs/evidence/orc44-attention-outcomes-2026-09-29.md`.

ORC-41 · Interactive tool approvals in Codex runs · branch `codex/current-integration`
(an opted-in pipeline step uses Codex App Server and the existing account/global MCP servers;
Approve once resumes the same session, Deny fails the step without writing, decisions persist
and waiting time is excluded from the active limit. Other steps keep their previous approval
policy. Independent real runs passed with both a disposable marker tool and Pencil `batch_design`.
At the user's direction, the Pencil smoke used the already-open main design: its one temporary
frame was verified by returned ID and removed, restoring the original node set. Filtered parser,
MCP readiness, invocation, time-accounting, store and i18n tests and `tools/check.ps1` passed.
The modal and step-switch ON/OFF states passed independent Pencil/app comparison after fixing
duplicate session-start activity and a duplicated switch label. The user saved the main design.
macOS core cross-check passed; the runner cross-build needs the unavailable macOS native toolchain,
and no Mac runtime test was run. Evidence: `docs/evidence/orc41-interactive-approvals-2026-09-29.md`).

Edit a task after its first run starts · branch `codex/edit-started-task` (the task form saves
future-run details in place while existing runs retain their snapshots; queued tasks remain locked
and started tasks cannot be saved incomplete. Independent filtered core, store, runner and desktop
tests, `tools/check.ps1`, and same-state Pencil/app verification passed. The design was saved.
Evidence: `docs/evidence/edit-started-task-2026-09-29.md`).

Overview · Failed task tab · branch `codex/overview-failed` (the tab has a count, newest-first
failed rows, its own route and an accurate empty state; independent filtered tests,
`tools/check.ps1`, and same-size Pencil/app verification passed. The design was saved.
Evidence: `docs/evidence/overview-failed-2026-09-29.md`).

D5 · Check free disk space outside the agent CLI · branch `codex/d5-final-port` (absolute shared `CARGO_TARGET_DIR`
checked by the host before run/worktree creation; truthful, locked queued task can be reopened and retried under the
same first-run key and pinned pipeline version. Post-rebase independent 17 named-filter Windows test commands and
`tools/check.ps1` passed; four exact-route, same-size Pencil/app comparisons found no blocking or major mismatch in
D5 regions. The user accepted the missing ORC-15 Skills sidebar item in the older D5 design frames as an exception.
macOS runner verification was unavailable and explicitly deferred by the user; intermittent verifier-window foreground
activation is recorded without an attributed D5 cause. Evidence: `docs/evidence/d5-final-verification-2026-09-29.md`).

ORC-16 · Run again · branch `claude/orc16` (a failed or cancelled task starts its next run from the first step in a
new worktree and branch, with the latest published pipeline version and the task's limits; the rule is core's
`run_again`, the runner's `RunAgain` command is idempotent by key and can first remove the run before's worktrees
with the *Remove worktree* rules; the task page offers *Run again* beside *Remove worktree*, the confirmation
`R5y5x`, and while blocked the action stays unavailable with the reason under the notice. Core `run`, store `runs`,
all runner binaries, `form_keyboard ::`, `tasks`, `i18n`, `theme` and `tools/check.ps1` passed. An independent
exact-PID comparison with `zM693`, `M1upHe`, `R5y5x` and `iE8eQ` reached every state and found no blocking or major
mismatch; the failed/cancelled Summary body predates ORC-16 and still differs from `zM693`).

ORC-14 · Show and edit a saved task's brief and acceptance criteria · branch `claude/orc14` (a task's Summary
shows its brief and numbered criteria, a draft's *This task* panel its pipeline and limits, and *Edit* reopens a draft
in the New task form under `tasks/<id>/edit`; *Save changes* updates the same task in place
(`TaskStore::update_draft`), *Create & run* starts it, a task with a run cannot be edited, and leaving the form with
unsaved changes asks *Discard changes?*. Latest activity now scrolls with the Summary after the brief. Independent
same-size Pencil/exact-PID app comparison of `tqn5P`, `h3IIs`, `E1o77O`, `ctHXE`, `kDmo9` and `uv4PY` found no blocking
or major mismatch; `dKiEj` could not be reached at design size with the fixture. Filtered core, store, `form_keyboard`,
tasks, navigation, tabs, i18n and theme tests and `tools/check.ps1` passed).

U6 · Verify conversation states and direct recipient timing · branch `codex/u6-final-port` (an independent agent
compared exact-PID Interrupted `sYuiX`, Not started `Ok14G`, real Live recipient menu `cj2a2`, and Paused/Stopped
component captures at matching sizes; none had blocking or major visual mismatches. A later-agent message initially
promised delivery before Builder's next action, which the runner cannot guarantee. The corrected Paused and real
Live Builder→Reviewer states now say it waits for Reviewer's next turn; independent recheck passed conversation
8/8, i18n 5/5 and `tools/check.ps1`, with no own app focus in 1,343 host samples. Pencil has no separate saved
later-recipient message frame, so its exact copy lacks a literal same-state design comparison; the verified line
fits the saved panel geometry. Evidence: `docs/evidence/u6-conversation-states-2026-09-29.md`.)
M1 · Confirm MCP behaviour in signed-in CLI turns · branch `codex/m1-claude-allow-rule` (Codex 0.139 under `-a never`
called a probe MCP tool, kept a test variable out of agent shell commands, and reported an unnamed startup warning
for a broken server while the turn completed. Two repeat unannotated calls succeeded after one unexplained
cancellation. After sign-in, Claude Code 2.1.89 called `mcp__m1probe__ping` with `--allowedTools mcp__m1probe` and
no other tools; the turn exited 0 with the inert result and zero permission denials. Observed CLI fixtures and parser
tests were updated. The independent Claude fixture test passed 1/1 and `tools/check.ps1` reported OK. A full
Orchester pipeline run was not part of this probe. Evidence: `docs/evidence/mcp-real-run-2026-09-29.md` and
`.tmp/verify/m1-claude-allow-rule/report.md`).
U8 · Align agent profile header lifecycle actions with Pencil · branch `codex/u8-final-port` (Save draft appears only
for unsaved edits, while Publish follows the saved-draft state. An independent verifier matched all five saved
1440 × 960 header frames `dWnw7`, `t2wCI`, `Jw98x`, `WPmtS` and `T1rjv` to source-matched exact-PID app captures at
2160 × 1440 physical px, with no blocking or major header mismatch. Named agent form tests passed 4/4,
`tools/check.ps1` passed, and the host watcher recorded 931 samples without app focus. Fixture-specific profile
bodies differ from Pencil and were not claimed as full-screen parity. Evidence:
`docs/evidence/u8-header-states-2026-09-29.md` and `.tmp/verify/u8-final-port/report.md`).
U2 · Guard merge and worktree removal while integration runs · branch `codex/u2-merged-fix` (project/run-scoped busy
state keeps conflicting controls unavailable, the Merging button retains its width, and a merged run shows its
historical branch with a fully visible `· deleted` suffix. Independent real-Git integration tests passed 11/11;
the corrected candidate passed the focused real-Git test, i18n 5/5 and `tools/check.ps1`. Exact-PID app comparisons
to Pencil `jnagB`, `cE50X` and `bMJUA` found no blocking or major mismatch; a host watcher sampled the final app
381 times without it taking focus. Pencil has a button specimen but no full in-flight merging page. Evidence:
`docs/evidence/u2-merge-worktree-controls-2026-09-29.md`).
M5 · Show who reaches a remote MCP server and wrap its Authentication value · branch `codex/m5-final-port`
(remote inspector adds *Reached by*, aligns property rows to the top, and lets only the long Authentication row
grow. Independent same-state 2160 × 1440 Pencil `a4lm6D` / exact-PID app comparison found no blocking or major
mismatch; the value was fully visible on two lines and sections below aligned within 2–3 physical px. Named MCP,
typography and i18n tests and `tools/check.ps1` passed; the host no-focus watcher passed. Evidence:
`docs/evidence/m5-remote-inspector-2026-09-29.md`).
U3 · Fit the Claude mechanism line and enforcement panel to Pencil · branch `codex/u3-final-port`
(shorter English copy and two theme typography roles align the one-line mechanism, row spacing and wrapping in the
300-wide panel. Independent exact-PID same-state comparison to Pencil `CIeqc` found no blocking or major mismatch;
the full app panel is 300 × 492.7 logical px versus 300 × 495 in Pencil. The 2.3px residual and pre-existing heading
copy are minor. Independent theme/i18n tests and `tools/check.ps1` passed; a separate bounded host watcher confirmed
the own app stayed out of focus. Evidence: `docs/evidence/u3-enforcement-panel-2026-09-29.md`).
M3 · List MCP servers under Secrets “Used by” · branch `codex/m3-final-port` (independent MCP-only and mixed-use
checks passed. Pencil `wPXgW`/`R2ImH`/`RxpL1` and exact-PID app captures had no blocking or major mismatch after the
archive geometry fix. A bounded Add Secret → Staging → masked value → Archive flow passed a 944-sample host no-focus
watcher; earlier foreground episodes during taskbar switching remain documented. Seven named Secrets/i18n tests and
`tools/check.ps1` passed on the current-master port. Evidence: `docs/evidence/m3-secret-mcp-users-2026-09-29.md`).
D4 · Verify a UI change inside an Orchester run · branch `codex/d4-current-port` (DOS-12 passed
Implement checks, Build+Pin, independent read-only Review and Verify. The verifier compared saved New Task design
`m82tU` with the exact-PID app at 2160 × 1440 and found no blocking or major mismatch; the candidate's host-side
focus watcher passed at sampled instants. The host Orchester watcher recorded a separate unexplained foreground
episode. Implement rechecked a pre-authored UI candidate, so this does not establish self-hosted UI authoring.
Filtered New Task/saved-draft/pipeline tests and `tools/check.ps1` passed on the integrated port. Evidence:
`docs/evidence/d4-run12-2026-09-29.md`).
U5 · Align the remaining task UI states with Pencil · branch `codex/u5-current-port` (independent source-matched
verification passed 10 named tests and `tools/check.ps1`. Exact-PID app checks covered parallel waiting rows,
pipeline and environment details, and the transition from a queued message to *Not delivered* after Remove worktree.
The reached changed regions match saved Pencil frames `PacyY`/`Y8hyj` without blocking or major mismatches; a host
watcher recorded 2,461 samples without the test app taking focus. The live queued transition used a typed store seed;
the runner refusal was verified by a focused test, not a live Send. Illustrative design row counts differ from the
disposable fixture. Evidence: `.tmp/verify/u5-current-port-verify/report.md` and `.tmp/verify/u5/preflight.md`).
D7 · Keep embedded MCP bearer tokens out of agent transcripts · branch `codex/d7-safe-mcp-client` (the exact-PID
client reads discovery only in memory, rejects redirects and non-loopback URLs, and prints ASCII-safe redacted tool
results. Independent synthetic tests 9/9, `tools/check.ps1`, `git diff --check`, and a real Implement → Review run
passed. The run used D6 to pin its own app pair, called `lurq_windows` and `lurq_read_tree` by spawned PID without
printing the credential, then closed the app; a 200 ms host watcher recorded an unchanged foreground PID. Evidence:
`docs/evidence/d7-safe-mcp-client-2026-09-29.md`; usage: `docs/mcp-verification-client.md`).
D6 · Pin the verification app built by a run · branch `codex/d6-pin-verifier-app` (`orchester-exec` CLI copies the
desktop and runner to one run-owned directory and verifies source/copy SHA-256; independent synthetic tests 3/3,
`tools/check.ps1`, `git diff --check`, and macOS cross-check passed. Replacing the shared-target source after pinning
left both copies unchanged. The helper cannot prove which build produced source bytes before it opened them. Usage:
`docs/verification/pin-verifier-app.md`; review: `.tmp/verify/d6/report.md`).
D3 · First real Orchester runs on an isolated clone · branches `codex/d3-first-run` and `codex/d3-mcp-inside-run`
(Codex sandbox passed core check, two filtered runner process/pipe tests, desktop+runner MCP build and
`tools/check.ps1`; an in-run no-focus desktop PID answered exact-PID `lurq_windows` and `lurq_read_tree`, then closed;
separate Review passed. Sandbox foreground PID was unavailable, and follow-up host replays were inconclusive; D4
subsequently completed the UI verification setup. Evidence: `docs/evidence/d3-first-real-run-2026-09-29.md` and
`docs/evidence/d3-mcp-inside-run-2026-09-29.md`).
M2 · Map imported profiles' MCP servers · branch `codex/import-mcp` (source server IDs are resolved to target servers
or left explicit as unresolved; independent core import 20/20, store 1/1, desktop 1/1, i18n 2/2 and
`tools/check.ps1` passed. No-focus MCP/Pencil verification found 0 blocking/major/minor in the generic mapping row;
the imported draft uses the target server ID and the source database was unchanged. Pencil has no MCP-specific frame;
evidence `.tmp/verify/m2/report.md`).
U7 · Truncate long project names in sidebar selector and Overview heading · branch `codex/long-project-name`
(headless geometry test 1/1, `tools/check.ps1` passed; independent MCP/Pencil verification at 1440 and 900 logical px
found painted ellipses, preserved actions, and 0 blocking/major/minor in these regions. Evidence `.tmp/verify/u7/report.md`).
U9 · Truncate long project names in breadcrumb and Recent menu row · branch `codex/long-project-name`
(headless geometry test 1/1 including hover actions, `tools/check.ps1` passed; independent MCP/Pencil verification at
1440 and 900 logical px found painted ellipses, visible `Overview` crumb, separate menu actions, full stored name,
and no remaining long-name overflow on the screen. Evidence `.tmp/verify/u9/report.md`).
U4 · Publish button on a published profile · branch `codex/publish-button` (no code change: a published version has no
draft, so `Publish v2` is correctly disabled until edits are saved as draft v2. Real app lifecycle and MCP state
confirmed; focused lifecycle test 1/1 and `tools/check.ps1` passed. Pencil has no no-draft frame; the adjacent
saved-draft header mismatch is tracked as U8. Evidence `.tmp/verify/u3-u4/report.md`).
U1 · Truncate long status-bar failures · branch `codex/status-bar-truncation` (headless regression 3/3,
`tools/check.ps1` passed; independent MCP/Pencil verification at 1440, 1100 and 900 logical px found
0 blocking/major/minor, a painted ellipsis, full wrapped hint and no overlap with project counts;
evidence `.tmp/verify/u1/report.md`).
T1 · Drop waits the store retry made unnecessary · commit `c13ece9`.
T2 · Poll-free plan tests · commit `8739a65`.
D2 · Extra writable folders for agents · branch `claude/writable-roots`.
T3 · Serialize the remaining run request fallbacks · commit `389ee3b`.
T5 · Continuing a run whose worktree was deleted by hand · branch `codex/missing-run-worktree`.
L1 · The window title follows the active tab · branch `codex/window-title` (MCP and Pencil verified).
L2 · Remove the held scroll states · branch `codex/scroll-state` (15 offstage-only states removed; purposeful navigation and programmatic scroll states kept; `form_keyboard ::` 213 passed, 1 ignored; MCP and Pencil verified on reachable screens).
L4 · Verify text inputs after the text fixes · branch `codex/text-input-verification` (independent MCP/Pencil check of focused New Task brief, conversation composer, single- and multi-line selection, caret blink and stable line baseline: 0 blocking/major; 3 focused tests and `tools/check.ps1` passed. Minor: New Task form controls sit about 2–3 logical px above Pencil. Pencil has no explicit text-selection specimen; selection was checked against field geometry and the planned glyph-height behavior. Local evidence: `.tmp/verify/l4/report.md`).
L3 · Outside presses only close popups · branch `codex/popup-dismiss` (lurq 0.30.1 with upstream chrome fix; tab list, project switcher, permissions popover and selects checked through headless tests and MCP on page/chrome. Two regression tests; `form_keyboard ::` 215 passed, 1 ignored; `tools/check.ps1` passed).
L5 · Select menus wider than their trigger · branch `codex/select-menu-width` (lurq 0.30.2 adds `SelectStyle::min_menu_width`; the MCP secret menu is 260px wide and shows `NAME (environment)` on one line. Independent app/Pencil check: 0 blocking/major; registry-backed integration test 1 passed; `tools/check.ps1` passed. Minor fixture difference: stored references use canonical `@staging`, while Pencil shows `@Staging`. Local evidence: `.tmp/verify/l5/report.md`).
L6 · Verifier app windows start without focus · branch `codex/l6-registry-0303` (lurq 0.30.3 registry build and filtered focus-related tests 11/11 passed; `tools/check.ps1` passed. Independent exact-PID MCP and host foreground verification observed no focus across 951 samples; live Pencil Welcome comparison found 0 blocking/major, 1 pre-existing minor copy difference. Evidence: `docs/evidence/l6-registry-0303-2026-09-29.md`).
ORC-15 · Repository agent files, Orchester-owned skills and import from repository · branch `claude/orc15` (per-project setting, off keeps AGENTS.md and `.agents/skills` out of Codex with `project_doc_max_bytes=0` and per-skill `skills.config` on new and resumed sessions; Claude Code already loads neither with `--setting-sources ""`; skills resource with reference-file blobs, pinned by runs, placed in `.orchester/skills/<execution>/` and listed in new-session prompts; Import from repository creates drafts only. Core, store, runner and desktop tests, `tools/check.ps1` passed; independent MCP/Pencil verification: 0 blocking/major. Open: the setup Ready-step entry point is not built, Claude Code `on` does not load CLAUDE.md (user decision), macOS unchecked).
