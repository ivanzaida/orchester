# OA-33 · A module's branch is made when its first task starts, in both kinds

- **Status:** todo
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
