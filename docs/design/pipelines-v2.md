# Pipelines · v2

The Pipelines page of a local project, rebuilt as the Agents hub's master–detail page: a filter, a list grouped by what
needs the person, and the chosen pipeline's panel, edited in place. Design: board `Pipelines · v2` `pCzIS` in
`crates/desktop-app/design/orchester.pen` (frames P1–P13, K1–K4). Tasks: `docs/plan/pipelines-v2/`. A team project keeps
the old page (`crates/desktop-app/src/screens/pipelines/`) until a team variant is designed.

## What was built

- The page (`screens/pipeline_hub/`): *All*, *Drafts* and *Archived* with counts; the list grouped *Needs you* and
  *Ready*; the panel with the steps' flow, the pipeline's settings, *Ready?* and the Save bar; bones while it reads,
  *none yet*, empty tabs and a failed read with *Try again*.
- The flow: the steps in the order they run, steps that run together side by side, Deliver last; the order is changed
  in the flow (drag, the card's ⋯, keys), never inside a step.
- A step opens in the same panel, with its strip, *Step*, *Run*, *Gate and access* and Instructions; its *Agent* field
  opens the Agent picker.
- Save makes the next version, Cancel puts the edit back; Versions lists them, a version opens read-only and *Restore*
  puts it in the edit.
- The pipeline's ⋯: *Rename*, *Duplicate*, *Make default for New task*, *Archive*; an archived pipeline has *Restore*.
- The compact *Runs on* picker (K1), which the orchestrator chat uses.
- Links to a pipeline from other pages (the task form, a tool's Access, a profile's *Used in pipelines*, the
  Orchestrator's cards) open the hub's panel in a local project and the old page in a team one.

## Decisions

Flow

- *Steps N* counts the steps, never Deliver; Deliver has its own node on the rail (D-014).
- A failure route reads by what the step is: an agent step *changes requested → 03*, a tool step *fails → 03* (D-014).
- The end marker says *MERGED INTO THE BASE BRANCH*; without Deliver it says what the run ends with (D-014).
- A card that is not ready has no repository-access pill (D-014).
- Nothing moves on hover or focus: the drag handle's and the ⋯'s room is always kept (D-022).
- A change to what a step waits for asks *Allow these changes?* only when it widens: when a step stops waiting for an
  approval-gated or tool step it waited for before (D-021).
- *Duplicate* puts the copy after the original, in series (D-023).
- *Move up* / *Move down* that takes a step out of its parallel group says so (*Move up, out of the group*) (D-023).
- A card with the keys keeps the outline as its only cue (D-023).

Step panel

- Permissions are inline in *Gate and access*, no sub-page; *Open Builder ↗* is under *Agent* for every profile
  (D-024).
- The rename hint and its error stand beside the name field, not under it, so nothing moves (D-028).
- A command the profile no longer allows shows as a field error, and Save refuses it with the reason (D-028).
- A tool step has no Sources row (D-028).
- The step head's ⋯ has the card menu's rows and rules; from the head, *Duplicate* and *Add a step after* open the new
  step, *Remove step* shows the flow (D-030, D-031).

Agent field

- A step chooses an agent (a profile or a tool), not a model; the profile says what it runs on (the person,
  2026-11-10).
- A step that already overrides the model keeps it, and the field shows *model: <id>* (D-003).
- A profile on another CLI drops the override and says so until Save or Cancel; going back to the override's CLI before
  Save puts it back (D-025).
- The override's CLI is read from what the step runs now, not from where the edit started (D-029).

Readiness

- The list row, its group and the panel's chips speak of the version tasks use; *Ready?* speaks of what the panel
  shows, and says so over a published version (*Ready? · this draft*) (D-019).
- When all is set, *Ready?* is one summary row (D-019).
- A failed read shows the store's typed message, not raw driver text (D-011).

Save and versions

- A draft is always saved; only publishing refuses, and Save says what it would refuse (D-027).
- *Ready?*'s *Save →* saves as the bar's Save does (D-026).
- A draft that cannot publish yet says *Save keeps it as a draft · v1 comes once every step is ready* (D-026).
- The disabled Save says why in its tooltip, in every hub (D-026).
- A version viewed read-only names itself, says what *Restore* does before it is pressed, and keeps unsaved changes
  (D-020).

Page

- *Make default for New task* is in the pipeline's ⋯ (D-006).
- An archived pipeline's panel shows its head, the notice and *Restore*, not its steps (D-018).
- Below 1280 logical px the list is 260 wide (D-026).
- The tab counts' bones are the app's shared 8×8 (D-013).
- P9 mentions the log once; the *Drafts* empty line is neutral (D-012).

## Open

- N-010: under ~1000 logical px the panel gets narrower than ~440; it needs a design frame.
- N-011: Tab never reaches the hubs' Save bars, *Add step* or their text fields; a shared fix for every hub.
- D-028: a viewed version's step cards do not open (P2f's read-only step); opening them would be a new task.
- New pipeline and *Duplicate*: the hub has no form of its own, so a local project still creates and copies a pipeline
  on the old page (`NewPipeline`, `CopyPipeline`).
- The Deliver step: the hub only describes it and edits *A run delivers*; its mode, merge method, integrator, recheck,
  and adding or removing it are still edited on the old page, where the Deliver card leads.
- Branched flows: a step that waits for only some earlier steps, or steps that wait on each other, cannot be drawn;
  *Edit steps* leads a local project to the old page's Steps tab to edit them.
- Slug and *who can use it*: only the old page's Settings tab has them; whether the hub keeps them is not decided.

Until the four last are decided and built, the old page is not retired for local projects (PV2-14).
