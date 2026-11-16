# OA-07 · Events that wait in one conversation are taken in one turn

- **Status:** done (2026-11-14)
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

## As built (2026-11-14)

- The turn the oldest due wait of a conversation starts takes the next due waits of that conversation with it, five
  events at most (`orchester_core::orchestrator::instructions::TOGETHER_MOST`, `orchestrator::turns::take_with`). Each
  taken entry thinks under a message of its own and names the turn that took it (`orchestrator_entries.taken_with`,
  schema 177); ending, releasing or interrupting that turn does the same to every entry it took, so a failed turn
  leaves each event failed with its own try, and each is tried again (together again when they are due together).
- The prompt tells every event as it is told alone, oldest first, under one opening line
  (`instructions::together_prompt`), each with its own note of an earlier failed turn.
- The turn-scoped rules read the turn's events: `decide_approval` takes any request among them, `resume_run` continues
  a run whose interruption is among them, and a start after a merge is open when a success among them was merged in
  the turn. A task created in such a turn follows the subscription of the oldest event.
- A success whose turn merges its run is taken only while no merge of another conversation waits before it, so merges
  still go in the order the runs ended; the successes one turn took merge one after the other in it.
- A daily report, the summary of a watching that ended and an event of a whole target (*work can start*, *stuck*,
  OA-13 and OA-15) keep turns of their own.
- The conversation shows the events of one turn as their lines, in order, with the one answer under the last of them
  (`screens/orchestrator/turns.rs`); the rows are today's rows, so no frame was needed.
- A conversation forked from one with such a turn shows each copied event with its own ended turn.

Tests: `orchester-workspace --test orchestrator_together together` (the taking, the cap, a failed turn, a turn nobody
took and one a runner did not hold, successes of one conversation, a merge another conversation waits for),
`orchester-core --test orchestrator_instructions together`, `orchester-app --test mcp_domain
orchestrator_approval_turn` (two requests in one turn).
