# OA-21 · A person-only request in a led run parks its task

- **Status:** todo
- **Kind:** workspace
- **Depends on:** OA-15
- **Branch:** —
- **Tokens:** —

## Goal

An agent that asks for something only a person may allow does not hold a module: the person finds the request, with
the chat's reading of it, and everything else went on.

## Today

For command requests in the person-only categories (a command not read to the end, rewriting history and a push,
deleting outside the temporary folder, a write outside the worktree, reading credentials, installing software, any
network category) the chat's `approve_once` is refused `person_only` (`crates/core/src/run/answer/auto.rs`, enforced
by the runner). Its turn tells the person what is asked and what it recommends; the run waits.

## Change

For a task led to the end, the turn of such a request:

- denies it with words for the agent when the task plainly does not need it, as today;
- otherwise parks the task (OA-15) as *an approval*, with what is asked, the hold, and its recommendation, and
  decides nothing. The request's card stays the person's; answering it ends the parking.

The categories stay person-only. This task adds no way for the chat to allow them.

## Edge cases

- The same agent asks again after a denial: the second request's turn sees the first in the conversation and parks
  rather than deny in a loop.
- Requests of several tasks arrive together (OA-07): each task is parked by itself.
- A request auto mode's reviewer still judges is `reviewer_judging`, as today: not parked until it comes to a
  person.

## Done when

- Workspace tests (`--test orchestrator_tools person_only_request`): the denial, the parking, the end of the parking
  when the person answers, the second request.
- `tools/check.ps1` passes; the manual's *Tool requests* paragraph says what a leading subscription does.
