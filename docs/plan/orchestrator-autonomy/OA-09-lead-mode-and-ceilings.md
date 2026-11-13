# OA-09 · Core: a subscription leads to the end, with ceilings

- **Status:** todo
- **Kind:** core + store + workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

The person can tell a subscription to carry its work to the end, and what the chat does on its own there is bounded
by rules its tools enforce, not by its prompt.

## Today

A `Subscription` is a conversation, a target, the person's order as text and since when
(`crates/core/src/orchestrator/watch/mod.rs`). The order is quoted to the agent as data and only narrows. Since
decision 14 nothing counts the chat's own returns, retries or raises: `continue_run` takes any of them while it
drives (`crates/workspace/src/app_mcp/tools/continue_run.rs`, `tasks/raise.rs`).

## Change

- **Mode.** A subscription is *Follow* (today's behaviour, the default, and what every stored subscription reads as)
  or *Lead to the end*. Only the person sets it, with the watch command (OA-10); the agent's `watch` tool always
  makes a *Follow* subscription and cannot change a mode.
- **Ceilings** of a leading subscription, checked by a constructor, stored with it:
  - runs of one task, the first included: 1–5, 3 by default (used by OA-14);
  - returns and retries of the chat's own per run (`continue_run` `return_to`, `allow_retry`, `resolve_conflicts`):
    0–10, 4 by default;
  - raises of a limit or a step's budget per run: 0–3, 1 by default, each by at most 50 % of the limit;
  - an optional budget in tokens for the subscription's work: what the runs of its tasks and its conversation's turns
    spent since it started.
- **Counting** is read from the project's database: the runs of the task, and the chat's own actions on the run as
  the conversations keep them. It counts per run and per task, whichever conversation acted.
- **Enforcement** in one place the chat's calls pass in a turn Orchester started: a call past its ceiling is refused
  `ceiling_reached`, naming the ceiling and what was used, and the turn parks the task (OA-15). In a turn a person's
  message started the ceilings do not hold: the person asked.
- A used budget of the subscription lets what runs finish and starts nothing new; OA-12's standing says so.
- Project settings › Orchestrator › *Watching* holds the defaults a new leading subscription takes.

## Edge cases

- A subscription that moves to another conversation (*Take it here*) keeps its mode, ceilings and what was used.
- A task subscription inside a leading module subscription: the more specific one decides, as for routing.
- A *Follow* subscription keeps today's unbounded behaviour; this task changes nothing for it.

## Done when

- Core tests for the constructor's bounds; workspace tests for each ceiling's refusal and for the person's turn
  (`--test orchestrator_ceilings`).
- Store migration with a test that an existing subscription reads as *Follow*.
- `tools/check.ps1` passes. Nothing visible yet; OA-10 draws it.
