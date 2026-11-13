# OA-03 · A used budget and a plan limit hold turns until the reset

- **Status:** todo
- **Kind:** core + runner + workspace + UI
- **Depends on:** OA-01 (and OA-02's frames for the held row)
- **Branch:** —
- **Tokens:** —

## Goal

While the project's daily budget is used, or the Orchestrator's account is at its plan limit, no turn starts by
itself, the events wait, and their turns start when the limit resets.

## Today

- The Orchestrator's usage counts toward the daily budget (`crates/store/src/project_db/activity.rs`), and the runner
  refuses a turn while the budget is used (`crates/runner/src/execution/coordinator/chat/mod.rs`); the event is spent.
- A plan limit ends the turn as a plain failure (`chat/turn.rs`; `chat/codex.rs`: "it waits for no plan limit").
- Runs stopped at a plan limit are continued at the reset by timers (`coordinator/plan_limit/waits.rs`).

## Change

- **Budget.** Before it starts a turn by itself the follower reads whether the budget is used; if so it starts none.
  A clock wakes it when the budget's day turns (the day the store counts usage by), and a change of the project's
  limits wakes it as any setting does.
- **Plan limit.** The runner tells a CLI's refusal at the plan limit apart for chat turns as it does for steps, with
  the reset the CLI reported: `TurnFailure` gains that kind, which OA-01 reads as held until the reset. While it
  holds, no turn starts by itself on that connection; a reset that passed while the app was closed starts them at the
  next open.
- **A reset the CLI did not report:** tried again on OA-01's schedule, then every hour, without counting toward its
  cap.
- **Seen.** The held events show *Held until 00:00 — today's budget is used* or *… — the account's plan limit*, and
  the conversation's header says the Orchestrator is held and until when (frames with OA-02's).
- A person's own message is refused as today, with the same words.

## Edge cases

- The budget runs out while a turn thinks: the turn ends as it does today; its event stays open and is held.
- The person raises the budget or signs the connection in to another account: the hold ends at once.
- A run and the Orchestrator on one account: both wait for the same reset; the run's timer and the chat's hold do not
  depend on each other.
- Compaction is refused while the budget is used (`chat/compact.rs`): OA-05 waits with the turn.

## Done when

- Runner test for the plan-limit kind with the fake CLIs (`crates/runner/tests/`), workspace tests for both holds and
  their ends (`--test orchestrator_waits held`).
- Design verification of the held rows and the header. `tools/check.ps1` passes.
