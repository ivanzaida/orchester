# OA-10 · Choosing *Lead to the end*, its ceilings and what is used

- **Status:** blocked (frames); chosen meanwhile by the first words of the /watch order (2026-11-14)
- **Kind:** design + UI
- **Depends on:** OA-09
- **Branch:** —
- **Tokens:** —

## Goal

The person turns a subscription into a leading one with a click, sees that it leads and how much of its ceilings is
used, and can change that without stopping the watching.

## Design first

On `Orchestrator watch · redesign` `QOPcg`, new frames beside 02 (choosing modules), 04 (leading) and 06 (/watching),
and on the settings row `VrdLr`:

- the watch pickers' footer with the choice *Follow* / *Lead to the end*, by keyboard too;
- the app's line for a leading subscription (*Leading Webhooks v2 to the end · 12 tasks — …*) and the header chip
  that tells a leading subscription from a following one without a word of jargon;
- `/watching` and the *+N* list with each ceiling and what is used, and the budget when one is set;
- changing the mode or a ceiling of a subscription that exists, from that list;
- Project settings › Orchestrator › *Watching*: the defaults a new leading subscription takes;
- the states: a ceiling reached, the budget used, light theme.

## Change

- The pickers, lines, chips, `/watching` answer and settings rows as drawn (`crates/desktop-app/src/screens/orchestrator/`,
  `ui/slash/`, `ui/mention/`, the project settings' Orchestrator tab). `get_watches` gives the mode, the ceilings and
  what is used.
- Choosing the mode is the person's: synthetic input is refused, as on *Take it here*.
- A subscription that is turned into a leading one raises OA-13's *work can start* when it has work that can.

## Done when

- `cargo test -p orchester-app --test orchestrator watch_lead` for the picker's choice, the lines, the list and the
  refusal of synthetic input; `--test i18n locales_`.
- An independent design verification of every new frame and state. `tools/check.ps1` passes.

## Until its frames (2026-11-14)

So that leading to the end can be chosen at all, the person's own `/watch-module` and `/watch-task` choose it when
their order begins with the words *lead to the end* (in any case; `orchester_core::orchestrator::watch::
asked_to_lead`): the targets are then led within the default ceilings, and what follows the words is the order. A
target the conversation only followed is led from then on. The watch the agent applies at the person's request
(`watch`) never chooses it, so only the person does. To stop leading, `/unwatch`. Nothing shows that a target is led
but `get_standing` (`leads_to_the_end`); the ceilings cannot be changed. All of that is this task's, with its frames.

Tests: `orchester-core --test orchestrator_ceilings` (the words), `orchester-workspace --test orchestrator_watch_lead`.
