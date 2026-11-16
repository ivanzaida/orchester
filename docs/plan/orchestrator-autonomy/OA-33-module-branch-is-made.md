# OA-33 · A module's branch is made when its first task starts, in both kinds

- **Status:** done for a team module (2026-11-14); a local module has no branch until OA-17
- **Kind:** workspace + runner
- **Depends on:** OA-17
- **Branch:** —
- **Tokens:** —

## Goal

Nobody has to run `git branch` by hand before a module's first task can start.

## Today

A team start reads the module's branch tip and is refused `branch_not_found` when the branch does not exist
(`app_mcp/tools/team/run.rs`); `docs/architecture/server.md` says the runner creates it from the checkout's base at
the first task start, and no code does. A local module has no branch of its own (OA-17).

## Change

One piece of code, used by both kinds, makes `module/<slug>` (or the name the Admin assigned) from the module's
base branch when a start finds it missing, before the runner reads the run's base. It never moves a branch that
exists, and never checks it out.

## Edge cases

- The name is taken by an unrelated branch: the start is refused with that, not silently based on it.
- Two starts at once: one makes the branch, the other finds it.

## Done when

- Workspace tests for both kinds. Its row leaves the parity table. `tools/check.ps1` passes.

## As built (2026-11-14)

`orchester_workspace::team_work::start_tip` gives a start the tip of its module's branch and makes the branch first
when the checkout does not have it: tracking the remote's when a remote has a branch of that name, else at the tip of
the branch the checkout has checked out, which is the base the module is delivered to
(`orchester_core::integration::base::create_branch_at`). A branch that exists is never moved, the checkout stays on
its branch, and two starts at once are safe: git refuses the second creation and the tip is read either way. Both
starts use it: `run_task` (`app_mcp/tools/team/run.rs`) and the task page's Run (`desktop-app/src/team/start.rs`).
With no branch checked out there is nothing to make it from, and the start is refused `branch_not_found` as before.

Differs from the task file:

- A local module has no branch of its own, so there is nothing to make for one until OA-17 gives it; the same
  helper is then its.
- A name taken by an unrelated branch is not told from the module's own: a branch of that name that exists is used
  as it is, as before.
- The case of a remote that has the branch is not covered by a test.

Tests: `orchester-workspace --test team_mcp team_mcp_a_start_makes`.

## A local module (2026-11-14)

Done with OA-17: the runner makes a local module's own branch when its first run starts, at the tip of the module's
base branch.
