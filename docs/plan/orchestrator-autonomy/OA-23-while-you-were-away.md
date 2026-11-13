# OA-23 · *While you were away*: decisions to review and what needs you

- **Status:** blocked (waits for the frames, `frames.md` E)
- **Kind:** design + UI
- **Depends on:** OA-15, OA-22
- **Branch:** —
- **Tokens:** —

## Goal

The person who comes back sees two things first: what waits for them, each with exactly what to do, and what the
Orchestrator decided on its own, each with its reason and a way to say "no".

## Design first

The design decides where this lives; the proposal is the existing *Needs you* page, not a new one:

- **Parked for you** at its top: a parked task's row with what is needed, why the chat could not, what to do, its
  action (the request's card, the question, *Give it back to the Orchestrator*) and the task it holds up;
- **Decided without you**: the open decisions, newest first, grouped by module; each with its reason, the link to
  the task and to the conversation's turn, *Seen* and *Overrule…* (opens that conversation with the decision quoted
  in the composer), and *Mark all seen*;
- the task page's Summary for a parked task (status line, the wait row, *Led in*), in the tone of a wait that cannot
  be met (`rlnEZ` f on the watch board);
- the leading conversation's *stuck* event and the parked rows;
- the states: nothing parked, nothing to review, loading, a read failure; dark and light.

No decision is undone by a button the design cannot explain: *Overrule* is the person's words to the chat, except
where OA-22 names an undo.

## Change

The page's sections, the task page's rows and the conversation's rows as drawn, reading OA-15's and OA-22's records;
the counts on *Needs you* in the project bar include what is parked. Strings in every locale.

## Done when

- App tests for both sections, *Seen*, *Overrule* and the refusal of synthetic input (`--test needs_you away`,
  `--test orchestrator parked`); `--test i18n locales_`.
- An independent design verification of every frame and state. `tools/check.ps1` passes.
