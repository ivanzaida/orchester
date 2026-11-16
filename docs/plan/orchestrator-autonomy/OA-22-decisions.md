# OA-22 · The chat's own decisions with their reasons

- **Status:** done (2026-11-14); its screen is OA-23
- **Kind:** core + store + workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

Everything the Orchestrator decided while nobody asked is one list the person can read in five minutes: what it
did, why, and how to undo it. This is what `decisions.md` is for work outside Orchester.

## Today

Every change the chat makes is an action in its conversation (`ChatAction`), and the daily report counts them
(`ReportAction`: tool, task, subject, way, time, link). Only a tool request's answer carries a reason
(`ChatAction::tool_answer`). To learn why a step was accepted, the person reads the turn's text in the right
conversation.

## Change

- **A reason is part of the call.** In a turn Orchester started, the calls that are a judgement take a required
  one-sentence `reason` and are refused `reason_required` without it: accepting a step, a return, a retry, a raise,
  an answer to a question, a decision on a plan (OA-11), running a task again (OA-14), parking (OA-15), a merge of a
  run with a step accepted unverified. A tool request's answer has one already.
- **Kept as a decision** of the project beside its action: what, on which task and run, the reason, the
  conversation and turn, and what undoes it where something does (a parked task given back, a run returned to a
  step); otherwise nothing is promised.
- **Reviewed.** A decision is open until the person marks it seen, or overrules it, which is a message of theirs in
  that conversation that quotes it. Marking is the person's; synthetic input is refused.
- **Read** with a tool, `get_decisions` (open ones first; by task, module, conversation or day), for every MCP client
  and the chat.
- `ReportAction` gains the reason (OA-25).

A person's turn makes no decisions of this kind: they asked.

## Edge cases

- The reason is the agent's text. It is shown as text, never rendered as a link that acts, and it is not sent
  outside the computer (OA-24).
- Decisions of a conversation that is archived stay in the project's list.
- A decision whose run or task is gone stays, with its link reading as gone.

## Done when

- Core and store tests for the record and its review states; workspace tests for `reason_required` on each call and
  for `get_decisions` (`--test orchestrator_decisions`).
- `tools/check.ps1` passes; `docs/manual/mcp-tools-work.md` and the chat's manual list the tool and the argument.
  The list's screen is OA-23.

## As built (2026-11-14)

- **The record:** `orchester_core::orchestrator::decision` (`Reason`, one line of at most 400 characters; `Decision`;
  `Review` open → seen → overruled, forward only; `Undo`), kept in `orchestrator_decisions` (migration 174).
- **The reason is part of the call** for `continue_run` (but `resume`), `answer_question` and `decide_plan` in a turn
  Orchester started (`orchestrator/decisions.rs`): refused `reason_required` before anything else is asked, taken off
  the arguments before the app's tool is reached, and, when the call was done, kept as a decision. `decide_approval`
  keeps its own required reason and is kept the same way. A person's turn drops a reason it was given.
- **Read** with `get_decisions` (the app's tool, so every client and the chat have it): by task, module,
  conversation, local day, open only. A team project answers as a local one.
- **Reviewed** by `orchestrator::decisions::review_in`, which no tool calls: the person's screen is OA-23.

Not here, with the task that brings each: running a task again (OA-14), parking (OA-15), and a merge of a run with a
step accepted unverified (the acceptance itself is a decision already; the merge rule is the chat's, not a
judgement, so it takes no second reason). `ReportAction`'s reason is OA-25.

Tests: `orchester-core --test orchestrator_decision`, `orchester-store --test orchestrator_decisions`,
`orchester-app --test mcp_domain orchestrator_decisions`.
