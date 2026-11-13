# OA-05 · The conversation is compacted before a turn that would not fit

- **Status:** todo
- **Kind:** workspace + runner
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

A conversation that leads a module takes hundreds of event turns. It keeps answering without a person pressing
*Compact*.

## Today

Compaction runs only from the person's action (`crates/desktop-app/src/screens/orchestrator/compact.rs`, the runner's
`chat/compact.rs`). The store already keeps a turn from starting while a compaction of its session runs
(`start_wait_turn`). The context a session used is read for the context meter (`docs/design/context-meter.md`).

## Change

- Before the follower starts a turn by itself, it reads what the conversation's session used of its window as the
  meter has it. At 70 % or more it starts a compaction through the path *Compact* takes, and the turn starts
  when it ended.
- A compaction that failed does not hold the turn: the turn starts, and one that then fails is OA-01's.
- The compaction's event in the conversation says Orchester started it.
- What the conversation leads, its orders and (OA-12) where its targets stand are in the standing instructions and
  the event's prompt, so they do not depend on what a compaction kept.

## Edge cases

- The budget is used: compaction is refused; it waits with the turn (OA-03).
- A person's queued message goes first, as today; it is not compacted under.
- A session whose use is unknown (a new one, a CLI that does not report it): no compaction.

## Done when

- Workspace test: a turn at 70 % starts after a compaction, one below it at once, one after a failed compaction
  anyway (`--test orchestrator_waits compaction`). Runner test with the fake CLI for the order of the two.
- `tools/check.ps1` passes. The conversation's compaction row is the existing one.
