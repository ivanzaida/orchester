# OA-31 · A claim holds while its work is led, and a run that lost it stops

- **Status:** in progress: the desktop's side built (2026-11-17); the renewal after success and of module claims needs
  the server's contract (D1, D8), and the chat's event for a lost claim is open (*Left*)
- **Kind:** contract + workspace + runner
- **Depends on:** —
- **Branch:** `claude/team-lifecycle`
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

## Built (2026-11-17), on the desktop

Found first: nothing called the exact heartbeat (`TeamWork::heartbeat`): `docs/cloud-api.md` said "Nothing calls the
team work yet", and the old sharing stream's heartbeat leaves a team run's task out. So no team claim was renewed at
all, whatever its run did, and the heartbeat named its claim with the work's id, which the server answers as lost (its
`exact`: "an authority that names a work is not current").

- The shared projects' minute round now renews every team project's claims (`sharing/project/team_reports.rs`), each
  claim once, by its exact authority with `work_id: null`.
- The server renews a claim while its latest work is `running`, and a run that waits for a person, is paused by a
  person or is interrupted reports nothing that ends the work there (`ExecutionOutcome::of_status`). So the desktop
  names every run whose work the server last answered as running (`team_work/heartbeat.rs`): at work, waiting for an
  approval, an answer or a plan, stuck for a decision, paused, interrupted. Work the server saw succeed or pause at a
  limit is not named: the server renews none (`transition_denied`), and one such authority refuses the whole heartbeat.
- A run whose authority the server took away (the heartbeat's `lost`, or a report's `claim_lost`, `membership_revoked`,
  `session_mismatch`, a newer run) is cancelled through its runner at the same round (`team_work/fenced.rs`,
  `Runners::cancel_run_at`), its worktree kept; the task page says why from the fence.
- The claim ends with the work: a cancelled run's report releases it (the server's rule), and the Done sent after a
  merge (OA-30) closes the task, which releases it.
- Tests: `crates/workspace/tests/team_work/heartbeat.rs` (renewed while a step waits and while paused; not named once
  succeeded or at a limit), `lost_claim.rs` (the run stopped, its reports ended).

## Left

- **The server's contract (the platform's).** A claim whose work is reported succeeded and waits to be merged or
  delivered, or is paused at a limit, is renewed by nothing: the server's heartbeat renews only `running` work
  (`renewal`, `crates/server/src/sharing/claim_authority.rs`). A module claim is never renewed (D1, "no module renewal
  yet"): it goes stale 15 minutes after it was taken, and only the D2 rule (no take-over while a task's work runs) holds
  it. Both need the contract to say what renews them, here first (`orchester-api`, `docs/cloud-api.md`), then the
  server; the desktop would then name those claims too. No route was invented for it.
- **The chat's event.** A cancel is no run end the Orchestrator follows (`orchestrator/run_ends.rs`), so the
  conversation that led the run learns of the lost claim only when it next reads the run (`get_run`'s `team.fence`,
  the `claim_fenced` refusals). An end of its own needs its words in the app's run-end card (a design).
