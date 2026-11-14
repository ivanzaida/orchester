# OA-10 · Choosing *Lead to the end*, its ceilings and what is used

- **Status:** partly built (2026-11-14): the line and the header chip of a leading subscription, design verification pending; the pickers, `/watching`'s mode and ceilings, changing them and the settings rows are not built
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

## The line and the chip, as built (2026-11-14)

Frames: `L01` `QMwUB` and `Parts · Lines` `e6nsBS` a.

- The header chip of a target led to the end has a `flag` for its icon and *to the end* in the muted tone after its
  name (`leads::chip`, from `LeadChip::to_the_end`); a followed one is as it was. The chip keeps its ×, which the
  frame's copy of 04 does not draw.
- The line of a /watch that led its targets to the end reads *Leading Webhooks v2 to the end · 12 tasks — it starts
  them, merges them and parks what needs you*, with a `flag` (`watch::lines`). The entry keeps no mode: the line is
  told by the command kept right before it, whose order begins with the words (`ChatView::led_to_end`,
  `orchestrator::watch::start::led_to_end`), until the picker of this task chooses the mode and keeps it.
- The turn an event of a led target started says which on its answer's tag: *work can start*, *stuck*, …
  (`started_by::standing_tag`).

Not built here: the *Lead to the end* chip in the command's bubble (`L01`), the pickers (`L03`), the mode control
and the ceilings row of `/watching` (`L02`), the ceilings' form and the settings rows.