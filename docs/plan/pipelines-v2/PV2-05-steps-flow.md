# PV2-05 · Steps drawn as the flow they run in

- **Status:** blocked (PV2-04)
- **Kind:** UI
- **Depends on:** PV2-04
- **Branch:** —
- **Tokens:** —

## Goal

The panel's *Steps* section draws the pipeline as it runs (P1 `s7LMyQ`) instead of a table of rows.

## Design (P1)

- A vertical line with a node per step (its number), from *ON TASK START* to *MERGED INTO main* (the end says what the
  pipeline delivers).
- A step card: the name, profile · model (or the tool, or the Deliver summary), a pill for repository access
  (`read-only`, `can write`) and `›` to open the step.
- An approval gate (`approval_gate`) is drawn on the line after its step: an amber hand node, *Waits for your
  approval* and what is approved.
- Steps with the same satisfied dependencies sit side by side in one frame *RUN IN PARALLEL · both must pass*; more
  than two stack inside it.
- A failure route shows on the card: `↩ changes requested → 03`.
- *Add step* in the section's head adds at the end; hovering between two steps shows *+ Add a step here*.
- A hovered card shows its drag handle and ⋯ (their actions are PV2-06).

## Done when

- The flow is derived from `depends_on`, `approval_gate` and `on_failure` (no layout is stored), for linear, parallel
  and gated pipelines; a pipeline whose dependencies cannot be drawn as one line says so instead of guessing.
- Clicking a card opens the step (PV2-07's route; until then it may open today's step page).
- An independent design verification of P1's flow (and of P8's amber card once PV2-09 lands).
- `tools/check.ps1` passes; tests of the flow's layout from the dependencies.
