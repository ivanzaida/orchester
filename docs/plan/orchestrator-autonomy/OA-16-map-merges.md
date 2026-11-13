# OA-16 · Map: how a run merges into a base branch that is not checked out

- **Status:** todo
- **Kind:** research
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

Know, before OA-17 is designed, what Orchester already does when a task's base branch is not the branch of the
person's checkout, and what a module's own branch would need beyond it.

## Questions

Answered with file and line, in `.tmp/orchestrator/orchestrator-autonomy/OA-16-map.md`:

1. `BaseChoice` (`crates/core/src/module.rs`, the task's and the project's): which values it has, where each is
   resolved to a branch, and what a run's worktree starts from.
2. `merge_run` for a base branch that is checked out in the main checkout, and for one that is not
   (`crates/workspace/src/app_mcp/tools/setup/merge.rs`, `tasks/integration.rs`): which touches the person's working
   tree, which needs Local integration, which can be refused `needs_decision` or `scoped_integration_required`.
3. The merge queue (`crates/workspace/src/merge_queue/`) for the same two cases: where it syncs and checks.
4. Base-moved notices (`crates/core/src/base_move.rs`): do they follow a base branch nobody has checked out.
5. Clean-up after a merge (`docs/architecture/storage.md`): which branches and worktrees go.
6. How a team module's branch works (`docs/architecture/server.md`, the module branch and delivery): who creates
   `module/<slug>`, what a task's run merges into, what delivery does and checks, and what of it is the server's and
   could not be reused on this computer.
7. What the chat's auto-merge rule reads of all this (`crates/workspace/src/orchestrator/auto_merge.rs`).

## Done when

The map answers each question and ends with what OA-17 can reuse and what it must add. Nothing is changed.
