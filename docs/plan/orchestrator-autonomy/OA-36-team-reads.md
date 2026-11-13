# OA-36 · Forecasts and cross-project readings answer for team projects

- **Status:** todo
- **Kind:** workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

What the Orchestrator reads to plan and to report answers in a team project too.

## Today

`get_delivery` refuses a team project; `list_tasks` without a project leaves a linked project out;
`docs/plan.md` S12 step 4 lists the reads that still look team tasks up in a cache (`get_run`, `send_message`,
`get_conversation`, `list_runs`, `hold_queue`).

## Change

`get_delivery` is computed from this computer's runs of the project's team tasks and the server's task list;
`list_tasks` across projects reads each linked project from its server and names the ones it could not ask. The reads
of S12 step 4 are finished as that plan says.

## Edge cases

- Signed out, or the server unreachable: the reading says which project it could not read, and gives the rest.

## Done when

- Workspace tests with the stand-in server. Its rows leave the parity table. `tools/check.ps1` passes.
