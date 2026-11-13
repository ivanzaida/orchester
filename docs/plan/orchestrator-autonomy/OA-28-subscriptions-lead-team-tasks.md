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

A task the server moved since its last run is led by where it is now: who leads a task reads its module from the
last reading of the watched team tasks (`orchestrator::team_status::module_now`, the reading the ending rule already
uses), and from the task's records only while no reading saw it. The follower, the ceilings, the start rule, a
created task's watch, the watch tool and `/watching`'s routing all read it there, so they agree on the leader.

Found while doing it: ending a watch on a team task's status (`watch_ends`, with `team_status`) was already done on
master, and a team task that never ran here raises no wait or run end on this computer, so the follower has nothing
of it to route.

**Left:**

- A task the server moved out of every watched module is in no reading (a reading lists the watched modules and the
  tasks watched by themselves), so it stays led by the module its run started in until it runs again. To close it,
  the pass reads by itself each team task whose runs started in a watched module and that the module's list no
  longer holds.
- `get_watches` for a team project, and what `/watching` lists under a module: both read the project's tasks from
  this computer's rows (`ProjectTasks`), which for a team task are the old stream's copy or nothing. They need the
  tasks of the watched modules as the reading holds them.
- A team task that never ran here in a start the chat makes by itself (`led_start` finds stored tasks): OA-29.
