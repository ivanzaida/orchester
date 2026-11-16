# PV2-06 · Reorder steps in the flow: drag, ⋯ menu, keys

- **Status:** done (2026-11-11; check 3 (review + design, one child) PASS and no blocking/major; D-021 and D-022 built; merged `915de1fb7`; follow-ups in PV2-17 items 25, 29–33)
- **Kind:** UI
- **Depends on:** PV2-01, PV2-05
- **Branch:** `claude/pv2-06-reorder` (`1f3788272` … `4162e1d73`)
- **Tokens:** 280.3M (out 461k) across 4 implementation rounds and 3 merges, review 1, a cancelled verification, check 2, check 3 (one cancelled)

## Goal

A step's order is changed in the flow, with PV2-01's core operations (design P3 `K4B56`, P4 `naN3G`).

## Design

- **Drag (P3):** the card's handle drags it; its place stays faded; a line with an `accent` node shows where it lands
  and says what it means (*runs after 01 Research, before 02 Plan*); dropping on a card makes the two parallel; Esc
  cancels.
- **⋯ on a card (P4):** *Move up* (Alt ↑), *Move down* (Alt ↓), *Run in parallel with …* (only the neighbours it can
  join), *Run on its own* (inside a parallel group), *Add a step after*, *Duplicate*, *Remove step* (`danger`).
- A refused move says why (PV2-01's reason). Nothing is saved until *Save* (PV2-10).

## Done when

- Every move works by mouse and by keyboard and redraws the flow at once.
- An independent design verification of P3 and P4.
- `tools/check.ps1` passes; UI tests for the menu's items per position and for a keyboard move.
