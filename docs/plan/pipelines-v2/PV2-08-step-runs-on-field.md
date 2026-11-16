# PV2-08 · The step's *Runs on* field and picker

- **Status:** blocked (PV2-02, PV2-03, PV2-07)
- **Kind:** UI
- **Depends on:** PV2-02 (the decision and its core change), PV2-03, PV2-07

## Goal

The step's *Runs on* field (P2 `u2rYEH`) shows *CLI · connection · model · effort* and opens the compact picker under
it, left-aligned (P10 `HjgZ8`). A choice applies to the step at once and is saved with the pipeline.

## Design

- The picker as in PV2-03; connections of other CLIs than the step's profile's are at half, their tooltip says why.
- What a step can override follows PV2-02's decision (if a step cannot change its connection, the rail shows only the
  profile's).

## Done when

- The field reads and writes the step's overrides; *Cancel* puts them back.
- An independent design verification of P10.
- `tools/check.ps1` passes; tests of what is written for a model, an effort and a connection choice.
