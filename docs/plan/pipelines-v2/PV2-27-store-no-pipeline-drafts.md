# PV2-27 · Store: a pipeline is created published; the migration deletes pipeline drafts

- **Status:** todo
- **Kind:** store + core
- **Depends on:** PV2-26
- **Branch:** —
- **Tokens:** —

## Goal

Pipelines join the created-published kinds; one entry publishes a pipeline body edited from vN (with rename) in one transaction; the migration deletes every pipeline draft and every pipeline with no version, with what names it; `RemoveMode::Reset` goes; a test helper replaces create_resource + publish_draft::<Pipeline>. The guard refusing a pipeline draft lands with PV2-28.

The map: PV2-26 (`.tmp/orchestrator/pipelines-v2/PV2-26-map.md`); decisions D-032, D-033.
