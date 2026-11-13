# PV2-01 · Core: move steps and run them in parallel

- **Status:** done (2026-11-10, review PASS; merged `310ce4f39`)
- **Kind:** core (no UI)
- **Depends on:** —
- **Branch:** `claude/pv2-01-step-order` (`01a9c523f`)
- **Tokens:** 4.84M — implementation 2.86M (output 50k), review 1.98M (output 21k)

## Goal

A step's place is changed in the pipeline's flow, not by a *Runs after* field (design P3 `K4B56`, P4 `naN3G`). Core
needs edit operations that say what the person did and rewrite `Step::depends_on` so the flow keeps its meaning.

Today `PipelineEdit` (`crates/core/src/resource/pipeline/edit.rs`) has `add_step`, `duplicate_step`, `remove_step` and
`set_dependency`; nothing moves a step or makes two steps parallel.

## Scope

On `PipelineEdit`, each returning whether it changed anything (or a typed error), keeping the rules of `Pipeline::new`
(known dependencies, no cycles, upstream failure routes):

- **Move up / Move down:** the step trades places with the step (or parallel group) before or after it in the flow.
- **Move to:** the step lands between two places of the flow (the drop line of P3): it runs after the steps before
  that place, and the steps after it run after it.
- **Run in parallel with:** the step joins a neighbour (or a neighbour's parallel group): the same `depends_on`, and
  the steps that waited for the neighbour also wait for it.
- **Run on its own:** the step leaves its parallel group and runs right after it.
- **Which moves are possible** for a step, for the ⋯ menu: only the neighbours it can join, Move up/down only when
  there is room.

A failure route (`on_failure.return_to`) must stay upstream; a move that would break it is refused with the reason
(the UI shows it), not changed silently.

## Out of scope

The UI (PV2-06). The MCP tools: `save_pipeline_draft` takes whole steps, so automation is not affected.

## Done when

- The operations exist with doc comments, typed errors and no `unwrap`.
- Tests in `crates/core/tests/` cover each operation on a linear pipeline and on one with a parallel group (the
  `Feature delivery` shape of P1: 01 → 02 (gate) → 03 → {04 ∥ 05} → 06), a refused move that would put a failure
  route downstream, and that the steps around a moved step keep their order.
- `powershell -File tools/check.ps1` passes; the filtered tests that were run are named in the report.
