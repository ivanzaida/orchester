# PV2-18 · The rest of a step in the panel: rename, Permissions, base branch, Builder, unavailable tool

- **Status:** todo (needs a design first)
- **Kind:** design + UI
- **Depends on:** PV2-07
- **Branch:** —
- **Tokens:** —

## Goal

PV2-07 opens a local project's step in the hub's panel (P2 `u2rYEH`), so the old step editor is no longer reachable
for local projects. P2 has no place for what only the old editor had. Before PV2-14 retires the old page, the panel
has all of it, edited through PV2-10's edit (Save/Cancel on the whole pipeline).

## Scope

1. **Step rename** (the old `step-name` field): in the step's head.
2. **Permissions**: the step's commands and network permissions (the old Permissions page). The only way to set them.
3. ***Merge the base branch when it moves*** (agent steps).
4. The profile's ***Open Builder*** action beside *Agent* (or in PV2-08's Agent picker).
5. **The tool-unavailable notice**: a tool step whose tool is unavailable says so, and hides Instructions as the old
   page did.
6. The ***Stopped*** read-only row.

## Done when

- A design for 1–6 in `Pipelines · v2` (a P2 variant), made in Pencil's live document (the person saves and commits it).
- Each works through the edit and is tested; the `pipelines` tests that PV2-07 marked `#[ignore = "PV2-18: …"]` run
  again against the panel (or are replaced by `agents/pipeline_step*` tests) and pass.
- An independent design verification of the new P2 variant.
- `tools/check.ps1` passes.
