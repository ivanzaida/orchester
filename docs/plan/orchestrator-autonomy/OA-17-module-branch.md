# OA-17 · A local module's own branch and its delivery, as a team module has

- **Status:** built (2026-11-14); design verification pending
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

## The form and the Summary, as built (2026-11-14)

Frames: `M01`, `M02`, `Parts · Delivery` and `Parts · Module form` on the board `Orchestrator autonomy · frames to
approve`.

- **Edit module and New module** have the group *Delivery* under the chips (`screens/modules/form_delivery.rs`):
  *Its own branch*, and only while it is on, *Acceptance pipeline* (none, or a published pipeline of the project)
  and *Deliver when accepted*, which stays off and faded, with why, while no acceptance pipeline is chosen. Saved
  with the form (`change_own_in`), on this computer.
- **The Summary** of a module with a branch of its own starts with its delivery
  (`screens/modules/delivery.rs`, read as `orchester_workspace::modules::delivery::DeliveryView` with the page):
  the branch and where it goes, how it stands (nothing merged yet; N of M tasks merged, with the runs at work;
  every task done with or without an acceptance; accepting; accepted; acceptances that did not pass twice;
  delivered, with the commit and when) and **Deliver**, which asks in a dialog before it merges and says under
  the row why a delivery was refused (a conflict, the person's changes in the way, runs at work). Its properties
  name *Branch*, *Acceptance* and *Deliver*. The module's acceptance tasks stand in their own group, tagged
  *Acceptance*, and are not counted among its tasks.
- **Differs from the frames:** the row is the strip a team module's delivery line is (`team::strip`), so the state
  is said in the title's words and not in a pill, and a warning row has no stroke; commits ahead of and behind the
  base branch are not counted (the row reads from the project's records alone, no git); *The base moved*,
  *Delivering* as a state of its own and *The checkout refused* / *Conflicts* as rows are the dialog's loader and
  the refusal under the row; turning the branch off while it holds undelivered work is not refused.

Tests: `orchester-workspace --test module_delivery module_delivery_view`; `orchester-app --test modules
modules_summary_of_a_module_with_its_own_branch`, `modules_edit_module_gives`.

## After the design verification (2026-11-14)

- The row's title is drawn as the frame draws it: the branch and its base in Geist Mono with an arrow between
  them, and the state in a badge (`team::strip::Strip::title_parts`).
- *Accepting* shows while the acceptance's own run is at work; an acceptance that did not pass says so, in the
  warning tone, and that one more runs (it read *has not run yet*).
- The module's progress, its header's facts and its burndown leave its acceptance tasks out, as its task list
  does (`ModulePage::read`). The Modules list and the delivery view still count them.
- *New task in module* is the quiet button while **Deliver** is the page's primary action.
- Turning *Its own branch* off is refused while the branch holds merged work that was not delivered
  (`modules.ownBranch.holdsWork`).
- *Deliver* among the properties reads *By you* for a module without an acceptance pipeline.
- A task's *Base* property names its module's own branch as what its next run starts from.

Still as built and not as drawn: commits ahead of and behind the base are not counted; *The base moved*, *The
checkout refused* and *Conflicts* are the refusal under the row, not rows of their own.
