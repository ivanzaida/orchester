# PV2-06 · Reorder steps in the flow: drag, ⋯ menu, keys

- **Status:** in progress
- **Kind:** UI
- **Depends on:** PV2-01, PV2-05
- **Branch:** `claude/pv2-06-reorder`
- **Tokens:** —

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
