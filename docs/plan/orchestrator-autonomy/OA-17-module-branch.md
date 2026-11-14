# OA-17 · A local module's own branch and its delivery, as a team module has

- **Status:** done but for what waits for frames (2026-11-14)
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

## The same in both kinds (2026-11-13)

A team module already has what this task gives a local one: a branch of its own and a delivery
(`docs/design/local-and-team-parity.md`). So this task does not invent a second model: the choice, the branch's
making (OA-33), the merge of a run into it (OA-27 for team runs), acceptance (OA-20) and the delivery's rule (OA-32)
are one design with two owners of the data, and the Summary's states are the team module's states.

## As built (2026-11-14)

Reused from OA-16's map: `BaseChoice::Branch`, `RunBaseSource::Module`, the landing beside the checkout, `merge_run`
and the merge queue as they are, `resolve_conflicts`, core's `merge_module` with `merge_module_in` and
`module_merged_in`, and the clean-up of run branches (a base branch is never removed, and a module's branch is one).

New:

- **The choice.** A module has an own branch or none (`orchester_core::module::delivery::OwnBranch`,
  `modules.own_branch`, schema 178), kept on this computer as its base branch is. Its name is `module/<slug>` of the
  module's name, with `-2`, `-3`, … while a module or a branch of the repository has it (`own_branch_name`); renaming
  the module does not rename it. `create_module` and `update_module` take `own_branch`, `acceptance_pipeline` and
  `deliver_when_accepted`; `get_module` gives them with what the module waits for.
- **The runs.** A module's tasks that set no base branch of their own take the module's own branch
  (`Module::run_base`, `ProjectDb::next_run_base`): they start from it and merge into it, beside the person's
  checkout. The module's base branch becomes where it is delivered.
- **The branch is made by the first start** (OA-33, a local module's half): the runner makes it at the tip of the
  module's base branch, or at the checkout's `HEAD` when the module set none, when the run starts
  (`runner/src/execution/coordinator/prepare.rs`); the preflight makes nothing.
- **Delivery** (`modules::delivery::deliver_in`, the tool `deliver_module`): the module's branch at its tip is merged
  into its base branch with a merge commit, under the merge hold, and the delivery is kept (`module_deliveries`).
  Refused while a run of the module has not ended, when the branch holds nothing its base lacks, and as the merge
  refuses: a conflict and a checkout that stands in the way leave everything as it was, for the person. A merge an
  earlier call made and did not record is found, never made twice.
- **A led module is not done until it is delivered**: its standing reads *to deliver* with what it waits for
  (`TargetStanding::ToDeliver`), its conversation is told (OA-20), and its watch does not end before.
- Nothing deletes a module's branch: taking the choice away leaves it in the repository, and closing a module
  touches no branch.

**Not built:**

- The module form's choice and the Summary's states: they wait for the frames of `frames.md` D. The choice is the
  tools' until then.
- Bringing the base branch into the module's branch when the base moved: a delivery that conflicts is refused and
  left to the person; nothing notices that a module's base moved.
- A conflict of a run's merge into the module's branch is still shown on Local integration in the window, which
  answers *Stale* there; the chat's path (`continue_run` `resolve_conflicts`) works by branch name and is not
  affected.

Tests: `orchester-core --test module_delivery delivery`, `orchester-workspace --test module_delivery
module_delivery` (a real repository), `orchester-runner --test execution run_base_resolution` (the first run makes
the branch).
