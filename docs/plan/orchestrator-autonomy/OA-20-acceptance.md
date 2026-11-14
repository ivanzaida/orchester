# OA-20 · A module's acceptance before its delivery

- **Status:** done but for what waits for frames (2026-11-14)
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

## As built (2026-11-14)

- A module with a branch of its own names an **acceptance pipeline** and whether the conversation that leads it may
  **deliver it when accepted** (`OwnBranch`, the tools' arguments; the form's rows wait for frames).
- `start_acceptance` makes one task in the module, on that pipeline, keeps it as the module's acceptance
  (`module_acceptances`) and starts it. Its run starts from the module's branch. Its brief is Orchester's
  (`orchestrator::instructions::acceptance_brief`): the module's tasks with their criteria, every step automation
  accepted unverified, and the instruction to review the module's whole diff against its base branch.
- What a module waits for once its every task is done is one rule (`DeliveryFacts::step`): its acceptance, then its
  delivery; *accepted* holds until more work is merged into the module's branch. An acceptance that failed or was
  canceled is done as an attempt, and the next one starts after the tasks made from its findings, twice at most by
  the chat itself (`ACCEPTANCES_MOST`).
- The conversation that leads the module to the end is told each step once (`Raised::Accept`, `Raised::Deliver`,
  `Raised::DeliveryWaits`), and in a turn nobody asked for it may call `start_acceptance` and `deliver_module` only
  at that step, with its reason, which is kept as a decision (`orchestrator::module_turn`). Without an acceptance
  pipeline, past the ceiling, or with *Deliver when accepted* off, the delivery is the person's and the conversation
  says so once.

**Decided while building:** the acceptance is never skipped, a module of one task included. Findings become tasks by
the chat's `create_task` in the turn of the acceptance run's end, as its instructions say: Orchester does not parse a
report into tasks.

**Not built:** the module form's rows and the Summary's *accepted* state (frames D); a team module's acceptance (a
team's tasks are made on its server: `start_acceptance` is refused there, a row of the parity table).
