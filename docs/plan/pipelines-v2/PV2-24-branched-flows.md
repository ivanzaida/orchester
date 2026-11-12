# PV2-24 · Flows the hub cannot draw

- **Status:** done (batch C; D-048)
- **Kind:** design + UI
- **Depends on:** PV2-14's map (N-012)
- **Branch:** —
- **Tokens:** —

## Goal

A step that waits on only some earlier steps, or a broken flow (a cycle), sends a local project to the old flow editor (*Edit steps*). The hub shows and edits them, or core rules them out and the hub offers a fix (frame P16).

## Done when

A design in `Pipelines · v2` (made in Pencil's live document; the person approves and saves it); built in the hub and
tested; an independent check against the frame; `tools/check.ps1` passes.
