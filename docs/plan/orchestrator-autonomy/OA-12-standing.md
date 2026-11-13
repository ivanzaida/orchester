# OA-12 · Core: where a led target stands

- **Status:** todo
- **Kind:** core + workspace
- **Depends on:** OA-09
- **Branch:** —
- **Tokens:** —

## Goal

One rule answers, for a module or any other target a conversation leads, what can start now, what works, what waits
and for what, what failed, what is left to the person, and whether the whole can still move. The app raises events
from it and every leading turn reads it first, so the chat does not rebuild it from ten tool calls each time.

## Today

There is no such reading. `get_module` lists the tasks; `list_tasks` gives `blocked_reason` and
`held_by_blocking_tasks`; `/watching` says what waits. The turn that starts tasks after a merge finds them itself.

## Change

- **Core**, a pure rule beside `watch` (`crates/core/src/orchestrator/`): from a target's tasks, their blockers (the
  `merge` gate), their latest runs, what is parked (OA-15), the free agent slots, the holds of new starts and the
  subscription's ceilings and budget, it gives
  - each task as one of: done or canceled; working; waiting for a person, for a task, for a limit's reset or for
    disk; ready to start; blocked by named tasks; failed, with its runs counted; parked;
  - the target as one of: working; work can start (which tasks, and whether a slot is free); stuck, with why (all
    that is left is parked, blocked by something parked or failed past its ceiling, or the budget is used); done.
- **Workspace** reads the inputs once from the project's database and gives the result to: `get_watches` and
  `get_module` (a `standing` field), the head of every event prompt of a leading subscription, and OA-13 and OA-15.

## Edge cases

- A task without a pipeline, or whose pipeline is not ready: not ready, and named as needing the person.
- A task marked Blocked by hand reads as parked by the person.
- A task that moved to another module since: it stands where it is now.
- A dependency outside the target: the task waits on it by name; it is never started from here.
- *Everything in the project* as a target: the same rule over every task no more specific subscription takes.

## Done when

- Core tests for every task state and every target state, the edge cases above among them
  (`cargo test -p orchester-core --test orchestrator_standing`).
- Workspace test that the reading matches a seeded project. `tools/check.ps1` passes.
- `docs/manual/mcp-tools-work.md` describes `standing`.
