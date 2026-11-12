# Pipelines · v2

The Pipelines page of a local project, rebuilt as the Agents hub's master–detail page: a filter, a list grouped by what
needs the person, and the chosen pipeline's panel, edited in place. Design: board `Pipelines · v2` `pCzIS` in
`crates/desktop-app/design/orchester.pen` (frames P1–P17, K1–K4). Tasks: `docs/plan/pipelines-v2/`. A team project keeps
the old page (`crates/desktop-app/src/screens/pipelines/`) with one Save that publishes (PV2-32); a team pipeline's save
waits for the team server (D-043, D-047).

## What was built

- The page (`screens/pipeline_hub/`): *All* and *Archived* with counts; the list grouped *Needs you* and *Ready*; the
  panel with the steps' flow, the pipeline's settings, *Ready?* and the Save bar; bones while it reads, *none yet*, empty
  tabs and a failed read with *Try again*.
- Pipelines have no drafts (D-032): a pipeline is stored only as a published version, and only when it can publish. An
  edit that cannot publish stays in the panel, unsaved, with *Save* off and why; leaving it asks first. There is no
  *Drafts* tab, draft chip or draft-over-version, and the migration deleted the drafts that were stored.
- The flow: the steps in the order they run, steps that run together side by side, the Deliver step last; the order is
  changed in the flow (drag, the card's ⋯, keys), never inside a step. A step that waits for only part of the stage
  before it is drawn with its *Waits for* row and is saved (P16, D-048); steps with no order (a cycle, a missing step)
  are refused, and the hub offers *Make it a line* (P16c) — PV2-24, in progress.
- A step opens in the same panel, with its strip, *Step*, *Run*, *Gate and access* and Instructions; its *Agent* field
  opens the Agent picker.
- The Deliver step opens in the same panel (P15): *Mode*, *Merge*, *Conflicts* and *Check*, and *Remove step*; a
  pipeline without one has *Add Deliver step* where its card would be (P15b) — PV2-23.
- New pipeline and *Duplicate* are an unsaved panel in the hub (P14–P14c): a starter, an empty pipeline or a copy (named
  *… copy*), shown as any pipeline; *Create v1* stores v1, and only when it can publish — PV2-22, in progress.
- The pipeline's settings: description, slug (P17, PV2-25), starts, when a step fails, sources for every step and what
  a run delivers. *Who can use it* was dropped.
- Save makes the next version, Cancel puts the edit back; Versions lists them, a version opens read-only and *Restore*
  puts it in the edit.
- The pipeline's ⋯: *Rename*, *Duplicate*, *Make default for New task*, *Archive*; an archived pipeline has *Restore*.
- The compact *Runs on* picker (K1), which the orchestrator chat uses.
- Links to a pipeline from other pages (the task form, a tool's Access, a profile's *Used in pipelines*, the
  Orchestrator's cards) open the hub's panel in a local project and the old page in a team one. A route of the old page
  that a local project is still asked for (Back, a restored tab, an older link, the MCP) opens the hub: a step or its
  page opens the step in the panel, the Deliver step opens there, and the Settings and Versions tabs, a published
  version and the publish review open the pipeline's panel, each under the hub's own route (PV2-14).
- Pipelines leave *Import resources* (PV2-33); the MCP tools save a pipeline only as a published version
  (`save_pipeline`, PV2-29), and the Orchestrator changes pipelines only while it drives (PV2-30).

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
  shows (D-019).
- When all is set, *Ready?* is one summary row (D-019).
- A stored version cannot have an unset profile; a profile that is gone reads *… runs a profile that is no longer
  available* (D-034).
- A failed read shows the store's typed message, not raw driver text (D-011).

Save and versions

- A pipeline has no drafts: Save publishes the next version or is off with every reason, and nothing is stored (D-032).
- A new pipeline exists only once *Create v1* stored it, and only when it can publish (D-032, PV2-22).
- An unsaved edit does not move a pipeline between the list's groups; its row says *Unsaved changes* (D-034).
- *Ready?*'s *Save →* saves as the bar's Save does (D-026).
- The disabled Save says why in its tooltip, in every hub (D-026).
- A version viewed read-only names itself, says what *Restore* does before it is pressed, and keeps unsaved changes
  (D-020).
- A team pipeline's Save, *Create v1*, Duplicate and rename go to the team server at once and are stored here only
  when it accepts them; a refusal or no server leaves the edit as it was (D-042, D-043, D-047).

Page

- *Make default for New task* is in the pipeline's ⋯ (D-006).
- An archived pipeline's panel shows its head, the notice and *Restore*, not its steps (D-018).
- Below 1280 logical px the list is 260 wide (D-026).
- The tab counts' bones are the app's shared 8×8 (D-013).
- P9 mentions the log once (D-012).
- A route the old page had opens the hub in a local project, under the hub's own route; only New pipeline and
  Duplicate (until PV2-22) and *Edit steps* (until PV2-24) still lead to the old page (PV2-14).

## Open

- N-010: under ~1000 logical px the panel gets narrower than ~440; it needs a design frame.
- N-011: Tab never reaches the hubs' Save bars, *Add step* or their text fields; a shared fix for every hub.
- D-028: a viewed version's step cards do not open (P2f's read-only step); opening them would be a new task.
- PV2-22 (New pipeline and *Duplicate*) and PV2-24 (flows the hub cannot draw) are in progress: until they land, a local
  project still reaches the old page's new-pipeline pages (`NewPipeline`, `NewEmptyPipeline`, `CopyPipeline`) and,
  through *Edit steps*, its Steps tab (`Pipeline(_, Steps)`). Each task removes its own route; then a local project
  reaches none of the old page, whose code stays for team projects.
- PV2-00: the person saves and commits the board.
