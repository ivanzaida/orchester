# OA-34 · A team run's limits are raised as a local run's are

- **Status:** built (2026-11-17): contract, server and desktop, on the option proposed below (decided by the implementing
  agent, to confirm with the person); the Admin's screen for the ceiling is left (*Left*)
- **Kind:** contract + workspace
- **Depends on:** OA-29
- **Branch:** `claude/team-contract` (platform: `claude/claims-and-fields`)
- **Tokens:** —

## Goal

A team run stopped at its budget does not wait for an Admin all night when a local one would go on.

## Today

`continue_run` `raise_limits` and `raise_step_budget` for a team run answer `decision_required` (D6): the limits
of a team task are its Admin's, and the contract has no way for its holder to raise them.

## Change

- **To decide with the person (D6), then in the contract:** what a holder may raise without an Admin. Proposed:
  the same ceiling the Orchestrator has for a local run (OA-09: one raise by at most half), set per workspace by its
  Admin, off by default.
- The desktop then raises through `continue_run` in both kinds, and the chat under OA-09's ceilings.

## Edge cases

- The workspace does not allow it: refused as today, and the task is parked for the Admin.

## Done when

- Set after the decision. Its row leaves the parity table when built.

## Built (2026-11-17)

D6 for a run's limits, decided by the implementing agent on 2026-11-17 by the option proposed above, to confirm with
the person (`docs/design/team-authority.md`, D6).

- **The contract** (`orchester-api` `v2::limit_raise`, `docs/cloud-api.md`, "Team run limit raises (OA-34)"):
  `WorkspacePolicy.limit_raises` (`LimitRaises {per_run 1..=3, percent 1..=50}`, left out while off) and
  `WorkspacePolicyChanges.limit_raises` (set, `null` off, left out unchanged); the command `work.raise_limits
  {authority, run_id, raise: LimitRaise}` (each limit raised, `{from, to}`, tokens or milliseconds) answered
  `limits_raised {work_id, run_id, raises}`; `LimitRaises::allows` is the one rule both sides read.
- **The server** (platform): the policy route keeps the ceiling (`projects.raises_per_run`, `.raise_percent`, schema
  39), `work.raise_limits` (`sharing/work_limits.rs`) is the holder's under the work's exact authority, of work paused
  at its limit (`transition_denied` otherwise), `decision_required [D6]` with no ceiling, `invalid` beyond it; each
  raise is kept (`work_limit_raises`) and audited.
- **The desktop:** `continue_run`'s `raise_limits` and `raise_step_budget` of a team run send the run's reports, then
  ask the server (`team_work/raise.rs`, `app_mcp/tools/team/raise.rs`) with each limit from what the run had; a lifted
  limit is `raise_beyond_ceiling` before anything is sent, and a limit added or lowered is no raise. Only a raise the
  server keeps goes on to the runner. OA-09's ceilings of a led subscription hold on top, as for a local run.
- Tests: `crates/workspace/tests/team_mcp/raise.rs`, `crates/api/tests/workspace_policy.rs`; the server's
  `tests/sharing/work_limits.rs`.

## Left

- The Admin sets the ceiling through `PATCH /api/v2/projects/{p}/policy` only: the desktop's Sharing settings and the
  team's web app have no control for it yet (a design).
- A raise whose answer was lost is asked again under a new operation, so it may count twice against the ceiling.
- An older server refuses `work.raise_limits` as a command it does not know; the raise is not made.
