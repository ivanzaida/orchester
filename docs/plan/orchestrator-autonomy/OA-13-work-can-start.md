# OA-13 · The event *work can start*

- **Status:** done (2026-11-14); its line in the conversation waits for a frame
- **Kind:** core + workspace
- **Depends on:** OA-09, OA-12
- **Branch:** —
- **Tokens:** —

## Goal

A leading subscription starts its tasks whenever they can start: the first ones right after the person subscribed,
the next when a slot frees or a blocker is merged by anyone, a task added to the module later. No person starts a
wave, and a refused start is not forgotten.

## Today

In a turn Orchester started, `run_task` is allowed only in the turn of a success the chat merged itself under an
order (`not_merge_turn`), for a task its conversation leads (`not_led`) that has no run (`already_ran`)
(`crates/workspace/src/orchestrator/led_start.rs`). A start a slot refused "waits for the next merged success of what
it leads; no other event brings it back" (`docs/manual/orchestrator-chat.md`, Orders). The watch commands start no
turn.

## Change

- A new event of a leading subscription, *work can start*, raised by the follower when OA-12's standing says tasks
  are ready and a slot is free, and that differs from what it last raised for. It is kept and starts a turn like any
  event (retries, holds and batching of OA-01, OA-03 and OA-07 apply).
- In that turn `run_task` starts the ready tasks the event names; `led_start` allows it there beside the merge's
  turn. The order still only narrows: an order that says "do not start anything" is quoted, and the turn starts
  nothing.
- No timer: a slot frees, a blocker merges or a task is saved, the project's state changes, and the follower is woken
  as it is today.
- The runner's reviewed groups, which also start tasks as blockers finish (`task_groups.rs`), are not used: no
  window makes a group, and here the chat's order and ceilings decide.

## Edge cases

- Two leading subscriptions and one free slot: the older subscription's event is raised first; the other's when the
  next slot frees.
- New starts are held (`hold_queue`), or the subscription's budget is used: not raised; the standing says why.
- The slot was taken between the event and the start: `no_agent_slot` is the refusal, and the event is raised again
  when a slot frees.
- One such event stands per subscription at a time; a changed set of ready tasks replaces what it names.
- A *Follow* subscription raises none: today's behaviour.

## Done when

- Workspace tests (`--test orchestrator_waits can_start`): the first wave after a subscription became a leading one,
  a freed slot, a blocker merged outside the chat, a task added to the module, the held queue, two subscriptions.
- `led_start`'s tests for the new turn kind and the refusals that stay.
- `tools/check.ps1` passes; the manual's *Orders* and *Run ends* paragraphs and decision 14 say the new rule.

## As built (2026-11-14)

- **The event** is `orchester_core::orchestrator::standing::StandingEvent` with `Raised::CanStart { tasks }`, kept
  as an entry of the leading conversation (`EntryKind::Standing`; stored as a `report` entry, as a subscription's end
  is, so it gets its turn the same way, and unlike a report's its turn may act). The follower raises it
  (`orchestrator::standing::raise`) when the target's standing says tasks can start and an agent is free, and only
  when that state differs from the one last seen (`orchestrator_standings`): the same state is never raised twice,
  and a state it comes back to after another is raised again.
- **Starts.** A conversation that leads a task to the end starts it with `run_task` in any turn Orchester started,
  the event's among them (`led_start`); one that only follows keeps the old rule (the turn of a success it merged
  under an order). Wider than the task file asked ("in that turn"): a leading turn of any event may find work to
  start, and the ceilings and the budget bound it either way.
- A *Follow* subscription raises none. New starts held, or the budget used: nothing can start, so nothing is raised.

Differs from the task file:

- Two leading subscriptions and one free agent: both are raised; the second start is refused by the runner and is
  raised again when an agent frees (its state went quiet in between).
- A turn of this event that fails is not tried again: OA-01's retries are for waits. It is raised again when the
  state changes.
- Its line in the conversation is drawn as the lines about what a conversation watches are; its own frame is in
  `frames.md` (H).

Tests: `orchester-workspace --test orchestrator_standing` (raised once per state, and again after a change).
