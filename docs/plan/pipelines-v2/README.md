# Pipelines · v2 — task tracker

The Pipelines page rebuilt as the Agents hub's master–detail page (`Agents · v2` `acFkz`, Section L): a filter, a list
grouped by what needs the person, and a panel with the chosen pipeline edited in place. Steps are drawn as the flow
they run in, their order is changed in that flow (not inside a step), and a step opens in the same panel. The
orchestrator chat's *Runs on* picker becomes the compact one designed here.

- **Design:** board `Pipelines · v2` `pCzIS` in `crates/desktop-app/design/orchester.pen` (frames P1–P13, K1–K4, the
  components `Runs on picker` `ZcGGy` and `Agent picker` `Zn8BM`). Every node id below is from that board. Until the
  person commits the `.pen` (PV2-00), Pencil's open document is the source: read it through the `pencil` MCP with the
  main checkout's path.
- **Branch:** `module/pipelines-v2`, worked outside Orchester (`.claude/skills/work-outside-orchester`); tasks merge
  into it, the person merges it into `master`.
- **Scope:** local projects. A team project's pipelines keep today's page until a team variant is designed.
- **How to take a task:** as in `docs/plan.md` (*How to take a task*). UI tasks need `lurq-ui`, `orchester-ui` and an
  independent design verification (`AGENTS.md` rule 15) against the frames the task names.
- **Updating this file:** set a task's status, branch and tokens here and in its own file in the same change.

Status: `todo` · `blocked` (waits for a task) · `needs the person` · `in progress` · `review` · `done` · `canceled`.
Tokens: what the task's children used in total (implementation, review, verification, fix rounds), as the
orchestration tool reported it; `—` until known.

## Person's decisions

- 2026-11-10: a step chooses an **agent** (profile or tool), not a model; the profile says what it runs on. PV2-02 is
  canceled; PV2-08 is the Agent picker (K4).
- 2026-11-10: the orchestrator chat moves to the compact *Runs on* picker (K1) — PV2-15, after its design (PV2-16).
- 2026-11-10: K1 is the picker; the model list is a fixed four rows high and scrolls; no *profile* tag (the check is
  enough).

## Tasks

| ID | Task | Kind | Depends on | Status | Branch | Tokens |
| --- | --- | --- | --- | --- | --- | --- |
| [PV2-00](PV2-00-design-saved.md) | Save and commit the `Pipelines · v2` design | person | — | needs the person | — | — |
| [PV2-01](PV2-01-core-step-order.md) | Core: move steps and run them in parallel | core | — | done | `claude/pv2-01-step-order` → `310ce4f39` | 4.84M (out 72k) |
| [PV2-02](PV2-02-step-runs-on.md) | Core: a step's connection and effort | core | — | canceled | — | — |
| [PV2-03](PV2-03-runs-on-picker.md) | The compact *Runs on* picker as a shared component | UI | — | done | `claude/pv2-03-runs-on-picker` → `c37c1caad` | 51.4M (out 209k) |
| [PV2-04](PV2-04-page-shell.md) | The page: filter, grouped list, panel, Save bar | UI | — | done | `claude/pv2-04-page-shell` → `27cde7e2c` | 97.7M (out 270k) |
| [PV2-05](PV2-05-steps-flow.md) | Steps drawn as the flow they run in | UI | PV2-04 | done | `claude/pv2-05-steps-flow` → `1b2aa8df7` | 77.6M (out 258k) |
| [PV2-06](PV2-06-reorder-in-flow.md) | Reorder steps in the flow: drag, ⋯ menu, keys | UI | PV2-01, PV2-05 | in progress | `claude/pv2-06-reorder` | — |
| [PV2-07](PV2-07-step-panel.md) | A step in the panel: strip, groups, Instructions | UI | PV2-04, PV2-05 | done | `claude/pv2-07-step-panel` → `c3419c6ec` | 148.3M (out 349k) |
| [PV2-08](PV2-08-step-agent-field.md) | The step's *Agent* field and Agent picker | UI | PV2-07 | in progress | `claude/pv2-08-agent-field` | — |
| [PV2-09](PV2-09-settings-and-ready.md) | Pipeline settings, Ready? and Needs you | UI | PV2-04 | done | `claude/pv2-09-ready` → `75aa67e32` | 61.7M (out 231k) |
| [PV2-10](PV2-10-save-and-cancel.md) | Edit in place: Save makes vN, Cancel puts it back | UI + workspace | PV2-04 | done | `claude/pv2-10-save-and-cancel` → `cca2e89e9` | 113.0M (out 342k) |
| [PV2-11](PV2-11-versions.md) | Versions list, a version read-only, Restore | UI | PV2-10 | in progress | `claude/pv2-11-versions` | — |
| [PV2-12](PV2-12-menu-archive.md) | The pipeline's ⋯: Rename, Duplicate, Archive; Archived tab | UI | PV2-04 | done | `claude/pv2-12-menu-archive` → `824561513` | 74.8M (out 239k) |
| [PV2-13](PV2-13-page-states.md) | Loading, none yet, empty tabs, read failure | UI | PV2-04 | done | `claude/pv2-13-page-states` → `ba2db9eea` | 42.0M (out 136k) |
| [PV2-14](PV2-14-retire-old-page.md) | Retire the old Pipelines page of local projects | UI | PV2-05 … PV2-13 | blocked | — | — |
| [PV2-15](PV2-15-orchestrator-picker.md) | The orchestrator chat adopts the compact picker | UI | PV2-03, PV2-16 | done | `claude/pv2-15-orchestrator-picker` → `e0c34aa8a` | 108.2M (out 318k) |
| [PV2-16](PV2-16-design-orchestrator-picker.md) | Design: the orchestrator's picker states with the compact picker | design | — | done | Pencil (unsaved) | 2.51M (out 27k) |
| [PV2-17](PV2-17-readiness-polish.md) | Readiness and save polish: one story in chips, Ready?, cards and the bar | UI | PV2-09, PV2-10 | todo | — | — |
| [PV2-18](PV2-18-step-panel-rest.md) | The rest of a step in the panel: rename, Permissions, base branch, Builder, unavailable tool | design + UI | PV2-07 | todo (design first) | — | — |

## Order

1. PV2-01, PV2-03, PV2-04 and PV2-16 can start at once.
2. After PV2-04: PV2-05, PV2-09, PV2-10, PV2-12 and PV2-13; PV2-07 after PV2-05.
3. PV2-06 after PV2-01 and PV2-05; PV2-11 after PV2-10; PV2-08 after PV2-07; PV2-15 after PV2-03 and PV2-16.
4. PV2-14 last.

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
| P10 · step · agent | `HjgZ8` | The step's *Agent* field with the Agent picker open |
| P11 · pipeline menu | `nrTt0` | ⋯ in the pipeline's head |
| P12 · versions | `nSr5q` | Versions list open |
| P13 · long pipeline | `KRQ5J` | The step strip scrolled |
| K1 · picker | `lCApz` | Compact *Runs on* picker (component `ZcGGy`) |
| K2 · picker searching | `rjPcV` | The picker filtering, same height |
| K3 · picker, two columns | `Aj9AU` | Rejected alternative; kept for reference only |
| K4 · agent picker | `CwLfL` | The Agent picker (component `Zn8BM`) under its field |
