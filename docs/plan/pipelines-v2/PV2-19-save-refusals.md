# PV2-19 · A draft is always saved; Save says what publishing would refuse, also while editing

- **Status:** canceled (D-032)
- **Kind:** UI + workspace
- **Depends on:** PV2-17
- **Branch:** —
- **Tokens:** —

## Goal

PV2-17a's second check (2026-11-11) found that saving and publishing still disagree with what the bar promises.

## Scope

1. **A draft with a step the person cannot publish yet is still saved.** `pipelines-agents`, *Agents tour* v1, step 08
   on an archived profile: change *Description*, Save → the bar promised *Save keeps it as a draft · v2 comes once
   every step is ready*, the press fails with *Couldn't save Agents tour: No published agent profile is named
   01a1c852-…* (a raw id). `save_named_in` (`workspace/src/pipelines/write.rs:67`) runs the publish check on the draft.
   A draft is stored whatever its steps' readiness; only publishing refuses. No raw ids in any message.
2. **One check.** `publish_in` (`write.rs:93-95`) still loops `check_step` itself; it calls `refused_step`, the one
   check the page reads, and `refused_step`'s doc says what is true.
3. **While editing.** `publish_blocked` returns `None` while `line.unsaved` (`bar.rs:58`): a Write step switched to a
   read-only profile keeps Write access, the bar says *Save keeps it as a draft*, and publishing refuses
   `stepCannotWrite` only after the press. The bar reads the edit as it stands.
4. App-level tests for the `Unpublishable::Refused` bar text and `version_limit`.

## Done when

Each item works and is tested in the app (`agents pipeline_save*`) and the workspace; an independent check;
`tools/check.ps1` passes.
