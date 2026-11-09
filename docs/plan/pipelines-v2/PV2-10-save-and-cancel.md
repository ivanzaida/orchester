# PV2-10 · Edit in place: Save makes vN, Cancel puts it back

- **Status:** blocked (PV2-04)
- **Kind:** UI + workspace
- **Depends on:** PV2-04
- **Branch:** —
- **Tokens:** —

## Goal

A local pipeline is edited in place like the hub's resources (`local_hub/save_bar.rs`): the panel holds every change
until *Save*, which publishes the next version at once, and *Cancel* puts back what is stored (P1 `s7LMyQ`'s bar:
*Save makes v4 · runs keep the version they started on*). This replaces *Save draft* and *Publish vN* for local
projects.

## Scope

- One edit spans the pipeline and all its steps: the flow's moves (PV2-06) and the step fields (PV2-07, PV2-08).
- A pipeline never published shows *Save makes v1 · New task offers it once every step is ready* (P8).
- Leaving with unsaved changes asks, as the hub does (`local_hub/leave.rs`).
- A refused save says why next to the bar (as the hub's *save refused*, L12 in `Agents · v2`).

## Done when

- *Save* publishes through the workspace's existing write path; runs keep their version.
- `tools/check.ps1` passes; workspace tests for saving as the next version and for Cancel; a UI test for the leave
  guard.
- An independent design verification of the bar in P1 and P8.
