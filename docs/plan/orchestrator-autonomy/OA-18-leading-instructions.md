# OA-18 · The instructions of a leading turn

- **Status:** done (2026-11-14); a run with a real model was not made
- **Kind:** core
- **Depends on:** OA-12, OA-13, OA-14, OA-15 (and OA-22 for the reasons)
- **Branch:** —
- **Tokens:** —

## Goal

In a turn of a subscription that leads to the end, the agent knows it is alone: it reads where the target stands,
moves everything it may move, and ends every event in an action, a parking or a named wait. The tools hold the
limits; the instructions hold the judgement.

## Today

`crates/core/src/orchestrator/instructions.rs` (with `instructions/`) builds the standing instructions and each
event's prompt. They are written for a person who reads along: on a question the brief does not answer the turn
"tells the person the question with options and a recommendation"; on a failure it "starts, resumes, cancels or
merges nothing"; on a stall it only writes.

## Change

A leading subscription's prompts say, beside what the others say:

- **Read first.** Where the target stands (OA-12) is at the head of the prompt; act on that, then on the event.
- **Order the work.** When it first takes a target, and when tasks are added, it reads the briefs and makes tasks
  that will change the same files wait for one another (`update_task` `blocked_by`), so they do not run side by side.
  This is the only guard against two runs editing one file before the fact; base-moved notices and
  `resolve_conflicts` stay the net.
- **Decide what is the task's to decide.** A question the brief, the criteria, the project's docs and AGENTS.md do
  not answer is decided in this order: what the design shows; what the app already does for the same thing; what
  surprises the person least; the smaller and more easily reverted change. The answer and its reason are kept
  (OA-22).
- **Park what is not.** Anything irreversible or outward (a push, a release, deleting work, another system, money),
  anything the tools refuse as the person's, a ceiling reached, a tool that did not answer after another try: park
  the task (OA-15) with what to do, and go on.
- **Unverified steps**: accept a step as it is only when what it could not check cannot be checked at all;
  a tool that did not answer is tried again, then parked. Say which step and what was left unverified.
- **Never wait in words.** A turn does not end by telling the person to press something its tools can do, nor by
  asking a question nobody is there to answer.
- **Give the reason** with every action that was a judgement, in one sentence.

The prompts of a *Follow* subscription, and of a conversation that leads nothing, do not change.

## Edge cases

- The order of the subscription contradicts these (it says "ask me first"): the order narrows, so the turn parks
  instead of deciding.
- `instructions.rs` and its modules are checked for size before anything is added (`AGENTS.md` rule 3); the leading
  text goes into a module of its own.

## Done when

- Core tests that pin the leading prompts (`cargo test -p orchester-core --test orchestrator_instructions leading`),
  and that the others are unchanged.
- OA-26 passes with these prompts and the scripted agent; a run with a real model is listed as not run unless the
  person asks for one.
- `tools/check.ps1` passes.

## As built (2026-11-14)

A turn an event started by itself is given the leading note beside its prompt when the subscription the event came
through leads its task to the end (`ProjectDb::wait_turn_prompt`;
`orchester_core::orchestrator::instructions::leading`). The note says: nobody is at the computer; read where the
target stands first (`get_standing`); decide what is the task's to decide, in the stated order of precedence; park
what is not, and where the conversation's order asks to be asked first; accept a step as it is only when what it
could not check cannot be checked at all; do not run tasks that change the same files side by side; never wait in
words; give a reason with every judgement. Beside it, what to do about a request only the person may allow (OA-21),
a failure (OA-14) and a stall, by its period (OA-06). The prompts of a subscription that only follows, and an event's
own prompt, are unchanged. The events of the whole target have prompts of their own (`instructions/standing.rs`).

Differs from the task file:

- Where the target stands is not put at the head of the prompt: the turn is told to read `get_standing` first. The
  prompt is built by the store, where the runner reads it, and the standing is read by the workspace.
- An event of a team task this computer keeps only as an identity names no subscription in its prompt (the store
  resolves it from a stored task row), so it gets no leading note: a row of the parity table.
- Not run with a real model. OA-26 is the scripted proof.

Tests: `orchester-core --test orchestrator_instructions leading`, `orchester-store --test
orchestrator_leading_prompt`.
