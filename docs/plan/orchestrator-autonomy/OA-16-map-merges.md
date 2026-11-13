# OA-16 · Map: how a run merges into a base branch that is not checked out

- **Status:** done (2026-11-13; read-only, nothing changed)
- **Kind:** research
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

Know, before OA-17 is designed, what Orchester already does when a task's base branch is not the branch of the
person's checkout, and what a module's own branch would need beyond it.

## What was found

Most of OA-17 exists: a local module can already name a base branch, and its tasks' runs then start from that
branch's tip and merge into it without the person's working tree. Paths are under `crates/`.

1. **The base of a run.** `BaseChoice::{CheckedOut, Branch}` (`core/src/integration/base/choice.rs`); a task's
   `TaskBase::{Inherit, CheckedOut, Branch}`; `resolve()` takes the task's branch, else its module's, else the
   checkout. The runner's `run_base()` (`runner/src/execution/coordinator/prepare.rs`) takes a named branch's tip
   wherever the checkout is, and refuses the start `BaseBranchMissing` when the branch does not exist.
2. **`merge_run`.** `merge_in_as` (`workspace/src/tasks/integration.rs`) → core `merge_source`
   (`core/src/integration/merge/`):
   - base checked out in the main checkout: fast-forward or a merge commit made in a temporary worktree, then
     brought into the person's working tree (`bring_in`). The only path that touches it; `UncommittedInTheWay` and
     `Conflict` continue on Local integration.
   - a named base nobody has checked out: `merge/beside.rs` makes the commit in the same temporary worktree and moves
     the ref with a compare-and-swap `git update-ref`; refused `CheckedOutIn` while any worktree has that branch
     checked out. The person's working tree and index are not read or written.
   - `needs_decision` and `program_running` are checked whatever the landing (a local integration under way holds
     every merge); `scoped_integration_required` counts only when the base is the checkout's branch.
   - **Gap:** a conflict of a merge beside the checkout is still routed to Local integration, which answers `Stale`
     because the checkout is not on that branch. It has to go to an agent (`resolve_conflicts`, which works by branch
     name).
3. **The merge queue** syncs and re-checks in the run's worktree and lands with `merge_in_as`; lanes are per base
   branch, and a lane beside the checkout does not wait for the checkout. `MAX_RESYNCS` is 3.
4. **Base-moved notices** follow `refs/heads/<base>` of unfinished runs, checked out or not. **Gap:** nothing watches
   the base *of a module's branch* (main moving under `module/x`), and no code merges a base into a branch that has
   no worktree.
5. **After a merge** the run's worktree and branch go once they are reachable from the run's base branch; a base
   branch is never removed.
6. **Team modules.** The docs say the runner creates `module/<slug>` at the first start; no such code is in this
   repository (a team start fails `branch_not_found`). Delivery is `merge_module_in`
   (`workspace/src/tasks/module_delivery.rs`) → core `merge_module`: always a merge commit of the module's branch
   into the base, under the merge hold, landing in the checkout or beside it, with the expected head and base
   commits; `module_merged` finds a merge whose record was lost. Its record (`team_deliveries`) and authorization are
   the server's protocol.
7. **The module** is `Module{details, created_at, base}` (`core/src/module.rs`), `modules.base_branch` in the store,
   the form in `desktop-app/src/screens/modules/` (`form_base.rs` is the base branch chip, `summary.rs` the Summary),
   and `create_module` / `update_module` with `base_branch`.
8. **`auto_merge.rs`** reads nothing about bases or modules: a merge into a module's branch passes as any merge.

## What OA-17 reuses

`BaseChoice::Branch`, `RunBaseSource::Module` and the landing beside the checkout; `merge_in_as`, `merge_run` and the
merge queue as they are; `resolve_conflicts`; `merge_module` / `merge_module_in` / `module_merged` for delivery;
the clean-up of run branches; base-moved notices for the runs on the module's branch.

## What OA-17 adds

- The module's choice *its own branch*, with the branch's name kept (core, a `modules` column, the form, the tools'
  argument), and a slug rule with a suffix on collision.
- Making `module/<slug>` from the module's base tip at the first start, before the runner reads the run's base.
- The resolve rule: the module's runs take its own branch; the module's base becomes where it is delivered.
- Conflicts of a merge beside the checkout go to an agent, not to Local integration.
- Bringing the module's base into its branch (a temporary worktree merge and `update-ref`), and noticing that the
  base moved.
- Local delivery: no run of the module still working, a merge commit of the branch into the base, a record of it,
  the Summary's states, and the chat's rule for when it may ask for it (OA-20).
- A check that nothing deletes `module/*` branches when a module closes.
