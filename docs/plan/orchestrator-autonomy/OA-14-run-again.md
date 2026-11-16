# OA-14 · A failed task is run again within its ceiling

- **Status:** done (2026-11-14)
- **Kind:** workspace
- **Depends on:** OA-09, OA-13
- **Branch:** —
- **Tokens:** —

## Goal

One failed run does not end a module. The chat gets the work going again, a bounded number of times, and then leaves
the task to the person.

## Today

A failure's turn may return the run to a step (`continue_run` `return_to`) and explains; `run_task` of a task that has
a run is refused `already_ran` in any turn Orchester started, "so a failure's turn can never start its task once
more" (`led_start.rs`).

## Change

For a task led to the end, in the turn of its run's failure:

- a run that can be returned is returned to the right step with a note, as today, counted against the returns of the
  run (OA-09);
- a run that cannot (its worktree is gone, the runner failed it, it cannot be continued) is followed by a new run of
  the task with `run_task`, while the task had fewer runs than its ceiling;
- past either ceiling the call is refused `ceiling_reached` and the turn parks the task (OA-15) with what failed each
  time.

A run the person canceled, or a task the person marked Blocked, is never run again by the chat.

## Edge cases

- A new run starts from the base branch, without the failed run's work: the turn says so, and prefers the return
  when the work is worth keeping.
- The failure is not the task's (the account's limit, low disk, the runner): it is not a failure to run again but a
  wait; OA-12 reads it so and no run is spent.
- The same step fails the same way twice: the second turn is told the first note and what came of it, from the
  conversation, and parks rather than repeat the note.

## Done when

- Workspace tests (`--test orchestrator_tools run_again`): the return, the new run, both ceilings, the person's
  cancel, a failure that is a wait.
- `tools/check.ps1` passes; `led_start.rs`'s head comment and the manual's table say the rule.

## As built (2026-11-14)

In a turn Orchester started, `run_task` of a task that has a run is allowed when its conversation leads it to the
end, its latest run failed and its ceiling of runs allows another, and the call gives the chat's reason
(`reason_required` without; kept as a decision, OA-22). Past the ceiling it is refused `ceiling_reached`; a task
whose latest run did not fail (it was canceled, succeeded, waits) is still `already_ran`; a task marked Blocked or
parked is not started at all. The standing says which failed tasks may run again (`may_run_again`), and such a task
counts as work that can start (OA-13). Returning a run to a step stays `continue_run`'s, counted against the run's
returns (OA-09). That the chat prefers a return when the work is worth keeping, and parks rather than repeat a note,
is its instructions' (OA-18).

Tests: `orchester-core --test orchestrator_standing` (a failed task within and past its ceiling),
`orchester-app --test mcp_domain orchestrator_parking` (a run that did not fail is not run again).
