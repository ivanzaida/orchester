# OA-01 · A turn Orchester started that failed leaves its event open

- **Status:** todo
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
- **Store.** The turns an event had are kept (which message, how it ended, when), and the entry points at the latest.
  The reading the follower starts turns from becomes *due*: no turn yet, or the latest failed in a way that is tried
  again, its time passed and tries are left. One more reading gives the soonest time an event is due, for OA-02's
  clock.
- **Workspace.** `follow_in` (`crates/workspace/src/orchestrator/waits.rs`, 530 lines: split the turn-starting part
  out first) starts due events as it starts new ones, still only while the event stands and the chat drives. The
  prompt of a later try names the earlier one and how it ended, and tells the agent to read the state before it acts.
- **After the last try** the entry reads as left to the person, counts in *Needs you* and raises the *Needs you*
  notification once.

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
