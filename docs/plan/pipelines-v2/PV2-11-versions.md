# PV2-11 · Versions list, a version read-only, Restore

- **Status:** done (2026-11-11; review 2 PASS, design verification 2 no blocking/major against round-1 Pencil captures; merged with PV2-07 in `8c6213b96`, into the module `a3f14fb18`; follow-ups in PV2-17 items 20–23)
- **Kind:** UI
- **Depends on:** PV2-10
- **Branch:** `claude/pv2-11-versions` (`0070c38c5` … `8c6213b96`)
- **Tokens:** 97.4M (out 261k) across 2 implementation rounds and a merge, 2 reviews, 2 design verifications

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
