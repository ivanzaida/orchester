# PV2-04 · The page: filter, grouped list, panel, Save bar

- **Status:** done (2026-11-10; review round 3 PASS, design verification round 2 PASS; merged `27cde7e2c`)
- **Kind:** UI
- **Depends on:** — (the design is read from Pencil's open document until PV2-00)
- **Branch:** `claude/pv2-04-page-shell` (`be3c181f7` … `d73317098`)
- **Tokens:** 97.7M — implementation 63.0M + 13.2M + 1.0M, review 6.1M + 2.7M + 0.9M, design verification 6.1M + 4.6M (output 270k)

## Goal

The Pipelines page of a local project becomes the same master–detail page as the local Agents hub
(`crates/desktop-app/src/screens/local_hub/`): design P1 `s7LMyQ`. This task builds the page's frame; the steps flow,
settings, Save, versions, menu and states are their own tasks.

## Design (P1)

- Header: *Pipelines*, then on the right the `laptop` icon with *Local* (as Agents shows *Local* or *Team*) and
  *New pipeline*.
- Filter: the tabs *All*, *Drafts* and *Archived* with counts; no hint text on the right.
- List (440 wide): the groups *Needs you* and *Ready* with counts. A row has the `workflow` mark, the name, *N steps ·
  used by N tasks* (or what needs the person, in `warning`) and the version (`v3`, or *draft*); the chosen row is on
  `hover` with the `accent` left edge.
- Panel: *Pipeline* · *v3 · published Oct 27*, the name, *Versions* and ⋯; the chips (version, *Ready* or what is
  missing) and *used by 13 tasks*; a scroll area with a scrollbar when it overflows; the Save bar at the bottom.
- The chosen pipeline and the tab are the route's, as `HubView` is the hub's.

## Out of scope

The flow (PV2-05), settings and Ready? (PV2-09), Save and Cancel (PV2-10), Versions (PV2-11), ⋯ (PV2-12) and the
states (PV2-13). Team projects keep today's page.

## Done when

- The page shows P1's frame with real pipelines, groups and counts; choosing a row shows its panel.
- Reads go through a cached query like the hub's (`queries::hub::local_hub`), never on a timer.
- An independent design verification of P1's header, filter, list and panel head.
- `tools/check.ps1` passes; tests of the grouping and counts in `crates/desktop-app/tests/` (or the workspace).
