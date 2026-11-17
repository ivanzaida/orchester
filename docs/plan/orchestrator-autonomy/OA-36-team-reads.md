# OA-36 · Forecasts and cross-project readings answer for team projects

- **Status:** in progress (`list_tasks` across projects: done 2026-11-14; `get_delivery` and Usage › Delivery: done
  2026-11-17)
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

## Done on 2026-11-14

`list_tasks` without a project reads each team project's tasks from its team server, each naming its project, as a
call naming that project gives them; a team project whose server could not be asked is listed among
`unreadable_projects` with that reason, and the rest is answered for. Test: `orchester-workspace --test team_mcp
team_mcp_reads` (the tools test).

**Left:** the reads of `docs/plan.md` S12 step 4.

## `get_delivery` done on 2026-11-17

A team project's figures are built as a local one's (`Delivery::read_team`) from every task and module its team server
holds, read now, and this computer's runs of those tasks: `get_delivery` answers with `team: true`, and Usage ›
Delivery reads them through the session's team queries (at most 2000 tasks, which it says when there are more; the
team's outage card while the server cannot be read). Two figures need the server's contract and are not built: the
cumulative flow (`{unavailable: team_no_status_history}`; the server keeps no history of a task's workflow status), and
work merged on a teammate's computer (not known here, so not counted as merged). Both are rows of the parity table.
Tests: `orchester-workspace --test team_mcp team_mcp_figures`, `--test delivery`.
