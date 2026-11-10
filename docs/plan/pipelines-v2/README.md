# Pipelines · v2 — task tracker

The Pipelines page rebuilt as the Agents hub's master–detail page (`Agents · v2` `acFkz`, Section L): a filter, a list
grouped by what needs the person, and a panel with the chosen pipeline edited in place. Steps are drawn as the flow
they run in, their order is changed in that flow (not inside a step), and a step opens in the same panel.

- **Design:** board `Pipelines · v2` `pCzIS` in `crates/desktop-app/design/orchester.pen` (frames P1–P13, K1–K3 and
  the component `Runs on picker` `ZcGGy`). Every node id below is from that board.
- **Scope:** local projects. A team project's pipelines keep today's page until a team variant is designed.
- **How to take a task:** as in `docs/plan.md` (*How to take a task*). Read `AGENTS.md`; UI tasks need `lurq-ui`,
  `orchester-ui` and an independent design verification (`AGENTS.md` rule 15) against the frames the task names.
  Do not widen a task's scope; report what you find outside it.
- **Updating this file:** set a task's status here and in its own file in the same change; when it is done, add the
  branch or commit.

Status: `todo` · `blocked` (waits for a task or the person) · `needs decision` (waits for the person; do not start) ·
`in progress` · `review` · `done`.

## Tasks

| ID | Task | Kind | Depends on | Status |
| --- | --- | --- | --- | --- |
| [PV2-00](PV2-00-design-saved.md) | Save and commit the `Pipelines · v2` design | person | — | todo |
| [PV2-01](PV2-01-core-step-order.md) | Core: move steps and run them in parallel | core | — | todo |
| [PV2-02](PV2-02-step-runs-on.md) | Core: a step's connection and effort (*Runs on*) | core | — | needs decision |
| [PV2-03](PV2-03-runs-on-picker.md) | The compact *Runs on* picker as a shared component | UI | PV2-00 | blocked |
| [PV2-04](PV2-04-page-shell.md) | The page: filter, grouped list, panel, Save bar | UI | PV2-00 | blocked |
| [PV2-05](PV2-05-steps-flow.md) | Steps drawn as the flow they run in | UI | PV2-04 | blocked |
| [PV2-06](PV2-06-reorder-in-flow.md) | Reorder steps in the flow: drag, ⋯ menu, keys | UI | PV2-01, PV2-05 | blocked |
| [PV2-07](PV2-07-step-panel.md) | A step in the panel: strip, groups, Instructions | UI | PV2-04, PV2-05 | blocked |
| [PV2-08](PV2-08-step-runs-on-field.md) | The step's *Runs on* field and picker | UI | PV2-02, PV2-03, PV2-07 | blocked |
| [PV2-09](PV2-09-settings-and-ready.md) | Pipeline settings, Ready? and Needs you | UI | PV2-04 | blocked |
| [PV2-10](PV2-10-save-and-cancel.md) | Edit in place: Save makes vN, Cancel puts it back | UI + workspace | PV2-04 | blocked |
| [PV2-11](PV2-11-versions.md) | Versions list, a version read-only, Restore | UI | PV2-10 | blocked |
| [PV2-12](PV2-12-menu-archive.md) | The pipeline's ⋯: Rename, Duplicate, Archive; Archived tab | UI | PV2-04 | blocked |
| [PV2-13](PV2-13-page-states.md) | Loading, none yet, empty tabs, read failure | UI | PV2-04 | blocked |
| [PV2-14](PV2-14-retire-old-page.md) | Retire the old Pipelines page of local projects | UI | PV2-05 … PV2-13 | blocked |
| [PV2-15](PV2-15-orchestrator-picker.md) | The orchestrator chat adopts the compact picker | UI | PV2-03 | needs decision |

## Order

1. PV2-00 first: no UI task starts before the design is committed.
2. PV2-01 and PV2-04 can run in parallel; PV2-02 waits for the person's decision.
3. After PV2-04: PV2-05, PV2-09, PV2-10, PV2-12 and PV2-13 in any order; PV2-07 after PV2-05.
4. PV2-06 after PV2-01 and PV2-05; PV2-11 after PV2-10; PV2-08 after PV2-02, PV2-03 and PV2-07.
5. PV2-14 last.

## Frames

| Frame | Node | Shows |
| --- | --- | --- |
| P1 · pipeline | `s7LMyQ` | List, chosen pipeline, steps flow, settings, Ready?, Save bar, hovered card |
| P2 · step | `u2rYEH` | A step in the panel, step strip, Step / Run / Gate and access |
| P3 · reorder by drag | `K4B56` | A card dragged, its place faded, the drop line and what it means |
| P4 · step menu | `naN3G` | A card's ⋯: Move up/down, Run in parallel with …, Add after, Duplicate, Remove |
| P5 · loading | `TF9O3` | Bones |
| P6 · none yet | `V2azC` | No pipelines |
| P7 · nothing archived | `mFNXi` | Archived tab empty |
| P8 · draft, a step not ready | `vujkk` | Needs you, amber card, Ready? with the fix |
| P9 · couldn't read | `H4fLj` | Read failure |
| P10 · step · model | `HjgZ8` | *Runs on* field with the compact picker open |
| P11 · pipeline menu | `nrTt0` | ⋯ in the pipeline's head |
| P12 · versions | `nSr5q` | Versions list open |
| P13 · long pipeline | `KRQ5J` | The step strip scrolled |
| K1 · picker | `lCApz` | Compact *Runs on* picker (component `ZcGGy`) |
| K2 · picker searching | `rjPcV` | The picker filtering, same height |
| K3 · picker, two columns | `Aj9AU` | Rejected alternative; kept for reference only |
