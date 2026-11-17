# OA-31 · A claim holds while its work is led, and a run that lost it stops

- **Status:** in progress: the desktop's side built (2026-11-17), the renewal after success, at a limit and of module
  claims in the contract and the server (2026-11-17); the chat's event for a lost claim is open (*Left*)
- **Kind:** contract + workspace + runner
- **Depends on:** —
- **Branch:** `claude/team-lifecycle`, `claude/team-contract` (platform: `claude/claims-and-fields`)
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

## Built (2026-11-17), the contract and the server

Decided by the implementing agent on 2026-11-17 on the person's demand that a team project's Orchestrator keep its
work as a local one does (`docs/design/local-and-team-parity.md`, *The rule*); D1's text for tasks ("only Running
work renews") is replaced by this for work started through `work.start`. To confirm with the person.

- **The contract** (`orchester-api` `v2::heartbeat`, `docs/cloud-api.md`, "Team authority P4 work"): a task's claim
  renews while its latest work is `running`, `succeeded` (waiting to be merged or delivered) or `paused` at a limit;
  `awaiting_human` stays `decision_required [D1]`, `failed` and `cancelled` `transition_denied`. A module claim
  renews whenever its holder names it from the session that acquired it. No type changed; the stand-in team server
  (`test_support`) follows the rule.
- **The server** (platform `claude/claims-and-fields`): `renewal` and the heartbeat in
  `sharing/claim_authority.rs`, `module_claims::renew` and schema 39 (`module_claims.heartbeat_at`); take-over,
  release and fencing are as they were.
- **The desktop** (`team_work/heartbeat.rs`): a claim is named by its newest run here: running work as before, work
  the server saw succeed until its run is merged here and its Done answered (OA-30), work paused at a limit; and the
  module claim of each module whose newest delivery goes on (requested, waiting for an Admin, merging, reporting). An
  older server refuses the whole heartbeat for those (`decision_required` or `transition_denied`): the running
  claims are then sent again alone.
- Tests: `crates/workspace/tests/team_work/heartbeat.rs` (kept in review until the Done, at a limit, a module claim
  while its delivery waits, an older server); the server's `tests/sharing/work_report_effects.rs`,
  `module_takeover.rs` and `tests/acceptance/ownership.rs`.

## Left

- **An older team server** (0.11.4 or older) still renews running work only; the person updates it.
- **The chat's event.** A cancel is no run end the Orchestrator follows (`orchestrator/run_ends.rs`), so the
  conversation that led the run learns of the lost claim only when it next reads the run (`get_run`'s `team.fence`,
  the `claim_fenced` refusals). An end of its own needs its words in the app's run-end card (a design).
