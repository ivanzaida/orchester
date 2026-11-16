# OA-25 · The daily report names reasons, what is parked and what is held

- **Status:** done in the tool's answer (2026-11-14); the report's entry does not show it
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

## As built (2026-11-14)

The report gains one section, `led` (`orchester_core::orchestrator::report::LedDay`, built by
`orchestrator/report/led.rs`), in what `get_daily_report` answers and in the report a conversation keeps:

- `decisions`: the Orchestrator's own decisions of that day, in the order it made them, each with its tool, task,
  reason and whether the person looked at it; `open_decisions` counts those not looked at;
- `parked`: the tasks parked for the person when the report was read, with what is needed and what to do;
- `stuck`: the targets led to the end that are stuck;
- `left_to_person`: the events whose turn failed as often as it is tried;
- `held`: what holds the turns Orchester starts by itself when the report was read (`budget` or `plan_limit`), and
  until when;
- `leading`: each subscription that leads to the end, where it stands, what its work spent and its budget.

A day with nothing of it has no such section, and a report kept before reads as it did. The report's turn still
changes nothing.

Differs from the task file:

- A reason is told with the decision, not on each of the report's actions: an action and its decision are kept
  apart, and the section lists the decisions.
- What was parked that day and is parked no more is told only as its decision (`park_task`): a parking that ended
  leaves no record of its own.
- How long turns were held is not told, only what holds them when the report is read: holds keep no history.
- Of a subscription's ceilings only its budget is told: the counts of returns and raises are per run.
- The report's entry in the conversation does not show the section (no frame): the tool's answer only.

Tests: `orchester-workspace --test orchestrator_report` (a day without, and each part of the section).
