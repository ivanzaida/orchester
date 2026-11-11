# PV2-20 · The step head's ⋯ menu

- **Status:** todo
- **Kind:** UI
- **Depends on:** PV2-18
- **Branch:** —
- **Tokens:** —

## Goal

P2 `u2rYEH` (`Xvbxz`) and P2b `EGEos` (`eUqAD`) draw a 28×28 ⋯ beside *Duplicate* in the step head; the app has none
since PV2-07 (D-030). Its menu is the flow card's (P4 `naN3G`): Move up (Alt ↑), Move down (Alt ↓), Run in parallel
with…, Add a step after, Duplicate, Remove step — same rows, same rules, same words, one implementation.

## Done when

The ⋯ opens the card's menu for the open step, each row works from the head and is tested; nothing moves on hover or
focus (D-022); an independent check against P2/P2b/P4; `tools/check.ps1` passes.
