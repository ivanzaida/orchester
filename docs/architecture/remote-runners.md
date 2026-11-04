# Remote runners — preparing the local runner

Date: 2026-11-03. Status: the preparation is agreed; the transport, pairing and the agent computer's own mode are
not decided yet (Orchester module *Remote runners*, branch `module/remote-runners`).

## Goal

The person runs agents on another computer of theirs (a mini PC) and controls and sees everything from the main one,
as if the run were local: the conversation, approvals, questions, live diff, browser, merge. The project, its tasks,
its checkout and the merge stay on the main computer. The agent computer keeps its own CLI sign-ins, API keys,
secrets and paths, and gets a command line of its own for setting it up.

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

The browser stays as it is during the preparation (one per project, `runner/src/browser/`); where it lives for a run
on another computer is decided with the transport.

**Rules of the boundary**

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
   new memories and their changes.
4. Every value that crosses the boundary is serializable (serde), and the tests run the local executor through a
   channel that serializes every message, so nothing that only works in one process slips in.
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
| `messages`, `attachments` and their files | `step/mod.rs`, `step/worker/steer.rs`, `recovery.rs` | coordinator link |
| `memories` | `step/memory.rs`, `handoff.rs`, `step/run_records.rs` | coordinator link |
| `approval_rules`, `project_approval_rules`, `tool_approvals`, `run_auto_approve`, `questions` | `step/answer.rs`, `step/approval.rs`, `question.rs` | coordinator link |
| Project settings an agent start reads (`code_index`, `repository_agent_files`, `memory_settings`, `approval_mode`, `auto_mode_settings`, `tells_base_moves`) | `step/start/*`, `step/skills.rs`, `handoff.rs`, `step/worker/review/setup.rs`, `step/base_notice.rs` | coordinator link |
| Step usage (`invocations` tokens, `reviewer_usage`) | `step/mod.rs`, `step/start/shared.rs` | coordinator link |
| `base_notices` | `step/base_notice.rs` | coordinator link |
| `connections`, account folders, CLI config folders | `step/start/{shared,cli}.rs`, `permissions.rs`, `step/worker/review/*`, `codebase_index.rs`, `step/roots.rs` | computer's resources |
| `mcp_servers`, secret metadata and values (`KeychainVault`, `server.rs:66`) | `step/mcp.rs`, `connection_check.rs`, `step/run_records.rs`, `plan.rs`, `step/memory.rs`, `handoff.rs`, `step/worker/review/*` | computer's resources |
| `project_environment` | `environment.rs`, `step/mod.rs`, `codebase_index.rs` | computer's resources |
| Settings › Browser from `orchester.db` (`runner/src/browser/mod.rs:403`) | browser | computer's resources (passed by the desktop) |

Where the desktop touches a run's worktree today (`crates/workspace/src/`):

| What | Where |
| --- | --- |
| Uncommitted changes, followed while the page is open: **through the runner** since protocol 40 (`Command::ReadWorktreeChanges`, ORC-682) | `tasks/changes/mod.rs` (`not_checkpointed_in`), `runner/worktree_changes.rs`; the runner's `execution/executor/worktree_changes.rs` |
| Whether a run's work is present (its own worktree folder, else its branch) | `tasks/changes/mod.rs` (`run_work_present`) |
| Conflict resolution in the worktree | `tasks/resolve/mod.rs`, `tasks/integration_jobs/resolve.rs`, `merge_queue/approve.rs` |
| Base synced into the worktree, Check tools run in it | `merge_queue/drive.rs:211`, `merge_queue/check.rs` |
| Local integration reading the worktree as a second repository | `tasks/local_integration/{mod,poll,view}.rs`, see [Local integration](#local-integration) |
| Removal, stopping its processes, index prune, sweep | `tasks/integration.rs:487`, `tasks/delete.rs`, `tasks/history.rs`, `app_mcp/tools/worktree.rs`, `tasks/worktree_sweep.rs` |
| Files of the run's version, Open in editor, copy path | `files/source.rs:39`, `files/open.rs`, `desktop-app/src/screens/task/changes/open_file.rs`, `run_facts.rs:329` |
| Recovery state of a run's folder: **through the runner** since protocol 44 (`Command::RunFolderState`, ORC-688) | `recovery/mod.rs` (`read_folder_states`), `runner/ask.rs`; the runner's `execution/coordinator/run_folder.rs`, `execution/executor/folder_state.rs` |
| Warm workspaces' folders and sizes, ended worktrees' sizes | through the runner since ORC-689 (`Command::Folders`, `runner/folders.rs`); the desktop keeps the database's reservations |
| Code index rebuild | Done (ORC-690): the runner builds it on `Command::RebuildCodeIndex` (protocol 43); `code_index::rebuild` asks it |
| Orchestrator chat git tools in a run's worktree | `orchestrator/git/place.rs:58` |

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
| Conflicts, file hashes and blobs, index entries and fingerprint, resolution bytes, writing a resolution (`write_regular`, `set_executable`, `set_index`) | `resolution.rs`, `sync.rs` `save_task_resolution`, `drafts.rs` `draft_identity`, `view.rs` `read_conflicts` | ORC-683's conflict-resolution requests (read conflicts and their versions, write a resolution) |
| Committing the resolved sync and proving it by receipt (`commit_task_resolution`, `HEAD^1`/`HEAD^2`, `has_task_commit_receipt`) | `sync.rs` `continue_task`, `task_commit.rs` | ORC-683's "continue after resolving" request |
| Aborting the sync merge: preview, retaining the files, `abort_task_merge` | `abort.rs` | ORC-683's abort request |
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

## Not part of the preparation

The network transport and pairing (direct TLS or a relay through the team server), `orchester-runner serve` and its
lifetime apart from a desktop connection, moving code between computers (git bundles), the agent computer's command
line and Settings › Computers, choosing a computer for a task. Opening a remote worktree in the person's editor is
open too.
