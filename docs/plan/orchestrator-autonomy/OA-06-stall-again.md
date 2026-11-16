# OA-06 · A run that stays stalled is raised again, and its turn may act

- **Status:** done but for the long tool call (2026-11-14)
- **Kind:** core + workspace
- **Depends on:** OA-09, OA-15
- **Branch:** —
- **Tokens:** —

## Goal

A run that stopped making progress does not hold a module for the rest of the night after one message nobody read.

## Today

`stalls.rs` raises a stall once, keyed by the run and the time of its last progress; a run that stays stalled is not
raised again. The stall turn's prompt tells it to start, resume, cancel and merge nothing; it may write to the agents.

## Change

- **Raised again** each further period of *A run without progress for* while the run stays stalled, three times at
  most per stall; progress re-arms it, as today.
- **While the task is led to the end** (OA-09) the turns act in steps, by the tools and not only the prompt:
  1. the first writes to the run's agents (`send_message`), as today;
  2. the second stops the step and continues the run from its last checkpoint;
  3. the third parks the task (OA-15) with what the run was doing.
  A *Follow* subscription keeps today's turn, only raised again.
- The second step needs a way to stop a working step and continue it from its checkpoint as the chat's own action;
  if `pause_run` and `resume_run` do not give it, this task adds nothing new to the runner: it says so and the second
  step becomes the third.

## Edge cases

- **A long tool call.** A build of forty minutes inside one tool call makes no transcript event. A tool call under
  way is not a stall before its own timeout; check what `stalls.rs` counts as progress first and fix that there.
- A run that waits for a person, is queued, paused, waits for a workspace or on another task never stalls, as today.
- A run held by low disk is not stalled.

## Done when

- Workspace tests (`--test orchestrator_stalls again`): the three raises, the re-arm, the tool call under way, each
  step of a led task.
- `tools/check.ps1` passes. The manual's *No progress* paragraph and the watch design's decision 5 say the new rule.

## As built (2026-11-14)

- **Raised again.** A run that stays stalled raises its *no progress* event once for each further period of *A run
  without progress for*, three times at most per stall (`orchester_core::orchestrator::task_event::stall_periods`,
  `STALL_RAISES`; `orchestrator/stalls.rs`). Each period is an entry of its own (*no progress for 20 minutes*, then
  *40*, then *60*), kept by the run, the time of its last progress and the period; progress re-arms it, as before.
  The app follows again at the next period. A subscription that only follows gets the same turns as before, three
  times.
- **A task led to the end acts in steps**, told by the leading note of its turn
  (`orchester_core::orchestrator::instructions::leading_note`, by the stall's period): the first writes to the
  agents, the second stops the run with `pause_run` and continues it from its last checkpoint with `resume_run`
  (both are the chat's tools already, so nothing was added to the runner), the third parks the task.

Differs from the task file:

- The steps are told by the prompt, not held by the tools: nothing refuses a second message in the second period.
- **A long tool call is not handled.** What counts as progress (`run_progressed_at`) was not changed, so a tool call
  of forty minutes that writes no transcript event still reads as a stall. To do: read the call under way and its
  timeout there.

Tests: `orchester-core --test orchestrator_task_events` (the periods), `orchester-workspace --test
orchestrator_stalls` (three raises, the re-arm, when to follow again), `orchester-core --test
orchestrator_instructions` (the steps).
