# PV2-11 · Versions list, a version read-only, Restore

- **Status:** blocked (PV2-10)
- **Kind:** UI
- **Depends on:** PV2-10

## Goal

*Versions* in the panel's head opens a list (P12 `nSr5q`); a row opens that version read-only in the panel with
*Restore* (today's `versions.rs`, `viewing.rs` and `restore.rs`).

## Design (P12)

- A popover under *Versions*: the heading *Versions* · the pipeline's name; a row per published version, the current
  one with *current* and a check, each with *Published Oct 27 · used by 13 tasks*, *2 runs still on it* or *no runs on
  it*; a hovered row shows *View ›*; the foot *Runs keep the version they started on.*

## Done when

- The list, the read-only view and *Restore* work with PV2-10's edit (a restore puts the version into the edit, which
  *Save* keeps).
- An independent design verification of P12.
- `tools/check.ps1` passes; tests for the list's counts and for Restore.
