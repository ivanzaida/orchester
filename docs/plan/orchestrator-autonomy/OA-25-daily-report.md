# OA-25 · The daily report names reasons, what is parked and what is held

- **Status:** todo
- **Kind:** workspace
- **Depends on:** OA-03, OA-15, OA-22
- **Branch:** —
- **Tokens:** —

## Goal

The report of a day the Orchestrator worked alone tells the person what it did and why, what it left to them and
what stood still, from the project's records and not from an agent's summary.

## Today

The report (`crates/workspace/src/orchestrator/report/`, `crates/core/src/orchestrator/report.rs`) lists merged
tasks with who merged them, runs ready, stopped and at work, what waits for the person, the Orchestrator's actions,
the day's usage and the queue. An action has no reason, and nothing of this plan is in it.

## Change

The report gains, each item with its link:

- each of the Orchestrator's own actions with its reason (OA-22), and how many decisions of the day are still open;
- what was parked that day and what is parked now, with what is needed (OA-15), and the leading subscriptions that
  are stuck;
- how long turns were held by the budget or a plan limit, and the events left to the person after their tries
  (OA-01, OA-03);
- for each leading subscription: what was used of its ceilings and its budget (OA-09).

`get_daily_report` gives the same. The report's turn still changes nothing (`report_turn`).

## Edge cases

- A day with nothing of this adds no empty section.
- Reports of days from before these records existed read as they do today.

## Done when

- Workspace tests for each new section and for a day without them (`--test orchestrator_report`).
- If the report's entry in the conversation shows the new sections, their frames and a design verification;
  otherwise the tool's answer only. `tools/check.ps1` passes.
