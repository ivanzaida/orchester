# PV2-07 · A step in the panel: strip, groups, Instructions

- **Status:** blocked (PV2-04, PV2-05)
- **Kind:** UI
- **Depends on:** PV2-04, PV2-05

## Goal

A step opens in the same panel (P2 `u2rYEH`) with ← back to its pipeline; a long pipeline's strip scrolls (P13
`KRQ5J`).

## Design

- Head: ← *Feature delivery* · *step 03 of 6*, the step's name, *Duplicate*, ⋯.
- Step strip: the whole flow in one row (number and name chips, the gate's hand, `04 ∥ 05` in one frame), the current
  step outlined in `accent`; a chip switches steps. When it does not fit (P13) it scrolls sideways, opens with the
  current step in view, shows ‹ › only on the side that has more and a thin thumb; ← → switch steps.
- *Step:* Executor (the profile), *Runs on* (a plain select until PV2-08), *Instructions* as a multi-line text area
  edited in place with the hint *{{ }} inserts task and run values* (no chevron, no separate page).
- *Run:* When changes are requested, Step budget. There is **no** *Runs after* field (order is PV2-06's).
- *Gate and access:* Wait for your approval (switch), Repository access, Sources (pills and Add; *1 own · 1 for every
  step*).
- *Save* and *Cancel* act on the whole pipeline.

## Done when

- The route holds the open step; Back and Forward move between the pipeline and the step.
- The placeholder check of today's Instructions page (`step_instructions.rs`) shows under the text area.
- An independent design verification of P2 and P13.
- `tools/check.ps1` passes; tests for the strip scrolling to the current step and for switching steps.
