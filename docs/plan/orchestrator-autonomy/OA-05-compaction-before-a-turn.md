# OA-05 · The conversation is compacted before a turn that would not fit

- **Status:** done (2026-11-13; independent review: one major finding fixed, see *Review*)
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

## As built (2026-11-13)

In the runner, not in the follower: `answer_in_chat` keeps a compaction as running for a turn Orchester started by
itself whose session is resumed on the Orchestrator's connection and filled 70 % of its window
(`chat/compact_first.rs`), and the turn's task carries it out before the turn's CLI starts, then reads the
conversation again. So nothing can loop between a refused compaction and a turn that waits for it, and a compaction
that failed never holds the turn. The compaction's line is the person's *Compact* line as it is: saying that Orchester
started it is a new sentence in the conversation and waits for a frame. *Stop response* pressed while it compacts
ends the compaction's CLI and the turn, which then starts none. The compaction and the turn each have an answer's ten
minutes. A turn tried again (OA-01) compacts again while the session is still that full: four times at most.

## Review (2026-11-13)

An independent reviewer read the change. Fixed: *Stop response* pressed while it compacted was swallowed until the
compaction ended; a compaction was kept for a session the Orchestrator's account cannot resume, and read as failed;
the compaction's command is made before its event is kept, so no event can stay running. The tests now check the
order of the two CLIs and a compaction that fails.

**Found on the way and fixed with it** (`chat/mod.rs` `over`, `chat/turn.rs`): a turn kept its end and then tidied
up, so a next turn asked for at once could be refused `ChatBusy` while the first's task was still returning. It
showed as `while_answering_queued_messages_wait_for_the_answer_and_stop_and_send_goes_first` failing about one run
in three of its binary. For the Orchestrator it meant an event right after a turn lost a minute to a retry. The end
is now kept last, and a conversation whose work kept its end is not busy. Six runs of that binary passed after it.

Tests run: `orchester-runner --features test-agent` `--test chat_compact compact`, `--test chat` (`chat_`, `a_turn`),
`--test chat_while_answering while_answering`, `--test chat_codex chat_codex`, `--test chat_rewind rewind`, `--test
chat_fork fork`; `tools/check.ps1`: OK. Not run: the Codex path of a compaction that runs first; *Stop response*
during the compaction (no fake that stays in `/compact`); a real CLI; macOS (no `cfg` code).
