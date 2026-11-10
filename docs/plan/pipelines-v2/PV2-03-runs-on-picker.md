# PV2-03 · The compact *Runs on* picker as a shared component

- **Status:** done (2026-11-10; review round 2 PASS; merged `c37c1caad`; design verification of K1/K2 in PV2-15, D-008)
- **Kind:** UI
- **Depends on:** — (the design is read from Pencil's open document until PV2-00)
- **Branch:** `claude/pv2-03-runs-on-picker` (`cfd06b483` … `79dfc3c98`)
- **Tokens:** 51.4M — implementation 29.0M + 16.4M (output 107k + 77k), review 3.7M + 2.2M (output 11k + 14k)

## Goal

The compact picker of K1 `lCApz` and K2 `rjPcV` (component `Runs on picker` `ZcGGy`), 300 wide instead of today's
640 × 320, as one component the orchestrator chat opens (PV2-15). (A step chooses an agent, not a model: PV2-08.)

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
