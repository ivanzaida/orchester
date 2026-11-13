# OA-07 · Events that wait in one conversation are taken in one turn

- **Status:** todo
- **Kind:** core + workspace
- **Depends on:** OA-01
- **Branch:** —
- **Tokens:** —

## Goal

A conversation that leads a module of twenty tasks is not an hour behind its own events.

## Today

Each event is an entry and a turn of its own, one at a time per conversation, each up to ten minutes; the prompt is
built for one wait (`crates/core/src/orchestrator/instructions.rs`). `docs/design/orchestrator-watch.md` (decision 5)
says events that come during a turn wait and are taken together in the next one; the code never did. The toolset
design lists "several requests that arrive together are not read in one turn" as not built.

## Change

- A turn Orchester starts takes every due event of its conversation, oldest first, five at most; the rest wait for
  the next turn.
- The rules that name "the event that started the turn" name the turn's events: the one merge of a run whose success
  is among them, the one continuation of an interrupted run among them, `decide_approval` for the requests among them
  (`not_this_request` for any other).
- Merging turns still start in the order the runs ended across conversations; a turn with a success among its events
  waits as that success would.
- Each event is settled or left open by itself (OA-01): a turn that failed leaves all of them open, and a try counts
  for each.
- The conversation shows the events of one turn together, with one answer under them.

## Edge cases

- An event that stopped standing while it waited is settled without the turn, as today.
- The limits of an answer (30 agent turns, 10 minutes) hold for five events too: the prompt tells the agent to settle
  the oldest first, and what it did not reach stays open.
- A daily report's turn and the summary of a watching that ended stay turns of their own (`report_turn`).

## Done when

- Workspace tests for the taking, the cap, the turn-scoped rules over several events and a partly settled turn
  (`--test orchestrator_waits together`).
- A design frame only if the grouped rows differ from today's; then its verification. `tools/check.ps1` passes.
