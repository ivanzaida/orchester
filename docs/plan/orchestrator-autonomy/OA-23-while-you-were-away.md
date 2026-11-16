# OA-23 · *While you were away*: decisions to review and what needs you

- **Status:** built (2026-11-14); design verification pending
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

## As built (2026-11-14)

Frames: `W01`, `W02`, `W03` and `Parts · While you were away` on the board `Orchestrator autonomy · frames to
approve`. Read by `orchester_workspace::orchestrator::away` (parked tasks oldest first, open decisions newest
first with their task's module), the same for a local and a team project.

- **Overview** (`screens/overview/away.rs`), over the waiting list: *N parked for you*, a row per task with what is
  needed, its age, **Give it back** and the task's link, and under it why and what to do in the Orchestrator's
  words; *N decided without you*, grouped by module, a row per decision with what was done, when, **Seen** and
  **Overrule…**, and under it the reason and the conversation; **Mark all seen** at the head. With neither, nothing
  shows.
- **Overrule…** marks the decision overruled, quotes it at the start of that conversation's draft
  (`screens/orchestrator/draft::quote`) and opens the conversation for the person's own words.
- **The task's Summary** (`screens/task/parked.rs`, `TaskDetail::parked`) starts with the parking's row in the
  warning tone: what is needed, why and what to do, and **Give it back**.
- All of these are a person's: synthetic input is refused outside a QA copy.
- **Differs from the frames:** the parked tasks are a frame of their own over the waiting list, not rows inside it,
  and are not added to its count; a row does not name the tasks a parked one holds up; the header's status line
  and the properties of a parked task are as they were; the conversation has no line of its own for a parking (the
  turn that parked says it, and the *stuck* line follows); *Overrule…* quotes as text, not as a block over the
  composer; the card's *nothing decided*, loading and unreadable states are no card at all, a line of the reason
  when the reading failed.

Tests: `orchester-workspace --test orchestrator_away away`; `orchester-app --test overview overview_away`.

## After the design verification (2026-11-14)

- What was parked stands over what waits for the person, what was decided under it (the component is mounted
  twice, `away::Part`).
- **Give it back** is refused, with the parking kept and the reason under the row, while a tool request of the
  run the task was parked with still waits (`orchestrator.parking.requestOpen`). What else a task was parked for
  is not read: giving it back says the chat may go on.
- A parked task's header says *Parked for you — an approval is needed* in the warning tone
  (`screens/task/parked::status_line`).

Tests: `orchester-app --test mcp_domain orchestrator_parking_is_not_given_back`, `--test overview overview_away`.
