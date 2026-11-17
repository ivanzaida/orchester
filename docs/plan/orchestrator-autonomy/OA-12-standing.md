# OA-12 · Core: where a led target stands

- **Status:** done (2026-11-14)
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

## As built (2026-11-14)

- **The rule:** `orchester_core::orchestrator::standing` (`TaskFacts` → `TaskStanding`; `standing()` → `Standing`
  with `TargetStanding`). A task: done, working, waits for a person, a limit or disk, paused, to merge, ready to
  start, held, blocked by named tasks, failed (with its runs and whether its ceiling allows another), canceled,
  parked (by the chat or by a person's Blocked mark), or not ready (no pipeline, or one not published). The target:
  done, can start (which tasks, and whether an agent is free), working, or stuck. A task blocked from outside the
  target moves only while its blockers do, however long the chain; a task outside that could start is nobody's to
  start from here.
- **Read once** from the project (`orchester_workspace::orchestrator::standing::Reads`): every task as its records
  give it (a team task as its runs recorded it, its module and whether it is closed being the team server's last
  reading when it saw the task), its latest run, its blockers, the holds of new starts, the parked tasks, the free
  agent slots, and for a subscription with a budget what its work spent.
- **Told** by `get_standing`, a tool of the app (so every client and the chat have it), for a local and a team
  project alike. `get_watches` and `get_module` do not carry it: `get_watches` still refuses a team project, and a
  leading turn is told to read `get_standing` first.

A team task reads as a local one since 2026-11-17: its Blocked mark is the team server's last reading of what is
watched (a module, a task, the tasks without a module or the whole project), its pipeline is ready when it is one of
the team's pipelines this session read with a published version (as a local task's must be published), and a team
task that never ran on this computer is among the tasks as the reading holds it (OA-28). Tests:
`orchester-workspace --test team_mcp led_reads::`.

Tests: `orchester-core --test orchestrator_standing` (9), `orchester-workspace --test orchestrator_standing`.
