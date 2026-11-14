# OA-36 · Forecasts and cross-project readings answer for team projects

- **Status:** in progress (`list_tasks` across projects: done 2026-11-14)
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

**Left:** `get_delivery` for a team project (throughput, cycle times and forecasts from this computer's runs of
the team's tasks and the server's task list; the cumulative flow needs the server's history of statuses, which this
computer does not hold), and the reads of `docs/plan.md` S12 step 4.
