# OA-10 · Choosing *Lead to the end*, its ceilings and what is used

- **Status:** blocked (waits for the frames, `frames.md` B)
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
