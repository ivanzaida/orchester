# PV2-16 · Design: the orchestrator's picker states with the compact picker

- **Status:** done (2026-11-10; in Pencil's open document, not saved — PV2-00)
- **Kind:** design (Pencil only; the person commits the `.pen`)
- **Depends on:** —
- **Branch:** —
- **Tokens:** 2.51M (input 54, cache write 106k, cache read 2.37M, output 27k)

## Goal

Redraw the orchestrator chat's *Runs on* picker on `Orchestrator · redesign` `hujxu` with the compact picker
(component `Runs on picker` `ZcGGy` on `Pipelines · v2`), so PV2-15 has frames to build and verify against.

## Frames to change

- `05 · Account and model` `v6mWp8`: the big panel `r2owi1` becomes an instance of `ZcGGy` over the chip, left-aligned
  with it: every connection choosable (none at half), *Work · Claude Code · Max plan · 5h 42%*, the Claude models with
  `claude-opus-5-5` checked, Effort *Medium*.
- `05b · On DeepSeek` `AZp3I` (`x6h0l`): DeepSeek chosen on the rail; *1.2M today · no plan limit*; *Search or type a
  model…* (a typed model is allowed); Effort typed, as the provider lists none (an input row *Type an effort…*, empty
  is Default); where the data goes (*Prompts, repository content and command output go to api.deepseek.com …*) kept as
  a compact `warning` row.
- `Runs-on picker · states` `t7cdi`: *Codex at its limit* (`OHrKD`), *Searching* (`P04aC`), *Many models* (`YYXey`)
  with the compact picker; *No connections* (`cjnbn`) stays the 360-wide foot; the captions and notes (`YLKHj`,
  `UrtUU` … `c0GDV`) rewritten for one panel 300 wide with the rail on top.
- The account-switch note (*Another account starts a new session: earlier messages aren't remembered*) moves to the
  tooltip of another account's connection on the rail and to the chip after a switch; draw that tooltip once.

## Done when

- Every frame above is redrawn; PNG exports of each are in `.tmp/orchestrator/pipelines-v2/shots/`; node ids reported.
- The `.pen` is not saved or committed by the design step (PV2-00 covers it).
