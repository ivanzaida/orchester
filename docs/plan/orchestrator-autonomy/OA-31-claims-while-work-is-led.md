# OA-31 · A claim holds while its work is led, and a run that lost it stops

- **Status:** needs decision (D1 and D8: for whom a claim is renewed while its work waits, and for how long); the contract changes here first, then the platform's side
- **Kind:** contract + workspace + runner
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

A task the Orchestrator leads overnight is still this member's in the morning; and a run whose claim someone
else took does not keep an agent working for nothing.

## Today

A claim lasts 15 minutes without a heartbeat (`docs/architecture/server.md`), and only a run at work renews it
(`renews_claim`, `crates/core/src/run/outcome.rs`; `sharing/project/heartbeat.rs`). The server answers
`decision_required` (D1) to a renewal while the work waits for a person or is in review, so the task page says *Your
claim isn't renewed while the run is in review* and the claim goes stale; then anyone may take the task over. A run
whose claim was lost is fenced (it reports nothing more) and keeps executing: the runner is not told.

## Change

- **The contract (D1, D8), changed here first** (`orchester-api`, `docs/cloud-api.md`): a claim is renewed while
  its run waits, is in review, or is merged and its module not delivered, when the desktop that holds it says the
  work is led. To decide with the person: led by the Orchestrator only, or by a person's open task too; and for how
  long at most.
- The desktop renews accordingly; nothing else changes for a claim nobody leads.
- A run fenced because its claim was lost is stopped at its next checkpoint, with the reason on its Summary, and
  raises its event to the conversation that led it.
- The server's side is the platform repository's: its change and the pin are the person's.

## Edge cases

- The server is an older one that refuses the renewal: the claim goes stale as today, and the readiness list
  (OA-19) says so before the person leaves.
- The computer slept past the claim's life: the claim is lost; the run stops as above.

## Done when

- Contract tests in `orchester-api`; workspace tests with the stand-in server for the renewal and the stop.
- Its two rows leave the parity table. `tools/check.ps1` passes. The platform's change is listed as not done here.
