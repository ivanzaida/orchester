# OA-01 · A turn Orchester started that failed leaves its event open

- **Status:** done (2026-11-13; independent review: one blocking and one major finding fixed, see *Review*)
- **Kind:** core + store + workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

An event that starts a turn by itself (a wait, a run's end, a stall, a conflict, a waited task's merge or close) is
settled only by a turn that completed. A turn that failed is tried again, a few times, and then the event is left to
the person where they see it; it is never lost in silence.

## Today

- `ProjectDb::start_wait_turn` gives the entry its `message_id` once, and `automatic_waits_without_turn` reads only
  entries with none (`crates/store/src/project_db/orchestrator_waits.rs`).
- `send::answer_wait` and `end_wait_turn` end a turn the runner did not take as `TurnFailure::CannotStart` and say it
  "is never asked for again" (`crates/workspace/src/orchestrator/send.rs`).
- The runner ends a turn as `Failed`, `TimeLimit`, `NotSignedIn`, `ConnectionGone`, `Account` or `Interrupted`
  (`TurnFailure`, `crates/core/src/orchestrator/chat.rs`, kept by `failure.rs`); a runner that attaches ends the turns
  still thinking as interrupted.

## Change

- **Core.** `TurnFailure` says what follows it: tried again later (`CannotStart`, `Failed`, `Interrupted`,
  `TimeLimit`), held for a reason with its own end (OA-03 adds the budget and the plan limit), or the person's
  (`NotSignedIn`, `ConnectionGone`, `Account`). The schedule is a constant of the rule: after 1, 5 and 15 minutes,
  four turns in all.
- **Store.** The entry keeps how its earlier turns ended (a `tries` column; the entry itself holds the latest), and
  a failed entry takes its next turn in place (`retry_wait_turn`, under the conditions a first turn starts under).
  The follower starts turns from the entries without a turn and from the failed ones whose time came.
- **Workspace.** `follow_in` (`crates/workspace/src/orchestrator/waits.rs`, 530 lines: split the turn-starting part
  out first) starts due events as it starts new ones, still only while the event stands and the chat drives. The
  prompt of a later try names the earlier one and how it ended, and tells the agent to read the state before it acts.
- **The clock** is here, not in OA-02: without it a try would start only when something else woke the follower. The
  app's follower sleeps until the soonest due time (`turns::next_retry_in`), as it does for stalls.
- **After the last try** nothing more starts by itself. The wait behind the event is already on its task's page and in
  *Needs you*, so nothing is added there; saying *left to you* on the entry, and **Try again**, are OA-02's.
- Not tried again: a daily report's turn and the summary of a watching that ended.

## Edge cases

- A turn that timed out after it acted (merged, answered): the next try reads the state; `merge_run` of a merged run
  and an answer to an answered question are refused by their tools. "One merge attempt per run end" stays counted by
  the run's end, across tries.
- A turn the person stopped (*Stop response*) is the person's choice and is not tried again.
- The drive switch turned off, the subscription stopped or moved, the conversation archived: no further try.
- An event that stopped standing between tries is settled without a turn, as today.
- A runner that restarts mid-turn: the turn ends interrupted and is tried again.

## Done when

- Workspace tests (`cargo test -p orchester-workspace --test orchestrator_waits retries`): each failure kind, the
  schedule, the cap, the person's stop, the switch going off, an event that stopped standing.
- Store tests for the due reading and the soonest due time.
- `tools/check.ps1` passes. Nothing visible changes here beyond what the entry already shows; OA-02 draws it.

## Review (2026-11-13)

An independent reviewer read the change. Fixed: a retry failed on a foreign key when the failed turn had kept a todo
list (the list now goes with the failed turn, in the same transaction); a merging turn waiting to be tried again did
not hold the merges after it. Left as it is: a wait whose next try cannot start (the chat stopped driving) still
counts as open, so git is asked about its run on each follow, as for a wait that never had a turn. Noted, not
changed: a turn cut short without a recorded end is due at once, so repeated crashes spend the four turns without
the waits between them.

Tests run: `orchester-core --test orchestrator_retry retry`; `orchester-store --test orchestrator_waits waits` and
`--test todo_lists todo`; `orchester-workspace --test orchestrator_led_waits led_waits`, `--test orchestrator_auto_merge
auto_merge` and `--test orchestrator_run_ends run_ends`; `tools/check.ps1`: OK. Not run: the app's own test binaries
(the follower's clock has no test of its own), a turn in a running app, anything on macOS (no `cfg` code changed).
