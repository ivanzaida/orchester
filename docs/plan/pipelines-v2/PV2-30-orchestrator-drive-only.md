# PV2-30 · Orchestrator chat: pipelines change only in drive mode

- **Status:** done
- **Kind:** core + workspace + UI
- **Depends on:** PV2-29
- **Branch:** —
- **Tokens:** —

## Goal

With *Let the Orchestrator drive* off the Orchestrator does not change pipelines (it says what to do); with drive on it uses save_pipeline. Old Publish{pipeline} entries read as changed.

The map: PV2-26 (`.tmp/orchestrator/pipelines-v2/PV2-26-map.md`); decisions D-032, D-033.
