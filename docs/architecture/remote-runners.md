# Remote runners — preparing the local runner

Date: 2026-11-03, revised 2026-11-08 and 2026-11-09. Status: the preparation is agreed; the user flow and the
person's decisions on the agent computer are recorded in [User flow](#user-flow-2026-11-08), with the questions still
open listed there; the wire protocol and the exact contents of what a run is sent are not designed yet (Orchester
module *Remote runners*, branch `module/remote-runners`).

## Goal

The person runs agents on another computer of theirs (a mini PC) and controls and sees everything from the main one,
as if the run were local: the conversation, approvals, questions, live diff, browser, merge. The project, its tasks,
its checkout and the merge stay on the main computer, and so do secrets, settings and pipelines (decision 7). The
agent computer keeps only its program, its pairing, its copies of the code and one CLI sign-in per CLI.

Nothing of that is built here. This document is the **preparation**: changes to the local runner and the desktop
that keep today's behaviour exactly, after which a run's execution can move to another process — later another
computer — without a second code path. Parity holds by construction: the local mode uses the same boundary every day.

## The split

`orchester-runner` does two different jobs today (`crates/runner/src/execution/`):

| Coordinator — one per project, beside the project database | Executor — one per run, where the run's worktree is |
| --- | --- |
| Run creation: number (`counters`), id, branch name, base revision and branch read in the person's checkout (`prepare.rs`, `again.rs`), then a serializable start order (`orchester_core::run::StartOrder`) for the executor | Creating and removing the run's worktrees at a given revision (`executor/start.rs`, `executor/removal.rs`, `executor/worktree_sweep.rs` over `runner/src/worktree.rs`), the tips of the runs' branches at startup (`executor/run_branches.rs`) |
| Queue, task groups, blocked runs, readiness (`task_groups.rs`, `blocked_runs.rs`, `prepare.rs`) | Steps, agent processes, the CLI relay (`approval_relay.rs`, `step/relay.rs`), checkpoints |
| Agent slots and the daily budget, plan limits, low disk, held runs as gates (`pool.rs` slots, `idle.rs`, `halt.rs`, `held.rs`, `low_disk.rs`, `plan_limit/`) | Step budgets, the run's tool calls and approvals relay, questions, plans |
| Orchestrator chat (`chat/`), task estimates (`estimate.rs`) | Code index of the run's base revision (`codebase_index.rs`), QA apps (`step/qa_app/`, `step/run_app/`) |
| Which runs have queued messages (`conversation.rs`), base notices to deliver (`step/base_notice.rs` reads what the desktop queued) | Warm workspaces it borrows (`pool.rs` folders, `workspace/`) |
| Every write to the project database | Nothing written to the database itself |

The browser stays as it is during the preparation (one per project, `runner/src/browser/`). For a run on an agent
computer it runs on that computer (decision 17); which browser program it uses there is set in Settings › Computers
from the programs the worker reports (decision 7).

**Rules of the boundary**

> Revised by decisions 4 and 7 (2026-11-09): the main computer's secret values, MCP servers and the project's
> variables leave it only to be sent to an agent computer for a run, end-to-end encrypted. Where rules 1, 3 and 4 and
> the Inventory below put them in *the computer's resources* or say values never cross, that holds for the local mode
> only; for an agent computer they come with the run through the coordinator link.

1. The executor never opens `ProjectDb`, `AppDb` or the keychain. What it needs comes through two narrow interfaces
   (real boundaries in the sense of AGENTS.md rule 8: a local and, later, a remote implementation):
   - **The coordinator link** — reads that belong to the project (resource versions by id, messages and attachments,
     memories, approval rules, pending approvals and questions, base notices, the project settings an agent start
     reads, the step's usage) and the sink of everything it records. Reads keep today's timing: what is read at each
     agent start today is still asked at each agent start, not frozen at the run's start.
   - **The computer's resources** — what belongs to the computer the executor runs on: connections and their
     accounts and CLI config folders, secret values from the keychain (MCP servers, API keys, redaction), the browser
     program, the project environment's values, and the executor's paths.
2. Everything the executor records is a value sent to the coordinator's sink; the local sink writes it with today's
   store functions in today's transactions. Records that update rows in place (`runs`, `step_executions`,
   `tool_approvals`, `questions`, `invocations`, deliveries) are sent as the change the store applies, so the order
   and the transaction stay the same.
3. Rows the desktop owns are written only by the coordinator: *Always allow* (`mcp_servers`, `resource_versions`,
   `resource_drafts`, `step/answer.rs`), the model requirement of a connection (`connections`, `step/cli_version.rs`),
   new memories and their changes. Done (ORC-680): the executor sends them through the sink as requests
   (`orchester_store::{RuleRecord, AlwaysAllowRecord, ModelRequirement, MemoryRecord}`), and the coordinator applies
   them with the store's own functions and answers what the executor needs to go on (the rule's id, Always allow
   unavailable, the kept note's id and status). Approval rules and their counts (`approval_rules`,
   `project_approval_rules`) go the same way. The executor screens memory content against its own computer's secret
   values before it sends it.
4. Every value that crosses the boundary is serializable (serde), and the tests run the local executor through a
   channel that serializes every message, so nothing that only works in one process slips in. Done (ORC-692): in
   test-agent builds `Scheduler::over_channel` (`coordinator/channel/`) gives a run's task a link, a sink and
   computer's resources that send each request as JSON to a thread of the coordinator's, which answers with the local
   implementation, and go on with the answer read back from JSON; `crates/runner/tests/serialized_boundary.rs` runs
   the main scenarios that way and in process and compares the execution rows they leave. Not text yet: the sink's
   store errors (`StoreError`, `RunStoreError` do not serialize), and the computer's secrets, values and MCP servers
   (`Secret`, `McpServer` do not serialize; values never cross). The executor's remaining `ProjectDb` uses are listed,
   with why, in `crates/runner/tests/execution_split.rs` (`STORE_USES`); a new one fails there.
5. The desktop reaches a run's worktree only through the runner: reading its uncommitted changes and files, resolving
   a conflict in it, syncing the base into it, running Check tools in it, removing it, its recovery state, warm
   workspaces' folders, the Orchestrator chat's git tools in it. The person's own checkout (local integration's merge,
   pull requests, unpushed, base-moved following, checkpoint diffs from the shared object database) stays the
   desktop's.
6. The executor's folders come from one place (its execution root) instead of being joined to the project root at
   each use: `.orchester/worktrees`, `workspaces`, `runs/<run>`, `skills/<execution>`, `tools`, `index`, the private
   secret files, the QA discovery and slot folders. Locally the execution root is the repository, as today.

## Inventory (2026-11-03)

What the executor side reads today, and through which interface it will:

| Read today | Where (`crates/runner/src/execution/`) | Through |
| --- | --- | --- |
| Pinned resource versions by id, skill blobs | `permissions.rs`, `step/skills.rs`, `step/start/{prompt,shared}.rs`, `tool/mod.rs`, `work.rs` | coordinator link |
| `messages`, `attachments` and their files | `step/mod.rs`, `step/worker/steer.rs`, `recovery.rs` | coordinator link: done (ORC-667, `executor/link/messages.rs`), with `conversation.rs` and `get_task` (`step/run_records.rs`); the files' content travels in the answer and the executor keeps its copies in its execution root's `attachments/` (`executor/attached.rs`). Delivery state is still written to the database (ORC-680) |
| `memories` | `step/memory.rs`, `handoff.rs`, `step/run_records.rs` | coordinator link: **done** (ORC-668, `executor/link/memory.rs`): the recall and the unreadable records. New records and their changes go through the sink (ORC-680, rule 3); the coordinator reads the Memory setting itself when it applies them, so the executor no longer asks the link's `settings` for them |
| `approval_rules`, `project_approval_rules`, `tool_approvals`, `run_auto_approve`, `questions` | `step/answer.rs`, `step/approval.rs`, `question.rs` | coordinator link: done (ORC-669, `executor/link/{approvals,questions}.rs`), with `tool_approval.rs`, `step/question.rs`, `step/worker/` (`in_autopilot`), `conversation.rs` and `get_task`; matching against the rules stays the executor's. Writes of approvals and questions go through the sink (ORC-679), rules and their counts too (ORC-680). The coordinator's own reads (`coordinator/attach.rs` interrupting a cut-off run's requests) stay on the database |
| Project settings an agent start reads (`code_index`, `repository_agent_files`, `memory_settings`, `approval_mode`, `auto_mode_settings`, `tells_base_moves`) | `step/start/*`, `step/skills.rs`, `handoff.rs`, `step/worker/review/setup.rs`, `step/base_notice.rs` | coordinator link |
| Step usage (`invocations` tokens, `reviewer_usage`) | `step/mod.rs`, `step/start/shared.rs` | coordinator link |
| `base_notices` | `step/base_notice.rs` | coordinator link |
| `connections`, account folders, CLI config folders | `step/start/{shared,cli}.rs`, `permissions.rs`, `step/worker/review/*`, `codebase_index.rs`, `step/roots.rs` | computer's resources |
| `mcp_servers`, secret metadata and values (`KeychainVault`, `server.rs:66`) | `step/mcp.rs`, `connection_check.rs`, `step/run_records.rs`, `plan.rs`, `step/memory.rs`, `handoff.rs`, `step/worker/review/*` | computer's resources: **done** (ORC-673, `runner/src/computer.rs`); the vault is the local computer's, the coordinator's chat, estimates and connection checks read through it too, and in tests every link message and sink record crosses as JSON (`executor/link/sent.rs`). Always allow's read of the servers' listed tools stays with its write (`step/answer.rs`, rule 3) |
| `project_environment` | `environment.rs`, `step/mod.rs`, `codebase_index.rs`, `merge_queue.rs`, `step/worker/review/*` | computer's resources: done (ORC-674). The project's variables come through the coordinator link (`executor/link/environment.rs`), at each use as before, and `ComputerResources::environment` makes the environment of that computer of them (locally the project's, unchanged); secret values never take this path. The Orchestrator chat's read (`coordinator/chat/turn.rs`) stays the coordinator's |
| Settings › Browser from `orchester.db` (`runner/src/browser/mod.rs:403`) | browser | computer's resources: done (ORC-675). The desktop reads the chosen program and the folder of the browser profiles (`Workspace::computer_browser`) and passes them as `ComputerBrowser` with `Command::Attach` and again with `Command::SetBrowser` when Settings › Browser changes (protocol 47, `Runners::set_browser`, `follow_browser_settings`); the runner keeps them in its `ProjectBrowser` (`set_computer`) and uses what it was passed last at each browser start. The runner no longer opens `AppDb` or reads its application data location |

Where the desktop touches a run's worktree today (`crates/workspace/src/`):

| What | Where |
| --- | --- |
| Uncommitted changes, followed while the page is open: **through the runner** since protocol 40 (`Command::ReadWorktreeChanges`, ORC-682) | `tasks/changes/mod.rs` (`not_checkpointed_in`), `runner/worktree_changes.rs`; the runner's `execution/executor/worktree_changes.rs` |
| Whether a run's work is present (its own worktree folder, else its branch) | `tasks/changes/mod.rs` (`run_work_present`) |
| Conflict resolution in the worktree (resolve by hand, a resolved finish, an abort, the worktree's head): **through the runner** since protocol 46 (`Command::Resolve`, ORC-683); the desktop keeps the resolution's record | `tasks/resolve/mod.rs`, `tasks/integration_jobs/resolve.rs`, `merge_queue/approve.rs`; the runner serves core's `integration::resolve::request::serve` |
| Base synced into the worktree, Check tools run in it | `merge_queue/drive.rs:211`, `merge_queue/check.rs` |
| Local integration reading the worktree as a second repository | `tasks/local_integration/{mod,poll,view}.rs`, see [Local integration](#local-integration) |
| Removal, stopping its processes, index prune, sweep: **through the runner** since protocol 49 (`Command::RemoveWorktrees`, ORC-686); the desktop records the removal before and after, and its own sweep is gone (the runner's covers the same folders) | `tasks/worktree_removal.rs` (`WorktreeRemover`), `tasks/integration.rs` (`clear`), `tasks/delete.rs`, `runner/worktree_removal.rs`; the runner's `execution/executor/worktree_removal.rs` |
| Whether a run's worktree folder is there, for `get_task`/`get_run` and the missing-worktree note | `app_mcp/tools/worktree.rs` (`worktree_state`), `tasks/history.rs` (`note_missing_worktrees`): still local reads, so read-only tools start no runner |
| Files of the run's version: **through the runner** since protocol 48 (`Command::RunFiles`, ORC-696; a listing travels in pages that each fit a frame) | `files/runs.rs`, `runner/run_files.rs`; the runner's `execution/executor/run_files.rs` |
| Open in editor, copy path of a run's file: still the local path, marked for a run on another computer | `files/open.rs` (`open_target`), `run_facts.rs:329` |
| Recovery state of a run's folder: **through the runner** since protocol 44 (`Command::RunFolderState`, ORC-688) | `recovery/mod.rs` (`read_folder_states`), `runner/ask.rs`; the runner's `execution/coordinator/run_folder.rs`, `execution/executor/folder_state.rs` |
| Warm workspaces' folders and sizes, ended worktrees' sizes | through the runner since ORC-689 (`Command::Folders`, `runner/folders.rs`); the desktop keeps the database's reservations |
| Storage's inventory: which worktree and workspace folders are there, each row's size, the clean-up dialog's sizes | through the runner since protocol 51 (`FoldersCommand::List`, `FoldersCommand::Measure`, ORC-746): listed unsized, then measured row by row as before; the rows' rules stay the desktop's, from the database (`storage/inventory.rs`) |
| Code index rebuild | Done (ORC-690): the runner builds it on `Command::RebuildCodeIndex` (protocol 43); `code_index::rebuild` asks it |
| Orchestrator chat git tools in a run's worktree: **through the runner** since protocol 45 (`Command::RunGit`, ORC-691) | `orchestrator/git/host.rs`; the runner's `execution/executor/run_git.rs` |

### Local integration

Local integration (`crates/workspace/src/tasks/local_integration/`) merges a run's checked result into the person's
checkout, and that merge stays on the main computer. The merge itself needs only the result's commit
(`LocalGit::merge_result` on the checkout, `has_merge_receipt`), which today is in the shared object database and
later comes with the run's branch. What the result *is*, though, is made in the run's worktree: the base is synced
into it, its conflicts are resolved in it, and Check runs in it. So these uses of the worktree stay until the runner
serves them (ORC-685 found them; no IPC was added there, and storage.md "Integration" and the ORC-630 holds are
unchanged):

| Use of the run's worktree | Where | Runner request it needs |
| --- | --- | --- |
| Open it as a linked worktree of the checkout, on the run's branch: `owned_directory`, `LocalGit::new`, `own_metadata`, `--git-common-dir` and `symbolic-ref HEAD` checks | `mod.rs` `open_expected`, `ensure_linked_task` | Worktree state (the branch, head and status of the run's folder), with ORC-684's sync request |
| Snapshot, clean check, tree entries and the merge of the base (`preview_sync`, `refresh_index`, `merge --no-ff --no-commit`), conflicts after it; recovery of an interrupted sync | `sync.rs` `sync`, `execute_sync`; `recover.rs` `reconcile_sync` | ORC-684's base-sync request |
| Conflicts, file hashes and blobs, index entries and fingerprint, resolution bytes, writing a resolution (`write_regular`, `set_executable`, `set_index`) | `resolution.rs`, `sync.rs` `save_task_resolution`, `drafts.rs` `draft_identity`, `view.rs` `read_conflicts` | Conflict-resolution requests like ORC-683's `Command::Resolve` (read conflicts and their versions, write a resolution); not built: ORC-683 moved only the run's own resolution |
| Committing the resolved sync and proving it by receipt (`commit_task_resolution`, `HEAD^1`/`HEAD^2`, `has_task_commit_receipt`) | `sync.rs` `continue_task`, `task_commit.rs` | A "continue after resolving" request like `Command::Resolve`'s finish; not built |
| Aborting the sync merge: preview, retaining the files, `abort_task_merge` | `abort.rs` | An abort request like `Command::Resolve`'s abort; not built |
| Running the pinned Check tools in the worktree, with the process journal, and its snapshot before and after | `checks.rs` `run_check` | ORC-684's Check request |
| The worktree still exactly the checked result, before the merge and while the page shows it; its status fingerprint for polling | `sync.rs` `approve_and_merge_at` (`ensure`), `view.rs` `checked_current`, `poll.rs` `fingerprint` | Worktree state (as above), verified (every tracked file hashed) before the merge |
| The worktree clean and still the owned one when a preparation is canceled or reconciled | `cancel.rs`, `recover.rs` (`TaskConflicts`, `Checking`) | Worktree state (as above) |

The view and the poll read the worktree only while the page shows something of it, task conflicts or a checked result
(`poll.rs` `shown_task_root`). Every other read takes the run's result from its branch and the checkout's repository.

**The scratch folder.** `LocalGit` gives each checkout it opens a scratch folder inside that checkout, so the run's
worktree gets `.orchester/local-integration`. Snapshots build their index there (`capture.rs` `rebuilt_status`),
and so do sync previews, abort previews and resolution merges. It has to be inside the opened root because
`LocalGit::new` refuses any other place, and on Windows `ExecutionOwnership` holds the root and the scratch folder
together and allows private copies only under that scratch folder. Proposal, not done here: once the runner serves
the worktree requests above, the scratch folder belongs to the executor. Put it in the run's storage under its
execution root, `.orchester/runs/<run>/local-integration` (rule 6, `ExecutionRoot::run_storage`). That keeps it with
the run's other executor files and out of the folder the agents and Check work in. It needs `LocalGit::new` to
accept a scratch folder its caller holds outside the root,
with `ExecutionOwnership` holding both the same way. On the main computer, local integration then keeps only the
checkout's own `.orchester/local-integration/<run>`.

## User flow (2026-11-08)

Agreed with the person in conversation on 2026-11-08. Words: the **main computer** runs the Orchester app and owns
the projects; an **agent computer** runs `orchester-worker`, a program with no window of its own, in the person's session; **Orchester Cloud** holds both
accounts, pairs the computers and relays their traffic.

### Decisions

| # | Decision |
| --- | --- |
| 1 | Windows and macOS only, on both sides. |
| 2 | No service (revised 2026-11-09): `orchester-worker` is an ordinary program in the person's desktop session, started at sign-in (a LaunchAgent on macOS, a start-at-sign-in entry on Windows) and needing no administrator rights. So it can start GUI programs (QA apps, the browser) and reads the CLIs' sign-ins and the keychain as the person does. It runs only while the person is signed in; a computer that stands alone should sign in automatically, which `doctor` and the computer's page suggest. |
| 3 | The program is `orchester-worker`, a separate binary, shipped as a zip/tar.gz with each release and signed with the release key. |
| 4 | Orchester Cloud relays the traffic between the computers and can read none of it (revised 2026-11-09): every message is end-to-end encrypted with keys made on the two computers and exchanged at pairing; both screens show the same short check code at pairing and the person compares them before **Connect**, so the cloud cannot put its own key in between. Through the relay go what a run is sent, its events, messages, approvals and questions, small on-demand reads (the live diff between checkpoints, a file of the run) and piped MCP streams (decision 17). Secrets, MCP servers and the project's variables leave the main computer only this way and only to the agent computer of the run. Code never goes through the cloud (decision 5). Remote computers need a Cloud sign-in in the app. |
| 5 | Code moves only through the project's git remote (`origin`); no bundles (2026-11-09). The agent computer fetches what a run needs and pushes the run's own branch (named as for a local run) to `origin` after each checkpoint; the main computer fetches it. A base `origin` lacks goes to the service ref `refs/orchester/base/<run>` (decision 14), deleted after the run's merge or deletion. |
| 6 | A computer belongs to one person's account, not to a project: paired once, any of that person's projects can send runs to it (no per-project allow list for now). It is not shared with a team; in a team project each person's runs go only to their own computers. A project holds only where its runs go and its per-computer variable overrides. |
| 7 | Secrets, settings and pipelines live only on the main computer. The agent computer has no store of project secrets, no project variables, no pipelines, profiles, instructions or limits, and no settings of its own that the app does not set. |
| 8 | Sign-ins: one account per CLI kind on an agent computer, made with that CLI's own sign-in there (`claude`, `codex login`); its tokens never leave it. API providers (DeepSeek, OpenRouter, the Anthropic API, …) stay on the main computer, and their keys travel with a run as secrets. |
| 9 | Agent slots are counted per computer and set in the app, shared by every project that sends runs there. A run starts only with both a slot of its project (the project's limit, as today) and a slot of its computer. A freed computer slot goes to the run that was queued first, whichever project it is from. Each project keeps its own code cache on the computer, and a run's package carries only its own project's secrets. Settings › Computers shows the computer's slots by project ("A: 2 · B: 2 · free 0"), and a waiting run says what it waits for ("mini-pc: all 4 slots taken"). |
| 10 | A run goes where its task says, or else where its project says ("This computer" by default). It never moves to another computer on its own; a run that cannot start there (computer offline, CLI missing or not signed in, worker version mismatched) waits with the reason shown. |
| 11 | The agent computer works only while it reaches the main computer (revised 2026-11-09). When the connection is lost, from either side, the worker stops the run's agents at once, as Pause does, and the run waits as "no connection to Ivan's PC since 14:32"; it goes on when the connection is back. So every read keeps today's timing through the coordinator link (rule 1 of the boundary) and nothing is frozen at the run's start. |
| 12 | Disconnecting a computer with active runs warns with the list of runs and, once confirmed, interrupts them; they go to recovery and can be run again elsewhere. |
| 13 | The agent computer reads and writes `origin` with a git token the main computer sends with the run as a secret (decision 7); it has no git sign-in of its own. A project without a remote cannot send runs to agent computers, with that reason shown. The agent computer pushes only the run's branch; pull requests and merges stay on the main computer. Git LFS objects and submodules are not carried yet: such a project is shown as not supported on agent computers yet, and its runs there do not start. |
| 14 | A run's base is fixed by the main computer, as for a local run: the base branch and its commit as they are in the person's checkout when the run starts (a module's branch included, unpushed commits included). When `origin` lacks that commit, the main computer pushes it to `refs/orchester/base/<run>`. The agent computer gets the exact commit, not a branch name, fetches it, creates the worktree at it and checks the worktree's HEAD is that commit before any agent starts; a mismatch stops the run with the reason. What it fetched before is only an object cache; no run starts from it. Uncommitted changes in the person's checkout are not part of the base, as locally. |
| 15 | When the base moves during a run (another task merged into the module on the main computer), the main computer pushes the new commit to the run's base ref and sends the base notice as today; the agent computer fetches it and syncs the base into the worktree. |
| 16 | The run's branch is pushed to `origin` after each checkpoint, so the main computer reads Changes, per-step diffs and History from its own repository and the work up to the last checkpoint survives the agent computer. |
| 17 | Scope of an agent computer (revised 2026-11-09): it writes code, builds the project and runs its tests, QA apps and their own MCP tools and the browser included, all on itself. MCP servers tied to the main computer (Pencil, services on its `localhost`) are **piped**: the agent on the agent computer gets `orchester-worker pipe <run> <server>` as that server's command, which passes the MCP stream byte for byte through the control channel (Orchester Cloud relay, decision 4) to the main computer, where Orchester starts the real server (or connects to its local address) and Pencil answers as for a local run, screenshots included. Steps do not move between computers for this. A person marks which servers are reached that way and allows it, per server (person-only, it widens what agents may do). While the main computer is unreachable the run is paused anyway (decision 11). Paths a piped server gets refer to the main computer, which matches the rule that Pencil is given the main checkout's `.pen` path. |
| 18 | Kept only in memory on the agent computer (2026-11-09): what a run is sent (secrets, variables, MCP servers, pinned resources), the journal of events not yet delivered, and anything else the main computer sends. Nothing of it is written to disk; a worker restart loses it and the run goes through recovery. The run's worktree, the build outputs and the per-project object cache are on disk, as building needs them. |
| 19 | One agent computer serves one main computer. The worker arbitrates its own slots: the projects' runners ask it for a slot with the time the run was queued, and it grants a freed slot to the earliest (decision 9). It reports its free disk and checks keep-free itself before a run starts there. |
| 20 | The worker's version equals the app's. When the app updates, runs on the agent computer pause, the worker updates itself to the same version and the runs go on through recovery; a worker that cannot update keeps its runs paused, with the reason and `orchester-worker update` shown. |
| 21 | In a team project an agent computer is one member's computer: where runs go (the project's default computer and its variable overrides) is that member's personal setting, kept on their main computer and not shared with the team. Teammates see a run's computer only as its name on the run. |
| 22 | Disconnecting (revised 2026-11-09): the main computer revokes the computer's key in the cloud at once. An online worker stops its runs, removes their worktrees and per-project caches and forgets its pairing. An offline worker reaches no main computer and so runs nothing (decision 11); the first time it reaches the cloud it learns of the revocation and cleans up the same way. The app then lists the secrets runs sent to that computer, so the person can rotate them. |
| 23 | Clean-up while the agent computer is offline: a removal the main computer asks for (after merge or deletion) is kept and done when the computer is back; Settings › Computers shows what waits. Per-project caches unused for 30 days are removed by the worker. Storage lists the agent computer's folders under its name. |

**What a run is sent.** At each agent start the agent computer asks the main computer, through the coordinator link,
for what that start reads today (rule 1 of the boundary), and with it the secrets, variables and MCP servers that
start needs. It keeps them in memory (decision 18) and drops them when the run ends. While the run is going its agents
see its secrets as they do locally: "only on the main computer" means not stored elsewhere, not hidden from the run
(see the open question on the git token).

### Setting up an agent computer

1. Unpack `orchester-worker` and run `orchester-worker install`, which copies it to its place and starts it at each
   sign-in (no administrator rights). It starts and says it is not paired yet.
2. `orchester-worker pair` prints a short code valid for ten minutes. In the app, **Settings › Computers › Add
   computer** takes the code and shows the computer's name, OS and worker version, with the warning that it will run
   agents and commands of the person's projects under their account there. Only a person in the window can press
   **Connect**; the MCP tools cannot. The console then shows which main computer and account it is paired with.
3. The person signs in to the CLIs on that computer. `orchester-worker doctor` (read-only) lists git, the CLIs and
   whether they are signed in, and free disk space; the worker reports the same to the app.

The worker's command line is only `install`, `pair`, `doctor`, `update`, `unpair` and
`uninstall [--purge]`.

**Settings › Computers › (computer)** on the main computer: online state, worker version, slots, keep-free disk
space, per-computer overrides of project variables, the CLIs found and their sign-ins, and in red what the projects
sent there lack ("Project X uses Codex — Codex is not signed in on mini-pc"). **Disconnect** revokes the computer's
key in the cloud (decision 12). A lost computer can also be disconnected from orchester.cloud.

### A run on an agent computer, seen from the main computer

1. **Queued**, then **Preparing on mini-pc…**: the main computer pushes the base when `origin` lacks it, and mini-pc
   fetches that exact commit and creates the worktree at it (decision 14).
2. **Running on mini-pc**, with the computer's badge beside the state. Conversation, approvals, questions, plan and
   live diff look as they do locally. QA apps and the browser run there; screenshots come into the conversation. The
   run's file viewer works; *Open in editor* and *Copy path* are unavailable with "the file is on mini-pc".
3. **Done**, then the run's branch is pushed to `origin` and fetched by the main computer; review and merge happen
   on the main computer as today. Changes and History read from the main computer's repository throughout
   (decision 16). Base sync, Check and conflict resolution of the run happen on the agent computer.
4. The run's worktree there is removed as today, after merge or deletion.

Usage of a CLI account signed in on an agent computer shows in Usage as its own row ("Claude · mini-pc").

### Without a connection

| Situation | What the person sees |
| --- | --- |
| The computer is offline before the start | The run waits in the queue: "mini-pc offline since 14:32" |
| The connection drops during a run | The worker pauses the run (decision 11); the main computer shows "no connection to mini-pc since 14:32". On reconnect the journal kept in memory is delivered and the run goes on |
| The main computer sleeps or is off | The worker pauses the run (decision 11) until the main computer is back |
| The agent computer restarts | The worker starts at the next sign-in; its agents count as interrupted and the run goes through recovery once the main computer is there |

### Updates

See decision 20: the worker's version always equals the app's.

### Open

- The daily token budget: shared per project across computers (proposed) or per computer.
- The wire protocol and exactly what each agent start is sent.
- Whether agents may see the git token (decision 13), or the worker keeps it for its own pushes only.
- How merging on the main computer replaces the run branch's reflog (storage.md "Rebased runs", `fork_point`).
- A base move (decision 15): the worker only fetches the new base and the agent merges it after the base notice, as
  locally (proposed), or the worker merges it.
- Base sync, Check and conflict resolution of a remote run before the merge: how the main computer knows the fetched
  commit is the checked result, and where a person resolves a conflict by hand.
- Sleep, sign-out and reboot of the agent computer: the worker keeps the computer awake while runs go (proposed).
- How a piped MCP stream is bound to its run (proposed: a one-time token per run and server in the agent's MCP
  configuration), and what a piped server's returned paths mean on the agent computer.
- CLI accounts: a profile that pins an account (Claude · Work) on an agent computer that has one account per CLI;
  plan limits and the CLI version requirement for that account.
- Moving a running task to another computer: proposed as Pause, then the new computer fetches the last checkpoint
  and the current step goes on in a new agent session with a handoff note; completed steps are not repeated.
- Syncing resources a message refers to (a skill, an attachment): proposed on demand through the control channel,
  cached on the agent computer by version id or content hash, no background sync.
- Choosing a computer per pipeline step (not planned: project and task only).

## Not part of the preparation

The network transport and pairing, the agent computer's program and its command line, moving code between computers,
Settings › Computers and choosing a computer for a task. Their user flow is agreed in
[User flow](#user-flow-2026-11-08); none of it is built by the preparation.
