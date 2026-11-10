# PV2-08 · The step's *Agent* field and Agent picker

- **Status:** in progress
- **Kind:** UI
- **Depends on:** PV2-07
- **Branch:** `claude/pv2-08-agent-field`
- **Tokens:** —

## Goal

A step chooses an **agent** — an agent profile or a tool — not a model (the person, 2026-11-10): the profile says
what it runs on (CLI, connection, model, effort). The step's *Executor* and model fields become one *Agent* field
(P2 `u2rYEH`) that opens the Agent picker under it, left-aligned (P10 `HjgZ8`; the picker alone is K4 `CwLfL`,
component `Agent picker` `Zn8BM`).

Today the step's Executor menu is `crates/desktop-app/src/screens/pipelines/executor_menu.rs` (profiles, a line, then
tools; a profile or tool that cannot run is faded with why) and the step's model override is a separate field.

## Design

- The field: the profile's mark, its name in 500, then what it runs on in `text_muted` (*Claude Code · Work ·
  claude-opus-5-5*), a chevron; `accent` border while open.
- The picker, 320 wide: Search (*Search agents and tools…*, focused on open, typing filters); a list of a fixed height
  that scrolls with a scrollbar, grouped *Agent profiles* and *Tools*; each row the mark, the name and what it runs on
  (a tool: its command and that its exit code decides the step); the chosen one with a check; one that is not ready
  says why in `warning` (*Choose a Claude Code connection*) and can still be chosen; the foot *+ New agent profile*
  (opens the Agents hub's create form) and *Agents ↗*.
- Keyboard: ↑ ↓, Enter chooses, Esc closes.
- The step's own model field is gone from the panel. A step that already overrides the model keeps working as stored
  (the run uses it); the panel shows it after the profile's line as *model: <id>* and the picker's chosen row says
  the same, so nothing stored is hidden. Do not remove the override from core.

## Done when

- Choosing an agent sets the step's executor at once (saved with the pipeline by PV2-10's Save; Cancel puts it back).
- An independent design verification of P10 and K4.
- `tools/check.ps1` passes; tests for filtering, grouping and a not-ready profile chosen.
