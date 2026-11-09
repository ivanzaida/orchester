# PV2-15 · The orchestrator chat adopts the compact picker

- **Status:** blocked (PV2-03, PV2-16)
- **Kind:** UI
- **Depends on:** PV2-03, PV2-16
- **Branch:** —
- **Tokens:** —

## Goal

The person preferred the compact picker (2026-11-10): the orchestrator chat's *Runs on* chip opens the shared compact
picker of PV2-03 instead of today's 640 × 320 panel in `crates/desktop-app/src/screens/orchestrator/runs_on/`.

## Design

`Orchestrator · redesign` `hujxu` as PV2-16 leaves it: `05 · Account and model` `v6mWp8`, `05b · On DeepSeek`
`AZp3I` and `Runs-on picker · states` `t7cdi` (*Codex at its limit*, *Searching*, *Many models*, *No connections*),
all drawn with the compact picker.

## Scope

- The chip opens the shared component over it (never under it), as today; choosing applies at once through today's
  path (`picking.rs`).
- What only the chat has (an API provider's typed model and effort, where its data goes, the account-switch note)
  appears as PV2-16 draws it.
- Remove the old panel's code that nothing uses any more.

## Done when

- Every state of PV2-16's frames works in a QA copy and is verified independently against its frame.
- `tools/check.ps1` passes; the orchestrator's runs-on tests updated and named in the report.
