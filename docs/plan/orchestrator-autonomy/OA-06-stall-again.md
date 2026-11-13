# OA-06 · A run that stays stalled is raised again, and its turn may act

- **Status:** todo
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
