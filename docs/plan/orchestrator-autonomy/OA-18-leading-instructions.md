# OA-18 · The instructions of a leading turn

- **Status:** todo
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
