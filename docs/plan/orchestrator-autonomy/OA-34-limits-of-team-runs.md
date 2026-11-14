# OA-34 · A team run's limits are raised as a local run's are

- **Status:** needs decision (D6: what a holder may raise without an Admin); the contract changes here first, then the platform's side
- **Kind:** contract + workspace
- **Depends on:** OA-29
- **Branch:** —
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
