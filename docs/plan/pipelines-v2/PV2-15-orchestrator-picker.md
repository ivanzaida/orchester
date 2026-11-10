# PV2-15 · The orchestrator chat adopts the compact picker

- **Status:** needs decision — do not start
- **Kind:** UI
- **Depends on:** PV2-03 and the person's decision

## Why it waits

The compact picker (K1 `lCApz`) was designed for the step editor. Using it in the orchestrator chat's composer as well
(one component for both, replacing the 640 × 320 panel of `screens/orchestrator/runs_on/`) changes the board
`Orchestrator · redesign` (`Runs-on picker · states` `t7cdi`: *Codex at its limit*, *Searching*, *Many models*, *No
connections*), which needs its own redesign first. The person decides whether to do it.

## Scope once decided

Redesign those four states with the compact picker on `Orchestrator · redesign` (a design step), then make the
composer's chip open the shared component, keep the chat's *Applies* choice if it is still wanted, and remove the old
panel's code.
