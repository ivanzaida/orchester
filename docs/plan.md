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
   Implement → command `powershell -File tools/check.ps1 -Tests changed` (the check and the tests of the changed
   crates, `docs/stable-build.md`) → Review → Ready for review.
   Set the Implement **step** to *Can write* as well as the profile: a new agent step starts *Read-only* and can narrow a
   write profile. Keep Review read-only.
5. Environment: nothing for builds. Cargo builds into each worktree's own git-ignored `target/`, which goes away
   with the worktree. A new worktree starts cold (several GB, several minutes).
6. Limits: start at about 3M tokens and 45 minutes of active agent time per run.
7. *Merge into main* merges into the user's checkout, which must be on `master`; uncommitted and untracked files the
   run does not change stay as they are (a run that changes one of them is refused, with Local integration offered).
   Do not merge from Orchester while another agent is merging branches into `master` by hand.

### D2 · Verify that could not verify, and the QA slot · branch `claude/verify-flow` (implemented 2026-10-14, waiting for review and design verification)

From `.tmp/analysis/agent-dev-blockers.md` §6, blockers 1 and 2. A step result may be `unverified` with what it could
not check and why: the run waits for the person (reason `unverified`) without the failure route or a retry, and the
person accepts the step, runs it again, or returns the work with a note (`continue_run` `accept`, `allow_retry`,
`return_to`; `get_run` lists the checks). A step whose latest failed or unverified result was for the revision it would
run on now is not run again unless a person asked: the run waits with reason `unchanged`. `start_qa_app(scenario)`
runs the worktree's `tools/qa-app.ps1` for the agent, behind the computer's QA slots (OS file locks every runner
takes; as many as the project's *QA apps at once*, ORC-64, default 2; a start while all are held waits, noted in the
transcript with who holds them), keeps the app for the invocation (`call_run_app` without a pid reaches it) and stops
it when the turn ends. The task page's Summary shows *Unverified: …* with the list, *Accept … as it is*, *Run …
again*, the return with a note and *Stop* (the `decisions` seed scenario holds one).
**Open:** design verification of the waits-unverified Summary (no Pencil frame of its own yet: it reuses `DVuXu`'s
parts); the deprecated desktop shows the state but offers no *Accept*; the runner's macOS side is not cross-checked
(its C dependencies need a macOS toolchain).

### D3 · Resolve a merge's conflicts, and return a finished run to a step · branch `claude/conflict-actions` (implemented 2026-10-19, waiting for review and design verification)

*Not merged into main* after a conflict offers, beside *Local integration*, *Resolve in editor* and *Ask an agent to
resolve* (Pencil `fvCVT`). *Resolve in editor* merges the base branch into the run's own worktree, recorded as the
run's resolution so the run neither continues nor merges meanwhile, and opens it in the editor `ORCHESTER_EDITOR`
names, else VS Code's `code`, else the folder; the index decides what is resolved, a binary or deleted-on-one-side file
waits for the person to keep a side, *Continue* commits the merge (git then proves the merged-in base for the merge into
main), *Abort* puts the worktree back. *Ask an agent to resolve* returns the run to a writing step with a note and the
conflicting files as data. The core of the project task *ORC-44 · Return a finished run to an earlier step*: a
succeeded or failed run is reopened from an executed agent step with a note (`continue_run` `return_to` takes ended
runs too); refused for a merged run, a gone worktree, a merge in the worktree and a run at work.
ORC-44 (2026-10-21) added the rest: a follow-up whose step reports `failed` is routed as that step's failure is (its
route and retries, a new attempt from the route's target with the follow-up's report), or waits for the person where
the step has no route; the task page returns a succeeded or failed run with *Return the work to an earlier step*
above *Run details*, and *Return to a step* in the run's header points at it (or at a waiting run's decision).
**Open:** the design-verification of boards 1–9 of `fvCVT`; a frame for files that wait for a side to be kept; a
frame for *Return the work to an earlier step* and the header's *Return to a step* (they reuse `DVuXu`'s return parts).

### D4 · Base-moved notices go only to steps that change the code (ORC-65) · branch `orchester/orc-65-run-1` (implemented 2026-10-21, waiting for review)

ORC-54 told a run's writing agent to merge the base branch when it moved; ORC-18 run #32's notice went to its Verify
step, which writes only build output and QA files. Now only the step that implements the change takes it
(`orchester_core::resource::Pipeline::takes_base_moves`): a step's own choice (`StepPermissions::takes_base_moves`,
the pipeline editor's *Merge the base branch when it moves* and `save_pipeline_draft`'s `takes_base_moves`), else an
agent step with repository write that is not a Review or Verify step (a word of its key, or a failure route back to a
writing step). The runner delivers a notice only to such a step whose agent writes in the run; *Ask an agent to
resolve* picks its default step among the same steps (`implementing_step`). A run past them keeps the notice, and the
Conversation says *waits until the work goes back to Implement* (`get_run`: `waits_until_work_goes_back`). Verify's
git-ignored writes make no checkpoint, so they never count as the run's work. Design:
`docs/design/orchester-workspace.md` › *When the base branch moves (ORC-54, ORC-65)*. Tests: `orchester-core --test
pipeline_base_moves base_moves`, `orchester-runner --features test-agent --test base_moved base_moved`,
`orchester-workspace --test base_moved_steps base_moved_steps`, `orchester-app --test pipelines pipelines_base_moves`,
`orchester-app --test mcp_domain domain_save_pipeline_draft_keeps`.
**Open:** design verification of the editor switch and the Conversation wording (no Pencil frame of its own); the
deprecated desktop's editor neither shows nor changes the choice (its edits change the other fields only).

### D5 · Attachments in chats (ORC-57) · branch `orchester/orc-57-run-1` (implemented 2026-10-21, waiting for review and design verification)

Both chats' composers (the Orchestrator's and a task's Conversation) take files: *Attach* (an rfd picker,
`services::attachment_picker`), a paste of an image or of text over 2,000 characters (a *Pasted text.txt* chip that
can be put back into the message), and a drop on the box, which highlights while files are over it. Chips show a
thumbnail or kind icon, name, size and remove; an image or text chip opens a preview (`ui::attachments`). At most 10
files a message, 20 MB each, 50 MB together; a folder, a shortcut, an empty file or another kind is refused under the
box with why (`orchester_workspace::attachments`). A message may be files only; drafts keep their files. Kinds are told
by content (`orchester_core::attachment`); the store keeps a copy in `.orchester/attachments/` and a row in
`attachments` (`docs/architecture/storage.md`). Sent messages show their files through `ui::chat`; a PDF opens in the
default program. Delivery: Claude Code as stream-json image and document blocks (the Orchestrator chat too), Codex App
Server as image items (no PDF: refused before sending), native agents as content blocks; a kind a connection cannot
take refuses the send with why. `send_message` and `get_conversation` carry the files' metadata, never their bytes.
*Ask again* and *Retry* are not offered for a turn with files. Tests: `orchester-core --test attachments`,
`orchester-store --test attachments`, `orchester-workspace --test attachments`, `orchester-runner --features test-agent
--test attachments` and `--test native native_attachments`, `orchester-app --test composer composer_attach` /
`composer_a_`, `orchester-app --test orchestrator attachments`.
**Open:** no Pencil frames exist for the chips, the drop highlight, the refusals or the previews; design verification
against the composer (`QQ2ZM`) and message (`mr4lC`) frames only. Not checked on macOS.

### D6 · The project's base branch (ORC-81) · branch `orchester/orc-81-run-1` (implemented 2026-10-23, waiting for review and design verification)

Project settings › General › *Base branch*: *Checked-out branch* (the default, exactly the behaviour before), a local
branch, or a remote-tracking one that creates its local branch when chosen; kept in the project's database on this
computer only (`project_base_branch`). With a branch set, a new run starts from its tip wherever the checkout is and
records it as its base (`runs.base_branch`), so every delivery follows it; a merge into a base the checkout does not
have checked out lands beside the checkout through the temporary worktree and `git update-ref`, never switching it
(`orchester_core::integration::base`, `merge::beside`); the merge queue lands the same way. A branch set that is gone
refuses new runs (preflight `base_branch_missing`) and the row shows the problem; a change says how many unfinished
runs keep their base; `get_project` reports `base_branch`. Design: `docs/design/orchester-workspace.md` › *Base branch
(ORC-81)*; storage: `docs/architecture/storage.md` › *Integration*. Tests: `orchester-core --test integration
base_branch`, `orchester-store --test integration integration_base`, `orchester-runner --features test-agent --test
execution execution_starts_from_the_base_branch_set`, `orchester-workspace --test base_branch base_branch`,
`orchester-workspace --test merge_queue merge_queue`, `orchester-workspace --test pull_requests pull_request_creation`,
`orchester-app --test project_settings project_settings_base_branch`.
**Open:** no Pencil frame exists for the row (built with General's select row, hint and error line); Local integration
works in the checkout, so it still needs the base branch checked out there; the Files page's *Base* version still
reads the checked-out branch.

### D7 · Runs whose result is not a commit (ORC-93) · branch `orchester/orc-93-run-1` (implemented 2026-10-25, waiting for review and design verification)

A pipeline (and a task, for its own runs) says what a run delivers: `changes` (merged; every older pipeline),
`report`, `outside` (work done through MCP tools) or `changes_or_report` (core `Deliverable`; Pipeline settings ›
Delivery; `create_pipeline`, `save_pipeline_draft`, `create_task`, `update_task`, `get_pipeline`, `get_task`). Every
step's prompt says it. A run that passed every step and committed nothing is completed rather than merged when its
result is its report: the Ready card offers *Complete task*, the Summary shows the report and, for `outside`, the MCP
tool calls approved outside Orchester's own tools; completing sets the task Done and keeps the report as the run's
artifact *Report* (`run_completions`, `run_artifacts`); the merge queue skips such runs and the Orchestrator completes
them by its merge rule when the person lets it merge finished runs. Where changes were expected the run is flagged in
the Summary and Needs you (*Complete anyway*, *Return to a step*, *Run again*), and the Orchestrator leaves it to the
person. `complete_run` completes as the Summary does (`anyway` under the person's consent for automation's
decisions); a completion through it or by an MCP client's window input is recorded as automation's. Design:
`docs/design/orchester-workspace.md` › *Runs whose result is not a commit (ORC-93)*. Tests:
`orchester-core --test pipeline_delivers delivers`, `orchester-core --test run delivers_prompt`, `orchester-core --test
run again`, `orchester-runner --features test-agent --test instructions instructions_say_what_the_run_delivers`,
`orchester-workspace --test run_completion completion`, `orchester-workspace --test merge_queue merge_queue`,
`orchester-app --test task_decisions completion`, `orchester-app --test pipelines pipelines_delivers`, `orchester-app
--test mcp_domain task_organization_delivers`, `orchester-app --test mcp_domain completion`.
**Open:** no Pencil frames exist for these states (built from the Ready card, the warning notice and Starts' option
cards); the Artifacts tab still says runs keep no artifacts; no notification for *Ready to complete* or *Committed
nothing*; no form field for a task's own choice (MCP only).

## A · New app (`crates/desktop-app`) — the path to developing Orchester in it

Merged on 2026-10-09/10 (see *Done*): F1, T1, F2, P1, N1, P2, C1, C2, C3, D1, I1, M1, LIM, M12 and its follow-ups. Still
needed before Orchester's own work moves into the new app, in this order:

### A1 · W1 Window lifecycle (UI)

What happens to running agents when the window closes (keep the runner and its agents, or stop them, with the
person's choice per the design), and how a reopened window picks the runs up again. Design: layer "App lifecycle"
`Ob1P7` in `data/orchester2.pen`. **Done when** closing with agents at work and reopening shows the same runs live,
tested with the fake agents, and an independent design verification passes.

### A3 · Set up the Orchester project through the new app's MCP tools (user + agent)

The person turns on *Let automation change agent permissions* in Settings; an agent then adds the project, CLI
connections, agent profiles, the *Orchester change* pipeline and instructions with the domain MCP tools (`D1` above
lists the content); the person turns the switch off again.

### A4 · lurq 0.43 (non-UI and UI)

lurq 0.43.0 is released (ivanzaida/lurq#47). Bump it and replace the workarounds it makes unnecessary: Markdown that
fits its content for the person's bubble (`MarkdownProps::fit_content`), `Ctx::scroll_into_view` for Summary's row
confirmations, `FutureAction::run_if_idle` / `FutureState::outcome()` for the hand-written press guards and
watches, `is_synthetic_input()` as a second lock on the Settings permission switch, `focus_visible` for keyboard-only
focus rings. Note the behaviour changes: `focus()` now scrolls into view; create-time actions now run.

### A5 · Follow-ups found during review and QA

- Codex steps that do NOT ask before MCP actions decline every MCP tool Codex itself asks about (non-read-only tools
  such as Pencil's `execute`): `runner/src/execution/step/app_server/mod.rs` `screen()` declines when
  `tool_approvals` is off, and `invocation/mcp.rs` `without_exemptions` drops the server's exempt tools. Such a step
  should approve the tools it gives the agent (or at least its exempt ones). Workaround today: the profile asks before
  tool calls, so exemptions apply.
- Program places (after `claude/app-connections`): `program_place` checks only the final target and the found
  file's folder, not links in the middle of a chain (refuse when any link on it lies inside the repository); the
  App Execution Alias exception accepts any file in `WindowsApps` without reading its reparse tag; launch checks skip
  programs stored by name or relative to the worktree before this change (show a problem for stored `InWorktree`
  programs); agent CLI connections get no `program_place` check at launch, and the file chosen on the start page gets
  none either, so a file inside one project's repository, which agents can rewrite, can become the program of
  connections in every listed project (`connections::choose_program`); the deprecated app's MCP dialog saves with
  `Allowed::Anything`; the Connections list shows a server whose secret is archived as plain "On".
- Command (tool) steps run agent-written worktree content (`tools/check.ps1`, build.rs, cargo/npm/git configs) with
  the person's rights, network on and no sandbox (`crates/runner/src/execution/tool/mod.rs`), escaping a writing
  step's narrowing. Run tool steps under the narrowest preceding writer's sandbox, or require an approval gate before a
  tool step that follows a writing agent.
- Secret values written literally into a command or an agent's description reach the approval card, the MCP tree and
  Copy (old app too): redact known secret values in approval text.
- Confirm the agent CLIs' sandboxes cannot write the project database (`.orchester/orchester.db`) or `.git` hooks; every
  person-only gate assumes they can't.
- A plain question at tab open focuses the card's *Answer* button although the composer is already in answer mode;
  focus the composer box instead. A plan at open focuses *Approve plan & continue* (a stray Enter approves).
- `tab_labels_pill_sums_every_open_project_and_splits_it_in_its_tooltip` is flaky (waits for the runner badge, not for
  the second project's tasks), and so is `summary_run_again_confirms_in_place_and_starts_the_next_run`
  (`task_summary.rs:177`, "task-tab-summary is laid out"; fails about once in a full run, passes alone).
- The title bar shows "0 need you" when nothing needs you; *Cancel run* has no confirmation.
- Per-role agent avatars (profiles have no icon field; the conversation uses a generic bot).
- Claude Code is not told that a person approved its outside read (its approval reply carries no message).
- The task-states seed's KON-10 "Canceled" shows "Ready to queue" (product check).
- Chat Orchestrator (OR1, layers `TrG83`/`Xir64`) and Overview (O1, `vgqpM`) are built as Orchester tasks once A1–A3
  are done.

## T · Tests and runner

### T4 · A listing survives a row whose id cannot be parsed (non-UI)

A run row whose id or task id cannot be parsed still fails the whole listing
(`crates/store/src/project_db/runs/load.rs`), because it cannot say which run it is. Decide with the store owner
whether such a row is reported as a project-level "unreadable rows" error that still lists the others. **needs
decision**.

## M · MCP servers

### M8 · Orchester's own MCP server · branch `claude/app-mcp` (from master `3d76beff`, UI)

Implemented 2026-10-05 (user approval of 2026-10-05), **waiting for review and an independent design verification**.
Automation, the coordinator agent included, controls Orchester through data-level tools instead of clicking the
window through lurq's MCP server, which stays for screenshots and the element tree (`docs/stable-build.md`, "Embedded
MCP server").

- `crates/desktop/src/app_mcp/`: a loopback streamable-HTTP server (rmcp on axum) started by the root on the app's
  runtime, with a bearer token in `orchester-mcp/<pid>.json` (`%LOCALAPPDATA%` on Windows, `~/Library/Application
  Support` on macOS, user-only); each process locks `<pid>.lock`, and a starting server removes the files of ended
  processes. Tools: `list_projects`, `list_tasks`, `get_task`, `create_task`, `update_task`, `run_task`, `get_run`,
  `pause_run`, `resume_run`, `cancel_run`, `list_approvals`, `decide_approval`, `list_pipelines`, strict input schemas,
  typed errors (`code` plus the app's own explanation). They go through the screens' services (`TaskForm`,
  `TaskWriter`, `save_edit`, `queue_in`, `RunAgainCheck`, `claim_for_work`, `approvals::pending`, `run_view`) and the
  runner (`Runners::*_in`, which start the named project's runner when no tab shows it). Saved tasks reload the open
  task lists (`tasks::outside::OutsideChanges`).
- *Let automation answer approvals*: app setting (`app_settings`, off by default) in the popover of the status bar's
  MCP item, which now shows Orchester's server; `decide_approval` is refused while it is off. Answers are approve once
  or deny, recorded as `DecidedBy::Automation` / `AutomationDeny` (core `Answerer`, ipc protocol 17, project schema
  rebuilds `tool_approvals` for the new values); the activity and the run's Approvals name them.
- Tests: `cargo test -p orchester-desktop --test form_keyboard app_mcp` (10), `--test status_bar status_bar`,
  `--test i18n`; `cargo test -p orchester-runner --features test-agent --test approval_choices` (automation answers);
  `cargo test -p orchester-store --test approval_rules`, `--test app_db automation`.
- Security review fixes (2026-10-05): merged master `47291954`; the automation rebuild of `tool_approvals` follows
  the `permissions` rebuild and admits all eleven answers (`permitted_migration` upgrades a populated database through
  both). Automation approves once only what auto-approval would approve (`given_by` takes the subject; runner
  rejections `person_only` with the reason, `automation_wider`); it may deny anything. lurq's MCP server only reads in
  release builds (`embedded_mcp::McpAccess`, `cfg(debug_assertions)`; no `Interact` scope), so it cannot flip the
  setting or press Approve. Origin other than loopback refused (403), `Bearer` case-insensitive, 32 connections with a
  10 s header/idle deadline, a same-pid stale file replaced, the Windows discovery file given a user-only access list
  (`icacls /inheritance:r`). Not added: limits on brief and criteria, which the new task form does not have either
  (only the title's 200 characters); a limit would be a new rule (**needs decision**).
- **Not done:** design verification of the status bar item and the new popover (no Pencil frame exists for the
  popover; it takes the operations popover's panel); the macOS build of the desktop was not checked (no Apple SDK on
  the Windows host; core checks for `aarch64-apple-darwin`).
- Design-verification fixes (branch `claude/mcp-ui-fixes`, 2026-10-05, waiting for an independent design
  verification): an inline approval card lays out its answers by the chat's width (one line; *Always allow…*
  above the buttons; each button on a full-width row), never shrinking one, and its title wraps before its
  status label; a transcript row's title keeps 120 while its mark, then its meta, show whole or drop out whole
  (`activity/header.rs`, lurq `shrink_order`), in Tools and in an open chip; automation's denial tells the agent
  that automation denied it (`denial_message` takes the `Answerer`); the profile's answers count as *Allowed by the
  profile*, no longer under *Always allowed*; inline cards answered by the profile or automation say so; the MCP
  item keeps `$hover` while its popover is open; the popover's title sits 14 from its top and its switch has the
  inset ring with its gap inside (`ExtraSpacing::FocusInset`). Live states: `ORCHESTER_KEEP_ANSWERED=<folder> cargo test -p orchester-desktop --test form_keyboard
  app_mcp_answered` keeps a project whose runs the profile and automation answered through the runner.
- **Follow-up (UI, needs design pass):** decision marks in the transcript and chips are plain mono text after the
  meta; the design draws them as the *Decision mark* chip (`M7pU0A`). Affects every mark, so it was left out of the
  fixes above.

### M10 · Auto mode for approvals · branch `claude/auto-mode` (from master `6d44bdfe`, UI)

Implemented 2026-10-06; the security review (`B1`, `B2`, `M1`–`M4` and the minors) and the design verification
findings of the same day are fixed, **waiting for a second review and design verification** (no Pencil frame; judge
consistency with *Approve everything in this run*, the approval dialog, the permissions popover's sections and
Project settings › General). A reviewer model judges the requests the rules leave to the person
(`orchester_core::run::review`, `RequestReviewer`; Claude Code in print mode without tools, `connection::reviewer`);
hard rules first on every part of a line (`review::hold`: credential reads, unreadable parts and scripts,
destructive parts, read-only step, network off, outside folders, unrecorded permissions, Codex escalations,
`run_project_tool`, requests too long to review whole), fail closed (error, 60 s timeout, unreadable answer). The
reviewer runs supervised in an empty temporary folder with its instructions as the system prompt and the agent's
text as JSON data; it is stopped when the person answers or the worker ends. Recorded as `decided_by = 'reviewer'`
and `tool_approvals.review` (JSON), the run's `auto_mode`, `project_auto_mode` (new runs, model).
`set_run_auto_mode` on Orchester's MCP server, gated by *Let automation answer approvals*. Open: turning auto mode on
while a request waits does not review that request.
The reviewer is any agent connection (`review::choice`: Claude Code, Codex through `codex exec` read-only, a native
Anthropic API connection through `orchester-agent`'s client; not the OpenAI API yet) with a model and effort, chosen in
Project settings or with `get_auto_mode`/`set_auto_mode`; default: the first Claude Code connection with Haiku 4.5,
else the first usable connection with its default model (`project_auto_mode.connection`, `.effort`). The Codex flags
were not run against a live Codex.
Tests: `cargo test -p orchester-core --test auto_mode auto_mode`, `-p orchester-store --test auto_mode auto_mode`,
`--test permitted_migration`, `--test approval_rules`, `-p orchester-runner --features test-agent --test auto_mode
auto_mode`, `-p orchester-desktop --test form_keyboard auto_mode`, `run_permissions_popover_fits`, `app_mcp`;
`-p orchester-core --test auto_mode_reviewers auto_mode`, `-p orchester-agent --test anthropic_wire anthropic_`.

UI states for the verifier (dev build, debug lurq MCP): (1) task page › This run › Approvals: the *Auto mode* option
under *Approve everything in this run*; without a Claude Code connection it is fixed, keeps its explanation and says
why after the lock glyph inside its row; (2) the run permissions popover (the run's permissions row › link): the
*Auto mode* section last, after a divider, a plain switch *Use auto mode in this run* with its hints and *Saves at
once…* (it is not part of Apply), at 960 × 700 scrolled with the text clear of the gutter; (3) Project settings ›
General › *Auto mode*: *New runs use auto mode* (fixed without a reviewer) and *Reviewer* (connection select with
*Default · …*, model select, effort select where the model has efforts, *Reviewed by <model> on <connection>*, or the
lock line saying why there is no reviewer); the Approvals option's note and the popover also end with *Reviewed by …*; (4) the approval dialog of a request auto mode left to the person:
the line under the explanation (auto mode glyph, *Auto mode asked you:* and the quoted reason, or *Auto mode left this
to you: …*); (5) the conversation's request card: the same line, not repeating the chip; (6) the activity rows: the
mark *approved by auto mode · <subject>* (or *approved by you · …*) and the review line under the row, at 1440 and
960; (7) the run's Approvals counts *Approved by auto mode* / *Auto mode asked you* once non-zero, before *Waiting*.
States 4–7 need a run with auto mode on and a Claude Code connection, or seeded `tool_approvals.review` and
`ToolDecided` activity.

### M11 · Orchester's MCP server works without disturbing the window · branch `claude/mcp-quiet` (UI)

Implemented 2026-10-07, **waiting for review and design verification**; moves to *Done* once merged. Tools change
data while a person uses the window: changes are counted per project and per kind (`tasks::outside`: tasks, runs,
auto mode, code index, limits, pipelines), so only the changed project's data is read again and only the screens
showing it read it, in place. A failed reading of the tasks keeps the tasks last read on every page; the project shell
says so above the page with *Retry*. Edit task keeps a person's edits and says the task changed elsewhere (*Reload*),
and saves on the version its fields came from (merged or a conflict, never a silent revert). Project settings ›
Auto mode, Code index and Limits show a tool's change in place, or hold it behind *Changed outside this screen* while
the person has a change under way; their saves and the tools' are one store transaction each
(`update_auto_mode_settings`, `update_project_limits`) and save only what the person changed. The run's auto mode
switch, Pipelines and the status bar's budget follow too. A tool's worktree removal shows *Removing…* in the
person's open dialog, which stays open and says the worktree is removed; a tool's failure is only in its result. A
queued task without a run that this window's form did not queue opens on its page with *Retry run* (design doc, "A
queued task without a run that the person's form did not queue"). New tools `return_to_draft` and `continue_run`
(`docs/stable-build.md`). Tests: `cargo test -p orchester-desktop --test form_keyboard app_mcp` (`app_mcp_quiet`,
`app_mcp_continue_run`, `app_mcp_return_to_draft`).

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
`approval_mode = "approve"` as an override of its process (`docs/interactive-tool-approvals.md`). **Done when** an
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

## U · Screens

### ORC-584 · Clone the repository from *Link a folder* in a console · branch `orchester/orc-584-run-1` (implemented 2026-10-31, waiting for review and verification)

*Clone it here* on *Link a folder* (06 `go2aM`): *Clone into* with *Choose…*, what Git will do with that folder, and
*Clone repository…*, which opens a console window the person sees (PowerShell on Windows, Terminal on macOS) running
`git clone -- https://<hint>.git <folder>` with each value as data. Git may ask for a sign-in there, with Git's own
credentials (`gh` is not needed). A clone counts only when Git ended with 0 and the folder checks as the project's
checkout; it then fills the folder field and is linked only when the person presses *Link and download tasks*. A
folder with anything in it is refused before a console opens, and what a failed or closed clone left is named and
kept. Choice A of `docs/design/startup.md` is revised; ORC-290 and ORC-296 (hidden clone with in-app progress) stay
canceled. As built: `docs/design/startup.md`, "Clone in a console". Tests: `cargo test -p orchester-exec --features
test-program --test clone_console clone_console_`, `cargo test -p orchester-workspace --test startup_clone
startup_clone_`, `cargo test -p orchester-app --features test-support --test shared_projects shared_projects_`.

The Pencil design is not updated for it (the person, 2026-10-31): 06 `go2aM` and 07 `phoYc` still draw the older
*Clone it here*, an agreed deviation (`docs/design/startup.md`, "Clone in a console").

**Done when** review and verification pass and a person has cloned once for real on Windows and on macOS (a real
console with Git's sign-in, Terminal's window closed mid-clone), which no test can do; then move this entry to *Done*.

### ORC-16 · All projects and Add project in the new app · branch `claude/all-projects` (implemented 2026-10-14, waiting for review and design verification)

All projects (`S5HDKf`, states `kwpO2`), the welcome of a first launch (`UQZRa`) and Add project as a page
(`/add-project`, `kkGWj`, `W5PIR`, `vJTcd`, detection states `gh5wV`) in `crates/desktop-app`; rules in
`orchester_workspace::project_list` and `project_setup` (`readiness`, `expected_account`, `create`). Neither page has
a sidebar; the switcher's *Add project* and *Set it up* lead to Add project. Usage across projects stays a locked tab
until the Usage dashboard comes to this app. Tests: `cargo test -p orchester-workspace --test project_list
project_list`, `--test project_setup setup_`, `cargo test -p orchester-app --test all_projects all_projects`, `--test
add_project add_project`. QA: `cargo run -p orchester-app --example projects_qa -- <state>`.

**Done when** an independent design verifier reports no blocking or major mismatches against `CGlZY` (960 and light
passes excluded); then move this entry to *Done*. The created project's Overview notice (`E5yOA`) waits for Overview
(O1).

### ORC-61C · Conversation as a chat with the step rail, and the one-box composer · branch `claude/chat-impl` (UI)

Design: `docs/design/orchester-workspace.md` on `claude/conv-chat` › *Decided: C with the rail on the right* (group
`A5H6ln`, C·01–C·14) and *Composer (approved …)* (group `YJfPo`, K·01–K·06), both approved by the user 2026-10-04.
Implemented 2026-10-04 on master `566ca413`, **waiting for an independent design verification**:

- Data (`tasks/timeline/view`, `tasks/timeline/rail`): an agent's message carries a chip per tool group after it
  (tool activity with no message before it is the agent's row of its own); finished steps fold, the ones in a row
  into one line (*01–02 · 2 steps done · Show them*); the step the run ended on, the running/waiting/failed step and
  the one before the last that ran stay open; the steps a failure kept from starting share one muted divider; a
  search counts its matches per step execution. The rail is one row per pipeline step, from its latest execution
  that ran (*Explorer · 41k · 6 min*, *Planner · waiting for approval*, *Tool · exit 1 · 2 min*, *Reviewer ·
  returned to Implement*, *Builder · not started* at 0.7); picking it shows every execution of the step.
- Chat (`screens/project/task/conversation`): Slack-style rows (28 round avatar, name, meta), thin dividers for a
  step's start (number, agent, engine, times, tokens — the tokens open Details on it) and end (*06 Fix notes done ·
  16:30 · Changes · 3 files ↗*), returns and restarts as dividers with what follows under them (or the person's
  note as their message, `j0qLQz`), the run's end; questions, plans and tool requests are cards in the asking agent's
  message; the step the run ended on closes with its result card; a tool step's failing command is its own message.
  A chip opens its group in place (bordered list, 7 rows, *Show N more*; while searching *2 of 12 calls match* and
  *Show N without a match*). *Tools* keeps the flat transcript.
- Right side: the 248 rail (head *Steps* + count + collapse), or its 56 strip (collapsed by hand at any width, by
  default below 1200; badges pick a step; its head opens the rail in place in a wide window, over the chat in a
  narrow one), or Details in its place (head *Details* + *Steps*), never two. The toolbar loses the step filter; the
  read-back lines are rows under it (*Showing 03 Implement only · 01, 02 and 04–07 are hidden ·*, *No matches for
  “…” · following live goes on ·*).
- Composer (`composer/`): one rounded box (`$raised`, radius 18, `$muted` while focused) — *+* Attach unavailable
  with its tooltip, the run's permissions chip (`$danger` once changed for the run; opens the ORC-44 popover; its
  shield alone below 980 less the side panel), the recipient as a quiet selector (name + what it does, a lock when
  fixed), the round send button (arrow-up now, clock-3 queues, loader while sending) with the help text as its
  tooltip, and the chevron with *Send now · Enter* / *Send after this turn · Ctrl+Enter* while the agent works
  mid-turn. Enter sends (now while mid-turn), Ctrl/Cmd+Enter after this turn, Shift+Enter adds a line. Context lines
  above the box; one quiet line under it only for queued, follow-up, continue and not started; closed keeps only the
  lock and the reason. Delivery semantics unchanged.

Tests: `timeline` (chips, folds, search counts by step, picked step), `form_keyboard` `conversation_*`
(`conversation_rail` new: picking, search counts, Details swap, collapse below 1200, strip at 960, fold line;
`conversation_composer_box` new: send icon, attach tooltip, permissions chip popover, 960), `composer_*`,
`task_graph`, `tool_run`, `first_edit`, `integration`, theme, i18n.

Deviations to confirm with the design verifier/user: marks follow C·13 (`j0qLQz`, with *automatic · retry N of M*)
where C·07 draws a shorter line; a chip names its summary as ORC-61 orders it (*Ran pnpm test · edited 3 files*), not
*Edited 3 files · ran …* or file names; the chat is not capped at 840; agent rows name their step only after a muted
divider; *All steps* reads *N of M steps ran* whenever a step has not run; the question card's hint is pressable
(focuses the composer); the plan card keeps what approving starts beside its button; problem rows keep the transcript
entry look; the recipient menu's options carry the state words, not *gets it at its next tool call*; quiet lines keep
the ORC-61 explanation texts; the tool step's divider no longer names its command and exit code (C·05).

Design-verification fixes (2026-10-04, on `7710f081`): the recipient selector has no fill of its own (lurq painted an
unfilled select trigger white; the fill can't be asserted headlessly, as lurq 0.37 paints it outside the element tree;
since lurq 0.39 it paints no fill, and `form_keyboard select_fill` reads it from `Tree::painted_quads`);
a group of notes reads its lines without *0 calls*, a working step without tokens reads *Builder · working*; the
permissions chip shows whenever the composer is open (else the run's first agent step); *Write to Explorer before it
starts…*; a chip's lines start closed, failures too; picking a step opens the chat at its start, *All steps* goes back
to the end; Details names the step and attempt (*03 Implement · attempt 1*); narrower than one line, the toolbar's state
goes beside the search so *Details* stays whole; the send menu and tooltips sit 8 above the box; the context line has
× and Esc closes the menu, then leaves the mode (a question not answered or a plan's changes not requested: the box
writes an ordinary message, `ComposerMode::without_context`); a failed run's composer says *Continue 03 Implement with
your instructions · attempt 3* (no ×: continuing is the only message a failed run takes).

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

Not done: Cancel run stays beside Continue run for an interrupted run (63.04/63.05 draw none). Design verification
(separate agent) is outstanding.


### Agent accounts · branch `claude/agent-accounts` (implemented 2026-10-13; reviewed and design-verified 2026-10-13)

Several Claude Code and Codex accounts per computer (`docs/design/agent-accounts.md`, decided 2026-10-12): app-wide in
`orchester.db` (`agent_accounts`, a fixed Default per CLI, removed accounts kept as tombstones), a project's agent
connection picks one and keeps its folder in the connection row (absent for Default, so existing connections are
unchanged); the runner starts the CLI with `CLAUDE_CONFIG_DIR` / `CODEX_HOME` set to it and records each invocation's
account. Plan readings and snapshots are per account. A turn refused at the account's plan limit stops the run in
place (`plan_limit`); the runner resumes the same session at the reset, or the person continues the step now on
another account's connection as a new session for this run only. Desktop app: Settings › Accounts (list by CLI,
add in place, sign-in through a visible terminal and Check again, details, Used by, rename, remove in place), the
Account field in Connections, read-backs in agents, pipeline steps and runs, the plan limits per account in the status
bar, the plan-limit choice on the task page with *Try again now*; a new run whose account is at its limit waits for the
reset holding no agent slot, with *Try now on …* and *Start now on …*; *Choose folder…* for a missing folder. MCP:
`list_accounts`, `set_connection_account`. QA: `cargo run -p orchester-app --example accounts_qa -- <state> [account]`.
Tests: `-p orchester-core --test accounts`, `-p orchester-store --test accounts`, `--test limits limits`,
`-p orchester-workspace --test accounts`, `--test account_moves`, `--test account_readings`,
`-p orchester-runner --features test-agent --test accounts`, `-p orchester-app --test settings_accounts`,
`--test connections accounts`, `--test status_bar plan_limits`, `--test mcp_domain accounts`,
`--test task_decisions decision_plan_limit`. Four code reviews (the last a final review) and three design
verifications passed (report: `.tmp/accounts-run/report.md`). Open v1 work: the Usage page's legend per account waits
for the Usage page port (`Workspace::last_plan_readings` and `plan_limits::account_readings_in` are ready for it); the
status bar's plan cells are not design-verified, since they show only this session's live readings (ORC-17) and the QA
window has none. Not in v1: Codex identity through App Server `account/read`.

### ORC-72 · Usage dashboard · branch `orchester/orc-72-run-1` (implemented 2026-10-06, waiting for design verification)

The Usage page (sidebar item after Pull requests, `/usage`) and All projects › *Projects* | *Usage* (`/projects?tab=usage`)
as designed in `docs/design/orchester-workspace.md`, "Usage dashboard": pinned toolbar (period, Group by, filter chips,
read-back with the previous period), Summary, Tokens over time, Plan limits, Where tokens go, Most expensive, Efficiency,
a table view for every chart and the loading, no-runs, filters-match-nothing and no-plan-limits states. The status bar's
*Tokens today* opens it. Data: `orchester_core::usage` aggregates what `ProjectDb::usage_facts` reads (per-turn usage
from `agent_events`, invocations with CLI, model, profile, step, code index and first edit, runs with task, pipeline,
tags and outcome); the runner stores every plan limits report in `plan_limit_snapshots` (project migration, kept 90
days); the warning threshold is an app-database setting (default 80). Charts are lurq canvases (`canvas` feature) in
the new `chart-series-1/2/3` colors. Tests: `cargo test -p orchester-core --test usage usage`, `cargo test -p
orchester-store --test usage usage`, `cargo test -p orchester-store --test app_db usage_warning`, `cargo test -p
orchester-runner --features test-agent --test plan_limits plan_limits`, `cargo test -p orchester-desktop --test
form_keyboard usage_`, `--test theme usage`, `--test navigation route`.

**Done when** an independent design verifier reports no blocking or major mismatches against `C2pTYF`, `p4eVV6`,
`e5PbG`, `sOQmy`, `BLY4R`, `CDcij`, `qQOKw`, `AY6FM`, `YxAed` and `T6i75`; then move this entry to *Done*. Decided while
building (not in the design): a stretch of more than an hour between two reports of a window is a gap; a profile ·
step row of *Where tokens go* filters by its profile; the custom range takes dates as `2026-09-01`.

First verification (2026-10-06) failed on layout; reworked the same day: Tokens over time beside Plan limits and the
three *Efficiency* cards side by side (stacked under lurq's `Xl` breakpoint, 1280 logical), plain summary figures over a
hairline with the failed-run share and counts for runs, legends and captions above the charts with the design's
footnotes, the two-column Plan limits legend, step names (from the run's pinned pipeline) in *Where tokens go*, the
table's Total row, the empty states with the compact Plan limits card (it keeps the … menu so the threshold stays
reachable), All projects' controls at the right of the filters and *Project* after *Task*, day labels on multi-day
hourly Plan limits axes, and charts that redraw when the layout moves them to a new canvas.

Second verification (2026-10-07) failed on three majors, fixed the same day: Tokens over time and Plan limits plots are
190 tall from the top grid line to the baseline (no headroom above the scale's top; only the cache share line keeps
room for its label) with the footnote right under the chart; day labels of Tokens over time are counted back from the
last bar so *Today* never overlaps another; table text gives way with an ellipsis in narrow columns (960 wide).

Third verification (2026-10-07) found two majors, fixed the same day: an x axis label that would run past the plot's
right end ends there (*Today* at 30 days and Custom, *24:00* by hour), days in the proportional axis font and Plan
limits ending on *Today*; *Most expensive* titles shrink with an ellipsis, and under the `Xl` breakpoint its fixed
columns narrow so titles and pipelines stay readable at 960. Left open: the CDcij footnote's *Select a row to see its
runs;* (the table's rows do not open anything yet, so the sentence would promise a missing interaction).

### Auto mode reviewer's usage · branch `claude/reviewer-usage` (from `claude/usage-finish`, implemented 2026-10-07, waiting for review and design verification)

What auto mode's reviewer spends is recorded and counted (`docs/design/orchester-workspace.md`, "Token budgets" ›
*Auto mode's reviewer*, and the Usage dashboard's decision of 2026-10-07). Core: `RequestReviewer::review` gives a
`Reviewed` (verdict, `ReviewSpend` reported / unknown / nothing, plan limits); `review_request` gives an `AutoReview`
with the `ReviewCost`; Claude Code's reviewer prints `--output-format stream-json --verbose` (its result's `usage`,
its `rate_limit_event`s), Codex's `turn.completed` usage, a native response's usage; `execution_tokens` counts the
reviews of a step. Store: `reviewer_usage` (migration after `plan_limits`), read by `tokens_used_since`, the step budget
checks, `usage_facts` and the run view. Runner: the review's blocking job records its cost (`review::ledger`), sends
its tokens to the run as a turn's usage and stores its plan limits, also when the worker no longer waits for it;
the worker reports its end only once its reviews have (so late tokens count once, toward the run that still listens).
A CLI that never started (`CommandError::NotStarted`) and a native request refused with an HTTP status spend
nothing; a native attempt that may have been billed for more than it reported makes the spend unknown. The reviewer's
output keeps its last 256 KiB, so Claude Code's result line survives a long stream; a Codex output cut at its start
fails closed with unknown spend. An ended reviewer gets 10 s to report its exit, so a review never holds up the run.
Desktop: *Where tokens go* reviewer rows with the model first in mono and *auto mode reviewer* after it (names give
way with an ellipsis, the model kept whole), a line under the card's title saying they only read, the invocation panel's *Auto mode reviewer* line,
`get_run`'s `tokens.reviewer`. Tests: `cargo test -p orchester-core --test reviewer_usage reviewer_usage`, `--test usage
usage`, `--test auto_mode auto_mode` (and `auto_mode_scope`, `auto_mode_reviewers`), `-p orchester-agent --test
anthropic_wire anthropic_review`, `--test responses_wire responses_review`, `-p orchester-store --test reviewer_usage
reviewer_usage`, `--test usage usage`, `-p orchester-runner --features test-agent --test reviewer_usage reviewer_usage`,
`--test auto_mode auto_mode`, `-p orchester-desktop --test form_keyboard usage_`. Not verified against a live CLI: the
Claude Code reviewer's stream-json output and Codex's `turn.completed` usage were read from fixtures and the installed
binaries' event names only. **Done when** a separate review passes and a design verifier accepts the reviewer row of
*Where tokens go* (`C2pTYF` has none; judge it against the profile rows) and the invocation panel's line (`n0cEOG`).

### ORC-17 · Plan limits in the status bar · branch `claude/orc17` (done 2026-10-02)

Independent review and design verification passed (all states, widths 960–1600). Known: a truncated activity label
could stay cut after the window widened; fixed by lurq 0.33.0 (text layout cache). Implemented on `claude/orc17`, with master merged into the branch on 2026-10-02 (merge `c290682`) and the review and
design follow-ups on top. Codex and Claude Code report their account's limits live during agent turns: Codex App
Server's `account/rateLimits/updated` notification and Claude Code's `stream-json` `rate_limit_event`. The runner hands
each reading to the desktop (since ORC-72 it also keeps it in the plan limits history); the status bar shows the newest reading per CLI across open projects,
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

## N · Native agent (P1 in progress, P2 waiting for review)

Orchester runs the coding-agent loop itself against model APIs (Anthropic Messages, OpenAI Responses, compatible
endpoints such as DeepSeek), next to the Codex and Claude Code CLIs. The design is
`docs/architecture/native-agent.md`: crate layout, model clients, tools, permissions, context, connections, tests,
and phases P1–P5 (with P3b, Chat Completions) with acceptance lists. The user's decisions of 2026-10-01 are recorded
there; turn the phases into tasks here when work starts, beginning with P1.

P1 is split into five tasks. P1a (ORC-56) and P1b (ORC-57) are done, P1c (ORC-58) is done (see *Done*). The rest:

- **P1c · ORC-58 (done 2026-10-05, branch `claude/orc58`, see *Done*):** the
  `orchester-agent` crate: Anthropic client (§3.1), file and search tools, fake model server (§8).
- **P1d · ORC-59 (implemented 2026-10-06 as ORC-64 run #22, branch `orchester/orc-64-run-1`; waits for an
  independent review and design verification, see *Done*):** runner integration: `Driver::Native`, approvals, persistence, resume (§2.2, §6). Native steps
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
- **P2 · OpenAI Responses · branch `claude/native-p2` (implemented 2026-10-06, waiting for review; from master
  `18168b3e`):**
  - `orchester_agent::ResponsesClient` implements core's `ModelClient` statelessly (§3.2): `store: false`, no
    `previous_response_id`, the whole conversation each turn; reasoning items are stored verbatim (with
    `encrypted_content`) and sent back as `{type, summary, encrypted_content}` without the id and status of an
    item the provider did not keep, as the Codex CLI does. Strict function tools sorted by name, `tool_choice:
    "auto"`, `parallel_tool_calls`, `reasoning: {effort, summary: "auto"}` with `include:
    ["reasoning.encrypted_content"]` (a model outside the table reasons only when the request names an effort),
    explicit `max_output_tokens`, `prompt_cache_key` = conversation id, `Authorization: Bearer` as a sensitive
    header. Usage per §2.2. A failed tool's output starts with `Error: ` (the wire has no error flag).
  - Errors as for Anthropic, OpenAI's codes first: 429 `insufficient_quota` is `Billing`, `context_length_exceeded`
    is `TooLarge` (core compacts); `retry-after-ms`, then `retry-after`, read defensively and capped at 5 min (shared
    with Anthropic). `error` events and `response.failed` map by code; `response.failed` with `bio_policy` or
    `misalignment_policy_violation` ends as a refusal with that category; a refusal content part ends as a refusal;
    `incomplete` maps `max_output_tokens`/`content_filter`. A cut call is asked again with the model's output room.
    A 400 `invalid_encrypted_content` leaves the conversation's reasoning out once and sends the turn again
    (`ThinkingStripped`); that code comes from reported answers, not the reference, so it is *to verify* live.
  - Shared code: `Http::pause` (retry waits) and `Http::read_events` (the bounded SSE loop) now serve both clients;
    core's `tools::not_json` is public. No new dependency.
  - Models: `GET {base}/models` joined with a built-in table, checked against developers.openai.com model pages
    on 2026-10-06: `gpt-6-astra` (922K input, 128K output, low–max), `gpt-6.1-sol` and `gpt-6-luna` (1.05M,
    128K; Luna adds `none`), `gpt-5.5` (1.05M, 128K, none–xhigh); `reasoning.summary: "auto"` per the reasoning
    guide. Other listed models come without limits or efforts (manual entry). `check_responses` serves *Test
    connection*.
  - Fake server: moved to `crates/agent/tests/fake_server/` and speaks both wire formats (`Wire`), with the
    append-only check of `instructions`, `tools` and the `input` prefix and the cache echo;
    `crates/agent/tests/responses_wire` ports every §8 case.
  - Desktop: the *OpenAI API (native)* kind in Add connection is unlocked (`AgentProvider::can_be_added` removed)
    and opens the same native form, models dialog and inspector as Anthropic. Test connection and Fetch models stay
    visibly unavailable for both, as on master, until P1d.
  - Tests: `cargo test -p orchester-agent --test responses_wire responses_` (22),
    `--test anthropic_wire anthropic_` (30), `cargo test -p orchester-desktop --test form_keyboard native_` (30),
    `--test i18n`, `cargo test -p orchester-core --test native_setup only_the_official`.
  - UI states for the design verifier (not verified by the implementer): Connections → *Add connection*: the four
    kind cards with none faded and no "later version" note (04.H1); choose *OpenAI API (native)*: the native form
    with the Official preset at `https://api.openai.com/v1`, DeepSeek and Custom locked with *Compatible endpoints
    come in a later version*, Test connection and Fetch models unavailable, provider fallback *Not offered*;
    *Advanced* shows *Wire format · OpenAI Responses*; after saving, the inspector (04.H4) and *Edit models*
    (04.H6) of the OpenAI connection.
  - Open: **one live run** (§8 live-test policy, `ORCHESTER_LIVE_OPENAI_KEY`) is not recorded: no real API was
    called. **P1d hookup:** the runner's `Command::CheckConnection` must call `check_responses` for `OpenAiApi`
    and run native steps on `ResponsesClient`; P1d's effort mapping drops `none`, which `connection::Effort` lacks
    (adding it needs a decision). The `safety_identifier` user tag is P3. `clippy --target aarch64-apple-darwin`
    could not run here (`ring` needs a C cross-compiler); the change has no platform-specific code.
- **ORC-148 · native agents in the new app (implemented 2026-10-28, waits for Check, Review and Verify):** the new app
  (`crates/desktop-app`) gets what P1e built in the frozen app, for the Anthropic API.
  - Add provider: the Anthropic API card is unlocked (`connections.kind.anthropicShort`); its page has the native
    fields (`screens/connections/native_form.rs`): the Official preset with DeepSeek and Custom locked with why, the
    base URL starting at the preset's, the key secret picked or created (`ProviderForm::for_native`), models added by
    hand (optional: the endpoint lists its own), the default model and effort, provider fallback (off by default).
    Saving is a person's decision (`SaveGuard::admit_key`).
  - Inspector (`inspector/native.rs`, `inspector/enforced.rs`): the statuses with what each asks for, *Test
    connection* and *Fetch models* through the runner (`Work::Test` → `connections::native::test_in` →
    `Command::CheckConnection`, recording the result and merging the listed models), the result in place, the models
    with their source, limits, efforts and default, *Edit models*, and *Enforced by Orchester* from core's capability
    table. Edit connection edits a native connection's key, endpoint, models and defaults
    (`edit_agent/native.rs`); a model a profile or step runs on is never dropped (`save_native_in` → `ensure_kept`).
  - Runs: profiles list a native connection's models and efforts (`agents::form::listed_models`, `first_model`,
    `effort_options`); the profile page says *Enforced by Orchester*; a native command's approval card shows its shell
    and time limit (04.H9). The conversation divider already names the native engine; Usage already lists the
    Anthropic API as its own provider (ORC-146).
  - **Follow-up (OpenAI API):** the runner's `Command::CheckConnection` still refuses `OpenAiApi` and native steps run
    on `AnthropicClient` only (`NativeSession`, `step/native/drive.rs`, the review worker, `NativeConversation.wire`);
    until that hookup, the OpenAI API card stays locked (`connections.kind.openaiUnavailable`).
  - Design: ORC-166 drew the native frames in `XGtkg`, Row 8 `OyEXT` (`docs/design/native-connections.md`): 10c–10f
    (Add provider), 05e–05i (the connection page), 05j (Edit connection), 05k (states and notes) and 15 (the
    approval card). The screens follow them: no lead under Add provider's title (as `10b`), the *Sent to* line with a
    send glyph, the person-only refusal in the action bar with a `warning` shield, Edit connection's borderless
    *Cancel* and *Save connection*, the page head's status as a dot (a native page says it in `text_secondary`),
    the test result's glyph by how it went (check, triangle, cross) and *Billing problem* as a warning.
  - QA: `tools/qa-app.ps1 -Scenario native` (seed `scenarios/native.rs`): Anthropic API · Kontur connected (Builder
    and Reviewer on it), · Old key (key rejected), · Archived (key missing), · New (not checked), and KON-1 waiting
    for a native command's approval; started without a runner (`-NoRunner`) so the recorded run keeps waiting.
  - A test whose connection's key or endpoint was saved while it ran records nothing and asks to test again
    (`connections::native::record_tested_in`).
  - P1 items still open after ORC-148: a live run with cache reads (needs a person's key), runner-level tests of
    budgets, the time limit, cancel and resume after a runner restart for a native step, and a refusal shown as a
    problem in the new app's conversation.

## L · After lurq 0.30.0

The tab and text-input fixes shipped in lurq 0.30.0, the outside-press fix for window chrome in 0.30.1, the
select-menu width option in 0.30.2, the verifier window focus option in 0.30.3, and text selection after a drag on
scaled displays in 0.30.4. This workspace uses 0.41.2 (a long text clipped to its paint is no longer cached as whole; MCP discovery files of ended processes are removed; 0.41.1: Canvas shaped-text and asset caches stay resident across frames; 0.40.1: inspection screenshots cover sensitive text): a resize restores a minimized window first, `lurq_resize` answers with the size the window took (or fails saying why), `lurq_windows` reports `minimized`, `maximized` and `full_screen`, `on_size_changed` reports the size read back from the window, an `Enter` marked `composing` triggers no lurq default (form submit, activation, newline, blur), and `Text::sensitive` with `lurq::core::Sensitive<T>` shows a value on screen while the MCP and DevTools trees show `•••` (the first password of 10.08 B) (0.40.0). A text input composes with an input method (IME), the keys the input method takes reach no handler, and a press it passes on is marked `KeyboardEvent::composing` (the composer's Enter skips it); a select trigger part without a background paints no fill (it was white), unstyled checkbox and slider fills follow the theme's roles, and `Tree::painted_quads` lets a headless test read fills (`form_keyboard select_fill`) (0.39.0). A shrinking child of a single-line row gives way in its `shrink_order`, down to its `shrink_limit` (`MinSize`, `Content`, or `Drop`: natural size or dropped whole), in whole pixels (0.35.0), or trims to `shrink_drop_below(size)` and then drops, and a drop gives its width back to the line (0.36.0); what gives way whole only accumulates as a line narrows, and an item holding droppable children keeps its natural size until its line collapses it (0.37.0); the status bar's one line uses them. Its layout cache restores text output (ellipsis, wrap, caret geometry) on a cache hit (0.33.0); a scrollbar takes the pointer around its thumb, or its whole lane when the lane is reserved or painted, so pressing there never selects text (0.33.1); and a window-sized layer (a `Modal` a component declares itself, a popup) is laid out at the current window size after a resize back (0.34.0, which also adds profiling sessions and the `lurq_profile_*` MCP tools). A component's `ctx.future`/`ctx.stream` tasks are aborted when it unmounts (0.34.2), so a page's watchers stop with the page. Since 0.31.0 the MCP server also offers `lurq_inspect` and
`lurq_act`, and `lurq_read_tree` quotes app-provided text that is not a plain word (`{tab="All projects"}`); since
0.32.0 percentage-width children of a wrapping row resolve against the row's width. A later patch release needs only `cargo update -p lurq` (the requirement in `[workspace.dependencies]` already
admits it) and, to keep naming the version in use, this line.

**Markdown code blocks scroll sideways (ORC-82, lurq 0.51.1).** A fenced code block keeps its lines unwrapped and,
since lurq 0.51.1, scrolls them sideways inside its box (`ScrollHorizontal` under the label, the theme's
`code_block_scrollbar`, `None` in `theme::markdown` so the app's own scrollbar draws it, overlaid on the box's bottom
padding); 0.50.0 painted a line wider than the column past the box, out of reach. Every chat and agent Markdown goes
through `ui::markdown::MarkdownView` or `ui::chat::agent_text` with the app's theme, so the Orchestrator chat, the task
Conversation, plans, Details' brief and decision cards all have it. Headless: `ui_kit markdown_code_scroll`,
`conversation code_block` and `orchestrator code_block` (the swipe moves the line and nothing else, the line's end
comes into the box, a wheel up over the block scrolls the chat and stops following, a swipe leaves it following, a
selection copies the long line whole).

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

### S1b · Projects, tasks, claims and events on the server (non-UI) · done 2026-10-04

Branch `codex/server-s1b`, worktree `.tmp/worktrees/codex-server-s1b`, from master `4ed11813`.
**Done 2026-10-04:** merged into master after two independent reviews (security/concurrency and spec/domain)
and a re-review of the fixes (PASS, "ready to merge"). Open minor for S2d: heartbeats emit no events, so a watching
desktop must re-read a task (or the server send a claim-only event) before showing a held claim as stale. The macOS cross-check remains blocked by missing Apple SDK headers.
Evidence: [server-s1b-2026-10-04.md](evidence/server-s1b-2026-10-04.md).

Done when these criteria, the required validation and independent review pass, and the phase is merged:

- Core: `sharing` (claim take, release, take-over, heartbeat, staleness at 15 minutes, the run phases that end a
  claim; tests over explicit times) and `RepositoryIdentity::from_remote_url` (HTTPS, SSH and scp-like forms of one
  repository match; local paths and no `origin` refused).
- Projects: create with upload (numbers kept, counter after the highest), lookup by remote (`403 not_member` with the
  owners), members added and removed by owners and admins only, `404` for non-members; of two simultaneous creates for
  one repository exactly one succeeds and the other gets `409 remote_taken` and changes nothing.
- Tasks: 5 clients creating 50 tasks each get numbers 1–250 with no gap or duplicate; a repeated idempotency key
  returns the first result; a stale revision gets `409 revision_conflict` with the current task; the edit lock and
  queueing follow core's rules; import gives new numbers and keeps ids and former keys. Upload keeps `has_run` for
  finished runs.
- Claims: of two simultaneous claims exactly one wins; a heartbeat refreshes and reports lost claims; stale after 15
  minutes of test clock; take-over refused while fresh or changed and recorded when done; removing a member or
  disabling a user releases their claims.
- Events (SSE): a stream resumed with `Last-Event-ID` receives exactly the missed events; a cursor older than the
  30-day retention gets `reset`; a revoked session or membership closes the stream; keep-alive every 20 s.
  Heartbeats and SSE capacity are bounded; retention and replay use bounded periodic work.
- Organization (user, 2026-10-03; `docs/architecture/server.md` decisions 23–24, *Task organization*): workflow,
  priority, tags, blocking links and the Blocked reason are fields of `Task` and `Changes` under the revision; core's
  ORC-39 rules refuse an empty reason, a cycle, an unknown tag and Canceled while the reported run has not ended
  (`422 organization`); every change writes a history row (person, actor, before and after, tag names) served by
  `GET …/tasks/{task}/history`; upload and import take tags by name and drop (and list) links outside the upload.
  Reuses ORC-39's `task::organization`, already merged into master before S1b began.
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
  10.06 `YMwe5`, 10.07 `BWEZ9`, 10.08 `kT0ui`; the status bar item 10.43 `hYres`. Implemented on `claude/s2a`
  (2026-10-04), waiting for design verification and review; see *Done*.
- **S2b** (share, members, make local, offline, conflict): 10.11 `xnhV2`, 10.12 `IDfLs`, 10.13 `V4ZxQG`, 10.14
  `NaUDV` (with roles), 10.17 `g5r5VU` (discovery), 10.42 `Cc4WH`, 10.44 `Y6H8A`, 10.41 `RmDpQ` (the revision; its live use is S2d).
  Implemented on `claude/s2b` (2026-10-05), waiting for design verification and review; see *Done*.
- **S2c** (join): 10.15 `y8uP2`, 10.16 `AZwJE`, 10.21 `UlHPB`, 10.22 `m9omas`, 10.23 `l9jYZS`, 10.24 `FAKNF`,
  10.25 `RuBhQ`, 10.26 `m7jOn9`. Implemented on `claude/s2c` (2026-10-05), waiting for design verification and
  review; see *Done*.
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

ORC-403 · A ready step that waits for an agent slot says so · branch `orchester/orc-403-run-1` (implemented 2026-10-30;
pipeline QA scenarios, Review and Verify follow). The runner reports ready agent steps held back only by the slots in
`WorkSummary.slot_waits` (protocol 33); nothing is stored. `get_run` gives `status: { state: "pending", waits_for:
"agent_slot" }`, `list_runs`' `detail` names the steps, the Summary's rail and Last event say it, and the Overview's *At
work* and the health card count them and name *Agents at once*. A run waiting for a workspace is left out: more slots
would not start it. A refused `Board::take` no longer announces a summary. **No Pencil frame draws this wait.**
Tests: `orchester-ipc --test framing frames_round_trip`, `orchester-runner --test graph graph_slot_wait`, `--test
workspaces workspaces_a_run_waiting_for_one`, `orchester-workspace --test slot_waits slot_wait`, `orchester-app --test
mcp_domain domain_slot_wait`, `--test status_bar status_health_card`.

**Still for the person to decide: who gets the next free slot.** Nothing about the order changed, so A holds until
they choose. A, keep today's order: nothing changes and the order stays unpredictable. B, a running run first: `admit`
refuses new starts while a step waits for a slot; small, and runs in progress finish sooner, but the queue waits longer
and it does not help the reported case, where the queue is already locked out. C, a queued run first: a parallel step
takes only slots the queue does not need; tasks start sooner, but pipelines look sequential more often. D, the oldest
run first for every waiting step: the board keeps a fair line that `take` and `admit` follow; the largest change, and
the only one that stops a busy run from keeping its slot, which is what the reported case shows. The plan recommended
B, with D as the follow-up if parallel steps still feel sequential.

ORC-100 · Task page redesign: one header, three tabs, Summary gathers the task's info · branch `orchester/orc-100-run-1`
(implemented 2026-10-26; pipeline Check, Review and Verify follow). Design: `Task page · redesign` `C3wC8` (`01 ·
Summary` `AsE7R`, `01b` `HvPSr`, `01c` `SaKMC`, `Decision cards` `CqVv3`, `02` `XIBg8`, `03` `X5ZKG`).
- **Header** (`screens/task/header`, `Task header` `j8DakG`): back row `← Tasks / KEY`, the title with one primary
  action the state calls for (`primary.rs`: Share task; Approve, Answer, Review or Merge while a card waits, left out on
  the card's own tab; else Run, Start run, Resume run or Continue run) and *More actions* (`more.rs`, `JapSp`) with
  every other: Edit task (E), Copy link, Open worktree in Files, Memory for this task, Pause run, Run again, Return to
  a step, closing, then Remove worktree, Cancel run and Delete task in `danger`; unavailable rows keep their reason.
  The status line reads "Interrupted at 03 Implement" and the run's number, attempt and start.
- **Tabs**: Summary · Conversation · Changes (count). Details, the run's Summary and Artifacts became the new Summary;
  Memory and Local integration are views of Summary and Changes (`?view=memory`, `?view=local-integration`, MCP
  `navigate`'s `view`), opened by the properties' memory link and Changes' *Try locally*. Links naming the former tabs
  open where they went; `navigate` refuses them with `unknown_tab`. A task's `link` is `/summary`.
- **Summary** (`screens/task/summary`): a waiting decision's card first (every decision view kept its logic and
  controls), the run (rail of steps, per-step tokens and time in their tooltips, the last event in one bordered line),
  Brief with its version chip, Acceptance criteria, Output, Runs (rows 36 high with their `…` menus); beside them the
  properties (Status, Priority, Pipeline, Tags, Depends on, Limits, Branch, Created; a run-captured value that differs
  says so), the run's facts (`Run N · this run`/`ended`, the rest behind a disclosure) and the memory link.
Tests: `orchester-app --test task_summary`, `--test task_details`, `--test task_decisions`, `--test links`, `--test
mcp_tools navigate`, `--test changes changes_try_locally`, `--test memory`.
**Open:** Output lists the run's step reports only (runs keep no other artifacts yet); the run row's *when* has no
duration; the macOS build was not cross-checked here.

ORC-79 · The Orchestrator chat makes and publishes skills: `save_skill_draft` and `publish_skill` · branch
`orchester/orc-79-run-1` (implemented 2026-10-23; pipeline Check, Review and Verify follow). Spec:
`docs/design/orchestrator-toolset.md`, decision 3 and the Skills row.
- **Domain** (`skills::editor`): the editor's writes name the rule that refused them (`Refusal`, `WriteError`;
  `save_checked_in`, `publish_checked_in`, `SkillForm::checked`; the Messages the window shows are unchanged). Its
  reference file limits are new and shared by the window and the tools: at most `MAX_FILES` (100) files of at most
  `MAX_FILE_BYTES` (1 MiB) each, files a skill already has (an import) kept; `files_in` stores files given as text
  at checked paths (`SkillFilePath`: inside the skill's folder, no absolute, `.`/`..`, drive or `SKILL.md`) and keeps
  the skill's file at a path given without text. `mark_automation_in` marks a draft the tools saved;
  `ProjectSkills::by_automation` reads it (the instructions' `automation_drafts` table). The Skills editor saves
  through `save_by_in`: edits an MCP client's synthetic input made (or a client's press of Create/Retry) are saved as
  automation's and marked, as the Instructions editor does, so a client's window save neither goes unmarked nor clears
  the mark; only a person's save does.
- **Tools** (`app_mcp/tools/setup/skill_input.rs`): `save_skill_draft` (new skill under `name` or a skill's draft by
  id, name or slug; fields left out keep their value; `expected_revision` as `save_tool_draft`; always open, marked as
  automation's) and `publish_skill` (`person_only` as `publish_instruction` per ORC-30, `prompt_changes::skill`;
  refused `blank_description`, `blank_body`, `stale_draft`, `no_draft`, `scope_unavailable`, `draft_changed`).
  `list_skills`/`get_skill` gain `draft_by_automation`, `get_skill` its `revision`. The chat's handling of
  `Publishing::Skill` was already in place; its proposals now also find a skill by its slug.
- **UI**: the Skills editor's header says *Draft written by automation · Published vN* / *· Not published* with the
  Bot icon, as the Instructions editor does (ORC-30). No Pencil frame shows either mark.
- Tests: `cargo test -p orchester-workspace --test skill_tools skill_tools`, `--test skills skills_`, `--test
  orchestrator_tools orchestrator_tools`, `--test i18n locales_`; `cargo test -p orchester-app --test orchestrator`
  (`skills::`), `--test skills skills`, `--test mcp_domain domain_reads`, `--test i18n -- locales area every_key`.
- **Open:** a skill draft an MCP client types into the window's Skills editor is not marked (the Instructions editor
  marks synthetic input; the Skills editor saves on its own and has no author tracking).

ORC-78 · Modules in shared projects: made, changed and synced through the team server · branch `orchester/orc-78-run-1`
(implemented 2026-10-22; pipeline Check and Review follow). Spec: `docs/architecture/server.md`, *Modules*, rows 66–71.
- **Server** (schema 7): `modules` under a revision (names unique per project without regard to case, core's
  `ModuleDetails` rules), `tasks.module` changed under the task's revision with every move kept (`module_changes`,
  append-only, with actor), `change_keys` answering a repeated module create or change, the `modules` event kind (the
  `events` table rebuilt with its sequence carried over). `GET …/modules?after=`, `POST …/modules`, `PATCH
  …/modules/{module}`; `409 revision_conflict { current }`, `409 module_exists { module }`, `422 module { reason }`;
  link and import uploads carry modules, each kept by id or mapped onto the project's module of its name, and record
  each uploaded task's move. API types in `orchester-api` (`modules.rs`, `client/modules.rs`; `chrono` added to the
  crate for the days, already built for core).
- **Desktop**: `TaskWriter` sends a shared project's module creates, changes and task moves through
  `RemoteTaskStore` (`sharing/project/modules.rs`), so the Modules page, the task form's and task page's module picker
  and the domain tools (`create_module`, `update_module`, the module of `create_task`, `update_task`) work in shared
  projects; refusals read as core's module errors (`ensure_modules` removed). The cache (`shared_modules.rs`, project
  migration `modules::SHARED_MIGRATION`) keeps module revisions and the server's moves by sequence; sync reads modules
  before tasks and moves after them; Share and Join upload every local module and adopt the server's (a local module of
  a server module's name takes its id, in its tasks and moves); Make local keeps them. Offline, module writes fail at
  once as task writes do (the person, 2026-10-22: no outbox until S4); no visible change, so no design verification.
- **The ORC project's `sharing` row** links to a live server: `https://orc.lurq.dev/orchester` (server
  `01a1590b-1605-7209-82c6-e7fc39b47e91`, server project `01a14a4f-889b-729f-8f44-6ca7164b63b6`), role owner, joined
  2026-10-20, last synced 2026-10-22 20:45 UTC, not removed; all 78 tasks are the server's. Not stale; left unchanged.
- Tests: `cargo test -p orchester-server --test sharing` (`modules` and the rest), `--test accounts host_locks`;
  `cargo test -p orchester-workspace --test shared_modules shared_modules_`, `--test shared_projects shared_`, `--test
  joined_projects joined_`, `--test joined_share_task joined_`, `--test shared_statuses statuses_`, `--test modules
  modules`; `cargo test -p orchester-store --test modules modules`, `--test shared_tasks shared_tasks`, `--test
  joined_tasks joined`, `--test task_creation_snapshots creation_snapshots`; `cargo test -p orchester-api --features
  client --test client sharing`; `cargo test -p orchester-runner --test pipeline_pin pin`; `cargo test -p
  orchester-app --test mcp_domain modules`.
- **Open:** with S4 the module entries of the outbox (row 71); with S2d the `modules` event and a `task` event's module
  applied from the stream.

ORC-19 · Delivery: merge queue and Deliver step · branch `orchester/orc-19-run-1` (implemented 2026-10-23; pipeline
Check, Review and Verify follow). Built against `data/orchester2.pen` layer `p9aqZc` *Delivery* (not `i2o8ko`); see
the design doc's Delivery › Built for what differs.
- **Domain**: `Pipeline.deliver` (`DeliverStep`: Approve to merge / Auto-merge, merge commit / squash, integrator,
  re-check locked on for Auto-merge; Pull request *Coming later*); `orchester_core::merge_queue` records, store
  `merge_queue` + append-only `merge_queue_events`, `run_merges.kind` `squash`; `orchester_workspace::merge_queue`
  (join, advance one step at a time: sync, Check on the merged result, Integrate via the run's own step, approval,
  land with clean-up; at most 3 re-syncs; dirty checkout blocks; move up / to top / remove; approve, reject, resolve
  myself, resolved, requeue). The final merge compares the checked base again under the integration lease
  (`Delivering`); approval is person-only and void once the run works again.
- **UI**: Merge queue screen and sidebar count; the pipeline editor's Deliver card, chip and inspector; the task
  Summary's delivering / integrating / review (Changes, Commits, Checks) / integrate failed / resolve myself (with
  its checks panel: checking, markers left) / merged views and Run details' Delivery rows; the Conversation's lines
  of what the queue did (`tasks::timeline::delivery`, `Urfxj`); Needs you cards (Ready to merge, Conflicts need you,
  Delivery stopped, Delivery blocked); Recent activity lines. QA seeds `delivery`, `delivery-blocked`,
  `delivery-empty`.
- **MCP**: `get_run`, `list_runs`, `get_daily_report` report delivery; `merge_run` refuses a run the queue delivers.
- Tests: `cargo test -p orchester-workspace --test merge_queue merge_queue`, `--test needs_you`, `--test overview`;
  `cargo test -p orchester-app --test merge_queue merge_queue`, `--test screens`, `--test i18n`.
- **Open:** task header status chip and *Open merge queue* in the header, per-file *Open file*, OS notifications for
  delivery, the 960 frame `MIQ3j`, a project base-branch setting; the Integrating state holds the queue while the
  Integrate step's agent waits for a person; a failed check's output reaches the Orchestrator's run reads as printed.

ORC-50 · Delivery charts and a delivery forecast · branch `orchester/orc-50-run-1` (implemented 2026-10-22; pipeline
Check and Review follow).
- **Domain** (`orchester_workspace::delivery`): facts read from the store only (`DeliveryFacts::read`: each task's
  creation, workflow history, first agent start, latest run merge in Orchester or found merged outside it by git, story
  points and attempts; each step execution's active time and whether a later attempt returned its work); throughput
  per day (per week past 31 days) with points, cycle time (first agent start → merge) and lead time (creation → merge)
  with median, quartiles and 85th percentile, active time and return rate per pipeline step with attempts per merged
  task, and cumulative flow by workflow status. The forecast (`delivery::forecast`) is a Monte Carlo of 2,000 trials
  over the tasks merged on each of the project's last 30 whole days (not before its first task), seeded with a fixed
  seed, giving 50 % and 85 % dates for the queued tasks and each open module; with fewer than 10 days or 3 merged tasks
  it says *not enough history yet* and how much there is. Counted in tasks, the burndown's unit; no money.
- **UI**: the Delivery view beside Usage (`/project/<id>/usage/delivery?period=30d|90d|365d`, the *Usage · Delivery*
  switch in both headers; `navigate` `usage` with `view` and `period`): the forecast table, Throughput beside Cycle
  time, Where time goes beside Cumulative flow, each with its table view and empty state, built from Usage's cards,
  plot pieces and tables. A module's burndown draws the 50 % and 85 % lines from today's remaining work and its days
  run on to the 85 % date; the card says when it is likely done or that history is too short.
- **Tools**: `get_delivery` (series and forecasts, `module` narrows them; in the Orchestrator chat's tools) and
  `get_module`'s `forecast`.
- Tests: `cargo test -p orchester-workspace --test delivery delivery`, `--test modules modules`, `--test
  orchestrator_tools orchestrator_tools`; `cargo test -p orchester-app --test links links`, `--test screens screens`,
  `--test modules modules`, `--test mcp_domain -- domain_delivery domain_modules`, `--test mcp_server
  tools_answer_from_the_window`, `--test i18n -- locales area every_key`.
- **Open:** no Pencil frames exist: the Delivery view, the *Usage · Delivery* switch in the Usage header and the
  burndown's forecast lines, legend, table columns and note need frames and a design verification. Points-based
  throughput is shown, but the forecast and the burndown count tasks (`modules::measure` still counts every module in
  tasks after ORC-49).

ORC-71 · Opening a chat doesn't freeze the app: profile and fix the main-thread work · branch `orchester/orc-71-run-1`
(implemented 2026-10-22; pipeline Check and Review follow). Measured with lurq profile sessions (`perf_profile`, now a
feature of `orchester-app`, which QA copies are built with) over headless openings of the `long-chats` QA seed's size
(two Orchestrator conversations of 300 turns with long Markdown replies and 50 cards; KON-12's run 1,500 events
longer, ~320 rows): `tests/orchestrator/open_long.rs`, `tests/conversation/open_long.rs`, helper `tests/support/profile.rs`,
which writes each opening's frames and lurq samples to `.tmp/orc-71/profile/<name>-<debug|release>.txt`. Numbers below
are release builds on this machine, a frame being delivering finished futures plus the pass.
- **Orchestrator chat, opening and switching in History** (unchanged by this task, lurq's): the frame that shows the chat
  takes ~200–225 ms (~70–84 ms building rows, ~100–145 ms laying them out), then one ~50–65 ms frame. Cause: lurq 0.49.1's
  `VirtualizedList` renders its first 64 rows (`BOOTSTRAP_ROWS`), from the top, before it has measured one, also after a
  switch (no key survives, so its heights are cleared); a list that opens at its end pays for 64 long turns at once.
  Repro: `tests/ui_kit/virtualized_list.rs` (300 rows 100 high, `scroll_to_bottom_pending`: rows 0–63 render on the
  first pass, then the end). Not worked around in the app; misses the targets by ~100–125 ms (first frame) and ~150–175
  ms (longest frame). The screen's own render with 300 turns is ~1 ms; a keystroke frame ~5 ms.
  **Fixed in lurq 0.50.0 (2026-10-22):** the list renders its last 16 rows on the first pass and none from its top;
  the repro now asserts that. Release timings not measured again yet.
- **Idle relayout, lurq's:** a Markdown document with a followed link leaves the layout dirty after every pass, so a chat
  showing a reply with a link lays the window out on every frame (~2–3 ms release, ~2–6 ms debug), forever. Repro:
  `tests/ui_kit/markdown_link_layout.rs`. **Fixed in lurq 0.50.0:** the document settles; the repro asserts 0 dirty passes.
- **Task Conversation, fixed:** before, the opening frame took 65–67 ms: the tab rendered twice in it (~18 ms each), each
  time building the whole timeline (~1–2 ms) and rendering again every row lurq had bootstrapped, because the tab
  provided `Decisions`, the composer's `Composing` and the rows' `Timeline` as contexts in `render`, and lurq renders
  every descendant again when a component provides during render. Now the timeline is built only when its inputs change
  (`conversation/built.rs`), its rows are shared `Arc`s, and the three values are provided once from `create`, the
  changing two as signals set when they change (`conversation/shared.rs`). After: the opening frame takes 47 ms (first
  frame within ~100 ms of the data, no frame over 50 ms), then 8 ms and 3 ms. No visible change.
- Not changed, measured small: card fact reads (only built cards read), follow-to-end (settles in 1–2 frames), reads in
  `render` (none block; the conversation's clone is ~0.3 ms). The task page frame (`TaskFrame`) still provides four
  contexts in `render`, so its re-renders re-render the whole tab: not on the opening path measured here; left as found.
- Not done: a side-by-side profile of real QA windows. A QA copy on `long-chats` starts with `perf_profile`
  (`lurq_profile_read`: available) and opens the 300-turn Orchestrator chat, but `lurq_profile_end` of even a 30-sample
  session exceeds `call_run_app`'s answer limit, so its samples could not be read back; QA copies are debug builds and
  run one at a time. Base-moved notices and step results are not in the long run's seed.
- Tests: `cargo test -p orchester-app --test orchestrator -- orchestrator_follow orchestrator_scroll orchestrator_open_long`,
  `--test conversation -- timeline_`, `--test decision_cards -- card_`, `--test composer -- composer_`, `--test ui_kit --
  virtualized_list markdown_link_layout`, `--test seed -- seed_every_scenario seed_names_every_scenario`; the release
  profiles with `cargo test --release -p orchester-app --features perf_profile --test orchestrator --test conversation
  open_long -- --test-threads=1`.

ORC-76 · A busy project database doesn't interrupt runs · branch `orchester/orc-76-run-2` (implemented 2026-10-22;
pipeline Check and Review follow). Most of it was on master already; this run adds the missing tests.
- **Already covered.** Long writers found, timed and shortened by ORC-74 (below): the cause was the log flush each
  commit made under the write lock (`synchronous = FULL`), not long SQL; the longest hold went from 24–135 ms to
  14 ms in the stress test, the longest wait from 1.1 s to 33 ms. Every hold is logged at debug level under
  `orchester_store::lock` with its first write and how long it waited, a hold of 1 s or more as a warning
  (`lock_watch`). No `VACUUM` or `ANALYZE` runs, the codebase index is files, not rows, blob files are written before
  their transaction, and a transaction holds only SQL. Retries: every connection's busy handler waits up to 60 s
  (growing pauses 1–50 ms), and the run's log restarts its whole transaction on a busy error after it began, logged
  as a warning; only past the minute is a run interrupted (`StoreError::Unavailable`, ORC-75's
  `storage_contention`, which the Orchestrator may continue). 60 s: thousands of times the longest hold measured
  under eight runs, so a healthy database never reaches it, and short enough that a lock held for good (a hung
  process) still surfaces. The runner pauses those waits off its async workers (`claude/db-reconcile`). Checkpoints:
  each commit checkpoints `PASSIVE` once the log holds 1000 pages, SQLite's default, and logs one that readers held
  back; a read outside a transaction ends with its statement.
- **Added.** `orchester_store::fail_next_writes_of_for_test(what, codes)`, a process-wide injected busy error for a
  named transaction, so a runner test can reach the runner's own threads. Tests: `crates/store/tests/
  parallel_writers.rs` (six writers in parallel outlast a 6 s holder of the lock and store every line once; a reader
  that keeps its connection, as the chat does, leaves a `TRUNCATE` checkpoint free to empty the log) and
  `crates/runner/tests/db_busy_retry.rs` (a run's event write meets `SQLITE_BUSY` then `SQLITE_BUSY_SNAPSHOT`, is
  retried and logged, and the run succeeds with no runner failure).
- **Found, not changed.** The status bar's *Stop* auto-approving (`stop_auto_approving_in`) reads every task and its
  runs inside its `BEGIN IMMEDIATE`, by design so no run starts auto-approving in between; it grows with the project
  and runs only when a person presses Stop. The runner logs since 2026-10-22 were not read: Orchester's tools refuse
  `.orchester/logs`.
- Tests: `cargo test -p orchester-store --test parallel_writers parallel_writers`, `--test busy_writes busy_writes`;
  `cargo test -p orchester-runner --features test-agent --test db_busy_retry db_busy_retry`.

ORC-74 · Runs don't fail on "database is locked" · branch `orchester/orc-74-run-1` (implemented 2026-10-21; pipeline
Check and Review follow). The rules are in `docs/architecture/storage.md`, "Concurrency".
- **Cause.** Every connection had `busy_timeout` 5 s and wrote in `BEGIN IMMEDIATE` transactions; no transaction ran
  long SQL. The write lock was saturated: in WAL mode with SQLite's default `synchronous = FULL` each commit flushed
  the log while holding the lock, and on Windows a flush waits for what the whole drive still has to write. With six
  or seven runs and their agents' builds on the drive of the project, writers queued past five seconds and got
  `SQLITE_BUSY` ("database is locked"), which interrupted the runs with `runner_failed`. Measured with the new stress
  test (`crates/runner/tests/db_contention.rs`: eight fake-Codex runs of Implement and Review flooding their
  transcripts, the app's, chat's, base-move follower's and plan-limit reads, settings writes and a build writing and
  flushing beside them) on drive H:, like the real project: before, write transactions held the lock 39–61 % of the
  time, the longest 24–135 ms (`INSERT runs`, `INSERT agent_events`), and a writer waited up to 1.1 s (an earlier
  measurement that still counted checkpoints as holds: 72–81 %, 269–278 ms, 2.5 s); on C: the holds were only
  4–6 ms. No deferred transaction wrote and no connection lacked a timeout.
- **Fix.** `synchronous = NORMAL` on every connection that writes: after, 7 % of the time, the longest hold 14 ms,
  the longest wait 33 ms, and the eight runs settle in 5.4 s instead of 6–8 s. Every connection's busy handler
  (`orchester_store::busy`) waits for a lock with short growing pauses for up to a minute; the run's log (events
  with step results and checkpoints, transcript, invocations, step outputs) also restarts its whole transaction on
  a busy error after it began (`SQLITE_BUSY_SNAPSHOT`, `SQLITE_BUSY`) within that minute, logged as warnings. Past
  it a write fails with `StoreError::Unavailable`, and the run is interrupted with that reason. Store errors name
  the lock's extended code. Blob files of diffs are written before their transaction begins. Each transaction's
  hold of the write lock, each lasting wait, each retry and checkpoints that readers held back are logged under
  `orchester_store::lock` (`lock_watch`).
- **Not done.** The app shows no "busy, retrying" state (a visual change, left for a task with its design): its
  writes wait for the lock up to the minute in whatever state they show while working, then show that the database
  is locked as before. The secret replacement writes the OS keychain inside its write transaction (by design,
  ORC-7); it is quick but not SQL.
- Tests: `cargo test -p orchester-runner --features test-agent --test db_contention db_contention`, `--test
  db_unavailable db_unavailable`, `--test store_refusal store_refusal`; `cargo test -p orchester-store --test
  busy_writes busy_writes`, `--test lock_budget lock_budget`, `--test contention contention`.

ORC-62 · Loader spinner turns at 24 frames a second instead of 12 · branch `orchester/orc-62-run-1` (implemented
2026-10-20; pipeline Check and Review follow). `LOADER_STEPS` is 24 with `LOADER_TURN` kept at 1 s: 24 poses 15° apart,
still stepped (`JumpEnd`) on lurq's one in-phase clock for every loader of every window, and a window without a loader
in its tree asks for no frame.
- **Turn rate.** The person saw about 12 turns a second. Sampled headless against lurq's animation clock for 1.25 s
  (`tests/ui_kit/loader.rs`), every loader shows the pose the clock gives: 12 poses 30° apart in one turn a second
  before, 24 poses 15° apart in one turn a second after. lurq 0.49's step easing, keyframes and decomposed rotation
  compute one turn a second, so no lurq fault was found; the pixels a running app draws were not sampled.
- **A/B on this machine** (2026-10-20): two release builds of `ui_gallery` with lurq's `perf_profile`, 12 and 24 steps,
  side by side on its Actions page (a loading button), over the same 30 s: CPU 1.750 s (5.83 % of one core) with 12
  steps, 1.734 s (5.77 %) with 24. Frames per second were not counted: `call_run_app` refuses the gallery copies, and
  `start_qa_app` runs one QA copy at a time, built without `perf_profile` (the `tasks` seed shows no loader on the
  Tasks list either: its running runs read as interrupted without a live runner). The person watched the copies, saw
  the 24-step loader turn faster and accepted it. **Open:** equal CPU for 12 and 24 steps is unexplained without frame
  counts.
- **What a frame redraws.** The whole window: while a loader turns, each frame skips lurq's layout fast path and runs
  the layout update (the nodes' layout caches stay valid, since a transform changes no size), resolves every quad of
  the window and presents the whole surface; lurq 0.49 has no damage regions.
- **Hidden windows.** A minimised window gets no frames and keeps its clock (`FramePacing::request_frame`), as does a
  window macOS reports occluded. On Windows a window that is covered or off-screen but not minimised keeps turning,
  since winit reports no occlusion there.
- Tests: `cargo test -p orchester-app --test ui_kit -- loader buttons blocks labels`, `--test status_bar -- activity
  server items`.

ORC-55 · Plan limits read on their own, not only when an agent turn reports them · branch `orchester/orc-55-run-1`
(implemented 2026-10-20; pipeline Check and an independent design Verify still follow). Codex accounts are read through
App Server's `account/rateLimits/read` outside runs, per account (its CLI and `CODEX_HOME`), with no thread, turn or
token (`plan_limits::read`, `orchester_exec`'s `LineExchange`: App Server ends at the end of its input without
answering). Claude Code 2.1.296 has no turn-free read; the person chose an explicit, costed *Check now* per Claude
account (a one-word Haiku turn, isolated like auto mode's reviewer), never automatic. Reads happen at start, on an
account change, when the status bar's hint or the Usage page opens, once after a known reset and every 5 minutes only
while one is shown, backing off after failures (`plan_limits::schedule`); only the app's own window starts the reader,
never a test or QA copy. A reading keeps its time and source (`ReadingSource`), reads are kept 90 days in
`orchester.db` (`plan_limit_reads`) beside the turns' reports; the hint says *as of 2 min ago · read* / *· from a
turn*, the account details *read …* / *reported … · from a turn*, the Usage table and tooltip *Read by Orchester*;
`list_accounts` returns each account's latest reading with source, `observed_at` and `age_seconds`. Docs:
`docs/design/agent-accounts.md` (*Plan limits read on their own*), `docs/design/orchester-workspace.md` (*Data
sources*). Tests: `cargo test -p orchester-core --test plan_limit_reads plan_limit_reads`, `-p orchester-store --test
plan_reads plan_reads`, `-p orchester-exec --test exchange exchange`, `-p orchester-workspace --test plan_reads
plan_reads`, `-p orchester-app --test status_bar plan_limits`, `--test mcp_domain domain_accounts`, `--test
settings_accounts settings_accounts_check_now`. Not drawn in `orchester2.pen` yet: the hint's age line, *Check now*
and its error.

ORC-31 · Person-only decisions refuse synthetic input everywhere · branch `orchester/orc-31-run-2`
(implemented 2026-10-18 in run 1, carried onto master in run 2; pipeline Check and independent design Verify still follow).
A shared workspace policy requires the approvals consent for approvals, questions, plans and stuck/retry/accept
choices, independently of permission-setup consent. The window and domain tools use it; synthetic decisions record
automation, native decisions record the person. Plan/retry/return/stop/accept audit events and persisted free-text
answers retain their decider, and conversation captions name automation. Native acceptance of an Orchestrator
proposal retains the person's identity. The frozen deprecated desktop is unchanged; its legacy IPC entry remains.
Filtered core replay, storage, IPC, shared-policy and headless control tests cover the change. The new headless
`mcp_domain person_decisions` group covers every changed control and all four domain tools, including refusal,
consented automation and native authorship. `tools/check.ps1` is delegated to this run's Check step. macOS is not
cross-checked on this machine (no macOS SDK/C cross-compiler); no new target-specific code was added. Verify the
plan bar (`G5378f`), composer (`QQ2ZM`), retry card (`j7DjNG`), conversation cards (`YhiGC`) and Needs you (`HQ6Y1`).
Accept/unverified still has no dedicated Pencil frame (D2); the verifier must report that limitation explicitly.

Review corrections (2026-10-18): outgoing consent follows the message's actual effect on waiting agents,
including Send after leaving a question's composer context. Attributed retry/return/stop/accept commands carry
the displayed run event sequence (IPC 28); a different wait rejects them before any transition or Accept revision
check. A failed task reread replaces retained controls with the existing load-error surface until fresh data
succeeds, including during retries. Regressions cover the leave/send path, retained-data read failures and
decisions from a previous wait. The failed-reread surface and Accept still need the verifier to identify missing
design frames explicitly. Sending an undelivered message again (*Send after this turn* and its variants) follows the
same rule: when the resent message would answer a free-text question its agent waits on, an MCP client's press needs
the approvals consent, and an automation resend is recorded as automation's message (`send_after_turn_as`).

Command classifier: compound lines and dependency sources · branch `claude/command-classifier` (2026-10-14).
From the analysis of 16 runs (106 of the person's 134 approvals were Claude Code's compound lines read as Unknown).
`echo` of `$?` / `${PIPESTATUS[N]}` (PowerShell `$?` / `$LASTEXITCODE`) is read, and a status anywhere else stays
Unknown. `echo` prints any argument. `tee` without a file is a filter. A literal variable may hold a backslash, read
only where it is used quoted. Reads of Cargo's registry sources and git checkouts and of the toolchains' rust-src
(also under `CARGO_HOME` / `RUSTUP_HOME`) are read like the worktree, as are `read_external_file` and
`search_external_file` there; the rest of a Cargo or rustup home stays a credential location. Auto mode already
reviewed Unknown lines (2026-10-06), and its hard rules are unchanged. Tests:
`cargo test -p orchester-core --test command_compound command_compound`, plus `command_sequences`,
`approval_fixtures` and `native known_safe`. Still Unknown by design: `$( )`, subshells, functions, `for … set --`,
`**` globs, a loop that is not the whole line.
Security review fix (same day): only plain readers (`cat`, `rg`, `grep -r`, `sed -n`, …) read the dependency
sources like the worktree; a `cd` there, any other program, and a `.git` there do not. Programs that load settings
or code from their folder (git, Cargo, package managers, build tools, Python, Node) are Unknown outside the worktree
and its writable folders, which also closes `cd /any && git status` on master. `diff -r`, `tree -l` and `du -L` are
Unknown. Tests: `cargo test -p orchester-core --test command_review_bypasses command_review`.

ORC-3 · Pipelines page and editor · branch `codex/orc3-integration` (accepted 2026-10-12, merged `39f945d2`).
Independent source review accepted final union `7668c94a` / tree `f7e145f2`; eight named-filter groups /
83 executions and `tools/check.ps1` passed. Pipeline writes retain same-snapshot CAS, immutable published
versions, reload locking, dirty/error preservation, and actual-resource links to Agents. The actual
publication receipt no longer obscures in-place decisions. Independent native/Pencil verification found
no blocking or major mismatch in reached action/recovery regions at 1440 dark. Final Publish v3,
Ask-off Save decline/confirm, Agent-to-Tool Save decline/confirm, Leave+Keep and History Restore outcomes
were permitted automation under the person's actual saved opt-in, with full read-only storage checks;
historical human checks keep their original revision boundaries. Explicit Restore settled through normal
checked autosave; a later stale element-ref Save error is retained, not reported as a successful click.
Post-merge workspace/all-targets/all-features cargo check and check.ps1 passed. Recorded visual minors
remain; native races/fault timing, separate Ask publication, other screens, macOS, 960/light, actual CLI/
runner execution and installed-version verification remain Unverified. Full test suites were not run.
The original run's exhausted Verify history is retained; domain workflow completion awaits the installed app.
Evidence: `.tmp/orchestrator/orc3/` and `.tmp/verify/orc3-policy-union-f7e145f2/report.md`.

Automation permissions switch (accepted 2026-10-12, merged `d50b240f`, source `4a55abd5`).
Native setup widening decisions now honor the person's saved permission opt-in, using the same policy as
domain writes and rechecking consent at execution time. Synchronous revocation and request attribution
remain enforced. Settings opt-ins, stale Agents publication and Secrets replacement remain human-only;
Auto mode/profile changes also require the independent approvals opt-in. Official modern lurq 0.43 is used;
the deprecated app retains 0.42 until its separate migration. Independent source and scoped native/Pencil
reviews passed, with actual Tool Save decline/confirm readback; nine named-filter groups /121 executions,
check.ps1 and post-merge all-target/all-feature cargo/check gates passed. macOS, full suites, other native
editors and per-invocation relay coverage were not performed. This completes the focused consent bug fix,
not every unrelated A4 follow-up. Evidence: `.tmp/orchestrator/automation-permissions/`.

ORC-2 · Agents page · branch `orchester/orc-2-run-4` (done 2026-10-11, merged `500f9965`).
Independent source review accepted integration tree `4a0021f3`; 23 filtered groups / 134 executions passed
on its unchanged production predecessor, followed by six inventory-correction checks and `tools/check.ps1`.
Independent scoped Pencil/native verification found no blocking or major mismatch in reached Agents and
shared-field states. The person completed stale publication decline/confirm and permission-widening
allow/decline; native and read-only storage outcomes were independently accepted. Post-merge workspace/
all-targets/all-features `cargo check` and `tools/check.ps1` passed. Recorded visual minors remain.
Native failed-refetch/fault/loading, other capability/model/menu combinations, active-run usage, host/real CLI
and macOS remain Unverified; 960/light, full suites and per-invocation relay coverage were not performed.

ORC-9 · Project settings page · branch `orchester/orc-9-run-1` (done 2026-10-11, merged `fc9d0c2c`).
Independent source review and scoped Pencil verification accepted the exact integration tree `a9d07d87`;
141 tests in 20 filtered groups and `tools/check.ps1` passed. The person completed Environment widening,
Auto On and a different offered Reviewer choice, each with decline and confirmation; native readback,
read-only persistence and input release were independently assessed. Post-merge workspace/all-targets/
all-features `cargo check` and `tools/check.ps1` passed. Nine visual minors remain. Unsupported active-run
authority, native faults/retry/Reload, host behavior, real vault/clipboard and macOS remain Unverified;
960/light and full test suites were not run. Visibly unavailable sections retain their scoped disposition.

Textarea click focus (done 2026-10-11): multiline FieldInput owns its full visible input area, including blank body and padding. Sixteen filtered regressions and check.ps1 passed; independent code review and ten native coordinate-focus checks passed. Shared dimensions remain unchanged; the existing local retry-note 84/58px Pencil discrepancy and macOS/native compact/locked states are outside this focused fix.

ORC-7 · Secrets page · branch `orchester/orc-7-run-1` (done 2026-10-11, merged `2b61296d`).
Independent code review and re-reviews resolved native/retained references, concurrent binding and post-vault COMMIT error reporting. Eleven targeted regressions passed independently; 50 integration regressions and required post-merge cargo check all targets/features and check.ps1 passed. Independent scoped Pencil verification found no blocking/major in inspected regions; nine minor findings remain. Exact historical visual evidence plus unchanged visual-source delta and fresh candidate production captures are recorded in the run worktree reports. Person-only/no-frame states, native post-vault COMMIT painting, real keychain/clipboard/navigation and macOS remain Unverified; 960/light excluded. Integration resolution independently reviewed with both Tools and Secrets variants preserved.

ORC-6 · Command Tools page · branch `orchester/orc-6-run-1` (done 2026-10-11, merged `08d8c007`).

The new app lists and edits project command tools. Save, stale-draft resolution and Publish check the expected
resource and permission widening inside the same immediate transaction as the write, preserving concurrent edits.
Independent review and re-review passed after reproducing and fixing a lost-update race; 38 filtered tests, the
original reproducer and `tools/check.ps1` passed. Post-merge workspace/all-targets/all-features `cargo check` and
`tools/check.ps1` passed. Independent scoped Pencil verification found no blocking or major mismatch in reached
command-tool states; Built in remains Unverified, with recorded visual minors and real-person acceptance omissions.
Full tests, macOS checks/runtime and fresh native captures of the domain-only transaction fix were not run.

ORC-23 · Agent tool: drive the app a run started, without asking (done 2026-10-11, merged `b2fa04f8`).
The person independently reviewed the exact integrated tree `73cf26b9`: PASS, no blocking findings.
Seven filtered integration groups passed (66 executions with overlapping filters, one intentional ignored entry),
as did `tools/check.ps1`. Post-merge workspace/all-targets/all-features `cargo check` and `tools/check.ps1` passed.
Full suites, fresh native macOS checks/runtime, GUI QA and live signed-in CLI runs were not performed here.
Original run: `01a12acf-2d86-700e-8094-befc53b4e0b5`; historical implementation evidence remains scoped below.

`call_run_app(pid, tool, arguments)` is served by the per-invocation relay to both CLIs without approval, including
read-only steps with network off and MCP approvals enabled. It reuses ORC-21's process validation, finds exact-pid
user or QA discovery, validates its pid/app/loopback endpoint, holds the credential only in the relay and redacts
returned text and structured content. PNGs are decoded and re-encoded without metadata under `.tmp/run-app/`, with
relative paths returned. JSON and SSE responses are bounded; redirects and proxies are disabled. Prompts, the
approval documentation and the design-verification skill steer agents to the relay instead of the Python script.
Core refusal/redaction tests and both CLI relay tests with a started stand-in pass; core and executor pass macOS
cross-check and cross-clippy. The runner macOS check is blocked by missing target C tooling in existing native
dependencies. No visual states or Pencil nodes changed. Commands and limits: `docs/evidence/orc-23-run-app-validation.md`.

ORC-21 · Agent tools to stop apps started in a run (implemented 2026-10-11 in run worktree
`01a127bc-59bd-774a-8edd-a0f0bc989311`; implementation checks passed, independent review/QA follows).

The per-invocation relay provides `list_run_processes` and `stop_run_process(pid)` to both agents without approval.
Executable paths resolve inside this run's worktree; outside processes, the runner and agent CLI are refused.
Descendants inside the worktree are stopped with the root; descendants outside it (such as `conhost.exe`) are left
running and named. Windows sends WM_CLOSE and macOS SIGTERM,
then a bounded grace period precedes forced termination. Prompts and `docs/interactive-tool-approvals.md` steer
both agents to these tools. Core path/permission tests and executor/runner relay tests passed; macOS is checked
and linted for `aarch64-apple-darwin`, with runtime QA on a Mac outstanding. No visual states or Pencil nodes changed.
The person separately approved the small `tools/check.ps1` change: check formatting per workspace manifest with
the same nightly flags to avoid Windows's command-length limit. Full gate result and each test command are recorded
in `.tmp/orc21/verification.md`.
Review corrections retain exited parents during child discovery, rescan after termination, and use macOS audit
tokens to bind executable lookup and signals to an exec generation. Exit races are treated as disappearance.
Regression tests cover children born during close, listing churn and macOS stale tokens/exits; the latter are
cross-checked here and need runtime QA on a Mac. Fresh correction evidence is in `.tmp/orc21/review-fixes.md`.
Security review fixes (2026-10-11): outside descendants are left running instead of refusing the stop; the
reported executable path must lie inside the worktree before links are resolved; after the first close request each
process gets an outcome and nothing aborts the stop; refusals show no path; macOS before 15 signals by pid after
re-checking the audit token, since `proc_signal_with_audittoken` first ships in macOS 15.
Re-review: only processes started at or after the run's creation are listed or stopped, since a running executable
moved into the worktree is reported at its new path.

ORC-22 · Codex ends its turn while an Orchester tool call waits for the person
(`orchester/orc-22-run-1`, 2026-10-11; review findings fixed).

The runner keeps the process and relay alive across a model turn's completion
while an Orchester call waits, then collects its result in the same thread,
invocation and attempt. Pause, cancel, runner loss and shutdown preserve the
attempt and interrupt the decision without granting it. Approval results remain
eligible for collection after their MCP item completes; a synchronous approved
result receives one continuation without repeating tools. Continuations are
bounded: one needs a relay call returned or a decision settled since the last, so a
call Codex never completes cannot start turn after turn; a failed or interrupted
turn is never continued; at most three follow in a row without a decision or a call
the runner waited for; a requested pause checkpoints instead. A decision a yielded
cell asks for its ended turn (a command approval) reaches a person, and no
continuation starts while it waits. `codex_tool_wait` (13) and
`codex_tool_wait_bounds` (9) cover delayed and fast approval and denial, late
turn-start replies, stops, the bounds and the yielded cell's command; Claude's
approval tests pass.
Final independent review fixed command completion and permission-checkpoint races:
commands keep the invocation alive, Apply now checkpoints before continuation,
and withdrawing Apply now releases it. The final 34 filtered runner tests and
`tools/check.ps1` passed; four new ordering regressions fail on the original code.
Cause and design: `docs/interactive-tool-approvals.md`, ORC-22 section; earlier
validation: `docs/evidence/orc-22-tool-wait-validation.md`.
The person approved a separate small `tools/check.ps1` change to check formatting
per workspace package, avoiding Windows error 206 in long run worktree paths.
No signed-in live agent run or UI change; the runner's macOS cross-check remains
blocked by the environment's missing C compiler.

New app, first working set (merged 2026-10-09/10 into master; each reviewed, QA'd live and design-verified by
separate agents): T1 Tasks list (`claude/app-tasks`), F2 shell live data (`claude/app-shell`), P1 task page and Summary
(`claude/app-task-page`), N1 New/Edit task with the per-field conflict chooser (`claude/app-task-form`), P2 Summary
decisions and merge (`claude/app-decisions`), C1 conversation timeline (`claude/app-conversation`), C2 composer
(`claude/app-composer`), C3 decision cards wired to the composer (`claude/app-cards`), D1 Changes tab (`claude/app-changes`), M1 domain MCP and setup tools
with the person-only switch *Let automation change agent permissions* (`claude/app-domain-mcp`, five security
review rounds), LIM optional limits with no maximum (`claude/limits-optional`, protocol 22), I1 install of both apps from one
build (`claude/app-install`; `tools/install.ps1 -InstallDir E:\projects\orchester`, back up `orchester.db` and each
project's `.orchester\orchester.db` first, see `docs/stable-build.md`).

M12 · Agent file tools, second version (`claude/agent-tools`, merged 2026-10-10, three security review rounds) and its
follow-ups (`claude/agent-tools-fixes`). A live run with the real Claude Code 2.1.286 and Codex 0.159.2 passed: both
used only Orchester's file tools (no built-ins, no shell), edits stayed in the worktree, outside reads asked and
source reads did not, `run_check` worked, approvals continued the run, no secret reached logs or the database.

Who decided a plan · branch `claude/plan-by` (merged 2026-10-08, reviewed, tests
passed, design-verified)

Plan decisions record who made them, the person or automation through Orchester's MCP server (`Answerer`):
`PlanReview::by` in core, `Command::DecidePlan { by }` (protocol 20), `plans.decided_by` (migration `plan_decided_by`:
existing decisions read as the person's; triggers tie it to `decision`), recorded by the runner. `decide_plan` and
`send_message` while a plan waits decide as automation, the plan card, the plan review and the composer as the person.
The plan card reads *Approved by automation* / *Changes requested by automation*, the plan review's *Plan version*
*v1 · … by automation* and an earlier note *Automation asked: …*; a person's decisions read as before. `get_run` lists
the run's decided plans and the waiting plan's `earlier` (also in `list_approvals`) shows its step's earlier
decisions, each with `decided_by` (`docs/stable-build.md`; design doc, "Who decided a plan"). Not in scope: the
`PlanDecided` run event carries no `by`. Tests: `orchester-store --test plans plans`, `orchester-ipc --test framing
frames`, `orchester-runner --test plans plans`, `orchester-desktop --test plan_review plan_review`,
`--test form_keyboard app_mcp_decide_plan`, `app_mcp_send_message_requests_plan_changes`,
`composer_requests_changes_to_a_plan`. Design verification outstanding: the plan card after automation's decision
and the plan review's version select with a version automation decided (no frame draws them).

Repository skills fail closed while agent files are off · branch `claude/skills-off` (merged 2026-10-08, reviewed;
tests passed)

A review found that the runner's search for the repository's skills Codex disables while *Repository agent files* is
off (`.agents/skills`, `.codex/skills`; the native agent uses it while the setting is on) followed links out of the
worktree without limit (a committed `.agents/skills/x -> ~` walked the home folder) and had no bounds. Confining it
would let Codex load the skills it missed, since Codex follows links, so it now fails closed in core
(`orchester_core::agent_files::skills`): a folder or `SKILL.md` that resolves out of the worktree is not walked and
refuses the agent's start, as does a folder in the worktree that cannot be read or a search past 8 folders deep or 500
folders below the skills folders. Every path to a folder is followed (a link loop refuses at the depth bound), and each
skill is disabled by every path through the worktree and by its resolved path; Codex 0.159.2 on Windows names a linked
skill by its resolved path and disables it by either (`skills/list` of `codex app-server`, scratch `CODEX_HOME`, no
model turn). `.git` entries are not walked; other hidden folders are. The native agent lists each skill once, reads at
most 64 KiB of a regular `SKILL.md` and refuses one that resolves into `.git` or `.orchester`. Each refusal names the
place in the worktree, never the link's target, and says how to fix it. `list_skills`'s search is unchanged
(`docs/interactive-tool-approvals.md`, "Codex profile"). Tests, not run yet: `cargo test -p orchester-core --test
repository_skills repository_skills` and `native_skills` (symbolic-link, permission and pipe cases on Unix, the junction
case on Windows); `cargo test -p orchester-runner --features test-agent --test codex_repository_settings
codex_repository_settings`, `--test agent_files agent_files`, `--test native native_skills`. Open: Codex's path
matching and hidden-entry rule were checked on Windows only.

Skill tools for agent CLIs · branch `claude/skill-tools` (implemented 2026-10-08, waiting for review)

Codex agents could not read their skills: the prompt listed each placed `SKILL.md` in
`<repo>/.orchester/skills/<execution>/`, which `read_outside_file` refuses as Orchester's state. Orchester's tools
gain `list_skills` and `read_skill {name, file?, offset?, limit?}` (read-only, never ask), served by the relay to
Claude Code and Codex in a step that has skills: the skills placed for the step execution and the repository's
skills the CLI itself loads under *Repository agent files* (Codex's `.agents/skills` and `.codex/skills` while it is
on; Claude Code none), found within the worktree only and within bounds, each read only within its own regular files
(`docs/interactive-tool-approvals.md`, "Orchester's tools"). The review's three majors and four minors are fixed
(links out of the worktree, a `SKILL.md` read before its target was checked, the setting widened for both CLIs). The prompt names skills without paths and
points at `read_skill`. Tests: `cargo test -p orchester-core --test orchester_tools orchester_tools`, `--test run
skill`, `--test run prompt`, `--test claude_invocation claude_invocation`, `--test claude_approvals claude_approvals`;
`cargo test -p orchester-runner --features test-agent --test orchester_skill_tools orchester_skill_tools` (and
`agent_files`, `codebase_index`, `orchester_tools`, `orchester_file_tools`, …). Open: the native agent does not get
Orchester's tools yet (P1d) and reads skills by path with its own `read_file`; not run with the real CLIs; the
file-link case is skipped on Windows without the privilege to make one, and the pipe case runs on Unix only. Not
changed: the runner's search that lists the repository's skills for Codex to disable (agent files off) and for the
native agent still follows links out of the worktree, as Codex would load what they lead to (since fixed on
`claude/skills-off`, above: it now refuses the start).

T5 · Re-measure the code index after ORC-48 (ORC-69) (done 2026-10-07)

Two Claude Code tasks, each run once with *Use code index* on and once off, through Orchester's file tools:
`docs/evidence/code-index-measurement-2026-10-07.md`. The agents called `find_symbol` once per run and opened no
index file; runs with the index spent about 15% more tokens and took longer, within the noise of n = 2. Decided:
*Use code index* is off until a person turns it on (a project that turned it on keeps it), and first-edit tracking
counts edits made through Orchester's `edit_file` and `write_file`, which it missed before (branch
`claude/index-off`). A new measurement needs 3+ alternated pairs per task, one agent at a time, and a record of
which `search`/`list_files` answers came from the index.

Quick rules for Verify steps · branch `claude/quick-rules` (done 2026-10-06)

In 15 minutes after an install, 44 of 218 requests still asked; the user decided these rules, each no wider than
what ran before (`crates/core/tests/quick_rules.rs`, with refusals):

1. Benign environment prefixes (`category::prefixes`): `PYTHONIOENCODING`, `PYTHONUTF8`, `PYTHONUNBUFFERED`,
   `NO_COLOR`, `FORCE_COLOR`, `TERM`, `LANG`, `LC_ALL`, `RUST_LOG`, `RUST_BACKTRACE`, `CARGO_TERM_COLOR` set to a
   plain literal before a program are dropped; any other variable keeps the part Unknown.
2. An executable built in the worktree (`.tmp/…/target/debug/seed.exe …`) runs like a worktree script
   (`worktree_program`: a regular file there, no link on its way, not in `.git`/`.orchester`), in a writing step.
3. Process listings are known-safe reads in every step (`category::processes`): `tasklist` with `/FI`, `/FO`, `/V`,
   `/NH` (also `//FI`), `ps` without `e`/`-E` (environments), and `pwsh -NoProfile -Command` whose script is only
   `Get-Process` / `Get-CimInstance Win32_Process` / `Get-WmiObject Win32_Process` piped into `Select-Object`,
   `Format-Table`, `Format-List`, `Sort-Object`, `Out-String` or a one-comparison `Where-Object`, parsed strictly
   (no `;`, `&`, `$(`, `@`, script blocks but `{ $_.X -eq Y }`). `taskkill` and everything else ask.
4. `for NAME in <literals>; do BODY; done` as the whole line reads as `NAME=v; BODY` per value (`category::loops`, at
   most 64); the word list ends at the first `;` or new line, and a keyword left in the body (a second or nested
   loop) is an unknown program; `;;` and other loops ask. Review fixes: the list no longer runs past a new line, and
   every program check (a shell or interpreter given by path) looks at the program after the benign prefixes.
5. Inline code (`python -c`, `py -c`, `node -e`/`-p`) runs like a worktree script in a writing step. A script,
   executable or inline code runs unasked only when every other part of the line that may change files is itself
   such a run (`sh m.sh … | python -c …` runs; `echo > a.py; python -c 'import a'` asks).

| real lines of the 44 | before | now (writing step) |
| --- | --- | --- |
| `PYTHONIOENCODING=utf-8 python .tmp/verify/orc50b/drive.py …` | asks | runs |
| `.tmp/verify/seed/target/debug/seed.exe …`, `…/orc50-seeder.exe …` | asks | runs |
| `tasklist //FI …` (2 forms) | asks | runs (any step) |
| `powershell -NoProfile -Command "Get-Process … \| Select-Object …"`, `Get-CimInstance Win32_Process …` | asks | runs (any step) |
| `for i in 1 … 12; do sh .tmp/verify/m.sh … \| grep -o … \| head -1; done` | asks | runs |
| `sh .tmp/verify/m.sh … \| python -c "import sys,json; …"` and other `python -c` | asks (reviewer) | runs |

Fixture of runs 22–25: read-only step 5 / 41 / 2 / 11, writing step 33 / 24 / 2 / 0 (the `python -c` line runs).
Still as before: `bash -c`/`sh -c` scripts are read part by part (their parts must run on their own);
`-EncodedCommand` and other `pwsh -Command` scripts go to the reviewer. Tests: `core` `quick_rules`,
`worktree_scripts`, `worktree_writes`, `command_probes`, `command_sequences`, `command_categories`,
`command_fail_closed`, `approval_answers`, `approval_fixtures`; `runner --features test-agent` `claude_unlisted`,
`approval_choices`, `command_approvals`, `auto_mode`, `send_now`. Nothing on macOS.

Worktree scripts and nested fixture repositories run like edits · branch `claude/worktree-scripts` (done 2026-10-06)

User decision: in a step that edits files, running a script file of the worktree adds nothing an agent cannot do by
writing a test and running an allowed `cargo test`. Verify steps asked about `python .tmp/verify/start.py`,
`python .tmp/verify/tree.py 39684 …`, `powershell -NoProfile -ExecutionPolicy Bypass -File .tmp/verify/start-app.ps1`
and `git -C .tmp/verify/kontur -c user.name=… -c user.email=… commit -q …`; they now run.

- `category::interpreters`: `python`/`python3`/`py` (flags `-u -B -E -s -I -O -OO`, `-X …`, `py -3`), `node`, `bash`/`sh`/
  `zsh` (`-e -u -x -v`), `pwsh`/`powershell` (`-NoProfile`, `-NonInteractive`, `-NoLogo`, `-ExecutionPolicy
  Bypass|RemoteSigned`, `-File`) running a file. The part is Worktree and an edit when the script is a regular file
  in the worktree or a writable folder, no link on its way, not in `.git` or `.orchester/` (`Placing::plain_file`,
  the same placing and `write_refused` as edits); any other script is Unknown. Inline code (`-c`, `-e`, `-Command`,
  `-EncodedCommand`), `-m`, `node -r`, `bash --rcfile`, `PYTHONPATH=`/`NODE_OPTIONS=` prefixes still ask; the
  script's arguments are placed like any other.
- git: `-c user.name=…` and `-c user.email=…` are read (every other `-c` is Unknown); `git -C <folder>` with a
  Worktree subcommand is an edit when the folder is a plain folder below the worktree (not the worktree itself)
  holding its own `.git` folder (`Placing::nested_repository`): the agent's fixture. In the run's own worktree, a
  git change still asks.
- Security review (`.tmp/review-worktree-scripts.md`, FAIL at `9740e844`): an interpreter given with a folder
  (`../x/bash`, `C:/evil/sh.exe`, `//server/share/bash.exe`) is Unknown; `native::scope::protected` refuses a stream
  spelling (`.git::$INDEX_ALLOCATION`) and the classifier checks a write or script path resolved as well, so a
  short name (`GIT~1`, `ORCHES~1`) is `.git` or `.orchester`; a script after a part that may change files (any
  part that is not Read-only: git in the fixture repository, an edit, another script) is Unknown, and after the
  re-review also with such a part after it or beside it in a pipeline (a script runs unasked only when every other
  part of the line is Read-only); scripts and
  nested repositories count only in the worktree, not the step's writable folders, which may hold the user's own
  repositories; `pwsh` needs `-NoProfile`, `zsh` (which reads `~/.zshenv`) asks, `-X` takes only `importtime`,
  `utf8`, `dev`, `faulthandler`, `tracemalloc`; `git -C <folder>` places its arguments from that folder; signing
  (`commit -S`, `--gpg-sign`, `tag -s`/`-u`) is Unknown. A shell given with a folder that runs inline code is read
  only when installed (an absolute path outside the worktree, the writable and temporary folders: Codex's
  `C:\…\pwsh.exe -Command …`), else Unknown (`../x/bash -c ls` ran unasked before), and what it runs is never
  answered by a rule for a bare program (`CommandPart::through_path`).
- Read-only steps are unchanged. The fixture table of runs 22–25 is unchanged (its scripts are `python -` and
  `python -c`).
- Tests: `core` `worktree_scripts` (new: the six real lines, more forms, every refusal above, a script through a link,
  outside, in `.git/hooks` and `.orchester`, a read-only step), `native` (`scope_windows`: the aliases of protected
  folders), `worktree_writes`, `command_probes`,
  `command_sequences`, `command_categories`, `command_fail_closed`, `approval_answers`, `approval_fixtures`, `native`;
  `runner --features test-agent` `claude_unlisted`, `approval_choices`, `claude`, `command_approvals`,
  `orchester_tools`. Nothing on macOS.

Shell writes in the worktree run like edits · branch `claude/worktree-writes` (done 2026-10-05)

User decision: in a step that edits files, a command line whose only effects are writes to files in the worktree
(or its writable folders), with parts that are allowed or known-safe reads on their own, runs without asking, as the
edit tools write there unasked. The request that prompted it (`cd <worktree>; f=…; sed -i '…' $f; cat >> $f
<<'EOF' … EOF` then `cargo test … 2>&1 | grep … | head -30` on the next line) now runs.

- `run::permitted` takes the step's effective `AgentPermissions`; with *Edit files* a part runs when
  `CommandPart::edits_worktree`: `sed -i`, `tee`, `mkdir`, `touch`, `cp`, `mv`, `rm`, `rmdir` (`Judged::edits`), or a
  known-safe read redirected into a file, every write placed in the worktree or a writable folder and nothing named
  outside. Listed commands may redirect into the worktree there too. Read-only steps are unchanged.
- Writes are refused (Unknown) where `native::scope::protected` refuses the native file tools' writes (any `.git`
  entry, `.orchester/` at the worktree's top), and a removal of the worktree or a writable folder itself
  (`CommandScope::write_refused`).
- Here-documents (`heredoc.rs`): `<<` and `<<-` with a quoted delimiter take the body as data; a bare delimiter's body
  with `$` or a backtick, a body without its delimiter and a here-string stay unread. New lines separate commands.
- Security review (`.tmp/review-worktree-writes.md`, FAIL at `3efb189d`, one major): paths are placed before the line
  runs, so `cp -r`/`-R`/`-a`/`-P`/`-d` and a `cp` or `mv` of a folder or a link are no edits (they could bring a link
  that leads out), and a write under a path an earlier part of the line wrote, created, copied or moved is Unknown
  unless every folder down to it is there now as a plain folder (`place::Line`). Minor: on Windows a path rooted
  without a drive (`/projects/x`, `/tmp/x`; Git Bash places it under its installation) is Unknown.
- `CommandPrefix::covers_argv`: `cargo +<toolchain> <sub>` is covered by `cargo <sub>`.
- Fixture of runs 22–25 (59 requests, builder listing `cargo test`, `check`, `clippy`, `fmt`):

  | 59 requests | runs unasked | asks, Approve similar | asks, Unknown | asks, other |
  | --- | --- | --- | --- | --- |
  | before this branch (any step) | 5 | 40 | 8 | 6 |
  | read-only step | 5 | 40 | 3 | 11 |
  | step that edits files | 32 | 24 | 3 | 0 |

  The 24 are reads outside the worktree (23 of lurq's source in the registry, one Windows `grep` pattern read as a
  root path); the 3 Unknown are `python3 - <<'EOF'`, `python -c` and `L=$(…)`. Quoted here-documents are now read,
  so in a read-only step 5 former Unknown lines ask as writes with several asking parts. On macOS 33 / 23.
- Not done: the 33 newest requests were not exported: reading the user's project database was refused by the
  session's permission check, so `.tmp/orchester-drive/approvals-recent.json` does not exist and the table covers
  the 59 only.
- Tests: `core` `worktree_writes` (new; allowed forms, refusals for `.git/config`, `.git/hooks`, `../`, a link out,
  `/etc`, `~/.bashrc`, an expanding here-document, `> .git/HEAD`, `rm -rf .` and `/`, `cp` from outside, `mv` out, a
  read-only step), `command_probes` (worktree redirects run only where edits are allowed), `command_sequences`,
  `command_categories`, `command_fail_closed`, `approval_answers`, `approval_fixtures`, `native`, `permissions`;
  `runner --features test-agent` `approval_choices`, `claude_unlisted` (a worktree `sed -i` is answered unasked; `.git`
  and `rm -rf ..` still ask), `claude_approvals`, `claude`, `command_approvals`, `orchester_tools`. Nothing on macOS.

Agent commands without approvals · branch `claude/agent-tools` (done 2026-10-05)

Four Claude Code runs of Orchester on itself (runs 22–25, Opus 5.5, about 1.5 h) asked 59 times: the agents read and
edited through Bash (`cat`, `sed -n`, `sed -i`, heredocs, `awk`), prefixed nearly every command with `cd <worktree>;`
and set variables (`f=…; sed -n 40,52p $f`), so whole lines were *Unknown*, and reads of a crate's source in Cargo's
registry were *Credential read*s. The requests are the fixture `crates/core/tests/fixtures/approvals/runs-22-25.json`.

- Prompt: a new Claude Code or Codex session's task part gets *Files and commands* (`run::WorkingTools`, from the
  CLI and the step's effective permissions): Claude Code reads and searches with Read, Grep, Glob, and edits with
  Edit and Write only in a writing step; Codex changes files with apply_patch only in a writing step; no `cd`;
  with allowed commands, one bare command per call without `;`, `&&` or variables; with *Ask before running
  commands*, that each command waits for a person. Resumed sessions keep it from their first prompt; native agents
  and the instruction preview get none.
- Classifier (`permission::category`): a `cd` to one folder named plainly in the worktree (`/h/…`, `H:/…`, quoted
  `H:\…`) is no part of its own; a `cd` with an option, `-`, `~`, a glob or a second operand (zsh's `cd old new`) is
  kept, and one with an option, `-` or a second operand leaves where later parts run unknown; a glob `cd` goes to its
  one match. Every part reads the folder it runs in. A glob names each path it matches now, links followed
  (`glob.rs`, bracket expressions too; `**` is Unknown). `>&file` and `<&file` write and read the file (`>&2`,
  `>&-` copy a descriptor). A literal assignment (`f=src/x.rs`, `L="C:/…"`: no expansion, quote or space; a name
  of one letter and digits, which no shell treats specially, so zsh's `path=` or `cdpath=` stays unread; unquoted,
  first in its list, not in a pipeline or the background) is read into its later `$f` and `${f}`; a part in the background is Unknown; a `..` after a glob is Unknown; a carriage return
  stays in its word. `sed` is read by its options and its script (`sed_script`); `awk` by an allow-list of the names
  its program uses (`awk_program`: no `ARGV`, `ARGC`, `ENVIRON`, `FILENAME`, `SYMTAB`, `getline`, `system`,
  `close`, `@`, or `print` with `>`/`|`; `-v` and operand assignments only of the variables it may set); `sort`
  without `-o`, `-T`, `--compress-program`, `--files0-from` or their abbreviations; `grep -R`, a `jq` module,
  `cargo --config`/`-Z` are Unknown; `git remote show` without `-n` reaches the network. Reads of
  `~/.cargo/registry/src` and `~/.rustup/toolchains/*/lib/rustlib/src` are reads outside the worktree; the rest of
  `~/.cargo` and `~/.rustup` stays a credential location, as does every path through a Windows share or `\\?\`.
- Answers: `run::permitted` runs a line Claude Code asked about when every part would run on its own: listed in the
  step's allowed commands (naming nothing outside the worktree, no redirection into a file) or a known-safe read
  inside the worktree. The runner answers it before a person (`step/answer.rs`) as `DecidedBy::Permissions`
  (`decided_by = 'permissions'`; a migration rebuilds `tool_approvals` for its `CHECK`), marked *allowed by the
  profile · <category>*; the Summary counts it with *Always allowed*. Approve similar also covers several asking
  parts that are alike (`sed -i …; sed -i …`). Shell writes still ask.
- Security review (`.tmp/review-agent-tools.md`, FAIL at `34a10f02`): every finding fixed; its probes are refusal
  tests in `tests/command_probes.rs` (links are junctions on Windows, symbolic links elsewhere).
- Fixture, with a builder that lists `cargo test`, `cargo check`, `cargo clippy`, `cargo fmt`
  (`tests/approval_fixtures.rs`; the user name in its paths replaced):

  | 59 requests | runs unasked | asks, Approve similar | asks, Unknown | asks, other |
  | --- | --- | --- | --- | --- |
  | before (master `3d76beff`) | 0 | 1 | 43 | 15 (credential reads, several asking parts) |
  | now | 5 | 40 | 8 | 6 (`sed -i` with a build in one line) |

  Of the 40, 16 are `sed -i` edits in the worktree and 24 reads outside it (23 of lurq's source in the registry; on
  Windows one `grep` pattern starting with `\` is a path at the drive's root). The 8 Unknown are heredocs and
  `python`/`$(…)` lines. On macOS the `grep` line runs unasked (6 / 39).
- Tests: `core` `command_probes`, `command_sequences`, `approval_fixtures`, `command_categories`,
  `command_fail_closed`, `approval_answers`, `run` (`prompt`, `plan`, `conversation`, `working`); `store`
  `approval_rules`, `tool_approvals`, `project_db`, `organization_migration`, `joined_tasks`; `desktop` `i18n`,
  `transcript`; `runner --features test-agent` `approval_choices` (a `$`-line of a read and a listed test runs
  unasked), `claude_unlisted`, `claude_approvals`, `command_approvals`, `claude`, `memory`. Nothing run on macOS.
- Open: the activity mark *allowed by the profile · …* is new text without a design frame and needs the design
  verification (`AGENTS.md` rule 15); reads outside the worktree that are no credential still ask (Approve similar
  available); a line mixing a shell edit with a listed build has no Approve similar; grep and rg patterns are placed
  like paths; `date --se…` and other abbreviations of options in the older tables are not refused.

S2c · Desktop: join a shared repository · branch `claude/s2c` (done 2026-10-05)

Implemented in `.tmp/worktrees/claude-s2c` from master `af858e1d` (S1a, S1b, S2a, S2b).

- Core: `sharing::similar_titles` (case-folded words; at least two words and three fifths of the shorter title's
  words in common; the most similar first), tested in `tests/sharing.rs`.
- Store (migration 42, after shared projects): `tasks` is rebuilt so a number is unique only among the tasks that
  are not *Not shared* (partial index), with `kept_key` and `kept_tag_names` for a task kept *Not shared*;
  `joined_keys` holds every local task's key before the join (and the run it was claimed for) until *Make local*.
  `join_sharing` keeps the local tasks not uploaded as *Not shared* (their tags by name, cleared from their
  organization), caches the server project's tasks and tags, and gives the project the server's key, in one
  transaction. `tasks()` leaves *Not shared* tasks out; `not_shared_tasks`, `former_keys`, `joined_keys`; a *Not
  shared* task refuses edits, organization changes and runs (`StoreError::NotSharedTask`). *Make local* returns
  them after the highest number with their key before as their former key and their tags matched by name (made
  when missing), and returns the mapping. `cache_shared_tasks` makes a kept task the server's (*Share task*).
- `orchester-api` client: `import_tasks` (`POST …/tasks/import`).
- Desktop service: `Servers::join_review` (the server project for the repository's identity, the local tasks with
  their latest run and the server's most similar title) and `Servers::join` (one import of the ticked tasks and of
  every task with a run that has not ended, `claim: true` for those, new numbers, ids and former keys; dropped
  links named by their keys before; the cache and the link written after; the application's project list takes
  the new key). A failure says whether the import reached the server; *Try again* sends the same key, so a lost
  answer joins once. `ProjectServer::share_kept` (*Share task*), `make_local` returns the returned tasks.
- Screens: Sharing asks every signed-in server for the repository when it opens and after *Check again*; a member
  gets *This repository is shared on … as ORC* with *Review and join…* (10.15), Share is not offered and *What
  joining sends* replaces *What sharing sends*; a person who is not a member gets the owners, their username with
  Copy, *Check again* and when it last asked (10.16 A); a Share that meets `409 remote_taken` shows *… already has
  this repository as ORC* with *Review and join…* (10.16 C, replacing S2b's "later version"); a repository that
  cannot be shared says why (10.16 B). The join page (`Page::Join`, 10.21) with the review bar, the table (local
  key, task with the similar-title hint and why a locked task cannot be unticked, workflow and run, what it
  becomes), the note with a branch example, and *Join and upload N*; a failed join (10.26 D) keeps the ticks.
  After the join Sharing shows *You joined ORC on …* (with the dropped links) and *Keys before joining* (10.22),
  which stays until *Make local*; after *Make local*, *Kontur is local again* with the returned tasks (10.26 C).
  Tasks: the *Not shared · N* toggle (tab counts follow it), its notice, rows and footer (10.23); *was KON-12*
  under a row's title; the search finds former keys (10.26 B). Task page: *· was KON-12* after the run line
  (10.25), the side panel's *On <server>* (holder and claim kind, since, heartbeat, shared status, created as,
  last edit); a *Not shared* task (10.24) with its label and kept key, its organization as plain labels with the
  kept tag names, its notice and lock line, *Edit* unavailable, *Share task* with *Share KON-15 on ORC?* (10.26 A),
  and the panel's *Sharing* (state, similar on ORC, share as).

Deviations: the server is asked when the Sharing tab opens (and on Share), not when the project opens elsewhere: no
frame shows a result outside Sharing. The not-a-member state names the owners by display name only and no key (the
`403 not_member` answer carries neither usernames nor the key). 10.15 does not say who shared it and when (the API's
project has no creator or date). The discovery offer is not drawn in its "steps 1 and 2 done" form (10.16 E): once
signed in, the join offer of 10.15 shows instead; 10.16 D (*Add the server named in orchester.toml?*) stays S2b's
route through Settings › Servers. The not-shared task page does not say which server task looked the same at the
join (not recorded); *Share as* reads *The next ORC number* (the next number is the server's). *Release* on the task
page's server section is S2d. The branch example in the review's note is the first uploaded task that has runs.

Tests: `cargo test -p orchester-core --test sharing s2c_` (1); `-p orchester-store _` (every store test binary,
including `--test joined_tasks joined_` (5, with the upgrade of a populated schema-41 database) and `--test
shared_tasks shared_`); `-p orchester-api --features client --test client client_`; `-p orchester-runner --test
pipeline_pin _`; `-p orchester-desktop --test joined_projects joined_` (5, service against a real server on loopback
with two people), `--test shared_projects shared_`, `--test shared_statuses statuses_`, `--test sharing sharing_`,
`--test tasks task`, `--test theme roles`, `--test status_bar status`, `--test i18n _`, `--test navigation route_`,
and `--test form_keyboard` `joining_` (7), `sharing_`, `edit_task`, `new_task`, `organization`, `task_run`,
`run_again`, `edit_conflict`, `task_bulk`, `plan_review`, `servers_`, `unreadable_run`, `run_history`, `history`,
`conversation_queued`, `changes`, `tabs`, `project_settings`, `sidebar`; `tools/check.ps1` passed; `cargo check
--target aarch64-apple-darwin -p orchester-core -p orchester-api` passed. Not run: the rest of the desktop's and
the runner's suites, and the server's (unchanged).

Review and verification round (2026-10-05): the Sharing tab asked the servers again with every answer (its lookup
was keyed by the server entries, which each answer changes) and never showed the answer; it is keyed by the servers'
ids now and asks once per visit, after a sign-in and on *Check again* (`form_keyboard joining_sharing_asks_once_*`
count the requests through a port in front of the server). Sharing's dialogs open in a Modal over the whole window
(`modal_layer::assert_dialog_in_screen` on Share and Share task). A join no longer depends on one key per visit of
the page: it reads which of the project's tasks the server holds and uploads only the others, so a join whose answer
was lost, or which could not finish here, links what reached the server instead of meeting `409 id_taken` forever
(`joined_projects joined_a_join_after_a_lost_answer_does_not_upload_twice`); a failure says whether tasks reached the
server, or that the connection was lost while they were sent. A run's change made while joining is sent on top of the
uploaded copy, as for Share (`joined_tasks joined_a_change_made_while_joining_is_reported`). A folder origin names
its path, with the git-branch icon. Privacy (kept, flagged): the repository's identity goes to every server the
person is signed in to, as `docs/architecture/server.md` ("Finding the project") says; asking only a chosen server
plus "Check other servers" would need the user's decision.

Design verification needed: 10.15 `y8uP2`, 10.16 `AZwJE` (A, B, C), 10.21 `UlHPB`, 10.22 `m9omas`, 10.23 `l9jYZS`,
10.24 `FAKNF`, 10.25 `RuBhQ`, 10.26 `m7jOn9` (A–D). **Done 2026-10-05:** security re-review PASS, independent design
verification PASS (all frames; one lookup per Sharing-tab visit; root scrims; Settings › Servers does not poll),
merged into master. Minors left: a server 422 in 10.26 D reads as *can't be reached*; a *Not shared* task page shows
*Ready to queue*; the Share task and Make local dialogs lack the Similar / Not-shared rows; a local edit made between
a failed join and its retry is replaced by the server copy.

Runner start under load · branch `claude/runner-start` (done 2026-10-05)

The desktop waits 30 s (was 10 s) for a started runner to open its endpoint and starts it once more when the first
start fails for a transient reason (slow, exited, connection or handshake failure), after killing and waiting for the
first runner; a missing program, a held project or a version mismatch is not retried. The state stays *Starting*
meanwhile; the last attempt's error is shown. Tested against `orchester-fake-runner` (feature `test-runner`):
`cargo test -p orchester-ipc --features test-runner --test runner_start runner_start_` (late, exit once, exit twice,
held project, missing program; `runner_start_slow` takes 11 s).

ORC-59 · Native agent P1d: runner integration · branch `orchester/orc-64-run-1` (implemented 2026-10-06 as ORC-64;
independent review and design verification pending)

- **Runs.** `AgentProvider::runs_agents` is true for the Anthropic API (OpenAI API waits for its client, P2). The
  worker drives a `Driver` (a CLI's process, or `Driver::Native` with a `NativeHandle`); `step/start/native.rs` builds
  the session (key read from the keychain for the invocation, `FileScope`, `CommandPolicy`, the shell, ripgrep, the
  prompt), `step/native/` runs core's loop in a task of its own: items stored before what follows them, model
  requests, tool calls (reads together, commands under `Supervised` with their time limit, 32 KiB head and tail to the
  model, 1 MiB to Activity), approvals, and the run's stops (`Now` drops the request or command in flight). The end
  becomes the invocation's outcome; a result or a question is the agent's last message, read by `end.rs` as for a CLI.
- **Persistence and resume.** Store migration `native_conversations` (`native_conversations`, append-only gapless
  `native_items` with segments, `invocation_engine`); core `native::store::NativeConversationStore`. The invocation's
  session is the conversation id; a resumed invocation continues the stored conversation (calls left unanswered get
  `NOT_RUN`, then the resume prompt, or the person's answer as the `ask_question` result).
- **Prompt (§6.1).** `PromptInput::assemble_native`: a preamble (tools, computer, shell), the repository's `AGENTS.md`
  while `AgentFilesMechanism::OrchesterLoads`, role, rules, pipeline, step in the system prompt; code index, skills
  (the repository's `.agents/skills` and `.codex/skills` added while it loads them), memory and task in the first
  user item; `ResultDelivery::Tool` names `finish_step`. With `OrchesterLeftOut` neither is added.
- **Approvals.** A command that asks is recorded with `PendingToolApproval::native` (shell, time limit), its action
  and target redacted with the key as a secret; rules, Always allow and auto-approval answer it as for the CLIs;
  Deny gives the agent `DENIED` and the step goes on. `ToolDecision::waited_ms` records the wait of a person's answer.
- **Models and effort.** `NativeModels::effort_for` (the profile's effort when the model takes it, else the
  connection's default effort), `context_window` (`DEFAULT_CONTEXT_WINDOW` when unknown), `NativeModel::listed`
  (unknown efforts and limits that do not fit dropped), `with_fetched` (fetched models replace those with their id,
  none is removed).
- **Test connection / Fetch models.** `Command::CheckConnection` (protocol 17) → `Reply::ConnectionChecked`; the
  runner reads the key and runs `check_anthropic`. The inspector records the result and merges the models
  (`record_check_in`); the dialog shows the result and merges the models into the form; a new key typed in the dialog
  is tested after saving. Both buttons are enabled for Anthropic API connections; for OpenAI API ones they stay
  unavailable in the inspector and the dialog, whose test row and models hint say why (fixed after the first review).
- **What the run used.** `InvocationEngine` (endpoint, vendor, effort) recorded before a native invocation starts;
  the task page's step view takes it for the 04.H10 header and panel (*Endpoint* instead of *Endpoint now*, the
  *Effort* row) and the 04.H9 agent row.
- **04.H10 rows.** `AgentEvent::NativeCall` (tool, `CallDetail`: lines read and size, entries, search scope with
  match and file counts, +/− lines, the shell; `Declined` by a person or by Orchester with the result the agent got),
  told before the call's end. Rows: file tools with their own glyph, the target as title, the tool first in the meta
  (*read_file · lines 1–140 · 4.7 KiB*, *search · src/\*\* · 9 matches in 4 files*, *edit_file · M · +38 −6 · open in
  Changes*, the separate file-change row folded in; a search titled by its quoted pattern, *“registerShortcut”*);
  `run_command` as a CLI command row plus *shell · pwsh*; an approval as one line of its own (hand glyph, *Approved by
  you · pnpm add @kontur/keymap*, meta *approve once · waited 48 s*); a denial *Denied by a person · the agent
  continues* (circle-x, meta *run_command · shell · pwsh*, the command in mono as body, the output block opening with
  *Given to the agent as the command’s result:*); Orchester's refusal (*refused by Orchester · the agent continues*).
  The *Tools* view leaves a native step's per-request *Usage reported* rows out (its header and panel carry the
  totals); the panel lists *Effort* after *Started*. The dialog's test row after a passed test reads *Key accepted* and
  *api.anthropic.com answered · 4 models listed · 14:02* (a test recorded before the dialog opened: *… answered ·
  14:02*). These follow the first design verification (four major findings, fixed). `RunnerProblem::ContextCompacted` (emitted by the loop with the
  tokens before and after), `ModelRetrying` and `ThinkingStripped` have Activity texts; a profile's MCP servers are left
  out with `McpServerProblem::NotSupportedByConnection`.
- **Tests run (all pass):** `cargo test -p orchester-runner --features test-agent --test native native_` (5: a step
  with read, edit and `finish_step`, the key in no activity, the engine and conversation stored; approve once and a
  denial that continues; pause at an approval and resume of the same conversation, append-only checked by the fake
  server; *Test connection* listing models, a rejected key, a missing secret); `cargo test -p orchester-core --test
  native native_` (76), `--test run native_prompt`, `--test run preflight`, `--test run prompt`, `--test
  native_setup native_setup`, `--test native_connection native`, `--test activity_redaction redact`; `cargo test -p
  orchester-store --test native_conversations native_` (2), `--test activity activity`, `--test tool_approvals tool`;
  `cargo test -p orchester-agent --test file_tools files_` (23); `cargo test -p orchester-ipc --test framing
  frames_round_trip`; `cargo test -p orchester-desktop --test form_keyboard native_` (33, with the new
  `native_rows`, `native_rows_leave_usage_out_of_the_tools_list`, `native_dialog_says_what_a_passed_test_found` and
  `native_dialog_keeps_testing_an_openai_connection_unavailable`), `connection`, `conversation`, `approval`, `activity`, `agent`; `--test transcript transcript`,
  `--test timeline timeline`, `--test i18n locales`/`every_key`. CLI regressions: runner `claude_approvals`, `claude`,
  `execution`, `codex_resume`, `send_now`, `command_approvals`, `approval_choices`. `tools/check.ps1`: OK.
- **Not run:** a live API run (P1's acceptance needs one with a key a person supplies); runner tests for a budget
  `AfterTurn`, the time limit, cancel and an `ask_question` round trip (the loop's own core tests cover them); `cargo
  clippy --target aarch64-apple-darwin` beyond `orchester-core` (ring and SQLite need a macOS C compiler this machine
  lacks).
- **Differs from the design:** a native command's approval names server `command`, as the CLIs' do, not
  `Orchester`; a retried model request is a `ModelRetrying` problem row and a dropped response's tokens a *Usage
  reported* row (`TurnCompleted`), which budgets count; a command's time limit is not shortened to the remaining
  active time; the conversation stores its system prompt as text and its repository access (which decides its tools)
  instead of blob hashes; conversation items are stored as the model was sent them, not passed through the key's
  redaction (§5.3 asks for it; redacting them would change the bytes a resumed request sends again, and the key
  reaches no command, prompt or tool output Orchester makes, only a file in the worktree could hold it), while
  Activity and approvals are redacted with the key; read stamps live per invocation, so a resumed agent reads a file again before it edits it;
  sizes read KiB as everywhere in the app, not *KB* as 04.H10 draws them; *Test again* and *Fetch models* both test
  and merge the listed models; a search's scope is the path or glob the agent passed (*src* when it named a folder,
  not the frame's *src/\*\**); the *All* view's open tool groups still list a native step's usage rows (the
  verification flagged only *Tools*). Not changed after the design verification (minor, follow-ups): the agent
  message line in *Tools* (one line *Builder: …* · *message*, as for the CLIs), and 04.H4's *Last checked* (relative
  time) and footer copy.
- **How to reach each state (design verifier):**
  - 04.H10 rows, header and panel with *Endpoint* and *Effort*: the headless harness seeds them in
    `crates/desktop/tests/form_keyboard/native_rows.rs` (`seeded`: a DeepSeek run, an `InvocationEngine` on
    `gateway.kontur.lan` with effort high, and the transcript). In the app: write the same records into a scratch
    project's `.orchester/orchester.db` (as `native_run` plus `save_invocation_engine` and `append_activity` do), open
    KON-1 › Conversation, open the tool group, then *Details*.
  - A live native run in the app: an *Anthropic API (native)* connection whose Advanced base URL is
    `http://127.0.0.1:<port>` of a local Messages API (the fake server of `crates/agent/tests/anthropic_wire/server.rs`
    scripted as in `crates/runner/tests/native/`), a key secret with any value, *Test again* in its inspector (lists
    the server's models), a profile on it, then *Create and run*. Its approvals show in 04.H9 when the profile asks
    before commands.
  - 04.H1/H4 test row and *Fetch models*: open Connections, select or edit an Anthropic API connection; without a
    reachable runner the row says the runner is not connected. *Key accepted* with *host answered · N models listed ·
    time*: press *Test again* against the fake Messages API (or edit a connection whose last check passed:
    *host answered · time*, seeded in `native_dialog::native_dialog_says_what_a_passed_test_found`). The unavailable state: edit an OpenAI API connection
    (seeded as in `native_connections::seed_native`; the dialog cannot add one).

ORC-58 · Native agent P1c: the `orchester-agent` crate · branch `claude/orc58` (done 2026-10-05: independent code and security
review, second round PASS)

- **Crate** `crates/agent` (`orchester-agent`), used by nothing yet (the runner takes it in P1d). No new workspace
  dependency: reqwest with rustls (ring, the OS trust store), tokio, serde_json, sha2, tempfile, parking_lot and
  orchester-exec are reused.
- **HTTP** (§3.0): redirects not followed, connect timeout 30 s, no total timeout but a 120 s idle timeout on every
  wait for bytes; `HTTPS_PROXY`/`NO_PROXY` from the environment are honoured for remote endpoints as the design says
  (unlike `orchester-api`, which uses no proxy), an `http://` endpoint on this computer is reached directly. A
  ~70-line SSE parser (`sse.rs`): comments, multi-line data, CRLF, chunks split anywhere.
- **Anthropic client** (`AnthropicClient`, implements core's `ModelClient`; §3.1): always streams; tools sorted by
  name with `strict` and, on the official endpoint, `eager_input_streaming`; the system prompt with a cache
  breakpoint plus top-level automatic caching; adaptive thinking with summaries and `block_binding:
  prefix_mismatch_behavior: "error"` (beta `thinking-binding-controls-2026-08-01`) where the model supports it,
  nothing for models without it (Haiku 4.5); `output_config.effort` as the request names it; `fallbacks: "default"`
  only when the connection turns provider fallback on. What a model supports and its output cap come from
  `GET /v1/models/{id}`, asked once per client and model (or from a listing); `max_tokens` is capped there. Streamed
  blocks become items verbatim (thinking with its signature, `redacted_thinking` as is); a response counts only
  after `message_stop`. Eager tool input is parsed strictly: input that is not a JSON object, or that the stop cut
  off, is kept as `{"INVALID_JSON": "<raw>"}`, and core answers such a call with exactly that text as the error
  result. `max_tokens` inside a tool call asks the same turn again once with the model's whole output cap.
  Statuses map as §3.1 says (a 400 naming the prompt as too long is `TooLarge`, so core compacts); 429, 5xx, 529,
  `error` events and transport failures (a broken or idle stream included) are retried after 2, 4, 8, 16 and 32 s
  plus up to 25 % jitter, or after `retry-after` (a wait over 5 min is not waited for). The preserved-thinking 400
  strips the thinking blocks the conversation had at that point and retries once; the client remembers it for the
  conversation. `GET /v1/models` page by page (efforts, context window, output cap); `check_anthropic` for *Test
  connection* (`CheckResult::Api`/`KeyRejected`/`BillingProblem`/`EndpointUnreachable`). The key is a sensitive
  header value; `Endpoint`'s `Debug` hides it and every error and event text is redacted.
- **Core additions:** `native::edit::apply_edits` (§2.1 `edit.rs`, missing from P1b: in order, all or none, CRLF and
  BOM kept); `tools::INVALID_JSON`/`invalid_json_input` and `ToolInputError::NotJson`; `StreamEvent::Retrying {
  attempt, wait, error }` (each wait, for the `RunnerProblem` P1d records), `ThinkingStripped` and `Spent(usage)` (a
  dropped response's tokens, which budgets must count; the response's own usage stays the final attempt's, so the
  context estimate is not doubled).
- **File tools** (`FileTools`; §4.1–§4.5, §5.1, §6.4): `read_file` (numbered lines, offset/limit, lines cut at 2000
  characters, binary files named with their size, files over 32 MiB refused), `list_files` (`rg --files`, sorted,
  1000 entries), `search` (ripgrep's plain output with `--max-count 200 --max-columns 500 --max-columns-preview`,
  200 matches or 64 KiB, `--no-config` so no configuration adds `--pre`; not `--json`, which carries every matched
  line whole however long it is), `edit_file` and `write_file` (read stamp of length, modified time and SHA-256 checked
  at edit time; atomic temp-and-rename in the target's folder, resolved again just before the rename; permissions
  of a replaced file kept). `FileTools::stored(items)` drops every read stamp when a `Compaction` item is stored.
  ripgrep runs through `orchester_exec::prepare`, read line by line (at most 16 KiB kept per line), stopped when
  enough is read or after 60 s.
- **Fake model server** (`crates/agent/tests/anthropic_wire/server.rs`): a tokio listener on `127.0.0.1:0` with
  scripted turns (`Script::new().expect(..).reply_sse(..)`, `reply_error`, `reply_status`, pauses and drops), the
  append-only check of `system`, `tools` and the message prefix on every Messages request (a test proves it catches an
  edit), a cache-read echo, and a model catalog with pagination.
- **Tests:** `cargo test -p orchester-agent --test anthropic_wire anthropic_` (25 before the review fixes: SSE, request shape, items and
  usage, append-only history with cache reads, invalid tool JSON, `max_tokens` re-ask with and without room,
  refusal, conservative quirks, a model without adaptive thinking with fallback on, every status, `retry-after`,
  backoff, five retries then the error, broken and idle streams, an `error` event, the thinking recovery, the key in
  no error, event or `Debug`, model pages, *Test connection*); `cargo test -p orchester-agent --test file_tools
  files_` (on real temporary folders: scope, `..`, absolute paths, a junction on Windows / symbolic link on macOS
  that leads out (read, write, both listings and search), `.git` and `.orchester/` and the index protected, a
  read-only step, CRLF/BOM, stale stamps, all-or-nothing multi-edit, no partial file, compaction dropping stamps,
  ripgrep and the walk without it; 20 before the review fixes); `cargo test -p orchester-core --test native native_` (73, with new `edit` and
  `NotJson` tests). The ripgrep tests need `rg` on `PATH` and fail without it.
- **Review fixes (2026-10-05):** search output bounded (plain rg output, lines capped while read, the no-rg search
  cuts lines and stops at 100 000 files, depth 64 or 60 s, with a note); a `retry-after` too large for a `Duration`
  no longer panics (it is the longest wait, which is not waited for) and odd values fall back to the backoff;
  `retry-after` is honoured on every retried status (529, 503, …), not only 429; `Endpoint::reached_directly`
  (scheme in any case, every loopback host) decides the proxy bypass; SSE events capped at 16 MiB, lone CR line ends
  and a leading BOM read as the standard says; globs without rg limited to 256 characters and 64 alternatives and
  matched without recursion; `read_file`, `edit_file` and `write_file` refuse files that are not regular (a FIFO)
  and files that grow past 32 MiB while read; a new file is renamed into place without replacing one that appeared
  meanwhile; a 400 naming the "context limit" also compacts; rg exit code 2 with some matches adds a note; the unused
  `FileTools::scope` is gone. Tests: 30 `anthropic_`, 22 `files_` (the FIFO test runs on macOS only). Not changed:
  new files still get mode 0644 whatever the umask (reading the umask needs a call that sets it); HTTP-date
  `retry-after` values are not read (model APIs send seconds); the model lookup before the first request stays
  (P2/P3 add metadata entered by hand for Custom endpoints); a file appearing between the check and the rename has no
  test (the race cannot be forced through the public API).
- **Differs from the design:**
  - Retries: "at most 5 attempts" is read as five retries after the first attempt, one per listed wait (2–32 s).
  - Without ripgrep, `list_files` and `search` walk the folders skipping hidden entries and links but do not read
    `.gitignore` (not "the code index's exclusions": the index reads git objects), and `search` matches literal text;
    the result says so. The runner's `RipgrepNotFound` problem stays P1d's.
  - `read_file` results are capped at 128 KiB (the any-result cap of §6.4), not 256 KiB; files over 32 MiB are refused
    (the stamp hashes the whole file).
  - The thinking recovery is remembered per client: a runner that makes a new client for a resumed invocation meets
    the 400 once more and recovers again.
  - `WireQuirks` holds only what the Anthropic client uses (`betas`, `eager_input_streaming`, `cache_control`);
    `WireQuirks::anthropic` gives the official quirks only to the Official preset at its own base URL. The other §3.3
    fields come with P2/P3. There is no `NativeClient` enum until a second client exists (P2).
  - Not run: `cargo clippy --target aarch64-apple-darwin` for `orchester-agent` (ring's C code needs a macOS C
    compiler this Windows machine lacks; core passes); a live API run (P1's acceptance needs one, with a key a person
    supplies).

S2b · Desktop: share, members, make local, offline, conflict · branch `claude/s2b` (implemented 2026-10-05; design verification and review pending)

Implemented in `.tmp/worktrees/claude-s2b` from master `2fb85037` (S1a, S1b, S2a), with master `aa979bfa` (lurq
0.40.0) merged in.

- Core: `TaskRevision` on `Task`; `TaskStore::save_task` / `update_task(id, read, details)` return `TaskUpdate`
  (`Saved` or `Conflict { current }`); `ContentMerge` (`task::content`) puts a change made on an older revision on top
  of the stored task field by field: a field only one side changed merges without asking, a field both changed to
  different values is the person's choice; organization fields rebase (decision 26).
- Store (`orchester-store`, migration after token budgets): `tasks.revision` (every stored change raises it; a local
  change on an older one is a conflict) and the shared-project cache: `sharing` (the link, role, event cursor, last
  sync, removal), the task columns `sharing`, `pipeline_slug`, `former_key`, `created_by`, `updated_by`,
  `updated_at`, and `task_claims`, `task_run_status`, `shared_automatic_status`. A shared project's tasks are written
  only from what the server confirmed and refuse local writes; the pipeline is the local pipeline of the task's slug
  (none when this computer lacks it); the workflow status a run of a shared task sets is queued for the desktop to
  send as Orchester's change; *Make local* keeps every cached task as a local one.
- `orchester-api` client: projects by repository identity, create, members and roles, tasks, task changes with the
  revision read (`409 revision_conflict` with the current task), claims, heartbeats, tags; `ClientError::details()`.
- Desktop service (`sharing::project`): `RemoteTaskStore` (reads the cache; writes go to the server with the
  revision read, a conflict comes back as a result, a clean one is merged and sent again up to three times); every
  desktop write goes through `TaskWriter` (local or shared). Share (repository identity from `origin`, confirmation,
  upload, optional `orchester.toml`), members and roles (the last owner refused), *Make local*, the discovery file
  (read only; its server is contacted only after the person adds it from Settings › Servers, prefilled; the file is
  written only when the person asks, never committed), run claims before starting or continuing work on a shared task
  (refusals name the holder), heartbeats every minute for held claims (≤ 256 ids a call, within 12 calls a minute),
  a sync every minute (tasks, tags, membership, automatic statuses), and the list's access: writable, offline (last
  sync, next try), signed out, or no longer a member. The runner starts runs from cached tasks unchanged.
- Screens: Project settings › Sharing (10.11–10.14 with roles, 10.17), the shared task list's badge, read-only notice
  and footer (10.42, 10.44; New task, Create multiple, Manage tags and bulk changes unavailable while read-only), the
  status bar server item for the servers of open shared projects with *Not a member* (10.43 E), the Edit task
  conflict view (10.41), a shared task's pipeline this computer lacks shown as *<slug> · not on this computer* on the
  list and the task page (an edit does not clear it), and Settings › Servers' details header giving way at 960 so
  *Sign out* stays whole.

Deviations: member rows show the display name only (the API's members carry no username); the Holder column of
10.31 is S2d; joining an existing project (`409 remote_taken`) says it comes in a later version (S2c); *Make
local…* in the tasks notice leads to the Sharing tab; task groups (bulk queue, release) refuse shared tasks with
*server only*; the conflict view does not offer editing the person's version in place (they choose a version and edit
after saving), and a non-conflicting outcome or title shows as read-only text; organization history of shared tasks is
not cached (S2d).

Tests: `cargo test -p orchester-core --test task_content content_` (3); `-p orchester-store --test shared_tasks
shared_` (7, including the upgrade of a populated schema-40 database), `--test task_edits task_`/`legacy_`, `--test
task_groups task_`, `--test task_organization organization_`, `--test project_db _`; `-p orchester-api --features
client --test client client_` (15); `-p orchester-runner --test pipeline_pin _` (3, including a shared cached task);
`-p orchester-desktop --test shared_projects shared_` (11, service against a real server on loopback), `--test sharing
sharing_`, `--test tasks task`, `--test i18n _`, `--test theme roles`, `--test navigation route_`, `--test status_bar
status`, and `--test form_keyboard` `sharing_` (8), `edit_conflict` (4), `servers_`, `edit_task`, `new_task`,
`organization`, `plan_review`, `agent_permissions`, `native_connections`, `task_bulk`, `run_again`, `task_run`,
`conversation`; `tools/check.ps1` passed; `cargo check --target aarch64-apple-darwin -p orchester-core -p
orchester-api` passed. Not run: the rest of the workspace's suites; the store's `app_db` (two tests expect schema 2
since S2a) and the runner's `task_groups` (time out on master too) fail before this branch as well.

Review and verification round (2026-10-05): a run's workflow status of a shared task is recorded with the status it
changed from and sent (as soon as a run changes, and with every sync) only on the revision read while the server still
has that status and the person holds the task; otherwise the server's stays and the task page says why, quietly, until
dismissed (`shared_statuses statuses_`, the reviewer's reproduction). `orchester.toml` is never written through a link
(created only where nothing is). A project's tasks take no change from the screens while it is being shared, and a
run's change made meanwhile is sent on top of the uploaded copy. A run claim taken for work that did not start is
released while no run of the task is under way. The Sharing tab and Settings › Servers ask the server who is signed in
as they open (a dropped admin flag leaves the member's view of 10.14 B), the person's role in the members list gates
the controls, and an error of one member change no longer shows in the next dialog. Settings › Servers details fit a
960 window (the list gives way to 220; the sessions drop Signed in, the users drop State). A save that meets a newer
version is not a status-bar failure, and the same work done after all settles its earlier failure. Signing in from a
discovery offer returns to the Sharing tab; *What sharing sends* has the raised surface. Not covered by a test: the
refusal of screen changes while a Share is under way and the re-sending of a run's change made meanwhile (no hook to
pause a Share in a test), and the link refusal of `orchester.toml` on Windows (making a link needs a privilege; the test
runs where links can be made, i.e. macOS).

Second review round (2026-10-05): `claim_for_work` says whether it took the claim or confirmed one the person held
(after a finished run, or working on their other computer); work that does not start releases only a claim its attempt
took. Before sending a run's status the tasks and claims are read from the server, so a claim taken over a moment ago
sends nothing; a status the server refuses is not retried and the task says so. The tasks changed while sharing are
read in the transaction that links the project. At 960 the server card's connection label gives way to its icon so
the name stays whole. Not covered by a test: a status the server refuses (no server rule refuses one to provoke).

Design verification needed: 10.11 `xnhV2`, 10.12 `IDfLs`, 10.13 `V4ZxQG`, 10.14 `NaUDV`, 10.17 `g5r5VU`, 10.41
`RmDpQ`, 10.42 `Cc4WH`, 10.44 `Y6H8A`, 10.43 E, and Settings › Servers' details header at 960.

lurq 0.40.0 · branch `claude/lurq-040` (2026-10-05)

- L1 (`lurq_resize` left a minimized window minimized) is fixed upstream: lurq 0.40.0 restores a minimized window
  before resizing it, `lurq_resize` reports the size the window took, and `lurq_windows` shows `minimized`,
  `maximized` and `full_screen`. Orchester neither persists its window size nor uses `on_size_changed` or
  `WindowHandle::resize` (`main.rs` asks for 1440×1020 on every launch), so nothing needed adapting; drivers may
  start the app minimized again. Only `lurq` moved in `Cargo.lock` (`windows` 0.62.2 stays for wgpu-hal and
  gpu-allocator).
- The first password of 10.08 B (`kT0ui`, `pUTsa`) is shown in clear as the design draws it, as `Text::sensitive`:
  the MCP and DevTools trees show `•••` with `sensitive=true`. S2a showed it as a masked, read-only input with Copy
  because lurq could not keep visible text out of the MCP tree. `FirstPassword` holds it as
  `Arc<Sensitive<Credential>>` (redacted in DevTools' signal values and history; `Credential` zeroizes when the last
  holder drops it, as the dialog closes). No other secret is shown in clear: typed ones stay in masked inputs, stored
  ones are shown only as a mask.
- Tests: `form_keyboard servers_admin_creates_resets_disables_and_enables_users` reads the tree through the embedded
  MCP server (`lurq_read_tree`, `lurq_find_by_id`, `lurq_inspect`: no password, `•••`, `sensitive=true`), finds the
  password among the painted quads and signs in with it; `servers_first_password_copy_puts_the_shown_password_on_the_clipboard`
  (ignored: it uses the system clipboard and restores it) checks Copy.

S2a · Desktop: servers and sign-in · branch `claude/s2a` (implemented 2026-10-04; design verification and review pending)

Implemented in `.tmp/worktrees/claude-s2a` from master `b96dbf27`, with master `bffeec30` (S1b) merged in.

- `orchester-api`'s `client` feature: reqwest 0.13 with rustls (ring) and the OS trust store through
  rustls-platform-verifier; `ServerAddress` normalizes an address and knows loopback hosts; plain HTTP to another
  computer is refused without the opt-in; redirects are never followed but returned with their target; the token only
  in a sensitive `Authorization` header.
- `servers` table in the application database (`orchester-store`); the token in the OS keychain under
  `orchester-server:<server id>` / `token` (`VaultKey::server_token`), tested against the real Windows Credential
  Manager.
- Desktop `sharing`: the servers service (add, sign in with the forced new password, sign out, sessions and revoke,
  change password, admin users), the connection monitor (`GET /auth/me` at start and every minute; backoff 1 s
  doubling to 60 s with jitter, then *offline*; `401` signs this computer out and removes the token) and the
  window-wide server feed. A server is *in use* while an account is signed in to it from this computer, or was until
  the server refused its token; until shared projects exist (S2b) those are the servers the status bar shows.
- Settings › Servers (`Page::Servers`, sidebar *Settings*), the list and details with Account and Users, the dialogs
  of 10.02–10.05 and 10.08, the signed-out state of 10.06, and the status bar server item of 10.43 in the bar's
  give-way order (its words before the runner's; its attempt and times drop out with the times). Passwords are typed
  into masked inputs (masked in the MCP tree too) kept out of DevTools' signal history and wiped when a dialog closes.

Not in S2a because it depends on shared projects (S2b): the *Sign out?* confirmation of 10.05 F (signing out acts at
once), the shared-project rows of 10.06 *While you are signed out*, *Shared projects here* (reads *None yet*), the
status bar's *Not a member* state (10.43 E) and the project names in its hint. Not reachable in practice: 10.08 E
(the last admin) needs the signed-in admin not to count as an admin; the server's `409 last_admin` shows it.

Tests: `cargo test -p orchester-api --features client --test client client_` (11) and `--test client_redaction
client_` (1); `cargo test -p orchester-store --test app_db servers` (2); `cargo test -p orchester-vault --test
credential_manager credential_manager` (3, Windows); `cargo test -p orchester-desktop --test sharing sharing_` (9),
`--test form_keyboard servers::servers_` (7), `--test navigation route_`, `--test theme roles`, `--test i18n`,
`--test form_keyboard sidebar`, `--test status_bar status`; `tools/check.ps1` passed. The macOS check
(`cargo check --target aarch64-apple-darwin`) passes for `orchester-core` and `orchester-vault`; the desktop and the
api client cannot complete on this Windows host without the Apple SDK (ring, tree-sitter and bundled SQLite need a C
compiler for the target), as for the rest of the workspace.

Follow-ups of the 2026-10-04 reviews and verifications · branch `claude/followups` (from master `bffeec30`)

- **Implemented 2026-10-04.** *Runs of this task* (ORC-63) ends the plan review (02.11, after the plan in its
  scrolling viewport) and the retry-limit page (02.B, after the evidence, which now scrolls with the scroll gutter
  beside the decision panel). A worktree folder git forgot is read only to its first 200 entries in name order
  (`FolderListing::Cut`: *more than N files*), and an entry that cannot be read ends its listing with why
  (`FolderListing::Unreadable`), shown under its run's row and in the removal dialog instead of failing the whole
  section; removing such a folder needs the person's agreement to the listing as read (IPC protocol **16**). The
  start-up recovery no longer lists a run that waits for a person's answer or plan decision as interrupted (the runner
  leaves it waiting and its agent's session resumes with the answer); one held at a tool approval is still listed. C6:
  the run permission popover's anchor in `W29Od2` is the superseded 02.12 Working context row; today it opens above
  the This run panel's row (ORC-44, verified) or above the composer's chip (ORC-61C), so nothing changed there; the
  approval dialog's explanation names no path since approval choices (2026-10-02) and long values show whole, and its
  working directory now reads `.orchester/worktrees/<id>` with `/` on Windows too. The minimized-window size is lurq's
  (L1). Tests: `cargo test -p orchester-desktop --test form_keyboard plan_review_shows_the_plan`, `... task_graph_retry_limit`,
  `... run_history`, `... command_approvals`, `cargo test -p orchester-core --test integration forgotten` (also
  `recorded`, `cleanup`), `cargo test -p orchester-ipc --test framing frames`, `cargo test -p orchester-desktop --test
  recovery recovery_lists`. **Design verification outstanding** for the plan review and retry-limit pages with the
  section (no frame draws it there; it follows 63.01's rows) and the *more than N files* / unreadable-entry lines
  (not drawn).

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
  - Following live (design verification, 2026-10-04): a timeline that follows goes to its end on every render, not
    only when it has more rows than before (a step folding as the next one starts can leave as many), and after each
    layout it goes there again when it came short (rows measured taller than estimated), so *Following live* never
    shows anything but the end; opened right after a fast run started, it no longer stays at the top.
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
  `conversation_a_step_that_judges_the_work_…` and `conversation_a_question_that_arrives_while_searching_…`,
  `conversation_following_keeps_the_end_in_view_…`),
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

U10 · Pull requests screen jumps while it loads · branch `claude/u10` (done 2026-10-05: independent design verification passed)

Until the stored inbox is read the screen shows its topbar and heading only; the tab row (linked repositories only,
08.D has none) and the list or unavailable panel then appear together in their final place, so nothing shows that
later moves or vanishes. The Open tab keeps room for a count of up to `SYNC_LIMIT` (two Geist Mono digits,
`Tab::count_room`, `theme::COUNT_DIGIT_WIDTH`), so a first count or one that grows to 50 moves no tab; a single-digit
count leaves the Open tab 6px wider than in `DF8II`. The topbar's spacer already kept Refresh in place while the sync
status changed (measured at 1440 and at the 960 minimum width). Tests: `cargo test -p orchester-desktop --test
form_keyboard pull_request` (the new `pull_requests_layout` tests hold the sync's fake `gh` behind a gate and record the
heading, tab, list and Refresh bounds before and after the stored inbox and the sync arrive), `cargo test -p
orchester-desktop --test theme pull_request`. **Done when** an independent design verification against `DF8II`
(08.01 `wGtxT`) and 08.D `dmib5` reports no blocking or major mismatches. Rechecked 2026-10-05 on master
58b076f3: the 28 `pull_request` form_keyboard tests and the 2 theme tests pass; the copy of this task under
*U · Screens* that the merge 851da1e5 brought back was removed.
Design verification 2026-10-05 (independent, 1440 and 960, fake `gh` behind a gate): no blocking or major
mismatch; tabs and Refresh keep their place through the count, Syncing, Synced and a Refresh sync, and 08.D shows no
tab row in any frame. Minor: the tab row sits ~3px lower than `DF8II`; on a first sync the list area stays blank
under the tabs until the sync arrives (no loading state is drawn). The frame before the stored inbox lasts under
0.2 s and was not captured live; the tests cover it.

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
C5 · Model requirements are recorded and saved without lost updates · branch `claude/c5` (`ConnectionStore::record_model_requirement`: one immediate transaction writes only the connection's settings; a removed connection is `ConnectionNotFound` and is not created again, a native one `NotAnAgentCliConnection`; the runner logs a removed connection instead of failing the run. `save_connection` merges the stored requirements into what it writes (`ModelRequirements::merge`, higher version wins), so a desktop copy read before the runner's record keeps it; no path clears a requirement. Tests: `orchester-store --test connections model_requirement` 5 passed and `connection` 14 passed, `orchester-runner --features test-agent --test claude claude_refused_model` and `--test execution execution_rejects_what_the_preflight` passed, `orchester-desktop --test connections connection` 8 passed; `tools/check.ps1` passed).
M8 · Orchester's typed agent tools (`git_status`, `git_diff`, `git_log`, `git_show`, `git_add`, `git_commit`,
`read_outside_file`) served to Claude Code and Codex by the runner's MCP relay · merge `6f027444` (review, re-review).
M9 · Orchester's file tools for Claude Code and Codex (`read_file`, `list_files`, `search`, `edit_file`, `write_file`,
`find_symbol` with the code index) · merge `6a7d57bd` (security review). A live run with the real CLIs was still open
when they were merged.
ORC-132 · Local use without an Orchester Cloud account · branch `orchester/orc-132-run-1`: the sign-in gate is gone; a
project or server added with nobody signed in is this computer's (`owner` NULL, no migration), moved by a person on All
projects; sharing, joining, invites and team servers offer the sign-in in place; MCP answers `sign_in_required` only for
a named project an account holds while nobody is signed in. Open: the *Verify* step's design verification, macOS
unchecked. The three-way start page stays ORC-293.
ORC-613 · Wait for required Codex MCP servers · branch `orchester/orc-613-run-1`: bounded cancellable thread-scoped readiness on fresh start, quota resume and retry; missing/terminal/timeout errors retain redaction and server names. Evidence: `docs/evidence/orc-613-codex-mcp-readiness.md`.
