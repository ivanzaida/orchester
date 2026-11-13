# OA-28 · Subscriptions lead team tasks by their module, and `/watching` answers

- **Status:** in progress (the module of a team task that ran here: done 2026-11-13; the rest below)
- **Kind:** workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

A conversation that watches a module of a team project gets the events of that module's tasks, and
`/watching` and `get_watches` say what it leads.

## Today

The follower finds a wait's leader through `tasks::recorded::task_of`, which gives a team task as a draft with no
module (`crates/workspace/src/tasks/recorded.rs`), so `Subscriptions::leading` never matches a module subscription:
the events go to *Tasks without a module*, or to nobody. The module is in the run's team inputs
(`TeamAcceptance.module`) and is not read. `get_watches` refuses a team project (`team_project_unsupported`,
`app_mcp/tools/watches.rs`), and the watch tool resolves its targets from stored rows only.

## Change

- A team task's module is read from what its newest run started with, and from the server's reading where the
  watch already polls it (`orchestrator::team_status`); the follower, the watch tool, the pickers and
  `settle_closed_in` use it.
- `get_watches` and `/watching` answer for a team project, with the note the watch design already has for a status
  the server cannot be asked for.

## Edge cases

- A task moved to another module on the server: it is led by where the server says it is now.
- A team task that never ran here has no stored inputs: its module is the server's reading, or it is not led until
  one is made.

## Done when

- Workspace tests: a module subscription leads a team task's wait and run end; `get_watches` for a team project.
- Its two rows leave the parity table. `tools/check.ps1` passes.

## Done so far (2026-11-13)

A team task as its runs recorded it is in the module its newest run started in (`RunBaseSource::Module`, kept with
every team run; `tasks::recorded`), so `Subscriptions::leading` finds the module's subscription and the follower
keeps the task's waits and run ends in the conversation that watches the module. Checked against a read-only copy of
the person's own project database: all its tasks are team tasks, its modules are stored on this computer with the
ids its runs name, and it has a module subscription that led nothing because of this.

**Left:** a team task that never ran here (no run to read its module from) and one the server moved to another
module since its last run: both need the server's reading of the task's module, where `team_status` already asks it.
`get_watches` and `/watching` for a team project. `settle_closed_in` for a team task.
