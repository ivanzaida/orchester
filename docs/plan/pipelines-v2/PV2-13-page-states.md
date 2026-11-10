# PV2-13 · Loading, none yet, empty tabs, read failure

- **Status:** blocked (PV2-04)
- **Kind:** UI
- **Depends on:** PV2-04

## Design

- **Loading (P5 `TF9O3`):** the title, *New pipeline* and the tab names stay; the counts, list rows, panel head, chips
  and step cards (with their rail nodes) are bones, lower ones fading. Shown after 300 ms, a gentle pulse (still with
  reduced motion), the accessible label `pipelines.reading`. Choosing another pipeline later shows bones in the panel
  only.
- **None yet (P6 `V2azC`):** one empty state across the body: *No pipelines yet*, one line, *New pipeline*.
- **Empty tab (P7 `mFNXi`):** *Nothing archived*; *Drafts* the same way with *No drafts*.
- **Read failure (P9 `H4fLj`):** *Couldn't read the pipelines*, *Nothing was changed …*, the reason as the store gave
  it (mono) and *Try again*; the counts stay bones; *New pipeline* is unavailable until a read succeeds. Nothing
  pretends to be empty.

## Done when

- Each state can be reached (a seed scenario or DevTools) and is verified independently against its frame.
- `tools/check.ps1` passes; the i18n keys are in every shipped locale.
