# PV2-12 · The pipeline's ⋯: Rename, Duplicate, Archive; Archived tab

- **Status:** blocked (PV2-04)
- **Kind:** UI
- **Depends on:** PV2-04

## Goal

The ⋯ in the pipeline's head (P11 `nrTt0`) and the *Archived* tab (P7 `mFNXi`).

## Design

- The menu: *Rename* (the title becomes editable in place), *Duplicate* (New pipeline filled from this one, as
  today's 04b), a separator, *Archive* with how many tasks use it.
- *Archive* asks first when tasks use it (as `L11` in `Agents · v2`); archiving goes through `local_archive`
  (`Archivable::Resource`).
- *Archived* lists the archived pipelines with *Restore*; when empty it shows P7's state (*Nothing archived* …, no
  action).

## Done when

- Rename, Duplicate, Archive and Restore work, and the lists and counts follow.
- An independent design verification of P11 and P7.
- `tools/check.ps1` passes; tests for archiving and restoring a pipeline in use.
