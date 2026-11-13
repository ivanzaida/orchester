# PV2-12 · The pipeline's ⋯: Rename, Duplicate, Archive; Archived tab

- **Status:** done (2026-11-10; review round 2 PASS, design verification round 2 PASS; merged `824561513`)
- **Kind:** UI
- **Depends on:** PV2-04
- **Branch:** `claude/pv2-12-menu-archive` (`595b9f2f6` … `5d73e6445`)
- **Tokens:** 74.8M across 2 implementation rounds, 2 reviews, 2 design verifications (output 239k)

## Goal

The ⋯ in the pipeline's head (P11 `nrTt0`) and the *Archived* tab (P7 `mFNXi`).

## Design

- The menu: *Rename* (the title becomes editable in place), *Duplicate* (New pipeline filled from this one, as
  today's 04b), *Make default for New task* (on the default pipeline: *Default for New task* with a check, inert), a
  separator, *Archive* with how many tasks use it.
- *Archive* asks first when tasks use it (as `L11` in `Agents · v2`); archiving goes through `local_archive`
  (`Archivable::Resource`).
- *Archived* lists the archived pipelines with *Restore*; when empty it shows P7's state (*Nothing archived* …, no
  action).

## Done when

- What the old library offered a local project is back: Duplicate, Make default, and a visible *default* chip on the
  default pipeline (in its list row's meta and the panel's chips), so a person sees which one New task offers first; the two tests PV2-04
  set `#[ignore]` in `crates/desktop-app/tests/pipelines/default.rs` run again (on the new page) and pass.

- Rename, Duplicate, Archive and Restore work, and the lists and counts follow.
- An independent design verification of P11 and P7.
- `tools/check.ps1` passes; tests for archiving and restoring a pipeline in use.
