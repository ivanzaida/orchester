# PV2-09 · Pipeline settings, Ready? and Needs you

- **Status:** in progress — scope: *Ready?*, *Needs you*, the head chips, a seed scenario with a draft (settings rows are PV2-10's)
- **Kind:** UI
- **Depends on:** PV2-04
- **Branch:** `claude/pv2-09-ready`
- **Tokens:** —

## Goal

Under the flow the panel holds the pipeline's settings and *Ready?* (P1 `s7LMyQ`); a pipeline that is not ready is
under *Needs you*, with the fix where it belongs (P8 `vujkk`).

## Design

- *Pipeline* rows: Description, Starts, When a step fails, Sources for every step (path pills and Add). The other
  settings of today's Settings tab (`settings.rs`, `delivers.rs`, `deliver.rs`) move here as rows in the same style;
  list them in the report.
- *Ready?* with *all set* (`success`) or *N missing* (`warning`): one row per problem with what to do (*Choose →*);
  all set shows one summary row.
- P8: the list row in `warning`, the chips *draft* and *1 step not ready*, the step's card and its node amber with
  *Choose profile* on the card.

## Done when

- Readiness comes from the workspace's readiness read (as the hub's), not computed again in the UI.
- An independent design verification of P1's lower half (scrolled) and of P8.
- `tools/check.ps1` passes; tests of the Needs you grouping and the Ready? rows.
