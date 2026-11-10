# PV2-03 · The compact *Runs on* picker as a shared component

- **Status:** blocked (PV2-00)
- **Kind:** UI
- **Depends on:** PV2-00

## Goal

The compact picker of K1 `lCApz` and K2 `rjPcV` (component `Runs on picker` `ZcGGy`), 300 wide instead of today's
640 × 320, as one component that the step editor opens (PV2-08) and the orchestrator chat may later adopt (PV2-15).

Today's picker lives in `crates/desktop-app/src/screens/orchestrator/runs_on/` (rail, header, models with Search,
effort, foot, keys, picking). Reuse its data and keyboard handling; do not change the orchestrator's chip or picker in
this task.

## Design

- The connections in a row along the top, grouped by CLI with a thin divider; the chosen one underlined in `accent`;
  a plan at its limit has the warning dot; one that cannot be chosen is at half and its tooltip says why.
- One line under it: the connection's name, CLI · plan, and its 5-hour use with a small meter.
- Search, then the models in the app font (not mono); the chosen one has a check and nothing else on its row.
- The model list is a fixed four rows high, so the picker never changes height; more rows scroll inside it with a
  scrollbar; while searching, fewer results leave the rest of the list empty (K2).
- Effort as one segmented row at the bottom: Default · Low · Med · High · X-high · Max, only the efforts the model
  offers.
- No foot. Keyboard as today: Search focused on open, ← → along the rail, ↑ ↓ in the list, Enter chooses, Esc
  closes.

## Done when

- A reusable component (under `crates/desktop-app/src/ui/` or a shared module) with the data it needs passed in and the
  choice handed back; nothing about pipelines or the orchestrator inside it.
- Strings through i18n, values through theme roles; files under 600 lines.
- An independent design verification of K1 and K2 (through a DevTools preview or the first screen that uses it).
- `tools/check.ps1` passes; tests for the component's keyboard and filtering in `crates/desktop-app/tests/`.
