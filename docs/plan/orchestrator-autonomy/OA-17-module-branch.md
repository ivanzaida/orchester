# OA-17 · A local module's own branch and its delivery

- **Status:** todo (the person, 2026-11-13: the module has its branch, and the Orchestrator delivers it)
- **Kind:** design + core + workspace + UI
- **Depends on:** OA-16; the frames of `frames.md` D for its visible part
- **Branch:** —
- **Tokens:** —

## Goal

A module's tasks can be merged one after another all night without touching the person's checkout, and the module
reaches the base branch as one reviewed whole.

## Today

A local module has a `base` choice and no branch of its own (`crates/core/src/module.rs`); its tasks merge into the
base branch, by default the one the person's checkout is on. A checkout with local changes the run also changes sends
the merge to the Local integration page, which is the person's. Only a team module has `module/<slug>` and a
delivery, on the team server.

## Decided (2026-11-13)

- A local module may have **its own branch** (the module form's choice, off by default): `module/<slug>`, made from
  the module's base branch when its first task starts. Its tasks' runs start from it and merge into it.
- Merging a task there never changes the person's working tree, so it needs no Local integration; conflicts go to an
  agent as today (`resolve_conflicts`).
- **Delivery** is merging the module's branch into its base branch. It is the person's, from the module's Summary,
  through the path a run's merge into the checkout takes (Local integration where it is needed). With *Deliver when
  accepted* ticked for the module and OA-20's acceptance passed, a chat that leads the module to the end may request
  it; a delivery that needs the person's checkout resolved is left to the person.
- The module's branch takes its base branch in when the base moved, before delivery and when a conflict would
  otherwise wait there; who does it follows from OA-16.
- A module that closes without delivery keeps its branch; nothing deletes work.

## Still open

The words: *delivery* is the team server's word; the frames decide whether a local module says the same or *Merge
the module*.

## Design first

The module form's choice, the module Summary with its branch, what is merged into it and *Deliver*, and the states
(nothing to deliver, the base moved, conflicts, delivered). Frames before any build.

## Done when

- Core and workspace tests: the branch made at the first start, runs that start from it and merge into it without
  the person's working tree, the base taken in, delivery by the person and by the chat under its conditions, a
  delivery the checkout refuses left to the person, a module closed without delivery.
- An independent design verification of the form's choice and the Summary's states. `tools/check.ps1` passes.
- The exact list of what is reused and what is new comes from OA-16's map and is written here before the build.
