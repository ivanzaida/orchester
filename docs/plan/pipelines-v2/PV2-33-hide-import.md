# PV2-33 · Pipelines leave Import resources

- **Status:** done (2026-11-11; the new app has no Import resources screen or MCP tool, so the logic layer leaves pipelines out: not offered, never followed as a dependency, kept in the source catalog for remapping; review PASS; merged)
- **Kind:** UI + MCP
- **Depends on:** —
- **Branch:** `claude/pv2-33-hide-import` → module
- **Tokens:** —

## Goal

Quickly (D-033): Import resources stays for other kinds; pipelines are not offered in its UI and its MCP tools neither list nor accept them; the pipeline import code stays.

The map: PV2-26 (`.tmp/orchestrator/pipelines-v2/PV2-26-map.md`); decisions D-032, D-033.
