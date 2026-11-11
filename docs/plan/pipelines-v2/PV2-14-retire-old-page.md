# PV2-14 · Retire the old Pipelines page of local projects

- **Status:** blocked (PV2-05 … PV2-13)
- **Kind:** UI
- **Depends on:** PV2-05 to PV2-13, PV2-17 to PV2-21
- **Branch:** —
- **Tokens:** —

## Goal

When the new page covers everything a local project's pipelines need, remove what only the old page used
(`crates/desktop-app/src/screens/pipelines/`: the library, the Steps, Settings and Versions tabs, the separate step
pages), with its routes and strings, keeping what team projects still use.

## Done when

- No dead code, routes or locale keys are left; team projects' pipelines are unchanged.
- `docs/design/` has a short `pipelines-v2.md` with this module's decisions, and the module's tasks are moved to *Done*
  in `docs/plan.md` with their branches.
- `tools/check.ps1` passes; the app's pipelines tests are updated and named in the report.
