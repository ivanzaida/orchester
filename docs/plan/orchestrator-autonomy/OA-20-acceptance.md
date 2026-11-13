# OA-20 · A module's acceptance before its delivery

- **Status:** todo (the person, 2026-11-13: the Orchestrator delivers after acceptance)
- **Kind:** core + workspace + UI
- **Depends on:** OA-17
- **Branch:** —
- **Tokens:** —

## Goal

What reaches the base branch from a module nobody watched was looked at as a whole by someone who did not write it.

## Today

Each task's run passes its own pipeline, and a step the chat accepted unverified counts as passed since 2026-11-11.
Nothing looks at the module's whole diff; work outside Orchester adds an independent reviewer and a verifier per
task because the pipeline's own review missed defects (`.claude/skills/work-outside-orchester`).

## Decided (2026-11-13)

No new kind of reviewer: the gate is a run, made of what Orchester has.

- A module with its own branch (OA-17) names an **acceptance pipeline** (for example Check → Review → Verify),
  chosen by the person in the module form.
- When every task of the module is done, the chat that leads it creates one acceptance task in the module, whose run
  starts from the module's branch with the module's whole diff against its base branch as what the steps review. Its
  brief is Orchester's, not the agent's: the module's tasks, their criteria, and every step the chat accepted
  unverified.
- Passed, the module reads *accepted*, and with *Deliver when accepted* the chat may request the delivery. Findings
  become tasks of the module (the chat creates them from the report), which reopen it; the acceptance runs again
  after them, within a ceiling of its own (2).
- Without an acceptance pipeline the module is never delivered by the chat.

## Still open

Whether the acceptance may be skipped for a module of one task. Built as: never skipped.

## Done when

- Core and workspace tests: the acceptance task made once when every task is done, its brief, *accepted*, findings
  that become tasks and reopen the module, the second acceptance, its ceiling, no delivery without a pipeline.
- The module form's acceptance pipeline row and the Summary's *accepted* state match their frames (`frames.md` D).
- `tools/check.ps1` passes.
