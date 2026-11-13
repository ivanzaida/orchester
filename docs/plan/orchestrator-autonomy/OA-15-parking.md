# OA-15 · Parking a task for the person, and a led target that is stuck

- **Status:** todo
- **Kind:** core + store + workspace
- **Depends on:** OA-12
- **Branch:** —
- **Tokens:** —

## Goal

When the chat cannot move a task (a decision that is the person's, a request it may not allow, a ceiling, a tool
that does not answer), it says so in a form the person can act on, stops spending on that task, and goes on with the
rest. When nothing can move any more, the person is told once, clearly.

## Today

The chat tells the person in the conversation and the run waits. It can set a task's manual Blocked with words
(`update_task` `blocked_reason`), which is informational: nothing records what the person must do, nothing ends the
event, and nothing tells the person that a module has stopped.

## Change

- **A parked task** is a record of the project: the task, its run if one waits, what kind of thing is needed (a
  decision, an approval, a sign-in or credentials, a tool that did not answer, a ceiling reached, something else),
  why the chat could not, and what exactly to do, in the chat's words; who parked it and when.
- **The chat's tool** `park_task` makes it, in any turn but a report's. The task reads as parked in OA-12's standing,
  nothing starts it, tasks that wait on it read *waits on a parked task*, and the events of the task that still stand
  are settled, so they are not tried again.
- **It ends** when the person acts on what it waits for (answers the run's card, decides, clears it on the task
  page) or gives it back to the chat. The leading conversation gets a line, and with work that can start, OA-13's
  event.
- **Stuck.** When OA-12 says a leading subscription is stuck, the follower keeps one *stuck* event in its
  conversation: one turn that lists what is parked and why, then nothing until the standing changes. It raises the
  *Needs you* notification, and OA-24's when it exists.
- The task page and *Needs you* show a parked task with its words; their frames are OA-23's.

## Edge cases

- A run that waits for a person keeps its place and its worktree while its task is parked. Whether it also keeps an
  agent slot is read from the scheduler first: if it does, the stuck rule counts it, and the task says so.
- A person takes the task over (starts, returns or cancels its run themselves): the parking ends, with a line.
- A parked task of a subscription that stops: the record stays; it is the person's either way.
- The chat may not park to avoid work: its instructions (OA-18) say when, and every parking is a decision of OA-22.

## Done when

- Core and store tests for the record and its ends; workspace tests for `park_task`, the settled events, the stuck
  event raised once and again only after a change (`--test orchestrator_parking`).
- `tools/check.ps1` passes; the manual lists the tool and the event.
