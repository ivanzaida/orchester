# OA-03 · A used budget and a plan limit hold turns until the reset

- **Status:** done; what is seen built 2026-11-14, design verification pending (a separate agent verifies)
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

## As built (2026-11-13)

- **The hold is the follower's, in one place** (`orchester_workspace::orchestrator::held`, rule in
  `orchester_core::orchestrator::hold`): before it starts any turn by itself it reads whether today's budget is used
  and whether a plan limit is kept of the account the Orchestrator's published connection runs on. Read only when a
  turn would start, since counting the day's tokens is not free. The follower's clock (`turns::next_due_in`, OA-01's)
  wakes at the hold's end when a turn waits.
- **The plan limit is kept by the runner** as the turn's CLI reports it (`chat/turn.rs`: `keep_plan_limits` for a
  limits report that refuses turns, `keep_usage_limit` for Codex's named failure), in `orchestrator_plan_holds`, one
  row per account; a turn that answers on the account forgets it.
- **No new failure kind.** The turn that hit the limit ends `Failed` with what the CLI said, as before, so nothing
  visible changes; it is tried again after the reset and counts as one of the event's four tries. The plan asked for
  a kind of its own that would not count: left out, because a new kind is a new sentence in the conversation, which
  waits for the frames with the held rows.
- **A reset the CLI did not name** holds an hour from the report (`UNKNOWN_RESET`), then the next turn is tried; a
  limit still there fails that turn and holds again. Four such hours spend the event's tries.
- **The budget's refusal by the runner** (the budget ran out between the follower's reading and the runner's) takes
  the turn back (`release_wait_turn`) instead of failing it.
- **A saved limit wakes the follower** (`tasks::usage::update_limits`), so a raised budget ends the hold at once.
- **A turn taken back** keeps the tries its event had and starts again as a first turn, without the wait a failed one
  has: nothing failed.
- **Not built here:** the held rows and the header line (frames A); telling the person *held until …* anywhere. Until
  then a held event simply shows no turn.

## Review (2026-11-13)

An independent reviewer read the change. Fixed: the plan limit was forgotten on any usage report, which a turn that
fails at the limit sends too, so the hold could vanish within the turn that set it; it is now forgotten only when a
turn ends answered. Fixed: a saved limit did not wake the follower. Left: rows of accounts no longer used stay in
`orchestrator_plan_holds` (one per account, read by none); the clock may wake once at the hold's end for an event
that no longer stands.

Tests run: `orchester-core --test orchestrator_hold hold`; `orchester-store --test orchestrator_waits waits`;
`orchester-workspace --test orchestrator_held held`, `--test orchestrator_led_waits led_waits`, `--test
orchestrator_auto_merge auto_merge`; `orchester-runner --features test-agent --test chat` (`a_turn`, `chat_`) and
`--test chat_codex chat_codex`; `tools/check.ps1`: OK. Not run: Codex's `usageLimitExceeded` against a fake that
names it (the chat's Codex fake has no such answer); the app's follower clock; a real CLI at its limit; macOS (no
`cfg` code).

## What is seen, as built (2026-11-14)

Frame: `Parts · An event whose turn failed or is held` `tkjdw` d–g.

- An event whose turn waits for the hold says so under its row (`wait::turn_state`, read with the conversation as
  OA-02's rows are): *Held until 00:00 — today's budget is used*, *Held until 16:40 — the account's plan limit is
  reached*, or, for a limit whose CLI named no reset, *Held — the account's plan limit is reached · trying again at
  15:00* (`orchestrator::held::Hold`: the hold, and whether its end was named). Any entry that starts a turn by
  itself says it: a wait that still stands, a run's end, an event of a target, a daily report, a subscription's end.
  A failed turn that waits for its next try while the hold stands says the hold, not the try.
- The header says *Orchestrator is held until 00:00* (or *… is held · trying again at 15:00*) while an event of the
  conversation is held. **Differs from the frame:** the app's Leads sit right after the title's time (2026-11-11),
  so the line is in the header's free room after them, where it takes only room nothing else uses and never folds a
  chip; the frame draws the Leads at the header's end and the line before them.
- The hold is read only while an event waits to start a turn, and only while the switch is on and the Orchestrator
  has something to run on: with the switch off nothing is said to wait for a reset.

Tests: `orchester-workspace --test orchestrator_held held`; `orchester-app --test orchestrator leading` (the held row
and the header's line); `--test seed orchestrator` (the scenarios `orchestrator-held-budget`, `-held-plan`,
`-held-plan-unnamed`).

## After the design verification (2026-11-14)

An event whose turn is held, waits for its next try or was left to the person no longer carries the chip
*Orchestrator is reading* over the row that says so: it reads *Waits for you* (`screens/orchestrator/timeline.rs`).
