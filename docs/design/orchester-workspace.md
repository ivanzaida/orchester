# Orchester — workspace, agents, connections and access

Date: 2026-09-23. Status: proposed design for review, not an approved implementation contract.

## Brief

Personal agent orchestration first, with a future path to teams. The user requested a convenient, restrained application without bright colors, then added first-class instructions inside Orchester, a discoverable variable catalog, and secrets for staging deployment and remote database access. The user subsequently established that tools own credential bindings; secret settings show read-only links to tools; resources can be imported from another project. The final scope clarification makes the project the owner of all operational resources: tasks/runs, pipelines, agent profiles, instructions, tools, secrets and connections. There is no workspace instruction layer or shared mutable workspace resource library.

Navigation selects a project first. Every functional section uses that project context. All projects is a project-switching entry point; it does not make operational resources global. Imported resources are independent local copies with dependency remapping.

The design is an editable desktop application reference at 1440 × 960. English UI copy, grayscale application surfaces, Inter for interface text and Geist Mono for identifiers and prompts. The user explicitly requested GitHub-like colored code: syntax and diff content now use a restrained GitHub Dark-inspired palette while application chrome remains neutral. No runtime, framework, provider or storage implementation has been selected by this work.

All project names, runs, PRs, credentials, reports, usage events, token estimates and budgets are illustrative fixtures. No repository connection, deployment, database query, approval or secret storage actually occurred.

## Editable source

File: `H:/projects/orchester/data/orchester.pen`, edited exclusively through Pencil MCP.

The root contains nine numbered sections in one horizontal band, ordered left to right with a 320px gutter. Reading order within a flow is left to right, then the next row. Main-flow labels use section.step numbers; alternative states use letters. The layer tree follows the same order.

| Section | Node |
| --- | --- |
| 00 / Foundations | `cuxUu` |
| 01 / Getting started | `K7IL6` |
| 02 / Tasks & runs | `g7ncNr` |
| 03 / Pipelines | `Y6XGt` |
| 04 / Agents & connections | `wNy9i` |
| 05 / Instructions & context | `MRxol` |
| 06 / Tools & imports | `V8h6d` |
| 07 / Secrets & access | `b3N1Ix` |
| 08 / Pull requests | `McWTM` |
| 10 / Shared server (column F, index `yuqEI`; see [Shared server](#shared-server)) | `yYqBE` |

Canonical screens and states, in canvas order:

| Screen | Node | Preview |
| --- | --- | --- |
| 01.01 · First launch | `ZBoBm` | [Preview](previews/ZBoBm.png) |
| 01.02 · Project folder | `jnpRq` | preview outdated (reworked 2026-09-25) |
| 01.03 · Project & detected GitHub repository | `W6uXiL` | preview outdated (reworked 2026-09-25) |
| 01.04 · Runner & agent models | `w6ODp4` | [Preview](previews/w6ODp4.png) |
| 01.05 · Ready → Create first task | `x1YpfN` | [Preview](previews/x1YpfN.png) |
| 01.A · Setup blocked / Restore provider access | `Q7htyU` | [Preview](previews/Q7htyU.png) |
| 01.C1–C10 · GitHub detection states (compact, see [GitHub](#github)) | `fL24i`, `rwnig`, `JjJiN`, `Yz9Ig`, `MMSQ9`, `fhqgC`, `C1JtM`, `v85QyU`, `w1Pvl`, `MTPqL` | not exported yet |
| 01.D · Change GitHub account or access (panel) | `hNPA1` | not exported yet |
| 01.06 · All projects | `Z7IZZ` | not exported yet |
| 01.06 A · Remove from list / Confirm | `EtMXj` | not exported yet |
| 01.06 B · Project switcher / Open | `n9vdhp` | not exported yet |
| 01.06 C · All projects / No projects in the list | `A4FJG` | not exported yet |
| 01.06 D · Open project folder failed | `ucI7v` | not exported yet |
| 01.06 E · Rows before their counts are read (compact) | `t8TEOZ` | not exported yet |
| 01.06 F · Remove from list / Agents at work | `glurU` | not exported yet |
| 02.01 · Orchestrator / New conversation | `dOIrp` | [Preview](previews/dOIrp.png) |
| 02.02 · Orchestrator / Task proposal | `I7TCI` | [Preview](previews/I7TCI.png) |
| 02.03 · Tasks / Select drafts to run | `J9467` | [Preview](previews/J9467.png) |
| 02.03 states · Task rows by run state (compact, see [Task library](#task-library-bulk-creation-and-scheduling)) | `r2al55` | not exported yet |
| ORC-39 · Task workflow, priorities and tags · 24 review frames | see [frame index and design contract](#task-workflow-priorities-and-tags-orc-39) | not exported; inspect live Pencil nodes |
| 02.03C · Tasks / Selection with workflow, priority and tags (ORC-30 + ORC-39, 1440) | `DWqjc` | not exported yet |
| 02.03D · Tasks / Held tasks selected · Return to drafts (1440) | `IYlGK` | not exported yet |
| 02.03E · Tasks / Narrow window 960 · three-line selection bar | `t7ETG` | not exported yet |
| 02.03F · Tasks / Sort menu open · five orders (1440) | `lRbit` | not exported yet |
| 02.04 · Create multiple tasks / Review briefs | `OqmXc` | [Preview](previews/OqmXc.png) |
| 02.05 · Run selected tasks / Parallel | `W1Dq6` | [Preview](previews/W1Dq6.png) |
| 02.06 · Run selected tasks / Sequential | `IyJ5a` | [Preview](previews/IyJ5a.png) |
| 02.07 · Task queue / Running and waiting | `Z22wg` | [Preview](previews/Z22wg.png) |
| 02.08 · Project overview | `Twtqc` | [Preview](previews/Twtqc.png) |
| 02.08F · Overview / Failed tasks | `pHNfB` | not exported yet |
| 02.08F2 · Overview / No failed tasks | `aSpMm` | not exported yet |
| 02.09 · New task / Brief, context & limits | `l2YA4` | [Preview](previews/l2YA4.png) |
| 02.10 · Research running | `RD4pc` | [Preview](previews/RD4pc.png) |
| 02.11 · Review & approve plan | `x3lVC` | [Preview](previews/x3lVC.png) |
| 02.12 · Working task / Conversation | `jgqPe` | [Preview](previews/jgqPe.png) |
| 02.13 · Ready for review | `vpgrJ` | [Preview](previews/vpgrJ.png) |
| 02.09A · D4 · New task / Draft saved, approval gate named | `m82tU` | not exported yet |
| 02.09B · D4 · New task / Not saved yet, no approval gates | `v3cdq` | not exported yet |
| 02.09C · D5 · New task / Queued, the runner refused to start the run | `WOdDe` | not exported yet |
| 02.09E · D5 · New task / Retrying the same run | `l801hD` | not exported yet |
| 02.09F · D5 · New task / Reopened queued task, no run | `T6bVyU` | [Preview](previews/T6bVyU.png) |
| 02.A · Paused / Resume from checkpoint | `Oqmog` | [Preview](previews/Oqmog.png) |
| 02.B · Retry limit / Review recovery | `J3JZaT` | [Preview](previews/J3JZaT.png) |
| 02.B2 · Retry limit / Another attempt not available yet | `z2xET` | not exported yet |
| 02.B3 · Waiting for you / Retries used up | `FjCWv` | not exported yet |
| 02.B4 · Waiting for you / Return to Implement with a note | `K7hbj` | not exported yet |
| 02.B5 · Waiting for you / Implement stopped again without a result | `m3pDW` | not exported yet |
| 02.B6 · Needs you / The waiting run in Overview and the status bar | `NM5My` | not exported yet |
| 02.B7 · Returns and restarts in the Conversation timeline (compact) | `zyusO` (removed 2026-10-04; now C·07 `o0KDAq` and C·13 `j0qLQz`) | not exported yet |
| 02.C · Close with active work / Confirm | `Xz30C` | [Preview](previews/Xz30C.png) |
| 02.D · Close / Waiting for agents to save | `a0tTV` | [Preview](previews/a0tTV.png) |
| 02.E · Next start / Session ended abnormally | `o8DCd` | [Preview](previews/o8DCd.png) |
| 02.F · Runner stopped / Reconnecting | `MAn7V` | [Preview](previews/MAn7V.png) |
| 02.G · Runner reconnected | `YHS1v` | [Preview](previews/YHS1v.png) |
| 02.H · Runner could not restart | `EFY7I` | [Preview](previews/EFY7I.png) |
| 02.I · Conversation · Tools / Live transcript | `SfXU1` | not exported yet |
| 02.J · Run permissions / Change for this run | `q4K1G` | not exported yet |
| 02.K · Blocked by permission / Needs you (later milestone) | `H03XN` | not exported yet |
| 02.L · Stopped at limit / Raise limit | `D6Nwxz` | not exported yet |
| 02.L2 · Stopped at step budget / Raise step budget | `A7nVbK` | not exported yet |
| 02.L3 · Stopped at daily project budget / Resume | `F2aBX` | not exported yet |
| 02.L3 states · Resume refused while the daily budget is still used (compact) | `uOLP2` | not exported yet |
| 02.I states · Activity empty, loading, failed, not following (compact) | `hig8O` (removed 2026-10-04; now C·13 `j0qLQz` (D–F) and C·06 `FBwIV`) | not exported yet |
| Token budgets · input states and display rule (compact) | `T1n5BA` | not exported yet |
| 02.J states · Apply now pending → applied (compact) | `LJmTH` | not exported yet |
| 02.M · Project settings / Limits | `c7e729` | not exported yet |
| 02.M states · Default time limit select, open (compact) | `u5Vn2` | not exported yet |
| 02.N · Project settings / General | `QV1af` | not exported yet |
| 02.O · Project settings / Environment | `tXahd` | not exported yet |
| 02.P · Ready for review / Merge into main | `GIiCb` | not exported yet |
| 02.P2 · Merge refused / Not merged into main · conflict | `lDcdB` | not exported yet |
| 02.P3 · Merged into main | `bMJUA` | not exported yet |
| 02.P states · Merge actions and the merged readiness card (compact) | `OP7dt` | not exported yet |
| 02.Q · Remove worktrees / Uncommitted files | `LMrbK` | not exported yet |
| 02.Q2 · Remove worktree / No uncommitted changes | `g1HS8` | not exported yet |
| 02.Q states · Removing, and a removal that failed (compact) | `XIEn8` | not exported yet |
| 02.P4 · U2 · Merging / Remove worktree unavailable | `d3oSWA` | not exported yet |
| 02.Q3 · U2 · Removing worktree / Cancel unavailable | `g8ZALb` | not exported yet |
| 02.Q4 · U5 · After Remove worktree / Summary | `PacyY` | not exported yet |
| 02.Q5 · U5 · After Remove worktree / Conversation | `Y8hyj` | not exported yet |
| 02.03 states · Task rows: Merged and Unreadable (compact) | `mGbYs` | not exported yet |
| 02.R · Unreadable run / Task page | `SUuwV` | not exported yet |
| 02.S · Working task / Conversation as built | `JOG1i` (removed 2026-10-04; now C·01 `kpnmP`) | not exported yet |
| 02.S2 · Conversation of a merged run | `m3gLZ` (removed 2026-10-04; now C·14 `Y6gMx` (A)) | not exported yet |
| 02.R2 · Unreadable run / Conversation, Changes and Activity | `lmlkQ` (removed 2026-10-04; now C·14 `Y6gMx` (F)) | not exported yet |
| 02.R3 · An earlier run could not be read / on every tab | `cP8oh` (removed 2026-10-04; now C·14 `Y6gMx` (F)) | not exported yet |
| 02.S3 · U6 · Conversation / Interrupted | `sYuiX` (removed 2026-10-04; now C·14 `Y6gMx` (B)) | not exported yet |
| 02.S4 · U6 · Conversation / Not started | `Ok14G` (removed 2026-10-04; now C·06 `FBwIV`, C·14 `Y6gMx` (H)) | not exported yet |
| 02.S5 · U6 · Conversation / Recipient menu open | `cj2a2` (removed 2026-10-04; now C·14 `Y6gMx` (C)) | not exported yet |
| Sidebar runner card states (compact) | `O5NoB1` | [Preview](previews/O5NoB1.png) |
| 03.01 · Pipeline / Feature delivery | `Q21HOA` | [Preview](previews/Q21HOA.png) |
| 03.02 · Publish v4 / Confirm | `zfjsK` | [Preview](previews/zfjsK.png) |
| 03.03 · Published v4 | `yf6XH` | [Preview](previews/yf6XH.png) |
| 03.04 · Pipelines library | `LRMQW` | [Preview](previews/LRMQW.png) |
| 03.05 · New pipeline / Name and starting point | `c3jD7` | [Preview](previews/c3jD7.png) |
| 03.06 · Empty pipeline / No steps yet | `angoF` | [Preview](previews/angoF.png) |
| 03.A · No pipelines yet | `MqRT7` | [Preview](previews/MqRT7.png) |
| 03.B · Draft has problems / Publish blocked | `UjHDL` | [Preview](previews/UjHDL.png) |
| 03.C · New pipeline / Name already used | `h5GPCL` | not exported yet |
| 03.07 · Version history / Viewing v4 · Read-only | `fvPvA` | not exported yet |
| 03.08 · Compare v3 with draft v5 | `Fb42P` | not exported yet |
| 03.09 · Restore v3 as draft / Click again to confirm | `NmNRM` | not exported yet |
| 03.07 states · History panel: loading, failed, first version, nothing to compare (compact) | `H0l9D` | not exported yet |
| 04.01 · Project connections | `a4Lqb` | [Preview](previews/a4Lqb.png) |
| 04.02 · Add agent provider | `h6CmJ` | [Preview](previews/h6CmJ.png) |
| 04.03 · Agent profiles / Assign connection & model | `WPmtS` | [Preview](previews/WPmtS.png) |
| 04.04 · Agent profile / Permissions | `T1rjv` | not exported yet |
| 04.05 · Connections / GitHub connection | `CHa9s` | not exported yet |
| 04.06 · Add GitHub connection | `D4mDB` | not exported yet |
| 04.A · Expired MCP access / Reconnect (not built; the MCP servers tab as built is 04.D) | `RZUKo` | [Preview](previews/RZUKo.png) |
| 04.B · Options the connection can't enforce are disabled (compact) | `W0Owo` | not exported yet |
| 04.C1–C5 · GitHub connection: not signed in, gh not found, wrong account, unrecognized output, missing scope (compact) | `Cr0P2`, `ve4G9`, `P89Dl`, `kqfaL`, `HwBI7` | not exported yet |
| 04.D · Connections / MCP servers as built | `QdLRd` | not exported yet |
| 04.E · Add MCP server / Program | `yRb2k` | not exported yet |
| 04.E2 · Add MCP server / Remote MCP | `Qvhc6` | not exported yet |
| 04.E3 · Edit MCP server / Values it cannot save | `Hpycl` | not exported yet |
| 04.F · Agent profile / Permissions · MCP servers (compact) | `VI4c6` | not exported yet |
| 04.G · Activity / Working without an MCP server (compact) | `a2PR7K` | not exported yet |
| 04.D2 · M5 · Connections / MCP servers · Remote server selected | `a4lm6D` | not exported yet |
| 04.03A · U8 · Agent profile / Published, no draft | `dWnw7` | not exported yet |
| 04.04A · U8 · Agent profile / Permissions · Published, no draft | `t2wCI` | not exported yet |
| 04.03B · U8 · Agent profile / Unsaved changes | `Jw98x` | not exported yet |
| 05.01 · Project rules & effective prompt | `yRKeF` | [Preview](previews/yRKeF.png) |
| 05.02 · Resolved prompt / Inspect sources | `fZa3J` | [Preview](previews/fZa3J.png) |
| 05.03 · Variables / Search, inspect & insert | `EfTiT` | [Preview](previews/EfTiT.png) |
| 05.A · Variable unavailable at this event | `m2WjL` | [Preview](previews/m2WjL.png) |
| 06.01 · Project tool library | `ZFwWl` | [Preview](previews/ZFwWl.png) |
| 06.02 · Tool configuration & credentials | `YEWFo` | [Preview](previews/YEWFo.png) |
| 06.03 · Tool access rules | `H97stw` | [Preview](previews/H97stw.png) |
| 06.04 · Import / Select project resources | `W4zGKx` | [Preview](previews/W4zGKx.png) |
| 06.05 · Import / Resolve dependencies | `QqnDd` | [Preview](previews/QqnDd.png) |
| 06.06 · Import / Ready to create local drafts | `hF664` | [Preview](previews/hF664.png) |
| 06.A · Create pull request / Settings, Open as draft (compact) | `mtPZd` | not exported yet |
| 06.B · Create pull request / Push rejected (compact) | `YDKJS` | not exported yet |
| 06.C · Import / Rename copy name field (compact) | `FKzQ9` | not exported yet |
| 07.01 · Project secrets | `R99lt` | [Preview](previews/R99lt.png) |
| 07.02 · Add project credential | `tpF7R` | [Preview](previews/tpF7R.png) |
| 07.03 · Runtime approval / One deployment | `nuuZL` | [Preview](previews/nuuZL.png) |
| 07.A · Duplicate secret name | `m9Dhp` | [Preview](previews/m9Dhp.png) |
| 07.B · M3 · Secrets · Used by an MCP server | `wPXgW` | not exported yet |
| 07.C · M3 · Secrets · Used by a tool and an MCP server | `R2ImH` | not exported yet |
| 07.D · M3 · Archive secret · Used by a tool and an MCP server | `RxpL1` | not exported yet |
| 08.01 · Pull requests / Review inbox | `wGtxT` | [Preview](previews/wGtxT.png) |
| 08.02 · Pull request / Request an agent fix | `Rrmhf` | [Preview](previews/Rrmhf.png) |
| 08.03 · Pull request / Verify the agent fix | `i4GUz` | [Preview](previews/i4GUz.png) |
| 08.A · Pull request / Agent answers a question | `qJKSC` | [Preview](previews/qJKSC.png) |
| 08.B · PR updated while composing (compact state) | `hfoDa` | [Preview](previews/hfoDa.png) |
| 08.C · Agent request queued (compact state) | `uD6Ho` | [Preview](previews/uD6Ho.png) |
| 08.D · Pull requests / GitHub unavailable | `dmib5` | not exported yet |
| 08.E · Pull requests / Stale, last synced (compact) | `FYQMv` | not exported yet |
| 08.F · Pull requests / Rate limited (compact) | `P1tSu` | not exported yet |
| 08.G · Pull requests / Wrong account (compact) | `E9ZThy` | not exported yet |
| 08.H · Pull requests / Missing scope: repo (compact) | `tu1fW` | not exported yet |

There are 110 full screen frames; earlier screen IDs are preserved. Screens remain 1440 × 960, at x 48 / 1568 / 3088 with 80px horizontal gaps, 88px row gaps and labels 32px above each frame. Compact recovery panels are counted separately. Section subgroups have a 96px gap. 02.09F `T6bVyU` fills the third position in row 2 of `F3fEH` at x 3088 / y 1148.

- Getting started: first launch → project folder → project with detected GitHub repository → runner/agents → ready. Setup blocking, compact empty-project feedback (`RlwqW`), the *Change GitHub account or access* panel (01.D, x 1568 / y 426) and the ten GitHub detection states (01.C1–C10, x 2128–4808, rows at y 100, 512 and 980) are in `GojEY` (height 1400) below the main flow. All projects, its removal, the project switcher and their states are in `wa6B7` ("All projects · list, removal, switcher", y 3920, height 3256: 01.06, 01.06 A, 01.06 B in row 1, 01.06 C, D, E in row 2, 01.06 F `glurU` at x 48 in row 3; section height 7272); see [All projects](#all-projects).
- Tasks & runs starts with project chat in `I0Ij1T`: new conversation → task proposal. `UIc5y` follows with task library, bulk creation, parallel/sequential scheduling and active queue, and the compact task-rows board `r2al55` (x 3088 / y 1148, row 2); the combined task list (ORC-30 + ORC-39, drawn 2026-10-02) is 02.03C `DWqjc` and 02.03D `IYlGK` beside 02.03B in row 3 (x 1568 / 3088, y 2196) and 02.03E `t7ETG` and 02.03F `lRbit` in row 4 (x 48 / 1568, y 3244). For row 4, `UIc5y` grew to 4264, `AAdQb` moved to y 5796 and 02.1 `g7ncNr` to 6976 high, and the sections below it in that column moved down 1048: 02.2 `eOjeq` to y 9176, 02.3 `Yn5Wo` to 17600, 05.2 `F1yFNk` to 27912. The individual flow is in `D4pfd`: overview → new task → research → plan approval → working conversation → ready for review. `F3fEH` ("States · New task truthful states & build space (D4, D5)", drawn 2026-09-29) follows it: 02.09A, 02.09B, 02.09C in row 1; 02.09E, 02.09F in row 2 (its first slot is empty since 02.09D was removed, 2026-10-02). Pause and retry are alternatives in `O4vgKt`, with 02.B2 `z2xET` at x 3088; runner/repository/loading recovery, status variants and interaction notes are in `y8TKW8`. Application lifecycle states (close with active work, runner connection, recovery on start) and the sidebar runner card states are in `Z7Fyr8`. Agent execution states (Activity transcript, run permissions, permission request, stopped at limit) with the compact Activity, Apply now and token budget states, plus Project settings / Limits and General (row 3, with the time-limit select board `u5Vn2` at x 1568 / y 2592) and the step-budget and daily-budget stops 02.L2 / 02.L3 (row 4, y 3260), with Project settings / Environment 02.O `tXahd` beside them at x 3088 under 02.N, are in `BnxJ1` (height 5324), with the compact board 02.L3 states `uOLP2` at x 48 / y 4308 (row 5, y 4308) and, beside it, the ORC-15 Project settings frames `qO84y` and `Sr5gP` (x 1568 / 3088, y 4308). Merging a run and removing its worktrees are in `gdgT2` ("States · Review, merge & worktrees": 02.P, 02.P2, 02.P3 in row 1; the compact merge board 02.P states, 02.Q and 02.Q2 in row 2; the compact board 02.Q states `XIEn8` at x 48 in row 3, y 2196, with 02.P4 `d3oSWA` and 02.Q3 `g8ZALb` beside it; 02.Q4 `PacyY` and 02.Q5 `Y8hyj` in row 4, y 3244; height 4300). The unreadable run and the task conversation as built are in `GvUXd` (removed 2026-10-04; now `A5H6ln` and `LpVFY`) ("States · Unreadable run & task conversation": the compact task-rows board for Merged and Unreadable, 02.R and 02.S in row 1; 02.S2, 02.R2 `lmlkQ` (removed 2026-10-04; now C·14 `Y6gMx` (F)) and 02.R3 `cP8oh` (removed 2026-10-04; now C·14 `Y6gMx` (F)) in row 2; 02.S3 `sYuiX` (removed 2026-10-04; now C·14 `Y6gMx` (B)), 02.S4 `Ok14G` (removed 2026-10-04; now C·06 `FBwIV`, C·14 `Y6gMx` (H)) and 02.S5 `cj2a2` (removed 2026-10-04; now C·14 `Y6gMx` (C)) in row 3, y 2196; height 3252). Group positions are y 160 / 1376 / 3640 / 5904 (`F3fEH`) / 8168 / 9384 / 10480 / 13096 / 17848 / 22244 respectively; the section height is 25592. `F3fEH` is the last layer of the section, although it sits after `D4pfd` on the canvas.
- Pipelines: the visual editor is separate from task execution. `UJUi0` holds edit → publish confirmation → published. `t9zWUe` holds the library (the Pipelines navigation destination) → new pipeline → an empty pipeline in the editor. `rBy9L` holds the empty library, the publish-blocked editor and 03.C `h5GPCL` (a name whose slug is taken, x 3088). `r8rPaS` holds the version history: viewing v4 → compare v3 with the draft → restore v3 (row 1) and the compact history-panel states (row 2). `Rg8DS` holds the pipeline presets (see [Presets](#presets)). Group positions are y 160 / 1376 / 2592 / 3808 / 5664; the section height is 10024.
- Agents & connections: connections → add provider → assign connection/model in the agent profile (`kOGgX` row 1), then the profile's Permissions tab (04.04), the GitHub connection (04.05) and Add GitHub connection (04.06) in row 2; expired-access recovery, the compact state 04.B (options the connection can't enforce) and the GitHub connection states 04.C1–C5 (x 2108 / 2492 / 2876 / 3260 / 3644) are in `DLRW5` (now at y 2424). The MCP servers are in `m3wObx` ("States · MCP servers", y 3640, height 2404): 04.D, 04.E, 04.E2 in row 1; 04.E3, the compact boards 04.F and 04.G in row 2 (04.F is 1160 high); 04.D2 `a4lm6D` at x 48 in row 3 (y 2396); height 3452. The agent profile header states are in `iWIfR` ("States · Agent profile header (U8)", y 7188, height 1120): 04.03A, 04.04A, 04.03B, and in row 2 (y 1148) the ORC-15 Permissions frame `bYDCI`; `iWIfR` is now 2168 high. The native agent connections are in `PNDQm` ("States · Native agent connections", y 9452, height 4468); the section is now 14016 high. See [MCP servers](#mcp-servers) and [Native agent connections](#native-agent-connections).
- Instructions: rules → composed prompt → variable catalog; unavailable-variable state is below. ORC-15 (skills and import from repository) is `HhPZg` at y 2592 (height 2168); the section is 4856 high.
- Tools & imports: library → configuration → access policy in the first group. Select → resolve dependencies → ready to import is a separate complete row in `TSGMX`. Success/failure panels remain in `tjNmM`, now with the Create pull request settings (06.A, x 1040) and push-rejected recovery (06.B, x 1600) and the Rename copy name field (06.C, x 2096); `tjNmM` is 720 high and the section height is 3408.
- Secrets & access: library → add credential → runtime approval; duplicate-name validation is below. `x91Ww` ("States · MCP server uses (M3)", y 2592, height 1120) holds 07.B, 07.C and 07.D; section height 3808.
- Pull requests: `scsYh` contains the review inbox → line-specific fix request → verification on a new commit. `u77pi` contains the question/answer alternative, compact stale-revision and queued-request states, the stale-sync and rate-limited strips (08.E, 08.F at x 2064), the GitHub-unavailable screen (08.D at x 3088) and the wrong-account and missing-scope strips (08.G, 08.H at y 1200); `u77pi` is 1680 high. The section is at x 41728 / y 0, width 4896 and height 3152.

Section previews: [Getting started](previews/organization/K7IL6.png), [Tasks & runs](previews/organization/g7ncNr.png), [Pipelines](previews/organization/Y6XGt.png), [Agents & connections](previews/organization/wNy9i.png), [Instructions](previews/organization/MRxol.png), [Tools & imports](previews/organization/V8h6d.png), [Secrets & access](previews/organization/b3N1Ix.png).

## Components and tokens

52 reusable components are defined. Most are in Foundations; the composed secret inspector, effective instruction preview and inline notice have canonical definitions within their owning screens, and the Rename copy mapping row within its board (06.C). The shared brand is canonical inside the sidebar. The conversation composer is canonical inside the task panel and reused by project chat and PR feedback. Screens use linked instances; content, icon, state and supported size are intended overrides.

| Family | Node |
| --- | --- |
| Button | `u1XBP` |
| Navigation item | `TSJpn` |
| Status label | `DeizF` |
| Pipeline step | `ajikA` |
| Task row | `zemhy` |
| Select/input field | `ZGwlv` |
| Select menu (open list, see [Select menu](#select-menu)) | `u0ByM` |
| Select option | `L20XvS` |
| Sidebar | `zo9Py` |
| Tab | `SOW6S` |
| Activity event | `mBkeJ` |
| Evidence check | `l3hyb` |
| Project metric | `dZ2Zt` |
| Run progress step | `i9rVFu` |
| Pipeline summary step | `XVkaB` |
| Key/value property | `tfGRt` |
| Recovery panel | `iN90G` |
| Instruction rule item | `jNTVN` |
| Instruction source row | `SV0BN` |
| Variable row | `ZJ6KN` |
| Secret row | `vxAMD` |
| Secret details inspector | `CQxCb` |
| Tool library row | `sFtAz` |
| Preview execution context | `TnjFo` |
| Effective instruction preview | `RGk5C` |
| Inline notice | `q5LzI` |
| Import selectable resource | `cuocA` |
| Import dependency mapping | `HfNoA` |
| Import dependency mapping · Rename copy (variant of `HfNoA` with the name field; canonical in 06.C `FKzQ9`) | `ixzlL` |
| Conversation message | `J5MIT` |
| Conversation composer | `C3nu0z` |
| Orchestrator proposed task | `SGjO0` |
| Task conversation panel | `kjsmd` |
| Agent model assignment row | `kOI6w` |
| Orchester brand | `a0S44m` |
| Task queue row | `clVjO` |
| Task execution mode | `lcwsy` |
| Pull request list row | `QHBNZ` |
| Pipelines library row | `hn8qO` |
| Changed file | `j9NDM` |
| Diff line | `vEZ42` |
| Window title bar (Windows) | `QQmsI` |
| Window title bar (macOS) | `V70DN` |
| Window tab (title bar tab, see [Tabs](#tabs)) | `nAKyb` |
| Window status bar | `pmOpt` |
| Window status bar operations popover | `E1msJS` |
| Focus ring overlay (design-only, see [Focus](#focus)) | `X9gPDr` |
| Field / Token budget (whole-number input with `tokens` unit) | `sN3du` |
| Control / Switch (off state; on = `$accent` fill, knob right) | `K8ItM` |
| Permissions / Command entry (allowlist chip with remove) | `bpmzN` |
| Activity / Transcript entry | `N4SlQq` |
| Activity / Step header | `mDnc3` |

Colors: canvas `#101010`, sidebar `#141414`, surface `#1A1A1A`, raised `#222222`, hover/selection `#2C2C2C`, border `#363636`, hairline `#2B2B2B`, text `#EEEEEE`, muted `#A3A3A3`, subtle `#8F8F8F`, primary action `#E6E6E6` with `#181818` text, danger `#F87171` (`danger`, only the *Unreadable* status label), focus ring `#EEEEEE` with a `#101010` gap (`focus-ring`, `focus-gap`). Variables also hold fonts, spacing and 7/12px control/panel radii. Status always includes text and an icon.

**Counts** (decided 2026-09-27): every count is the plain number — `2`, `12`, `0` — never zero-padded (`02`, `00`) and never a dash when it is known. This applies to the Overview metrics (`dZ2Zt`, e.g. `C95Z93`), the Overview *Needs you* heading count and the status bar. A dash (`—`) means only "not known yet" (agent slots while the runner is not online).

**Overview task filters** (`i2uFe` in 02.08): *All tasks*, *Running*, *Needs you*, *Queued* and *Failed* each show their count. *Failed* lists newest tasks whose latest run failed; a cancelled run, an unreadable latest run and a task with an earlier failed run but a newer non-failed run do not appear there. An empty Failed list says *No failed tasks.*
The selected Failed states are 02.08F `pHNfB` (two failed rows) and 02.08F2 `aSpMm` (count zero and empty message), both 1440 × 960 in `AAdQb`; titles and run data are illustrative fixtures.

**Scroll gutter** (decided 2026-09-27, drawn in 04.01 `JuVaA` and 02.L2 `A7nVbK`): a side panel or column whose content can outgrow the window (connection inspectors, *This run*, the Summary column) scrolls its details in a viewport that reserves a 12px gutter at its right edge, so the bar never covers a value. The panel's right padding shrinks by the same 12px, so content still ends 24px (or 32px) from the edge. Thumb: 4px wide, `$border`, radius 2, centred in the gutter (4px from each side), 4px from the top and bottom of the viewport, length = track × visible / total and at least 24px; no track. It shows only while the content overflows. Actions pinned below the viewport (inspector buttons, the status detail) never scroll. Select menus keep their own overlay thumb (`YD8PV`).

## Window chrome

The application draws its own window chrome; the operating system's title bar and window border
are not shown. A 32px title bar in `$canvas` sits above every 1440 × 960 screen frame and a 28px
status bar (`pmOpt`, see [Status bar](#status-bar)) sits below it, so the window is
1440 × 1020 (32 + 960 + 28). The title bar holds the window's tabs (see [Tabs](#tabs)); its
empty parts are the drag area (double-click maximizes).

- Windows (`QQmsI`): the 216px drag area over the sidebar (`D6SIx6`), the tab strip (`tfnSm`,
  slot for `Window / Tab`), the new-tab button (`K3W9DL`), the drag area (`Q5v1g`), then
  minimize, maximize/restore and close on the right, 46 × 32 each, lucide icons at 14px in
  `$muted`; hover fills the control with `$hover`.
- macOS (`V70DN`): the three window buttons on the left inside the 216px area over the sidebar
  (`R3RtGs`), 12px, 8px apart, 12px from the edge, in the platform's standard colors — the only
  non-grayscale colors in the application; then the tab strip (`N4qch`), the new-tab button
  (`ivN8x`) and the drag area up to the right edge.

A dialog's backdrop covers the screen frame only, so both bars stay undimmed. While a dialog is
open only the dialog and the title bar (drag, minimize, maximize, close) take input: the status
bar shows no hover and its items open nothing until the dialog closes, and the tabs, + and tab
list do not react (a dialog belongs to its tab's page, so switching away would hide it).

Screens include neither bar; design verification captures the app window at 1440 × 1020 and
compares the title bar (top 32px) with the platform component, the screen area (next 960px) with
the screen frame, and the status bar (bottom 28px) with the matching `pmOpt` state. A worked
example is the window composition `cpoal` in Foundations; with several tabs, `xjpkC` in
[Tabs](#tabs).

## Tabs

Proposed 2026-09-28 in answer to the user's request for several browser-like tabs in the title
bar. Canvas group `rOh31` ("Window · Tabs") in Foundations, between the status bar group and the
focus group: the states board `UZF1Q` and two window compositions. The component
`Window / Tab` (`nAKyb`) is in the Components group next to the title bars. The title bar
components `QQmsI` and `V70DN` now contain the tab strip; `cpoal` shows one tab.

**Model.** A tab is an independent location: a project and a route (Overview, Tasks, a task, a
pipeline, Settings…), or All projects, which has no project. Each tab has its own Back/Forward
history (mouse back/forward buttons and Alt+Left/Right act on the active tab). Two tabs may show
the same project or different ones; nothing is shared between them except the project data. The
sidebar, the page and the status bar always belong to the active tab's project. A new tab opens
All projects.

**Title bar layout** (the bar stays 32 high; the tabs are 28 high, bottom-aligned, so 4px of
title bar shows above them):

| Part | Windows (`QQmsI`) | macOS (`V70DN`) |
| --- | --- | --- |
| 0–216 (sidebar width) | drag area | traffic lights, then drag area |
| from 216 | tab strip | tab strip |
| after the last tab | + (24 × 24, radius 6, lucide `plus` 14 `$muted`, hover `$hover`) | + |
| rest | drag area, at least 48px, then minimize / maximize / close | drag area to the right edge |

The tabs start at x 216 on both platforms and on every page, over the content column, so they
never move when the platform, the project or the page changes, and the drag area over the sidebar
is always available however many tabs are open. Why 32 is kept: a 28px tab holds a 16px avatar,
a 12px label and a 16px close button with 6px to spare, the drag area never depends on space above
the tabs, and keeping 32 leaves the window at 1440 × 1020, every composition and the design
verification crop unchanged.

**Tab** (`nAKyb`): 224 wide at most, 28 high, padding 0 6 0 10, gap 8, top corners
`$radius-control` (7), clipped. Children: project avatar 16 × 16 radius 4 `$hover` with the
initial (Inter 9 semibold `$text`) — for All projects the lucide `folders` 14 `$muted`
instead —, the label (Inter 12, one line, ending with an ellipsis), the close button (16 × 16,
radius 4, lucide `x` 12 `$muted`). Label: `<project> · <page>`, where page is the section name or
the object's key or name (*Kontur · ORC-12*, *Kontur · Feature delivery*); the tooltip adds the
object title (*Kontur · ORC-12 · Add retry to webhook sender*). The OS window title is the active
tab's label followed by ` — Orchester`.

| State | Look |
| --- | --- |
| Active | `$surface` fill, joins the screen below (no border); label `$text`; close visible |
| Inactive | no fill; label `$muted`; the close slot is kept but invisible, so the label does not move on hover |
| Hover (inactive) | `$sidebar` fill; label `$text`; close visible |
| Close hovered | close `$hover` fill, icon `$text` |
| Divider | 1 × 14 `$border`, only between two inactive, unhovered tabs |
| Attention | 8px `$text` dot on the avatar's top-right corner (x 10, y −2) with a 2px ring in the tab's background |
| Dragged | `$raised` fill, `$radius-control` on all corners, label `$text`, shadow `0 8 24 #00000066` |
| Only tab, All projects | no close button |

**Attention dot.** Every tab of a project that has runs waiting for the user (the status bar's
*Needs you* count) or an unresolved failure (status bar state H) shows the dot, the active tab
included. The tooltip names the reason: *Personal website · 1 needs you* or *Personal website ·
Could not create worktree for PW-14*. It clears when the count is 0 and the failure is opened or
dismissed. All projects tabs never show it.

**Overflow** (decided 2026-10-15; the new app's frames are `oHEGQ`, `qWCBQ`, `a1i6Ws` and `YAOSy` in
`orchester2.pen`). While every tab fits, a labelled tab keeps its natural width with its label capped at
160. As tabs open, the inactive labelled tabs shrink together down to 120 (`Kontur · Ove…`); the active tab
never shrinks, and All projects stays 28. When 120-wide tabs no longer fit, the strip does not scroll:
it shows the longest run of tabs from the left that fits beside the active tab, those tabs at 120, and
the rest are hidden. An active tab outside that run takes the last slot after it, so the tabs on the left
never move under the pointer. The + and 48px of drag area always stay visible.

The tab list button sits after the last visible tab, before +, only while a tab is hidden: the count of
hidden tabs (Geist Mono 11 `$text_secondary`) and `chevron-down` 12, 26 high, radius 6. Hovered it takes
`$hover` and says *N more tabs · Ctrl Shift A*; open it takes `$pressed` and `chevron-up`. Ctrl/Cmd+Shift+A
opens and closes the list. The list is the menu surface (`$surface_raised`, radius 10, `$border_strong`,
padding 6), 320 wide, its start under the button's, 4 under the title bar, and 8 inside the window's right
edge where it would cross it; it grows up to the window height less 48, then its rows scroll. It holds
every open tab in strip order: the tab's icon, its page, its project, `eye-off` while the tab is hidden,
the needs-you mark, a check on the current tab (filled `$accent_muted`), and a close button in a reserved
slot while the row is hovered. With more than eight tabs a search field leads the list, focused, matching
page and project; with no match it says *No open tab matches “…”*. Up/Down move a focus ring over the rows
(a list without search opens with the current row ringed), Enter switches, Ctrl/Cmd+W closes the ringed
tab, Escape clears the search and then closes the list; a click switches and closes the list, a
middle-click closes that tab. A tab chosen from the list becomes active, so it takes the last slot. A
choice or Escape gives the focus back to the button (a Tab stop whose ring shows only after keyboard use);
when the list goes away because every tab fits again, *New tab* takes the focus.

**Order.** Press on a tab and move 4px to drag it along the strip (horizontal only; it does not
activate the tab). The other tabs slide aside to show where it will land; release drops it there,
Escape or releasing away from the title bar puts it back. A tab cannot be torn out into a new
window: there is one window.

**Opening and closing.**

- New tab (+, Ctrl/Cmd+T): All projects, inserted right of the active tab, and activated.
- The sidebar project selector (`ZcJDx`, menu 01.06 B `n9vdhp`) changes the **current** tab: it
  opens the chosen project's Overview in it, as a new history entry (Back returns). Ctrl/Cmd+click
  or middle-click on a project opens it in a new tab right of the current one without switching
  (Ctrl/Cmd+Shift+click switches). The hovered menu row shows an *Open in new tab* button
  (`NcJQ0`, lucide `square-plus`, tooltip *Open in new tab · Ctrl+click*).
- All projects (01.06): a click on a row opens that project's Overview in the current tab (the
  All projects tab becomes the project's tab); Ctrl/Cmd+click or middle-click opens it in a new
  tab. The sidebar's *All projects* item and the menu's *All projects* open it in the current tab.
- Link-like navigation inside a page (a task row, a pipeline, a pull request) stays in the tab;
  middle-click or Ctrl/Cmd+click on those opens a new tab only where the target is a whole page.
- Close: the × button, middle-click, Ctrl/Cmd+W. The next tab to the right becomes active (the
  left one if it was the last). Closing an inactive tab does not switch tabs. Closing asks only
  when the tab's page would ask on navigating away (unsaved edits, e.g. an agent profile before
  *Save draft*); that question is not drawn yet (see "Implemented" below).
- **Closing the last tab** opens All projects in a fresh tab; the window stays open (decided in
  this proposal). Closing the window quits the application and stops every runner and agent
  (there is no background mode, see [Application lifecycle](#application-lifecycle-and-runner-connection)),
  so a repeated Ctrl/Cmd+W must not do that. The window is closed only with the window controls,
  Alt+F4 or Cmd+Q, which keep the close-with-active-work dialog (02.C). When the only tab is
  already All projects, it has no close button and Ctrl/Cmd+W does nothing.

**Keyboard** (application shortcuts; tabs themselves are pointer-only — never focused, no ring,
no Tab stop, following [Focus](#focus)):

| Keys | Action |
| --- | --- |
| Ctrl+T / Cmd+T | New tab (All projects) |
| Ctrl+W / Cmd+W | Close the active tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab, wrapping (Ctrl on macOS too) |
| Ctrl+PageDown / Ctrl+PageUp, Cmd+Shift+] / Cmd+Shift+[ | Next / previous tab |
| Ctrl+1…8 / Cmd+1…8 | Tab 1…8 |
| Ctrl+9 / Cmd+9 | Last tab |
| Alt+Left / Alt+Right, Cmd+[ / Cmd+] | Back / Forward in the active tab |

Shortcuts work while a dialog is closed; while a dialog is open only the dialog takes input
(as for the status bar). Ctrl+W inside a text field still closes the tab.

**Restore.** The tabs (order, location, active tab) are saved in the application database
(`ui_state`) whenever they change and restored on the next start; the Back/Forward history is not
restored. A tab whose project can no longer be opened (folder missing, newer data) is restored as
that project's row state in All projects, never as an error page. If the previous session ended
abnormally, the recovery dialog (02.E) opens over the restored tabs.

**Runners.** One runner per open project, as today. A project is open while at least one tab shows
it **or** its runner has work: a run in progress or queued, or a checkpoint being saved. When the
last tab of a project closes:

- with no work, its runner stops at once (runs waiting for the user — plan approval, a question, a
  review — need no runner: an answer given later starts it again and the run resumes, as in
  [storage.md](../architecture/storage.md));
- with work, the runner keeps running in the background until the project is idle, then stops. Its
  runs keep their normal status everywhere: the project's row in All projects shows *Runner online*
  and its Active runs, the status bar's operations popover lists its operations with the project
  name, and its *Needs you* count reaches every tab of it through the attention dot.

Closing the window stops everything, exactly as before: the close-with-active-work dialog lists
active runs of every project, with or without a tab. Opening a project in a tab starts its runner
if it is not running.

**States** (`UZF1Q`; each shows the title bar over the top 20px of the screen, sidebar and
content):

| State | Node |
| --- | --- |
| A · One tab | `Mwbxc` |
| B · New tab · All projects (only tab, no close) | `GS4ZC` |
| C · Several tabs, one active | `v12rJe` |
| D · Hover · close button hovered, tooltip | `hWPxE` |
| E · A project needs you (dot, tooltip) | `V1IUt5` |
| F · Overflow · tab list open (menu `R4rtN1`) | `FtUhB` |
| G · Dragging to reorder (dragged tab `mNVeh`) | `Ymhc7` |
| H · macOS | `DF2hE` |

**Window compositions** (1440 × 1020): `xjpkC` — three tabs over 02.03 Tasks (*Kontur · Tasks*
active, *Kontur · ORC-12*, *Personal website · Overview* with the dot) with the idle status bar;
`OOkFf` — a new tab on All projects (01.06) next to three project tabs. On All projects the status
bar keeps only the activity slot, labelled with the project (*Kontur · Run #128 ready for
review*), and the version: Runner, Agents, Queued, Needs you and Tokens belong to a project and
are hidden.

**All projects in a tab.** All projects (group `wa6B7` in 01 / Getting started: 01.06 list
`Z7IZZ`, 01.06 A remove from list `EtMXj`, 01.06 B project switcher `n9vdhp`, 01.06 C empty
`A4FJG`) is the new-tab page. Screens stay 960 high without chrome, as everywhere; `OOkFf` shows
it in the window. Added for tabs on 2026-09-28:

- 01.06 B `n9vdhp`: the hovered row (*Personal website*, `qUmfQ`) shows the *Open in new tab*
  button `NcJQ0` (20 × 20, radius 5, lucide `square-plus` 14 `$muted`) before the check slot,
  with the tooltip `yRd7u`. Only the hovered row shows it; the current project may be opened in
  a second tab too.
- 01.06 D `ucI7v` (x 1568 / y 1148): *Open project folder…* failed. An error line `tmE5H`
  between the heading and the list — lucide `circle-alert` 14 and Inter 12, both `$text`, 8
  apart: *Couldn’t open D:\work\notes — it has no .orchester project data. Use Add project to set
  it up as a new project.* It stays until the next *Open project folder…* or *Add project*. Other
  reasons use the same line: the folder's data was written by a newer version; the folder could
  not be read (with the system's reason).
- 01.06 E `t8TEOZ` (x 3088 / y 1148, compact): rows before their counts are read. The list, paths,
  folder checks and runner states are known at once; Active runs and Needs you come from each
  project's database and show a dash (“not known yet”, never 0) until read. No page-level loading
  state or spinner.
- The runner states in the list follow the runner rule above: *Runner online* for a project with a
  running runner, with or without a tab; *Runner not started* for a project whose runner is not
  running.

**Not designed:** several windows, tearing a tab out, pinned tabs, reopening a closed tab
(Ctrl+Shift+T), tab groups, and the unsaved-edits question on close.

**Implemented** (2026-09-28, branch `claude/tabs`):

- *Discard changes?* is implemented, not drawn yet: closing a tab whose page holds edits that are
  not saved (an agent profile or a command tool before *Save draft*; a new one once anything was
  written) asks in the confirmation dialog's frame (`P8IN7Z`): *Keep open* or *Discard and close*;
  Escape keeps the tab. Pages whose drafts save themselves never ask.
- The OS window title does not follow the active tab yet: lurq 0.29 has no way to change a
  window's title after it opened.
- macOS Cmd+click: lurq's mouse events carry Ctrl, Shift and Alt only, so the window follows the
  Cmd key's presses to tell a Cmd+click.
- Links inside pages (2026-09-28, branch `claude/tab-links`) open the same way as the switcher's
  rows: task, pipeline, library, profile, tool, connection, instruction, secret and pull request
  rows, page tabs, sidebar items, breadcrumbs, the Overview's cards, the status bar's counts and
  its activity item and popover rows, and buttons that only go to another page (*New task*,
  *Open connections*, *Open tool*). All read the press through `ui::link::Link`. Enter or Space on
  a focused link opens it in the current tab whatever keys are held. Submitting a form, a dialog's
  result, a redirect after an action, *Import resources* (a dialog over the page), closing the
  version history and *Ask agent* (which also focuses the composer) stay in the current tab.
- The inactive tabs are mounted offstage: their pages keep their state (fields, scroll positions)
  but are not rendered, laid out or painted until the tab is active again. Each project's tasks,
  usage and runner status are read once for all its tabs.

Copy namespace: `tabs.*` (`tabs.label`, `tabs.labelWithTitle`, `tabs.attention.*`,
`tabs.openInNewTab.*`, `tabs.discard.*`). Labels are built from project and page names (data)
with one separator key, not by concatenating translated fragments.

## Status bar

Proposed 2026-09-24 in answer to the user's request for a bar at the bottom of the window that
says what is happening. Canvas group `Tg1WX` ("Window · Status bar") in Foundations below the
components: the states `keJ9Z`, the anchored operations popover `imT2E` and the window
composition `cpoal`. Previews: [component](previews/pmOpt.png), [states](previews/keJ9Z.png),
[popover](previews/imT2E.png), [window composition](previews/cpoal.png).

**Placement and style.** `Window / Status bar` (`pmOpt`, next to the title bars in Foundations) is
applied like the title bar: below every 1440 × 960 screen frame, which stays unchanged. Window:
1440 × 1020. Height 28px, `$canvas` (the same frame colour as the title bar, so content sits
between two chrome bands), hairline top border, 8px side padding. Every item is 22px high with 8px
padding, 6px gap and a 5px hover radius (`$hover` fill); labels are Inter 11 `$muted`, counts,
times, amounts and the version are Geist Mono 11 (`$text` for counts, `$subtle` for times and
zeros). Icons are lucide 12px; status is always icon + label. It belongs to the selected project,
like the rest of the window.

**Content** (left to right). Every item maps to data the app has or will have; nothing is shown
that is not measured.

| Item | Copy (fixture) | Data source | Click target |
| --- | --- | --- | --- |
| Runner | plug · *Runner online* | runner connection state for the selected project (IPC) | Opens the runner log `.orchester/logs/runner-<date>.log` (the only runner surface that exists) |
| Activity | check · *Run #128 ready for review* `14:32` | latest background operation or finished event (runner events, desktop jobs) | The object it names (task, pipeline, connection); with 2+ operations, the popover |
| Agents | bot · *Agents* `2/4` | agent slots in use / configured (Project settings › Limits › *Agents working at once*, as the runner reports it) | Tasks › Queue (`Z22wg`) |
| Queued | clock · *Queued* `2` | queued task runs in the project | Tasks › Queue (`Z22wg`) |
| Needs you | message · *Needs you* `2` | runs waiting for the user: plan approvals, questions, reviews — exactly the count of the Overview metric *Need your input* and the Overview *Needs you* heading (decided 2026-09-27). The plain number; `0` in `$subtle`, never `00` or a dash | Overview › Needs you panel |
| Tokens | gauge · *Tokens today* `3.8M / 20M` | counted tokens (input + output, see [Token budgets](#token-budgets)) of all runs in the project today vs. the project daily token budget | The project's [Usage dashboard](#usage-dashboard) (built 2026-10-06, ORC-72); the tooltip names the exact counts |
| Plan limits (`XCXvw`, proposed 2026-09-29, hidden until approved) | hourglass · *Codex* `65%` · *Claude* `42%` | the `account/rateLimits/updated` / `rate_limit_event` data each agent CLI reports with its turns, for the account it is signed in to (see [Plan limits in the status bar](#status-bar)) | Non-interactive (pointer only, no focus); the hint lists each window, its reset, and the turn the numbers came from |
| MCP (`JaMJ7`, added 2026-09-29) | radio · *MCP* `127.0.0.1:52341` | the port the embedded MCP server bound at start (every build; loopback, a free port the OS picks) | Non-interactive (pointer only, no focus); the hint names the endpoint `http://127.0.0.1:<port>/mcp` and says clients read the access token from this process's discovery file (`lurq/mcp/<pid>.json` in the local app data folder). The token is never shown, copied or logged. Shown with and without a project |
| Version | `v0.1.0` | application version | Non-interactive; tooltip with build info |

Not included on purpose: a notifications counter (the Overview top bar already has a bell, and a
notification centre is not designed), the project name (the sidebar and breadcrumb show it), and
per-run progress percentages (runs report steps, not percentages). Background operations that
the activity item can name, each with a real start and end: creating a worktree, publishing a
pipeline/profile/instruction/tool version, checking a connection or CLI (*Checking Codex CLI…*),
importing resources, syncing pull requests, saving before close. Draft saves are local and fast;
they appear only as finished events. Operations are listed across open projects in the popover,
each labelled with its project.

**Activity slot priority**: runner problem > unresolved failure > running operations (one: its
description and elapsed time; two or more: *N operations · <newest>…* with a chevron) > latest
finished event with its time. Elapsed time only — never a fake percentage.

**States** (instances of `pmOpt` with overrides, in `keJ9Z`):

- A · Idle: *Runner online* · check *Run #128 ready for review* `14:32` · Agents `0/4`, Queued `0`, Needs you `1`. Zero counts stay visible in `$subtle` so items never shift.
- B · Runner starting: loader *Starting runner…*; the activity item is hidden; Agents `—` (unknown, not 0).
- C · One operation: loader *Creating worktree for ORC-129…* `4 s`.
- D · Several operations: loader *3 operations · Publishing Feature delivery v4…* with chevron-up; the item is highlighted (`$hover`) while the popover is open.
- E · Reconnecting: refresh *Reconnecting to runner* `attempt 2 of 5 · next in 8 s` · pause *2 runs interrupted · they continue from their last checkpoint*; Agents `—`.
- F · Reconnected: *Runner online* · check *Runner reconnected · 2 interrupted runs continued* `14:08`. It is the latest event until the next one replaces it; no timer.
- G · Offline: unplug *Runner offline* `5 attempts since 14:02` · pause *2 runs interrupted · nothing runs until the runner starts* · actions *Restart runner*, *Open runner log*. Persistent.
- H · Failure: circle-alert *Could not create worktree for ORC-131 · branch orc-131 already exists* `14:41` on a `$raised` fill · *Open task* · dismiss (x). Persists until opened or dismissed; never auto-dismissed. Newer events wait behind it in the popover.
- I · Permission request waiting (later milestone, `OwnMs`): hand *Builder needs permission · ORC-128 · pnpm add …* `9:42 left` on a `$raised` fill · *Open request*; *Needs you* `3`. The countdown runs until the 10-minute timeout; then the slot shows *Request denied · no answer* as the latest event.
- J · Not done (`XlbDa`, added 2026-09-27): circle-alert *Pull requests not synced* `14:52`, icon and label in `$muted`, time in `$subtle`, **no** raised fill, no dismiss, no action. For work that ended without doing what it set out to where the screen it belongs to already says why — a pull request sync GitHub refused (rate limit, wrong account, missing scope, unreachable) and a pull request GitHub did not return (*Pull request #42 not read*). It is the latest event, not a failure: it is never pinned (the Pull requests sync repeats every minute and would pin one each time), the next event replaces it, and a click opens the object (Pull requests). In the popover it is a finished row with the alert icon. A failure (H) is work that could not run at all (storage, the program did not start). *Needs you* shows `0` in `$subtle` here.

**MCP states** (frame `lulpS`, *MCP port · status bar*, in `Tg1WX` at 1616, 660 below the popover `imT2E`; added 2026-09-29). The MCP item `JaMJ7` sits in the right group of `pmOpt` between the right separator and the version, so every instance shows it:

- K · MCP listening, hint open (`ikORn`; bar `Uo7v7`, hint `ejqcH`): radio · *MCP* in `$muted`, the address in Geist Mono 11 `$subtle`. The hint is the status bar hint style drawn for the first time: `$raised`, `$border`, `radius-control`, padding 4/8, Inter 11 `$text` at the font's own line height (1.2), as wide as its text up to 420, 6px above the bar, kept inside the window (flush with its right edge here).
- L · MCP unavailable (`m1KCFv`; bar `lRrZH`): server-off · *MCP unavailable* in `$text`, no address, when the server did not start. Hint: *The MCP server did not start, so agents cannot drive this window. The app log has the reason.* lurq does not hand the reason to the app (its `enable_mcp` returns a handle on port 0 and logs the cause), so the hint points to the log.

`keJ9Z` is now 1123 high; its group `Tg1WX` is 1300 high, so the Focus group `xURvJ` moved to y 3656 and Foundations `cuxUu` is 7576 high. Since the Tabs group `rOh31` (y 3656, 1700 high, 2026-09-28) the Focus group is at y 5452, the app icon group `f6IhTB` at y 9352 and Foundations is 9932 high. Since the plan limits group `poPso` (2026-09-29, below) `Tg1WX` is 3130 high, the Tabs group is at y 5486, the Focus group at y 7282, the app icon group at y 11182 and Foundations is 11762 high.

**Plan limits in the status bar** (proposed 2026-09-29, awaiting the user's approval; frame group `poPso`, *Plan limits · status bar*, in `Tg1WX` at 1616, 1036 below the MCP states `lulpS`). The user wants to see, from anywhere in the app, how much of the Codex and Claude plan limits is used and how much is left. The item `XCXvw` (*Plan limits*) is added to the right group of `pmOpt` right after *Tokens today* and before the right separator, **hidden by default** (`enabled: false`) so no existing screen changes until the proposal is approved; the states below turn it on. Fixture values are illustrative.

- Compact item: hourglass · one entry per agent CLI that the selected project's connections use (two connections of one CLI share the account the CLI is signed in to, so they are one entry), CLI name in Inter 11 `$muted`, the used share of its fullest window in Geist Mono 11 `$text` (5-hour 12 % and weekly 65 % → *Codex 65%*), entries separated by a `$subtle` middle dot. Unknown values are a dash in `$subtle` (Counts rule). Pointer only, no focus, no click target: there is no usage screen to open.
- Hint: the status bar hint style (`$raised`, `$border`, `radius-control`, 6px above the bar, left-aligned with the item and kept inside the window), 360 wide, padding 8, gap 8 because it holds rows. Per CLI: name (Inter 11/500 `$text`) and plan type (`$muted`, Codex only — Claude Code does not report one); each window as a row of label (*5-hour*, *Weekly*, 44 wide), a 100 × 4 meter (`$border` track, `$muted` fill, radius 2), the used percentage (Geist Mono `$text`, right-aligned) and the reset (Geist Mono `$subtle`: *resets in 2 h 10 min* under 24 hours, *resets Mon 09:00* from a day on); credits and extra usage as rows when reported; the source line *From the last Codex turn · 14:26, 8 min ago*. Hairlines between CLIs; the footnote *Plan limits belong to the account each CLI is signed in to and include its use outside Orchester. Tokens today is this project's own daily budget.*
- Near the limit: a window at 80 % or more, or a CLI that says it is warning (Claude Code `allowed_warning`). Grayscale emphasis only: the item icon becomes circle-alert in `$text`, that CLI's name and percentage `$text` (percentage 500); in the hint its meter fill, percentage and reset are `$text` and the heading says *Near the weekly limit*. `$danger` stays reserved for *Unreadable*.
- Reached: the CLI says so (Codex `rateLimitReachedType` set, Claude Code `status` rejected — the rejected value is presumed, not seen yet) or a window reads 100 % without extra usage. The item takes the status bar's existing blocking emphasis (states H and I): `$raised` fill, octagon-pause (the *Stopped at limit* icon) in `$text`, and the reset time instead of the percentage: *Codex limit reached* · `resets 16:40` (*resets Mon 09:00* when a day or more away). The hint names the full window (*· 5-hour limit reached*) and says what the CLI does: *Codex refuses new turns until 16:40.*
- At the limit, no new run behaviour: a run whose agent hits a plan limit ends as it does today (its task shows the stop or failure with the CLI's message; the activity slot shows whatever event that produces). The item only tells, on every screen, when work can start again. When the reset time passes, the used share is no longer known: the entry becomes a dash and the hint row reads *Reset at 16:40 · read again with the next turn* until a turn reports again.
- With extra usage in use (Claude Code `isUsingOverage`) a full window does not block turns, so it gets the near-limit emphasis, not the reached one, and the hint adds *Extra usage · In use: turns past the limit use it*. Codex credits (`credits.has_credits`) show *Credits · 1,250 left* from `balance` as reported (its unit is unverified); `unlimited` reads *Unlimited*.
- Not read yet: no turn of that CLI since the app started. Readings are kept for the session only and not stored, so every start begins here. Hint: *Codex reports its limits with each turn. They appear after the next Codex turn in any open project.* The newest reading of a CLI across all open projects is shown, because the account is the same.
- Not signed in: the connection check found the CLI signed out. Orchester never signs in, so the hint says *Claude Code is not signed in on this computer, so it reports no limits. Sign in to it in a terminal, then test the connection in Connections.* with *Connection checked · 14:02*.
- No working connection: the project has a connection for the CLI but its last check could not start it: *Claude · Kontur could not start Claude Code when it was checked at 14:02, so no limits can be read. Connections shows why.* A CLI without a connection in the project is not listed; a project without any agent connection has no plan limits item, as a window without a project has no project items.

| State | Frame | Bar | Hint |
| --- | --- | --- | --- |
| M · Known, hint open (*Codex 65% · Claude 42%*; Claude's 5-hour *Not reported yet*) | `t8nGQ` | `pMa0F` | `mFTm5` |
| N · Near the limit (*Claude 96%*, weekly, warning) | `e1TK9` | `b5WjRd` | `pNIEg` |
| O · Limit reached (*Codex limit reached · resets 16:40*) | `WuIp9` | `p25jK` | `DCH2x` |
| P · Not read yet (Codex) and not signed in (Claude), *Codex — · Claude —* | `mMpm5` | `mxRJV` | `k7yqJ` |
| Q · Hint rows: no working connection, after a reset, credits and extra usage | `YlYEY` | — | `FNywP`, `A6EsUV`, `b3WD6` |
| R · At 960: normal, and reached with the count labels dropped | `VTnmr` | `A3flo`, `ScSdZ` | — |

**Beside Tokens today, not combined — decision for the user.** Tokens today is this project's own daily budget: Orchester counts it and stops runs at it. Plan limits belong to the CLI's account, are counted by the provider, include use outside Orchester and are the same in every project. One item for both would suggest one number explains the other. They sit next to each other so *how much can I still run* reads in one place, and the hint's footnote draws the line. Width: the normal item is 173 px (the right group grows from 685 to 858 at 1440; the activity keeps 566), the reached item 303 px. At the minimum width of 960 the narrow-window order is extended: drop the version, then *Tokens today*, then the count labels; the plan limits item stays because nothing else shows plan limits, while *Tokens today* is also the Overview metric. The normal item fits at 960 with every label (the activity keeps about 210 px); the reached item needs the count labels dropped (R).

**Data sources.**

- Codex (every step runs through App Server): App Server sends `account/rateLimits/updated` during a turn, e.g. `{"rateLimits":{"limitId":"codex","primary":{"usedPercent":65,"windowDurationMins":10080,"resetsAt":<unix>},"secondary":null,"credits":{"hasCredits":false,"unlimited":false,"balance":"0"},"planType":"pro","rateLimitReachedType":null}}` (v2 schema of codex-cli 0.159.2). The update is sparse: what it leaves out keeps its last value. `primary` and `secondary` are windows, either may be null; a window without `windowDurationMins` cannot be labelled and is not shown; 300 minutes is labelled *5-hour* and 10080 *Weekly* (another length would read *N-hour* / *N-day*). Only the `codex` bucket (`limitId`) is the plan; other buckets meter one model and are not shown. `rateLimitReachedType` names why turns are refused, not a window: `rate_limit_reached` (lifts at the reset), depleted workspace credits or a reached workspace usage limit (a reset does not lift them). The values are as of the last Codex turn. App Server also answers `account/rateLimits/read` outside a run; that is not designed or used.
- Claude Code (verified on 2.1.89 signed in with claude.ai, `claude -p … --output-format stream-json --verbose`): the stream carries `{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":<unix>,"rateLimitType":"seven_day","utilization":0.96,"isUsingOverage":false,"surpassedThreshold":0.75}}`. `rateLimitType` names the window (`seven_day`; `five_hour` expected), `utilization` is a 0–1 fraction, `status` is `allowed`, `allowed_warning` or (presumably) `rejected`. One event may cover only one window, so the other reads *Not reported yet*. No plan type is reported. Also as of the last Claude Code turn. `utilization`, `resetsAt` and `rateLimitType` are left out when the CLI does not know them, and `rateLimitType` may be null; a window without its use shows its status without a percentage, and a rejection without its window still reads as reached. Windows of one model or kind of use (`seven_day_opus`, `seven_day_sonnet`, `seven_day_overage_included`, `seven_day_oauth_apps`, `seven_day_cowork`, seen in 2.1.286) are not the plan and are not shown. Not known: whether an API-key login emits the event at all.
- Both arrive only with agent turns; the hint always says from which turn and how old the numbers are. Orchester never estimates or extrapolates them.

Proposed copy namespace: `statusBar.planLimits.*`.

**Operations popover** (`E1msJS`, anchored example `imT2E`). Opens upward from the activity
item, left-aligned with it, 6px above the bar; 420px wide, `$raised`, `$border`, 12px radius,
soft shadow. Header *Background operations* with the count. Running rows: loader, title, meta
line (project · object · what clicking opens), elapsed time, chevron. *RECENT* lists finished
events newest first (unresolved failures are pinned at the top of this list with a dismiss x).
Footer: *Finished operations stay here until the app closes* and *Open runner log*. Escape or a
click outside closes it and returns focus to the activity item. Finished-event retention beyond
the session is not designed.

**Runner card and notices — recommendation for the product owner.**

- Sidebar runner card (`O2Jjjf` in `zo9Py`, states `O5NoB1`): remove it once the status bar
  ships. It repeats the runner state and slot count that the bar shows on every screen, and it
  costs about 70px at the bottom of the navigation. The existing components are not edited by
  this proposal.
- Reconnecting notice (02.F `MAn7V`): drop the top-of-content notice; the bar (state E) carries
  it. Reconnecting is automatic and needs no decision, and interrupted task rows still show
  *Interrupted* in the lists.
- Reconnected notice (02.G `YHS1v`): drop it; it becomes the latest event (state F).
- Could not restart (02.H `EFY7I`): keep the inline notice on the page — it blocks all work and
  needs a decision (*Restart runner*). The bar (state G) repeats the actions so they are reachable
  from every screen, not only the Overview.

Rule: the status bar is for ambient state; an inline notice is only for something that needs the
user's decision.

**Narrow windows.** Drop the version first, then the token item, then labels (icon + count only); the plan limits item (proposed) is never dropped. Built (2026-10-03) as a give-way order of the whole bar's one line, after the left group's optional details and before its activity and counts, not at fixed widths (see *A request whose agent is gone* under Approval choices). The
activity text truncates with an ellipsis; its tooltip holds the full text.

Proposed copy namespace: `statusBar.*` (`statusBar.runner.*`, `statusBar.activity.*`,
`statusBar.operations.*`). Fixture values (times, counts, amounts, v0.1.0) are illustrative.

## Focus

Proposed 2026-09-24 for keyboard navigation; narrowed 2026-09-25 to forms.

**Only form controls take focus; everything outside forms is pointer-only.** Form controls are
text fields, text areas, selects, checkboxes, toggles and switches, option cards, and the buttons
of a form or dialog (dialog Cancel / Save / Create / Close, the new task form, project setup, the
agent profile's fields, the step and pipeline inspector with Duplicate / Remove step). Tab and
Shift+Tab move only between them, in the order they appear, and wrap; dialogs trap Tab. The close
dialog, which has only buttons, keeps its buttons focusable so it can be answered by keyboard.
Everything else is not focusable: no Tab stop, no ring, no focused style. That covers sidebar
navigation items (sidebar navigation items are not focusable), tabs, list rows, pipeline canvas
step cards and the add-first-step slot, toolbars, topbars, breadcrumbs and back arrows, page
actions outside forms (New task, New pipeline, Add connection, Publish, Settings, View all tasks,
the agent editor's Save draft and Publish), the status bar and the operations popover. A click on
a pointer-only control acts without taking focus and leaves the focus where it is; a click on an
inert item (selected or unavailable navigation item, empty space) clears focus. Escape still
closes the operations popover.

Exception: tabs inside a dialog are controls of the dialog's form. Import resources' kind tabs
(`RDOlV`) are Tab stops between the source select and the search, the selected one included, with
the outer ring; the selected tab keeps its underline and the ring is added. Enter or Space selects
the focused tab, which keeps the focus. An unavailable tab (Secrets) is no stop.

lurq has one focused state for mouse and keyboard focus. A click on a form control focuses it and
shows the ring. Canvas group `xURvJ` ("Interaction · Focus") in Foundations, below the status bar
group:

| Node | Content |
| --- | --- |
| `Ayb2K` | Specimen board: each control in default / hover / focused / focused + hover / selected / focused + selected, with its rule. The pointer-only rows (navigation item, tab, list row, pipeline step card, popover row, status bar item) show "Not focusable" and no ring in the focused cells |
| `Oz9EN` | Anatomy at 4×, values and combination rules |
| `X9gPDr` | `Focus / Ring`, the overlay that draws the ring in Pencil (gap child `N0nV4`). Design-only: implement it as box shadows, not as an element |
| `CfdDd` | Example: dialog primary button focused (Create draft), from 03.05. The name field is unfocused here |
| `g3poI` | Select menu board: components, option states, trigger and menu, values (see [Select menu](#select-menu)) |
| `kdxtI` | Example: Add connection dialog, *Provider / adapter* open with *Claude Code CLI* selected, from 04.02 `h6CmJ` |

The examples are copies. The canonical screens are unchanged and show no focused control.

**Rule.** Focus is a 2px light ring behind a 2px dark gap. It is paint-only (box shadows), so it
never moves layout and never replaces a border. It is the only use of a box shadow on these
controls. Hover, pressed and selected styles change fill, border and text. They never set a shadow,
so they cannot remove the ring.

| Variable | Value | Use |
| --- | --- | --- |
| `focus-ring` | `#EEEEEE` | ring colour |
| `focus-gap` | `#101010` | gap colour (same as `canvas`). It separates the ring from light fills (primary `#E6E6E6`) and from selection strokes |
| `focus-width` | `2` | ring and gap thickness; spread of the first shadow |
| `focus-extent` | `4` | gap + ring; spread of the second shadow |

- **Outer ring** (default): `0 0 0 2px $focus-gap, 0 0 0 4px $focus-ring`, first shadow on top,
  blur 0, offset 0. The corners follow the control radius (r + 2, r + 4). It needs 4px of unclipped
  room around the control.
- **Inset ring** (where there is no room): `inset 0 0 0 2px $focus-ring, inset 0 0 0 4px $focus-gap`.
  The ring sits on the control edge and the gap is inside it, at radius r and r − 2. It paints above
  the fill and below border and content.

| Control | Ring | Selected look (unchanged) |
| --- | --- | --- |
| Button primary / secondary (`u1XBP`) in a form or dialog | outer, radius 11 | — |
| Icon button (32 × 32, no fill) in a form or dialog | outer, radius 11 | — |
| Button or icon button outside forms (toolbar, topbar, page action) | none: not focusable | — |
| Navigation item (`TSJpn`) | none: not focusable | `$hover` fill, `$text` label |
| Pipeline step card (`ajikA`) | none: not focusable | 1.5 inside `$text` stroke |
| Option card (new pipeline "Start from") | outer, radius 11; none while selected | 1.5 inside `$text` stroke + radio dot |
| Tab (`SOW6S`) | none: not focusable | 2px `$text` bottom underline |
| Window tab (`nAKyb`), new-tab and tab list buttons | none: not focusable | `$surface` fill (active tab) |
| List row (e.g. `SGjO0`, `clVjO`, `QHBNZ`, `hn8qO`, `sFtAz`, agent list `jNTVN`) | none: not focusable | — |
| Popover row (`E1msJS`), its dismiss control and Open runner log | none: not focusable | — |
| Status bar item (`pmOpt`) | none: not focusable | `$hover` fill while its popover is open |
| Toggle row in a form (step "runs after", approval) | inset, radius of the row | — |
| Field / select (`ZGwlv`) | none: keeps `o8Oug` (1.5 inside `$text`) and the caret | — |
| Select option (`L20XvS`, keyboard highlight) | inset, radius 7 | trailing check, no fill |

Combinations:

- Focused + hover: the hover look plus the ring.
- Focused + pressed: the pressed look plus the ring.
- Focused + selected: the selection look stays and the ring is added (outside it for outer
  controls, on the edge for inset controls) when the selection is a fill or an underline.
- Selection drawn as a stroke hides the focus ring (decided 2026-09-25). This applies to
  stroke-selected form controls: a selected option card shows only its 1.5 `$text` stroke,
  focused or not, since stroke, dark gap and ring read as a double border. It stays a Tab stop,
  and shows the ring when focused while not selected. The pipeline step card also draws its
  selection as a stroke but is outside forms and never focusable.
- Disabled controls are not focusable and show no ring.

Containers: an outer ring must not be cut by a clipping ancestor. Scroll viewports and clipped
panels that hold outer-ring controls keep at least 4px (`focus-extent`) of inner padding on
their clipped edges. If they cannot, the control uses the inset ring. Do not add padding
just for focus. A focusable row in a form needs horizontal padding of at least 12px so the inset
ring does not touch its content. List rows outside forms are not focusable; the 12px of the
pipeline library row `hn8qO` and its column headings in `LRMQW` stay as designed.

Implementation (lurq 0.24): `focused_style(Style::new().box_shadow([...]))` with two
`BoxShadow::new(0.0, 0.0, 0.0, color).spread(n)` values (`.inset()` for the inset variant). The
colours are two new `ExtraColor` roles (`FocusRing`, `FocusGap`). lurq merges state styles in the
order focused → hovered → active, so hover and active styles must not set `box_shadow`. It replaces the
field-style focused border (`Border::inside(BorderSize::Sm, border_focus)`) that buttons use today. Pencil cannot render a crisp spread shadow,
so the design draws the ring with the `Focus / Ring` overlay (`X9gPDr`). Its geometry equals the
shadow: control + 4px on each side for outer, the control bounds for inset.

### Select menu

Proposed 2026-09-25 for the open state of `Field / Select` (`ZGwlv`). Board `g3poI` in
`xURvJ`; the *Selected* and *Focused + selected* cells of the `ZGwlv` specimen row (`U8mZ0s`) now
show the menu open with the pointer (`w0KYb`) and with the keyboard (`MUhOt`); worked example
`kdxtI` (Add connection, *Provider / adapter* open, *Claude Code CLI* selected; the canonical
04.02 `h6CmJ` is unchanged and already uses the lucide `chevron-down`).

| Part | Value |
| --- | --- |
| Menu (`u0ByM`) | `$raised` fill, 1px `$border`, radius 11 (option radius 7 + padding 4), padding 4, no gap between options, clipped |
| Menu shadow | `0 8 24 #00000066`, the same elevation shadow as the operations popover `E1msJS`. Only floating layers (popover, select menu) carry it; controls never do (the focus ring is their only shadow) |
| Placement | 4px below the trigger, same width as the trigger; flips above when there is no room below; stays inside the window, above dialogs (not clipped by the dialog) |
| Max height | 8 options: 266px including the border. Longer lists scroll inside the menu with an overlay thumb and no track; the highlight is scrolled into view |
| Scroll thumb (`YD8PV`) | 4px wide, radius 2, `$border`, drawn over the options. Its right edge is 3px from the menu's outer edge (2px clear of the 1px border). It runs only along the straight part of the edge: 11px from the top and bottom outer edges (the corner radius), so it never touches the border or the rounded corners. Track = menu height − 22; length = track × visible / total, at least 24px. Example: 12 options in 266px give a 163px thumb at y 11 |
| Option (`L20XvS`) | 32px high, padding 0 8, gap 8, radius 7 (`$radius-control`). Label Inter 12 `$text`, the field text; with the menu padding its text lines up with the trigger text 12px from the edge |
| Check | lucide `check` 14 `$text` in a trailing slot that is always reserved (labels never shift). It sits exactly under the trigger chevron |
| Hovered | `$hover` fill (pointer only) |
| Selected | check, no fill. Fill always means "pointer or highlight is here", never "this is the value" |
| Selected + hovered | check + `$hover` fill |
| Keyboard highlight | inset focus ring (`inset 0 0 0 2 $focus-ring, inset 0 0 0 4 $focus-gap`), radius 7 / gap 5, no fill, the same as a focused popover row (`ncCFU`). With hover: fill + ring |
| Highlight + selected | check + inset ring |
| Disabled option | label `$subtle`; optional reason Inter 11 `$subtle` before the check slot (e.g. *Not found on PATH*); no hover, no highlight, skipped by arrows and type-ahead |
| Trigger while open | keeps the focused stroke `o8Oug` (1.5 inside `$text`); lucide `chevron-up` 14 `$muted` (closed: `chevron-down`) |

Behaviour: Enter, Space, Alt+Down or Down on the focused trigger opens the menu with the highlight
on the selected option (first enabled option if none). A pointer click opens it with no highlight;
the first arrow key places it on the selected option. Up/Down/Home/End and type-ahead move the
highlight over enabled options; Enter or a click selects and closes; Escape, Tab or a click outside
closes without a change. Focus returns to the trigger.

Implementation (lurq `SelectStyle`), replacing the current fallback (option padding 12, selected =
`$hover` fill, keyboard highlight = `$hover` + field stroke, text triangle chevron):

- `menu`: fill raised, `Border::inside(1, border)`, radius 11, padding 4, the popover's elevation
  shadow; offset 4 below the trigger; max height 266.
- `option`: height 32, padding 0 8, radius 7 (`radius-control`), field text style, no fill.
- `option_hovered`: fill hover.
- `option_selected`: no fill, check visible.
- `option_selected_hovered`: fill hover, check visible.
- keyboard highlight: the inset focus box shadows (`FocusRing`, `FocusGap`, `.inset()`), no fill,
  no border; combined with the hover fill when both apply. Not the field stroke: the stroke is the
  trigger's focus, and the ring is how every row-like item in Orchester shows focus.
- `chevron`: lucide `chevron-down` 14 `$muted`, `chevron-up` while open (an icon, not a text glyph).
- `checkmark`: lucide `check` 14 `$text`, trailing, slot reserved on every option.
- Disabled option: text subtle, optional reason text 11 subtle, excluded from hover and highlight.

## Core workflow

- New task collects a brief, acceptance criteria, context, pipeline and limits in its selected project. Project, agent-role and pipeline/step instructions are included automatically. Repository files are optional context, not the primary rules interface.
- Create & run submits once and displays pending feedback; retry must not create a duplicate run. Draft save and run creation have separate states.
- The sample pipeline is Research → Plan → Implement → Run checks and Review in parallel → Create pull request. Explorer researches, Planner proposes the plan, Builder implements and Reviewer independently reviews. Plan approval occurs before implementation; the user reviews the PR before merging. Failed verification returns to Implement, up to the configured retry limit.
- The task page binds evidence to an exact revision. New code invalidates stale evidence. Review actions expose changes; a report alone is not proof of implemented acceptance.
- Run snapshots retain effective pipeline, agent-profile, model, instruction and tool versions, explicit step overrides and context. Runner loss preserves checkpoints and exposes recovery; it must not manufacture progress or completion.
- Pipelines, profiles, instructions and tools share Draft saved → Publish vN → Published vN. Saving a draft does not publish it. The pipeline editor shows draft v4, published v3 and Run v3; tool setup shows draft v2 and published v1. Publishing affects new runs. Dragging, keyboard selection, undo and persistence remain implementation work.

**New task as built (drawn 2026-09-29 for D4 and D5, `F3fEH`).** Nothing on the form promises what the chosen pipeline does not do. The description reads *Describe the outcome and how to check it. The selected pipeline defines the steps.* Each step shows its name and its executor (*Explorer · Claude / Opus*, *Tool · Run tests*); a step with an approval gate adds *· waits for approval*. *View pipeline ↗* opens the pipeline's published version in a new tab. The approval card (hand) names the gates: *Approval is required for Plan.*, *Approval is required for Plan, Review.* or *This pipeline has no approval gates.* Context offers only *Attach context*, unavailable (0.4, trailing lock), and the footer *Project, agent and pipeline instructions are included automatically.* has *Preview* unavailable (0.4, lock) at its right. A criterion's remove control (x) shows while the row is hovered or the control has focus. The time limit is a unit input (*30* · *minutes*); the budget and the time limit are each optional — an empty field reads back *No limit* and saves no limit (user decision 2026-10-09). *Create* (decided 2026-10-02, replacing *Save as draft*) is a small muted text action under Create & run, in the same place and style: it saves the task as a draft without running it, with the same validation (unavailable without a title or with invalid limits, which the reasons under the actions say), and returns to Tasks in the same tab — back to the list entry the form was opened from, on its tab, or in the form's place when it was opened elsewhere, so the form is not visited again. Tasks then scrolls the new task's row into view and highlights it in `$hover` while the list stays shown. The task is a draft (status *Draft*) and is run later from its page. Enter in the form is Create. The topbar says what the form holds: *Not saved yet*, *Creating the task…* (the action reads the same while it works), *Task queued*, *Starting the run…*; *Draft saved locally* is Edit task's. 02.09A `m82tU`: Feature delivery v3 with the Plan gate (drawn as a saved draft that stays on the form, which New task no longer shows). 02.09B `v3cdq`: not saved yet, Quick fix v2 without gates, a criterion hovered. Kept from 02.09 as a proposal: the project field shows the linked repository and base branch.

**A queued task whose run did not start (D5).** Create & run saves and queues the task before the runner checks the run; when the runner refuses to start it (for example when the repository has no commit to start from), the form stays on the same task: the topbar reads *Queued · run not started*, the description *This task is saved and queued. Its details cannot be edited here. Fix the issue below, then retry the run.*, the fields and the pipeline are fixed (the pipeline is a value box without a chevron), *Create* is gone and the primary reads *Retry run* (play). The reason is a field error under it: circle-alert 12 in a 16.5 slot, 4 from Inter 11/1.5 `$text`, wrapping in the column, in the runner's words, for example *The repository has no commit to start the run from.* (02.09C `WOdDe`; 02.09D, a low free-space state, was removed on 2026-10-02 with the Cargo build-folder check). *Retry run* starts the same task under the same run key: while it waits the topbar and the button read *Starting the run…* (the button with a loader, 0.4) and *Retrying the same run request with the runner.* is under it (02.09E `l801hD`). When the runner stops answering, the topbar reads *Task saved · run status unconfirmed* with its own description, and retrying never starts a second run. A long error should scroll the plan above the pinned actions with the [scroll gutter](#components-and-tokens) instead of pushing them off the screen.

**Reopened queued task without a run (D5, 02.09F `T6bVyU`).** Opening the task again after leaving its form or restarting Orchester uses the same fixed task details and pinned pipeline, with *Retry run* in the primary action and no Save draft action. The breadcrumb is *Kontur / Tasks / ORC-140 / Retry run* (ORC-140 is fixture data); the topbar is *Queued · run not started*. The heading stays *What would you like to build?* and its description is *This task is saved and queued. No run is recorded. Its details are locked; retry to ask the runner to start it.* Under the primary action, a muted clock and note replace the old error: *No run is recorded for this task. Retry uses the saved settings and the same start request, so it cannot create a second run.* The previous specific failure is not persisted and is not reconstructed or claimed. The saved 1440 × 960 Pencil frame exports at 1.5× as the 2160 × 1440 preview; `snapshot_layout` reported no clipped or overlapping nodes.

**A queued task without a run that the person's form did not queue (decided 2026-10-07).** Automation can queue a task through Orchester's MCP server (`run_task` on a draft); when the runner then does not start the run, the task is queued without a run, as in D5, but the person never filled a form for it. Opening it in the locked form read as if automation had put the person on New task. So 02.09F is kept only for a task this window's New task form queued in this session (leaving the form and opening the task again): any other queued task without a run, including the person's own after Orchester restarted (nothing stored says who queued it), opens on its task page. The page reads *Queued · its run has not started* under the title (as before the redirect existed), its Summary is the empty Summary before the first run (ORC-25, `Xxrdy`, see *Task Details and the empty Summary* below) with *Start run*, its notice reads *The run has not started* · *The task is queued, and no run is recorded for it. Start run asks the runner to start it with the saved settings; it cannot start a second run.*, and the topbar has the primary *Start run* (play, `kh5Er`; it read *Retry run* before ORC-25) where run controls go: it asks the runner to start the task's first run under the task's first-run key, as the form's Retry run does, so it never starts a second run; while it waits it reads *Starting the run…* with a loader, unavailable, and the runner's refusal shows as the field error under the heading, as a run control's does. While unfinished blocking tasks hold the task, *Start run* is unavailable and the Blocked notice says why.

**Task Details and the empty Summary (ORC-25, 2026-10-18; Pencil section `vShIt`: `UxsRH`, `R3Wkvd`, `ZSiHd`, `Dxn8C`, `xTGz2`, `Xxrdy`).** A task page has a *Details* tab beside *Summary*: the complete title, description (Markdown, one body line between paragraphs as `ZSiHd` draws them) and numbered acceptance criteria, either as recorded when the task was created (kept unchanged through later edits, `task_creation_snapshots`) or as they are now; a task created before creation snapshots existed falls back to what its first run started with, and says so. Before a task's first run its Summary is the empty Summary (`Xxrdy`): the header's run line reads *No runs yet*, the Summary says so with *Start run* (the same control as the topbar's), and no run panels show. A task kept Not shared (N3) skips the empty Summary and shows its read-only Summary (`R4elt`); a never-run task of a shared project keeps the *This task* panel beside the empty Summary (a state `Xxrdy` does not draw). Known gap: a shared task whose run a teammate reported shows as run while the server reports it, but the server forgets the run status when the claim is released, so such a task can read *No runs yet* here afterwards; fixing it needs a server-side fact that the task ran.

**Changes made outside the window (decided 2026-10-07).** Orchester's MCP server changes data while a person may be looking at it. The window shows the new data in place and never switches the page, resets or closes what is open, or puts a failure or attention mark in front of the person for a tool's call; a tool's failure is in its result only. Only the changed project's data is read again. When reading a project's tasks again fails, every page keeps the tasks last read (the overview, the task list, its empty state, Edit task, the locked retry form, New task's pipelines) and the project shell says so above whichever page is shown, 16 below the page's top and 32 from its sides, in the inline notice (`q5LzI`, circle-alert): *The tasks could not be read again. The list shows them as they were last read.* with the reason and *Retry*. The pipeline library does the same for its pipelines, without Retry, 16 above its filters and clear of the rows' scroll gutter; a task page and Project settings › Auto mode, Code index and Limits keep what they show and give the reason as a field error under their heading, 16 below what is above it. An Edit task form without edits takes the newer version; one with edits keeps them and shows the inline notice under the heading, refresh glyph, *Changed outside this form* and *ORC-7 was saved elsewhere, for example by automation, while you edited it. Your edits are kept: saving them keeps the other changes, or shows where both changed the same field. Reload shows the task as stored and discards your edits.*, with *Reload*; a save is made on the version the fields were filled from, so the newer one is merged or shown as a conflict (10.41), never reverted silently. Project settings › Auto mode, Code index and Limits each show a change made elsewhere to their own setting at once. One that comes while the person has a change of that setting under way (being saved, or typed in a Limits field and not saved yet) keeps what the person sees and shows *Changed outside this screen* (*These settings were changed elsewhere, for example by automation, while you were changing them. Saving your change keeps theirs. Reload shows the settings as they are stored now.*) with *Reload*; once the person's change is saved the notice goes and the section shows what is stored. A save changes only what the person changed, read and written in one transaction with what is stored; when Limits no longer fit together that way, nothing is saved and the topbar says *Not saved: the limits were changed elsewhere meanwhile, and together with your change they no longer fit. Reload shows them as stored.* until the tab shows the stored limits again (*Reload*, or the next change made elsewhere). In Limits the notice keeps the section's 24 above *Token budgets* (`c7e729`). While automation removes a run's worktree, that run's open Remove worktree dialog and its row's button read *Removing…* as for a person's removal, and the dialog stays open after it: *The worktree of run #12 is removed* and *It was removed elsewhere, for example by automation, while this dialog was open. Committed work stays on its branches; there is nothing left to remove.*, with *Close* as its only action. A removal automation asked for that fails leaves no event in the status bar and no mark on the tab: its reason is in the tool's result only. A task automation creates appears in the task list in place; the list keeps the first row in view where it was while the list scrolls, but when every row fits without scrolling a new row above the pointer moves the rows below it down by one row (there is no scroll range to absorb it). No frames yet for these notices.

The task creation fixture includes four acceptance criteria, including cancelled OAuth, and shows the six steps from published Feature delivery v3. Create & run leads to research and planning; it does not bypass plan approval. The plan screen exposes plan v1, scope and acceptance criteria, with Approve plan & continue and Request changes. Requested changes return to Planner; a revised plan requires a new decision.

Implemented (M7, 2026-09-27): any agent step with an approval gate proposes its result as plan vN of that step and the run waits for the user (`docs/architecture/storage.md`, "Approval gates"). 02.11 is the task's Summary while a plan waits: the plan is the agent's own text rendered as Markdown under *Plan vN · <task title>* (the design's structured rows and scope are fixture content); *Request changes* needs a note (disabled while it is empty) and resumes the gated step's agent with it; with several versions *Plan version* becomes a select, where earlier versions read without a decision. Pause is not offered while a run only waits. The Overview's *Needs you* panel shows a waiting plan as a review card (`dpeD7`: *REVIEW PLAN*, the task, *Review plan*), and the status bar's latest event reads *Plan ready for review · ORC-12* and opens the task.

**Who decided a plan (2026-10-08, at the user's request; no frame draws it).** A plan can be decided through Orchester's MCP server (`decide_plan`, and a change requested with `send_message` while a plan waits) as well as on the plan card and the plan review. The run records who decided — the person or automation — with the decision, and a decision automation made says so where a decision is shown, in the words of an answered tool request's card (*Approved by automation*): the plan card's state reads *Approved by automation* or *Changes requested by automation* (check / undo-2, as the person's), the plan review's *Plan version* reads *v1 · approved by automation* or *v1 · changes requested by automation*, and an earlier version's note reads *Automation asked: <note>*. A person's decisions read as before (*Approved*, *Changes requested*, *v1 · approved*, *You asked: <note>*). Same text and status label roles as the person's; no new theme roles. Decisions recorded before who decided was kept read as the person's. The step rail keeps *approved* without naming who.

A run that reaches its time or token budget stops its agents, commits a checkpoint and waits as *Stopped at limit* until the user raises the limit ([Stopped at limit](#stopped-at-limit)). Pause first settles at a safe checkpoint and stops new scheduling/tool calls; only then may the UI claim Paused. Resume continues the same run and version snapshot. Cancel stops new scheduling and preserves work and artifacts; it does not undo completed external operations, and unresolved in-flight outcomes remain visible. Retry-limit recovery offers one explicitly authorized additional attempt within the remaining budget or revision of the task first. As built (2026-10-02): a run whose step used up its retries, or broke again after its automatic restart, waits for a person (*Needs you*, task status *Needs input*) instead of failing; 02.B shows its evidence with the panel's decisions live — *Allow 1 retry & continue*, a *Return the work to* step choice with a note for its agent and *Return to {step} with note*, and *Stop and fail the run*. A run that failed after its retries (stopped, or recorded before) shows the evidence without the panel. Drawn in `O4vgKt` as 02.B3 `FjCWv` (retries used up), 02.B4 `K7hbj` (a step chosen, Implement by default, and a note), 02.B5 `m3pDW` (broken twice), 02.B6 `NM5My` (*Needs you* in Overview, *ORC-128 · Review still fails after 2 retries*, and the status bar, *Review needs your decision · ORC-128* with a hand until the task is opened or the item dismissed) and 02.B7 `zyusO` (removed 2026-10-04; now C·07 `o0KDAq` and C·13 `j0qLQz`) (the Activity transcript's marks for an automatic return, an automatic restart and your return with a note, each a row of its own before the step header it starts; the runner's refusal above *Stop and fail the run*). The exhausted example is an alternative Run 129 / Attempt 3; the normal Run 128 shows intermediate checkpoint `4b92ad0` and final reviewed revision `8f3a2c1`.

**02.B2 · Another attempt not available yet (`z2xET`, drawn 2026-09-27 from the app; superseded 2026-10-02, when retry recovery shipped).** Until retry recovery ships, 02.B is shown with its actions visibly unavailable rather than hidden or faked. The run has failed, so the top bar has no run controls. Evidence: notice *Run checks still fails after 2 retries* · *Automatic execution has stopped. Review what the latest attempt left behind.*; *Latest attempt · 6a71bd4* with the progress rows of the steps that ran in it (*Complete · attempt 3 · 96k tokens · 7 min*, *Reported failed · attempt 3 · 18k tokens · 2 min*); *What Run checks reported* with the agent's own words in mono; *Open verification report* disabled (0.4); *Changes and messages are preserved* · *The work of every step stays committed on orc/github-auth. Nothing was merged into your branch.* Panel *Allow another attempt*: *Tokens used*, *Remaining budget*, then *Additional attempts* and *Resume from* as unavailable fields (`$sidebar`, value in `$subtle`, a 12px lock where the chevron was), the sentence *Allowing another attempt is not available in this version yet. The run stays failed and its work stays on its branch.*, and *Allow 1 retry & continue* / *Revise plan first* disabled (0.4). The *You can revise the plan…* line of 02.B is not shown. When retry recovery ships, 02.B (`J3JZaT`) applies unchanged.

**A run that waits for a person (02.B3–02.B7, drawn 2026-10-02).** User decisions 2026-10-02: when Check, Review or Verify fails, the run returns the work to Implement by itself, up to the step's retry limit, and the retried step receives the failure report. When the retries are used up, or a step breaks again after its one automatic restart, the run no longer fails: it waits for a person (*Needs you*, task status *Needs input*). These frames are the final drawing of that state, which branch `claude/retry-feedback` built on 02.B with existing components. For the waiting state they replace the panel `Lk43S` of 02.B. 02.B and 02.B2 stay as they were drawn. They are in `O4vgKt` (now 3244 high), in rows 2 and 3 at y 1148 and 2196; the fixture is Run 129 of ORC-128 with a retry limit of 2. To make room, `y8TKW8`, `Z7Fyr8`, `BnxJ1` and `nNtAZ` moved down 2124 within 02.4 `U0KbN` (now 15968 high), and the sections 02.5 `yXEwR` and 09 `yqO5T` below it moved down 2124 as well.

- **02.B3 `FjCWv`: retries used up.** Review reported failed in attempts 1, 2 and 3. The top bar keeps *Open conversation* and *Cancel run*, because a waiting run can still be cancelled. The page description reads *Run 129 · Attempt 3 · Automatic retries exhausted*. The evidence column (`Q4LnK`) is as built, like 02.B2: notice *Review still fails after 2 retries*, *Latest attempt · 6a71bd4*, one row for each step that ran in the attempt (Implement, Run checks, Review), the box *What Review reported* with the reviewer's findings in mono, *Open verification report* unavailable (0.4) and *Changes and messages are preserved*. The panel *Allow another attempt* (`Bi6pU`, `$sidebar`, 360 wide, padding 24, gap 22) holds *Tokens used* and *Remaining budget* (8 apart), then *Additional attempts* (*1 additional attempt*) and *Resume from* (*Implement · latest checkpoint*). These two are plain value boxes (Field / Select with no chevron), 16 apart, followed by the sentence *Implement receives what Review reported. The steps after it run again on the new work.* (12/1.7 `$muted`). Next comes the primary *Allow 1 retry & continue* (refresh-cw, `$accent`). After it is *Return the work to*: a step select (*Research*, the first step the work can return to, as built), 9 apart from a note box (the plan-review text area: `$surface`, 1 `$border`, radius 8, padding 14, 76 high, placeholder *What should change before the next attempt*). Below the note box is *Return to Research with note*, unavailable (0.4) while the note is blank. A spacer follows, and *Stop and fail the run* (square) sits at the bottom. *Revise plan first* and the *You can revise the plan…* line are gone.
- **02.B4 `K7hbj`: a step chosen and a note typed.** The select reads *Implement*. The note box has focus (1.5 `$text`) and holds *Store the OAuth state before the redirect and keep the email provider unchanged.* in `$text`. *Return to Implement with note* is available. Retry stays the primary action.
- **02.B5 `m3pDW`: broke twice.** The page description reads *Run 129 · Attempt 2 · Implement stopped again without a result*. The notice *Implement stopped again without a result* says *Its agent or command ended without a verdict on the work, also after a fresh restart. Review what the latest attempt left behind.* Under *Latest attempt · 4b92ad0* is one row (*Implement · Builder*, *Failed · attempt 2 · 41k tokens · 6 min*). There is no *What … reported* box, because a breakage leaves no report. The panel's sentence reads *Implement starts again as a fresh session from the latest checkpoint.*; the rest is as in 02.B3.
- **02.B6 `NM5My`: Needs you (1440 × 988, with the status bar).** This is the Overview on its *Needs you* tab (2). The task row reads *Add GitHub authentication* · *ORC-128 · Waiting for you* · *Needs input* (message-circle), the same as the row of a plan that waits. The metric *Need your input* reads 2. The *Needs you* panel shows the waiting run as an open-decision card (`Mt22P` pattern): *NEEDS INPUT* · *Add GitHub authentication* · *ORC-128 · Review still fails after 2 retries* · *Open task →*. It sits under the plan review card of ORC-134. Recent activity reads *Run #129 needs your input*. In the status bar, *Needs you* reads 2 and the activity slot (`$raised`, hand) reads *Review needs your decision · ORC-128* `14:41` with *Open*, which opens the task's Summary.
- **02.B7 `zyusO` (removed 2026-10-04; now C·07 `o0KDAq` and C·13 `j0qLQz`): timeline marks (compact).** A return or restart is a row of its own between the step headers in the merged Conversation timeline (ORC-61, `fTfl8` (removed 2026-10-04; now `A5H6ln`)). It uses the problem row's Activity / Transcript entry (`N4SlQq`) without the time gutter and never folds into a tool group. The Tools view lists the same rows. The rows are:
  - *Returned to Implement · Review failed (attempt 2)*: undo-2 `$muted`, meta *automatic · retry 2 of 2 · 14:39*, *Builder receives what Review reported (2 findings). Run checks and Review run again on the new work.*
  - *Restarted Implement · it stopped without a result*: rotate-ccw `$muted`, meta *automatic · attempt 2 · 12:16*, *A fresh session from checkpoint 4b92ad0. If it stops again without a result, the run waits for you.* It follows the problem row that says why.
  - *Returned to Implement with your note*: undo-2 `$text`, meta *by you · 14:52*, with the note quoted in `$text`.

  Each row is followed by the step header of the new attempt. The board also shows the panel's refusal: the runner's reason (*The run does not wait for your decision any more.*) is a field error (circle-alert 12, Inter 11/1.5 `$text`) directly above *Stop and fail the run*.

Decided 2026-10-02: the Overview card names why the run waits (*ORC-128 · Review still fails after 2 retries*), the status bar shows *Review needs your decision · ORC-128*, the three timeline marks are built as drawn, and *Return the work to* defaults to the step a retry would resume (*Implement*), not the first step; 02.B3 and 02.B5 still draw *Research* there and the text wins.

## Pipelines library, creation and publishing

Pipelines in the navigation opens the library (`LRMQW`), not an editor. It lists the project's pipelines with name, step summary, step count, version state, the number of tasks that use it and the last publish date. Version state is always a status label with an icon: *Published v1*, *v3 · Draft v4 saved* (a published version plus unpublished changes) or *Draft v1 · not published* (never published, so no task can select it). Selecting a row opens the editor (`Q21HOA`). *Import from project* opens the existing import flow (`W4zGKx`) and creates independent local copies. A side panel explains the version chain: Draft saved → Publish vN → Runs keep their version. The fixture has four pipelines (Deploy staging, Feature delivery, Investigate issue, Release notes); Deploy staging, Feature delivery and Investigate issue match the pipelines named on the Tools screen.

*No pipelines yet* (`MqRT7`) explains in two sentences what a pipeline is and that a task can only run a published pipeline. It shows the starter Feature delivery with its six steps (Research → Plan (approval) → Implement → Run checks ∥ Review → Create pull request) using `XVkaB` rows. *Create from starter* creates draft v1 with those steps; *Start empty* creates draft v1 with no steps. Neither publishes nor starts a run. The starter refers to the project's own Explorer, Planner, Builder and Reviewer profiles and the Run test suite and Create pull request tools; missing ones are flagged in the editor and block publishing. The empty-library fixture hides the Tasks count, because tasks cannot exist without a pipeline.

*New pipeline* (`c3jD7`) is a modal over the library (`#000000A6` backdrop, `$surface` panel, `$radius-panel`, hairline header and footer). It asks for a name and shows the generated slug (`dependency-updates`), which is unique in the project. Start from is a choice between the starter and an empty pipeline, and the starter's step chain is shown in the option. *Create draft* creates draft v1 once, then opens the editor; a retry must not create a second pipeline. A name that produces an existing slug is rejected with the existing pipeline named (03.C `h5GPCL`, drawn 2026-09-28 as built): the slug line under the name gives way to the field error (circle-alert 12 and Inter 11, `$text`, 4 apart) *The slug dependency-updates is already used by the pipeline Dependency updates. Choose another name.*, and *Create draft* is unavailable (0.4). Choosing the starter leads to the editor with the six steps as draft v1; choosing Empty leads to `angoF`.

*Empty pipeline* (`angoF`) is the editor with no steps: a slot under ON TASK START invites *Add first step* (an agent profile or a tool), and a short explanation with *Add step* and *Use starter steps*. With no step selected, the inspector shows the pipeline itself: name, slug, optional description, trigger, *Published version: None yet*, *Used by: No tasks*, *Steps: 0*, and a notice that tasks can select the pipeline only after v1 is published. Run and Publish v1 are disabled; the toolbar says why ("Add a step before you can publish v1").

*Publish blocked* (`UjHDL`) is draft v4 with two problems: the Review step has no agent profile, and Plan waits for Review, which closes the cycle Plan → Implement → Review → Plan. A strip under the toolbar says publishing is blocked by 2 problems and that published v3 still runs; each problem is a button that selects the step or edge. The cycle edge is drawn and labelled on the canvas, the Review card says "No agent profile", and the inspector marks the profile field as required with a sentence under it. Publish v4 is disabled; Run v3 stays available because it runs the published version. Problems are computed from the saved draft; the exact validation list (missing profiles, tools or credentials, cycles, unreachable steps, a join that can never complete) is implementation work.

*Publish v4 / Confirm* (`zfjsK`) opens from Publish v4. It shows *Published now: v3* next to *After publishing: v4*, the changes from v3, and that runs already started or queued pinned v3 and keep it. *Publish v4* publishes once; a retry must not create v5. *Published v4* (`yf6XH`) shows the result: the version label becomes *Published v4*, the save status says there are no draft changes and that the next edit starts draft v5, Run becomes *Run v4*, Publish is replaced by *Version history*, and a notice repeats that in-progress and queued runs keep v3. Publishing never changes a run that already pinned a version; moving an active run to a newer version would be a separate explicit action and is not designed.

The library row is a new reusable component, `hn8qO` (Pipelines / Library row), canonical inside `LRMQW`. The screens otherwise reuse the sidebar, button, status label, select field, key/value, summary step, pipeline step and inline notice components. Proposed copy namespace: `pipelines.library.*`, `pipelines.create.*`, `pipelines.empty.*`, `pipelines.validation.*`, `pipelines.publish.*`. Change counts, run counts, dates and the Dependency updates / Release notes pipelines are illustrative fixtures. Library filtering, archive/delete, rename and slug collision states are not drawn.

**Version history** (implemented 2026-09-27, drawn in `r8rPaS`). *Version history* (history) is in the topbar in place of Publish when the draft has no changes (03.03 `yf6XH`), and a small toolbar button after *Pipeline settings* while it has. It opens the latest version read-only. *03.07 · Viewing v4 · Read-only* (`fvPvA`): the topbar stays the draft's (*Draft v5*, *Draft saved · Published v4*, *Run v4*, *Publish v5*); the toolbar becomes *Back to editing* (arrow-left, 30 high) and, at the right, a lock with *Viewing v4 · Read-only*; the canvas shows the version's steps with no selection and no published notice; the history panel replaces the step inspector in the same 304-wide frame (padding 24, gap 16): history icon, *Version history* (16/500) and a 24 × 24 close (x). Versions are listed newest first, 8 apart, each a row (padding 12, radius 7; selected `$hover`, hover the same): *v4* (13/500), the mono 9 *LATEST* tag on the newest only, the age at the right (11, *2d ago*), and *3 changes · 2 runs* (11). Under the selected row, indented 12: *Changes from v3* and one line per change (pencil / plus / minus 12 `$muted`, 12 `$text`, the icon on the first line), *Runs that pinned it* with *#131, #133* (or *No run pinned this version*), then *Compare with draft v5* (git-compare-arrows) and *Restore v4 as draft* (undo-2), full width. *03.08 · Compare v3 with draft v5* (`Fb42P`): the list heading reads *From v3 to draft v5*, the button *Hide comparison*, and each step the draft changes carries a *Changed in draft v5* tag (pencil 10, 10/500, `$hover` with `$border`, radius 5) at its card's top-right edge — the tag is a proposal, the app does not mark the canvas yet. *03.09 · Restore v3 as draft* (`NmNRM`): with draft changes the first click arms the button, which becomes primary and reads *Replace draft v5? Click again*; the second click restores and returns to the draft (primary while armed is a proposal; the app keeps it secondary). Compact states (`H0l9D`): *Reading the published versions…*, the notice *The published versions could not be read*, a first version (*First version · No runs*, *This is the first version: every step is new.*, Compare and Restore disabled because the editor holds the same steps) and a comparison with an invalid draft (*Draft v5 has problems to fix before it can be compared.*). Fixture versions: v4 2d ago (3 changes, runs #131, #133), v3 12d ago (2 changes, six runs incl. #128), v2 21d ago, v1 34d ago; draft v5 changes Implement's model.

## Presets

Drawn 2026-10-01, not built. Group `Rg8DS` ("Presets · entry, choose, preview, finish setup, save", last in 03 / Pipelines at x 0 / y 5664, height 4264; the section is now 10024 high). It sits in 03 / Pipelines because a preset is mainly a way to get pipelines: *Add from preset* lives in the Pipelines library, presets replace the single starter of 03.A and 03.05, and the profiles and tools a preset creates exist for its pipelines. Fixture: Kontur (and Atlas in 03.D); the presets *Website content update* and *Kontur feature flow*, the dates, the commands and every count are illustrative. Proposed copy namespaces: `presets.*` (`presets.picker.*`, `presets.preview.*`, `presets.finishSetup.*`, `presets.save.*`, `presets.provenance.*`), `setup.startingPoint.*`.

**What a preset is.** A ready-made bundle that Orchester copies into a project as drafts. It holds:

- **Agent profiles:** name, role and responsibility, role instructions, and default permissions (repository access, commands, network; MCP servers are left out). Never a connection or a model: the person chooses them.
- **Pipelines:** the steps, their gates, the profile or tool each step uses, each step's repository access, and the Deliver step's mode and merge method.
- **Tool placeholders:** a command tool such as *Check* whose command the person fills in (`cargo test`, `npm test`). A saved preset may also keep a tool's command (*Run test suite* keeps `pnpm test`).
- Optionally project instructions and skills.

Built-in presets ship with Orchester: *Simple change* (Implement → Check → Review → Deliver), *UI change with design* (Design with an approval gate → Implement → Check → Review → Verify → Deliver; adds the skill `verify-against-design`), *Bug fix* (Investigate, read-only → Fix → Check → Review; no Deliver step, so its runs keep the manual merge of 02.P) and *Research only* (one read-only agent, no Deliver). *Your presets* are the ones the person saved from a project with *Save as preset*. They are stored for the user on this computer, outside any project, because they are reused across projects.

**Creation rules.**

- **Everything is a draft.** Each resource is created as *Draft v1*. Nothing is published and no run starts. The usual draft rules apply: tasks can only select a published pipeline.
- **Exactly what the preview lists is created, once.** *Create N drafts* creates them in one operation; a retry must not create a second set.
- **Name conflicts are renamed with the next free number, never skipped or reused.** *Builder* becomes *Builder 2* (slug `builder-2`), or *Builder 3* when *Builder 2* is taken too, and the pipeline's steps point at the renamed copies. Skipping would bind the preset's pipeline to the project's existing profile of the same name, whose permissions and instructions can differ: an existing *Reviewer* that edits files would turn a read-only review step into one that writes, so the pipeline would no longer do what its preview says. Renaming keeps the preset consistent and never changes, replaces or reuses what the project already has. The person can rename the copies, or point a step at an existing profile in the editor, afterwards. The preview names every rename (*renamed · Kontur already has Builder*) and lists them beside it.
- **Connection and model.** The preview offers *Leave unset* (default) or *Use one connection for all agents* with one of the project's connections and a model. The connection is set only on profiles whose permissions it can enforce; the others stay unset and say why (03.D).
- **Permissions a connection can't enforce.** The rule of 04.B applies. When a profile's default permission can only be enforced by a CLI the project has no connection for, the preview marks the profile (*Needs Codex CLI*) and offers two choices: keep the permission, so the profile stays unset until such a connection exists, or narrow it to the most restrictive option that is available. Orchester never falls back to an unenforced option.
- **Placeholders are marked *Needs setup*.** A tool without its command, a profile without a connection or model, and a tool whose credential was left out all show the *Needs setup* label (circle-dashed, `DeizF`) in the preview, the lists and the editors. A step that uses an unset tool can't run.
- **Provenance.** Every created resource records the preset (built-in or yours) and the time: *From preset ‘UI change with design’ · created Oct 1, 14:02*. It is shown in the editors and on the library rows and is only informational. The resources are independent copies: editing them does not change the preset, and later preset changes do not reach them (as with Import from project).
- **Never in a preset:** secrets, connections, models, MCP servers, tool credentials, tasks, runs and memory.

**Finish setup checklist.** After *Create N drafts*, Orchester opens *Finish setting up <preset>*, a page under Pipelines. It lists every created resource with what it still needs, grouped in the order they can be published:

1. Agent profiles: *Needs a connection and a model before it can be published*, or *… is set · publish it*.
2. Tools and skills: *Needs a command, such as cargo test or npm test*.
3. The pipeline, last. It stays locked with the reason *Can be published once Builder 2, Reviewer 2, Verifier and Check are published*: a pipeline can't be published until every profile and tool its steps use has a published version.

Each item links to its resource (*Open Builder 2*). Progress counts the items that are done (*2 of 8 done*). Until everything is published, the checklist stays reachable from a banner in the Pipelines library and from *Finish setup · N left* in the editors. *Dismiss checklist* hides it and keeps the drafts.

**Save as preset.** *Save as preset* in the Pipelines topbar opens a dialog with a name, a description (shown on the preset card) and the pipelines to include.

- The profiles and tools the chosen pipelines use are included and can't be unchecked. Other profiles, instructions and skills are optional.
- Only published versions are saved; a pipeline that was never published can't be chosen, and says why.
- A command tool can keep its command or be saved as a *Needs setup* placeholder.
- The dialog states what is never included: secrets, connections and models. MCP servers are left out, a tool that uses a secret is saved without it and arrives as *Needs setup*, and tasks, runs and memory are never part of a preset.

**Frames** (x 48 / 1568 / 3088; rows at y 100 / 1148 / 2196 / 3244, labels 32 above):

| Frame | Node |
| --- | --- |
| 03.10 · Project setup / Start from a preset or start empty: built-in and your presets as options, *Start empty*, the selected preset's contents; the rail gains *2. Starting point* | `am0Dq` |
| 03.11 · No pipelines yet / Add from preset: four built-in cards replace the starter; *Add from preset* in the topbar and as the primary action | `imNnP` |
| 03.12 · Add from preset / Choose a preset: cards with name, description, step chips (approval, read-only, tool, Deliver) and *Creates N agents · M pipelines · K tools*; built-in and *Your presets* are separate groups | `imgjE` |
| 03.13 · Preview and confirm: profiles with their permissions; the pipeline's steps with profile, access and gate; the tool and the skill, all *Draft v1*; the renames; *Leave unset* / *Use one connection for all agents* | `G4DHd` |
| 03.14 · Finish setup / Checklist after creating *UI change with design* | `pD3c2` |
| 03.15 · Pipeline editor / Created from a preset: provenance strip, *Needs setup* badges on steps, the Check step's command field, Publish v1 unavailable with the reason | `zB3Gh` |
| 03.16 · Pipelines library / *Needs setup · 6* marker and provenance on the row, the Finish setup banner, *Save as preset* in the topbar | `T7He6` |
| 03.17 · Agent profile / From a preset: provenance line, connection required, *Needs setup* in the profile list | `lF2E8` |
| 03.18 · Save this project's setup as a preset | `cf6cq` |
| 03.D · Preview / The preset needs a CLI the project has no connection for (*Writer needs a Codex CLI connection*) | `sxzDL` |
| 03.E · Preview / Every name in the preset is already taken (*Kontur feature flow* added to Kontur again) | `Sg6A8` |

Reused: sidebar `zo9Py` with `TSJpn`, `u1XBP`, `DeizF`, `ZGwlv`, `q5LzI`, `tfGRt`, `jNTVN`, `SOW6S`, `ajikA` and `hn8qO`; the setup layout of 01.04 `w6ODp4`, the dialog layout of 03.05 `c3jD7`, the library of 03.04 and 03.A, the editor of `j8OB3` and the profile editor of 04.03 `WPmtS`. No new components or variables.

**Decisions (2026-10-01).** The user approved the frames and answered their open questions. Where a frame differs, these decisions win.

- **No Starting point step yet.** Until the later setup steps ship, a new project is offered the presets on its empty Pipelines (03.11 `imNnP`). The setup step of 03.10 `am0Dq` comes later.
- **Always a renamed copy.** A preview row never offers to use the project's existing profile (*Use Kontur's Builder instead*). A name conflict always creates a renamed copy, for the permissions reason above.
- **Managing your presets is out of scope.** Renaming, updating, deleting and sharing a saved preset come in a later task.
- **Skill scope follows ids.** A skill's step scope refers to the step by id, not by name, so it survives the renames of a preset's pipeline and steps.

## Task library, bulk creation and scheduling

The user explicitly requested creating ten tasks and letting agents work on them either concurrently or in sequence. A pipeline defines the steps within one task; the task queue coordinates separate task runs. All of this remains within the selected project.

Tasks (`J9467`) is the main backlog, with All tasks, Drafts, Queue, Needs review and Done tabs. New task opens the existing single-task form, whose *Create* returns here with the new task's row scrolled into view and highlighted in `$hover` (no frame draws this state yet); Create multiple opens `OqmXc`. Rows expose task ID, title, published pipeline, status and readiness/dependency, with selection and direct entry to task details/conversation. Run selected opens a scheduling review rather than immediately starting work. *The tabs, query lines, selection bar and columns described here and below are superseded by the combined task list that follows.*

**Combined task list, ORC-30 + ORC-39 (drawn 2026-10-02: 02.03C `DWqjc`, 02.03D `IYlGK`, 02.03E `t7ETG`, 02.03F `lRbit`).** Decision (user, 2026-10-02): ORC-39's workflow tabs replace ORC-30's Drafts / Queue / Needs review. Also the user's (2026-10-02): the designer's deviations from the as-built branch are approved (12 bar padding with 32 lines 8 apart; Return to drafts, Set pipeline, then Run selected last at the right; select-all shows square-minus for a partial selection; a successful bulk change is one summary line, failures one line per task; PIPELINE upper case), the held problem line is shortened (*2 tasks held after a task failed · Return to drafts*), the selection bar is hidden while nothing is selected, and rows are 60 high at 960 once the tag chips are dropped. Tasks has **All, Needs you, Backlog, Todo, In progress, In review, Done**, each with its count (`SOW6S` with the count shown); Canceled is reached through All and the Status filter. The frames are drawn from the as-built branch `claude/orc39`; tasks, counts, pipelines, tags and priorities are illustrative fixtures (ORC-129–138 in project Kontur).

| Frame | Node | State |
| --- | --- | --- |
| 02.03C · 1440 | `DWqjc` | All tab, three ready drafts selected (some visible rows selected), both query lines, the two-line selection bar with Run selected enabled and a bulk outcome line; the full table |
| 02.03D · 1440 | `IYlGK` | Todo tab, the same three drafts plus two held tasks of a group (ORC-137, ORC-138, *Held · a task failed*): Return to drafts, Run selected unavailable, the problem line under the bar |
| 02.03E · 960 | `t7ETG` | The selection of 02.03C at 960: the three-line bar; Pipeline, Priority and the tag chips dropped, Workflow / run 164, titles cut with an ellipsis |
| 02.03F · 1440 | `lRbit` | The sort select open (pointer) with the five orders, Priority chosen; the list in priority order |

- **Query, line 1:** search (fills), pipeline filter 184, sort 164 (`ZGwlv`, 36 high). Sort orders: *Manual order*, *Newest first*, *Title*, *Status* (workflow order: Backlog, Todo, In progress, In review, Done, Canceled), *Priority* (Urgent first). Status and Priority break ties by most recently updated. The menu is the standard select menu (`u0ByM` / `L20XvS`, 164 wide, 4 below the trigger, check on the chosen order, trigger stroke 1.5 `$text` and chevron-up while open). Sorting never changes the runner queue. ORC-39's separate *Sort: Updated* button is gone.
- **Query, line 2:** *Status*, *Priority* (chevron-down) and *Tags* (tag) buttons (`u1XBP`, 36 high, 12 apart), then *Clear filters* (x) only while a facet is active, and *Manage tags* (tags) at the right. An active facet reads *Status · 2*; the active values show as chips under the line (ORC-39 `l7kBSq`).
- **Selection bar** (`task-selection`): `$raised`, radius 6, padding 12 on every side, lines 32 high and 8 apart. From a 1348 window, line 1 is the select-all box, *N selected* (Inter 12 `$text`), the readiness summary (Inter 11 `$muted`, fills and gives way with an ellipsis: *3 ready to queue · 0 incomplete*, with *· 2 already queued or started* when it applies), then at the right *Return to drafts* (undo-2, only while held never-started tasks are selected), *Set pipeline* (workflow) and the primary *Run selected · N* (play, 158), all 32 high, 14 apart. Line 2 is *Set status* (circle), *Set priority* (signal), *Add tag* and *Remove tag* (tag), 12 apart. Under it, the bulk outcome. Below a 1348 window: line 1 is the selection and summary alone, line 2 the run actions right-aligned, line 3 the organization group. Actions keep their full width at every size.
- **Run selected** is enabled only when every selected task is a ready draft; otherwise it is drawn unavailable (0.4) and the summary and problem lines say why. ORC-30's problem lines sit under the bar (circle-alert 12 + Inter 11, both `$text`, 8 apart), one per reason: *2 tasks: Held after a task failed or was cancelled. Select the waiting tasks and choose Return to drafts to schedule them again.*
- **Bulk outcome:** one line per result in ORC-39's words (*Status set to Todo for 3 tasks.*, *Added Accessibility to 3 tasks.*, *Removed Desktop from 2 tasks.*), check 12 + Inter 11 in `$muted`; a task that could not be changed gets its own line in the problem style (*ORC-131 could not be updated: …*). The selection and entered values stay.
- **Select all** shows square-check when every visible row is selected, square-minus when some are, square when none; it acts on the visible (filtered) rows, and selections hidden by the query stay selected and counted.
- **Columns** (headings Inter 10 `$muted`, upper case; row 76 high, `clVjO` with a gap of 16 and padding 0 12): checkbox 18, ID 72, *TASK / TAGS* (title Inter 12 `$text`, tag chips under it, never under 120), *PIPELINE* 136, *PRIORITY* 96 (icon 14 + Inter 11 `$muted`), *WORKFLOW / RUN* 212 (workflow label `DeizF`, run label Inter 10 `$muted`, *⊘ Blocked* under them when flagged), *DEPENDENCY / READINESS* 184, open 14. Below a 1348 window Pipeline goes; below 1196 the tag chips go; below 1116 Priority goes and Workflow / run narrows to 164. At 960 the title has 124 and is cut with an ellipsis.
- **Rows:** a draft reads its workflow status and *No active run* with its readiness (*Ready to queue*, *Needs a brief*, *Blocked · ORC-129 unfinished*); a held member of a group reads *Queued* with *Held · a task failed* (or the group's other hold reasons); a task with a run reads the run label and its progress text (see the table below).
- **Footer:** *12 tasks · 4 runs started* and *Open a task to edit its brief, pipeline and limits.* Create multiple stays unavailable (0.4, trailing lock).

Changes from the as-built branch (to implement): the bar's padding is 12 on every side with 32-high lines (as built, line 1 is 44 high with no top padding, so its buttons sit 5 from the top and 15 above line 2); the run actions run Return to drafts, Set pipeline, Run selected with the primary last (as built, Run selected comes first; this restores `J9467` and the order above); select-all shows square-minus for a partial selection (as built, an empty box); a successful bulk change is one line in ORC-39's words (as built, one *ORC-129 updated.* line per task); the Pipeline heading is upper case like the others (as built, *Pipeline*).

**ORC-30 implementation (2026-09-30; independent design verification pending).** Checkboxes select rows without
opening their task. Select all affects the visible filtered rows; selections hidden by a query remain selected and
counted. Search matches title, task key, internal ID and brief, combined with the pipeline filter, sort and tab.
The selection bar counts ready and incomplete selections and lists each selected task's readiness problems.
Run selected requires every selected task to be a complete draft with a published pipeline and no previous run.
The review permits dragging or keyboard editing of task order, parallel concurrency or sequential scheduling, per-task token/time
limits and the aggregate cap; cancelling returns to the selection without scheduling work. Queue saves the whole
group atomically, pins pipeline versions, and the runner starts one idempotent first run per task. Waiting for input
or approval occupies a task slot. Sequential scheduling waits until the previous run succeeds (ready for review).
The aggregate cap applies only to the group's first runs, including further attempts of those same runs. Cancel
followed by Run again creates a separate run with its own limits; it neither inherits the exhausted group cap nor
contributes usage to that group. Original group membership remains stored for first-run idempotency and history.
An unrecovered failure or cancellation holds unstarted members; existing agents retain their normal controls.
The open review retains the published pipeline versions and task order it displays. Submission and preflight use
those exact immutable versions, even if a newer publication appears in the meantime; atomic submission rejects a
changed task pipeline assignment and asks for a renewed review. First-run snapshots use the queued reviewed versions.
On a Fresh attachment after a desktop session ends, unstarted members (including Pending first runs) receive a
persisted scheduling hold before agent slots become available. Reconnect does not clear that hold. Open a queued
member and explicitly start its reviewed first run, or explicitly continue an interrupted first run, to release the
group's session hold. Existing concurrency, cap, preflight and previous-task checks still apply. A later Run again
cannot release the old group's hold. These recovery reasons reuse the task list's waiting text (implemented, not
drawn yet); they preserve group order, original run keys, checkpoints and snapshots rather than creating a new group.
A recorded Pending original first run exposes Start run in its Summary topbar (implemented, not drawn yet), using
that existing run's stable first-run key. It retries worktree/start preparation with the recorded ID and snapshot,
not a new run or the latest publication. Its Summary notice and heading explicitly name a session scheduling hold;
a Pending record without that hold says it has not started. The list's pipeline column uses the latest readable
run's pinned pipeline/version, otherwise the queued task's reviewed version, and the latest publication for a draft.
All five task tabs round-trip through navigation routes; search, pipeline filter and sort remain combined when
switching tabs. Unknown task-tab routes still fall back to All.

**Implemented, not drawn yet:** inline Set pipeline picker/apply/cancel and skipped-task reasons (only drafts without
runs are changed); empty search results and non-fixture filter/sort combinations; review validation, offline runner,
pending/error and queued-success states; one selected task; group waiting reasons for concurrency, previous task,
agent capacity, daily/shared budgets, preflight or start failure. These reuse existing fields, buttons, notices and
error text. Values are derived from real selections, never the illustrative ten-task defaults. Group caps stop
agents at turn boundaries and can overshoot by already-running turns. Queue editing and explicit task prerequisites remain
separate backlog work; this change does not introduce those controls or claim dependency enforcement (pause scheduling
came later, with the Orchestrator chat: *Hold new starts* in Tasks). Shared-cap recovery is unavailable in the per-run raise-limit panel because raising one run's limit
cannot raise a group cap. The independent verifier must compare J9467/W1Dq6/IyJ5a and these reachable states.
**Held groups (implemented; drawn 2026-10-02 in 02.03D `IYlGK`, where Return to drafts sits left of Set pipeline as written here):** a failed or cancelled task, the
spent group cap, or a start the person must fix (preflight problems, a failed start) holds a group's unstarted tasks
for good; their rows say so (*Held after a task failed or was cancelled. Select the waiting tasks and choose Return to
drafts to schedule them again.*, and the same for the cap). Selecting any of them adds a secondary **Return to
drafts** button (undo-2 icon, 32px like Set pipeline) to the selection bar, left of Set pipeline. It takes the
selected held, never-started tasks back to the drafts and out of their group, together; the group keeps its started
members, and a group left without members is removed. The drafts can then be reviewed and queued again. The
selection bar's split names tasks that are neither ready nor incomplete: *0 ready to queue · 1 incomplete · 2 already
queued or started*. When the selected tasks had different budgets or time limits, the review says under its token
fields that *These limits replace each selected task’s own token budget and time limit.* (the shared limits apply to
every task, as the review's per-task fields show). The runner retries a start the person must fix with a growing
wait (2 s, doubling to 1 min) and stops treating a group as work once it is held for good or every first run ended.
**Narrow windows and empty searches (implemented; the breakpoints below are superseded by the combined task list's 1348 / 1196 / 1116, drawn in 02.03E `t7ETG`):** below a 1260 window the scheduling review puts
the settings under the order, both as wide as the page, and the page scrolls; below 1152 the task table leaves out its
Pipeline column so the title keeps at least 120 (the fixed column widths otherwise leave the title nothing at 960). Below the same width the selection bar puts its actions (Return to drafts, Set pipeline, Run selected) on a second
line, right-aligned; at any width they keep their full width and the readiness summary gives way with an ellipsis.
When the search or pipeline filter hides every row of a tab, the table says *No tasks in this tab match the search or
pipeline filter.* with a *Clear search and filter* button. The drawn copy that names merge dependencies (selection
summary, sequential hint and notice, branch note) is not used: dependencies are not built, so the app does not claim
them.
The scheduling-review order column includes a Task to reorder selector and keyboard-reachable Move up, Move down
and Undo buttons beneath the scrolling rows (implemented, not drawn yet). Select a task with the picker, then use
Tab and Enter on its move action. Boundary moves and actions after submission are disabled. Each real keyboard
move or drag replaces the single Undo snapshot; Undo restores that snapshot once. Choosing a task or attempting
a no-op move does not replace history. Both review modes share this behavior and queue the resulting order.

**After the failed 2026-09-30 visual review:** scheduling reviews now show the task-count/concurrency footer,
token units and compact readbacks, the selected published pipeline versions, and compact active-time properties.
Activating the time property opens a minutes field and Done button (implemented, not drawn yet). Mixed pipeline
versions are listed explicitly in a wrapping property; pipelines without plan gates keep the generic approvals
notice. Gate and failure selects offer only the scheduler's supported policy. Actual runner slots appear when
reported. The footer follows the visible task rows, including the draft-editing hint. Independent re-verification
is still required. Order counts stay unpadded under the shared counts rule. Create multiple and Orchestrator remain
visibly unavailable; their implementation is outside ORC-30. The older frames omit the existing Skills sidebar
entry. The person's scope decision retains existing global navigation and records this omission as a pre-existing
design/state difference, consistent with runtime UI verification. Fresh independent Verify compares the changed
task-list and scheduling-review regions against their references; ORC-30 does not remove Skills navigation.

**Scheduling-review recheck corrections:** settings keep the 32px inset after the divider and 508px controls
width at 1440px. Their reserved 12px scrollbar gutter occupies part of the page's right padding, preserving
the controls' right edge. Execution-mode cards use the drawn 88px height and 12px gap (`YZcH2`, `jPX6u`).
Order titles keep the 12px field typography with primary text color; subtitles remain muted. These corrections
apply to both W1Dq6 and IyJ5a and require a fresh independent visual comparison. The original evidence in
`.tmp/verify/recheck/report.md` remains preserved.

**Task rows by run state (`r2al55`, drawn 2026-09-27 from the app).** A task with a run shows the run's state as the status label and, in the last column (and after the key in the Overview list `zemhy`), what that means for the task now:

| Status (icon) | Last column |
| --- | --- |
| Queued (clock-3) | *Waiting for its run to start* |
| Running (loader) | *Agent at work*; after Pause run until the checkpoint settles: *Pausing at the next checkpoint* (the label stays *Running*) |
| Needs input (message-circle) | *Waiting for you* |
| Paused (pause) | *Paused at 4b92ad0*; before any checkpoint *Paused before its first checkpoint* |
| Stopped at limit (octagon-pause) | *Raise the limit to continue*; stopped by the daily budget *Waits for the daily token budget* |
| Interrupted (circle-pause) | *Last checkpoint 1b8e5c2*; without one *No checkpoint yet* |
| Completed (circle-check) | *Every step passed* |
| Merged (git-merge) | *Merged into main* — the branch the run's work went into |
| Unreadable (circle-alert, `$danger`) | *Run #131 could not be read* — the latest run's record could not be read |
| Failed (circle-x) | *Run failed* |
| Cancelled (square) | *Run cancelled* |
| Draft (pencil) | readiness (*Ready to queue*, *Needs a brief* …) |

The task table's *Status* column is 128 wide (was 112) so *Stopped at limit* fits, and *Dependency / readiness* is 184 (was 146) so its longest text fits on one line (`clVjO` cells `VizsW`, `S2kP9L`; heading `B5FaIC`). Interrupted, Cancelled and Draft were added to the status variants `P3HONM`. The selection checkbox is drawn unselected in `$muted` there, since selecting tasks for a run is not built. Merged and Unreadable are drawn in the compact board `mGbYs` (task table and Overview rows, in `GvUXd` (removed 2026-10-04; now `A5H6ln` and `LpVFY`)). *Unreadable* is the one status label in colour (user decision 2026-09-27): its icon and word use `danger` `#F87171` on the usual `$hover` fill (5.0:1 on `$hover`, so the 11px label stays readable). Every other status label, *Merged* included, stays neutral. `danger` is the design value for lurq's danger role (`PaletteColor::Danger`), which today keeps lurq's default `#DC2626`; that default is too dark on these fills (about 3:1).

**Unreadable run (02.R `SUuwV`, drawn 2026-09-27 to the spec being implemented).** When the latest run's record cannot be read (a status this version does not know, a record it cannot decode), the task still opens. Page description *Run 131 · could not be read*. An emphasized notice (`q5LzI`, circle-alert in `$text`, heading 12/500) *Run #131 could not be read* says in one sentence what could not be read (*Its record has a step status this version of Orchester doesn’t know (awaiting_merge in Review), so where the run stands is unknown.*) and that *The database file is kept unchanged, so a newer version can still read the run. The log has the details.* Under it, a lock (12, `$muted`) and *Cancel, pause, continue, merge and Remove worktree are unavailable for run #131 until it can be read.* (Inter 11 `$muted`, indented to the notice's icon); the topbar's run control is shown unavailable (0.4, trailing lock), never hidden and never enabled. *Other runs of this task* (15/500) with *They were read as usual. Their changes stay on their branches.* lists the task's other runs, newest first, as 46-high rows: *Run #130*, its status label, what happened (*Cancelled at Implement · work kept on orc/repo-switcher*) and when (mono 11). No right panel: nothing about the unreadable run is known. The task list row reads *Unreadable* (`$danger`) with *Run #131 could not be read*. The page itself has no status label; the notice keeps the emphasized notice's `$text` icon, because notices carry no status colour.

Create multiple supports pasting one title per line, adding rows and editing each task's brief, acceptance criteria, pipeline, limits and dependencies. Pasting titles does not invent accepted requirements. The drawn state shows ten completed briefs, with Task 03 selected. Create 10 tasks creates ten drafts and returns to Tasks; it does not start agents. It remains valid to save incomplete drafts, but missing required details are surfaced before queueing. Creating and queueing must each be idempotent; a failed response must not result in duplicate tasks or runs on retry.

The fixture uses tasks ORC-129–138 in a separate sample backlog from the original overview. ORC-131, Add switcher keyboard shortcuts, depends on ORC-129, Add repository switcher, being merged to main. References to Task 01 before creation are mapped to the new local task identity during creation. All ten tasks use published Feature delivery v3, a 1.4M-token per-task budget and 30-minute run limit; the selected group cap is 14M tokens. These remain fictional values and records.

| Setting | Proposed behavior |
| --- | --- |
| Parallel (`W1Dq6`) | Start eligible tasks in the configured queue order up to the configured task-run limit. Task priority is for list sorting and filtering only (ORC-39); it does not reorder the runner queue. The fixture uses two concurrent tasks. A task waiting for plan approval still occupies a task-run slot. Dependency-blocked tasks have not started and occupy no slot; other eligible tasks may proceed. |
| Sequential (`IyJ5a`) | Run one task at a time in the chosen order. The drawn completion gate is Previous task is ready for review. A blocked next task holds this sequence rather than silently skipping ahead. |
| Dependencies | Both modes enforce explicit prerequisites. Ready for review is distinct from Merged to main. A merge dependency releases only after the merge is verified; the dependent task starts from the updated base. |
| Agent capacity | Concurrent task runs and agent slots are separate constraints. A pipeline can itself use several agents. The runner also enforces its available agent slots; insufficient capacity leaves work waiting with a reason. |
| Failure policy | The fixture pauses remaining tasks on an unrecovered task failure. Already active tasks continue; no automatic discard or cancellation is implied. Explicit recovery can retry the failed task or resume the remaining queue. |
| Pause scheduling | Stops new task starts in the selected group. Active task runs continue. Their existing per-task Pause and Cancel controls remain separate. |
| Budgets | Enforce both per-task and group caps. Exhausting the shared cap stops further scheduling and exposes a budget reason; the exact accounting and in-flight settlement remain runtime engineering work. |

Queue 10 tasks schedules the reviewed group with pinned configuration versions. Each task retains its own run, conversation, branch and result. The group does not become one shared agent conversation. Plan approvals and tool access decisions come from each pipeline and tool policy; bulk scheduling does not bypass them. Separate branches/workspaces are required for concurrent edits; reviewed PRs and explicit merges determine integration order. Parallel execution does not guarantee conflict-free merging.

The active queue (`Z22wg`) demonstrates Group 07 with two tasks researching, seven waiting for concurrency and one blocked on merge. Each waiting row explains why it has not started. Edit queue changes pending order/limits; reductions in concurrency drain through existing work rather than cancelling it. Changes to already-started run snapshots require a separate explicit operation. Queue-tab counts include all unfinished scheduled work, including running and dependency-blocked tasks; the status summary disambiguates those counts.

Preflight must validate required briefs, available published configuration, authorization, runner readiness, dependency cycles and impossible sequential order. Invalid work stays intact with actionable reasons; any partial queueing must be an explicit selection, not a silent skip. Self/cyclic dependencies are rejected. A prerequisite outside the selected group must be visibly identified and must not be automatically started without selection. If a prerequisite fails, is cancelled or its merge is reverted, dependent work remains blocked for review.

Interaction notes: select-all applies to the current filtered set and displays its count. Task-row activation opens details; the checkbox only changes selection. Drag reorder also needs keyboard Move up/down actions and one undoable reorder. Paste a task list is an explicit user action with a reviewable result. Closing retains the batch draft; destructive discard is separate. Disabled/pending/error states, live scheduling and delivery remain implementation work. Proposed strings extend `tasks.bulkCreate.*`, `tasks.selection.*`, `taskQueue.*`, `taskDependencies.*` and `runGroups.*`.

Implemented (2026-09-27): **a run that cannot be read**, drawn in `mGbYs` and 02.R `SUuwV`. A run whose stored record breaks the run's rules or cannot be decoded (edited or damaged outside Orchester) no longer stops the task list, the Overview or the task page from loading; the record is kept unchanged and the log names the run and why. The task's row reads *Unreadable* (circle-alert; icon and word in `danger`) with *Run #131 could not be read*, in the task table and the Overview list. The Overview's metrics, tab counts and footer, the status bar's *Queued* and *Needs you*, and the task list's run count leave the run out; Recent activity does not list it, since when it last changed is not known; when that leaves it nothing to list, it says *No run to show: the latest run of a task could not be read. The task list marks it.* rather than *No runs yet*. A task whose latest run cannot be read opens as 02.R: *Cancel run* unavailable in the topbar, *Run 131 · could not be read* under the title, and as the Summary the emphasized notice, the lock line naming the unavailable actions and *Other runs of this task*, newest first (*Failed at Run checks*, *Cancelled at Implement · work kept on orc/repo-switcher*, otherwise the task row's words, and the local date and time of the run's last change). Deviations: the unavailable *Cancel run* is the shared unavailable button (muted, trailing lock) rather than the frame at 0.4; the notice's first sentence is one of two general ones (a value in the record is invalid or unknown to this version of Orchester, or the database could not read the record). Drawn 2026-09-28: that task's Conversation, Changes and Activity tabs, 02.R2 `lmlkQ` (removed 2026-10-04; now C·14 `Y6gMx` (F)) (the Activity tab): the tab's centred state, circle-alert 20 `$muted`, *Run #131 could not be read* (14/500) and *Nothing this run did can be shown until its record can be read. Summary lists the task’s other runs.* (12 `$muted`, centred, 420 wide), 10 apart; the conversation is not shown while the run it would go to cannot be read. A run that cannot be read while a later run of its task can, 02.R3 `cP8oh` (removed 2026-10-04; now C·14 `Y6gMx` (F)) (on the Conversation tab of working run 128): the same emphasized notice (`q5LzI`, circle-alert and heading 12/500 in `$text`) for *Run #127*, under the page heading and above the tabs (padding 0 / 32 / 16), on every tab; one notice per such run. The runner leaves such a run as it is when it attaches or recovers and rejects any request for it; the instruction preview does not offer it, and the start-up recovery leaves it out.

**ORC-12 · Draft Summary and editing (drawn 2026-09-29, section `AmJPj` "States · ORC-12 · Draft summary and editing", fixture draft ORC-7).** A saved task's brief and acceptance criteria are shown on its Summary and a draft can be reopened in the New task form. *Draft task page* (`tqn5P`): the draft notice stays first (`q5LzI`, pencil, *This task is a draft*), then *Brief* (15/500) with the brief (13 `$text`, line height 1.7) and *Acceptance criteria* (15/500) as a numbered list (number 12 `$subtle`, criterion 12 `$text`); the right panel *This task* lists Pipeline, Budget and Time limit, with *A run keeps the pipeline version and limits it starts with.* at the foot; the topbar has one secondary button, *Edit* (pencil), which opens the form below. The same draft with nothing chosen (`h3IIs`): Pipeline *No pipeline chosen* and Limits *No limits set*, both `$muted`. *Task with a run* (`uv4PY`, running run 128): the same Brief and Acceptance criteria section sits after the step list and before *Latest activity*, so the run's notice and steps keep their places; the column scrolls with the scroll gutter, and `dKiEj` shows it scrolled to the end. A task with a run has no Edit. *Edit task* (`E1o77O`, based on 02.09A `m82tU`): breadcrumb *Kontur / Tasks / ORC-7 / Edit*, title *Edit task*, description *Changes apply to draft ORC-7. Nothing runs until you choose Create & run.*, the fields filled with the saved values, topbar *Draft saved locally*, and *Save changes* in place of *Create* (it stays on the form after saving), unavailable (0.4) while nothing differs from the saved draft; *Create & run* stays. After an edit (`ctHXE`, title changed) the topbar reads *Unsaved changes* (`$text`) and *Save changes* is available. *Leaving with unsaved changes* (`kDmo9`): Back, the breadcrumb or another page in the same tab asks in the confirmation dialog's frame (`P8IN7Z`) over the backdrop: *Discard changes?*, *Your edits to ORC-7 are not saved yet. Leaving discards them; the saved draft stays as it was.*, *Keep editing* (secondary) and *Discard changes* (primary); Escape keeps editing. Closing the tab uses the tab's existing *Discard changes?* wording (*Keep open* / *Discard and close*).

**ORC-16 · Run again (drawn 2026-09-29, section `nNtAZ` "States · ORC-16 · Run again", y 29572; `gdgT2` and `GvUXd` (removed 2026-10-04; now `A5H6ln` and `LpVFY`) moved down 668 to y 21828 / 26224 to make room for ORC-15 in `BnxJ1`; section height 31836. Fixture: ORC-131 *Add switcher keyboard shortcuts*, run #131 failed; ORC-132 *Show recent repositories first*, run #133 cancelled).** A task whose latest run failed or was cancelled can start a new run of the same task without a new task. *Failed* (`zM693`, the Ready for review Summary as built, 02.P, with the run's words from the app): status label *Failed* (circle-x), *Run #131 · Feature delivery v3 · Failed at Run checks, 14:32*; the card *The run failed at Run checks* · *Activity shows the command, its exit code and its output.*; the stage after the failed one has no mark and a `$hover` line; Verification lists *Run checks* *Failed · attempt 1 · exit 1 · 2 min* and the steps that never ran as *Not started* (circle-dashed, `$muted`); no review actions. The topbar holds *Remove worktree* (secondary) and, at its right, **Run again** (rotate-ccw, primary). *Cancelled* (`M1upHe`): *Cancelled* (square), *Run #133 · Feature delivery v3 · Cancelled at Implement, 11:05*, the card *The run was cancelled* · *Completed work stays on orc/recent-repositories.*, the same topbar. **Confirmation** (`R5y5x`, the dialog frame of 02.Q2, 600 wide over the backdrop): *Start run #132 of ORC-131?* · *Run #132 starts from the first step in a new worktree, on a new branch from main as it is now. Run #131 keeps its record, its branch and, unless you remove it below, its worktree.*; property rows *Pipeline* **Feature delivery v4 · latest published**, *Run #131 used* *Feature delivery v3*, *Starts from* *Research · step 1 of 5*, *Base* `main at fa7e675`, *New branch* `orc/switcher-shortcuts-2`, *New worktree* `.orchester/worktrees/orc-131-run-132`, *Limits* *1.4M tokens · 30 min · from the task*; then the switch (off) **Remove the worktree of run #131** with *No uncommitted changes in it. Its commits stay on orc/switcher-shortcuts.*; *Cancel* and primary *Start run #132* (play). With uncommitted files the switch line names them instead (*2 files not committed would be discarded: …*), as in 02.Q. **Unavailable** (compact board `iE8eQ`): *Run again* stays in its place at 0.4 with a trailing lock, never hidden, and the reason is the lock line under the run's notice (lock 12 and Inter 11 `$muted`, as in 02.R): another run of the task is active — *Run again is unavailable: run #132 of this task is running. A task has one active run at a time; Run again returns when it ends.*; the pipeline is no longer published — *Run again is unavailable: Feature delivery has no published version any more. Publish it again in Pipelines to run this task again.*; the daily budget is used — *Run again is unavailable: the daily token budget is used (20M of 20M today). New runs wait until tomorrow, or raise it in Project settings › Limits.* **Task row menu:** task rows (`clVjO`, `zemhy`) have no menu — a row opens its task and the checkbox only selects it — so there is no row entry; `vmupO` notes this beside the failed and cancelled rows (*Run failed*, *Run cancelled*). Run again is on the task page only. Proposed copy namespace: `runs.runAgain.*`.

**ORC-12 follow-up · Editing after a run starts (drawn 2026-09-29).** This supersedes the earlier ORC-12 restriction that a task with a run has no Edit. The run Summary (`uv4PY`) has *Edit* after the run controls. The started-task form (`dRN4m`) explains that changes apply to future runs and existing runs keep their original task details. It has *Save changes* and no *Create & run*. A started task must remain complete. Every recorded run keeps its title, brief, criteria, pipeline and limits even if the runner restarts. A run's Summary keeps showing that run's brief and criteria after a later edit. A queued task with no recorded run stays locked. The draft form (`E1o77O`) retains *Create & run*.

## Task workflow, priorities and tags (ORC-39)

Drawn 2026-09-30. **Design approved for implementation.** The person explicitly authorized the canonical shared source `H:/projects/orchester/data/orchester.pen`, rather than the task worktree's copy. All design reads and edits used Pencil MCP. The designer did not save the document: the user saves it. The 24 existing frames below are 1440 × 960, named with ORC-39, arranged in five columns starting at x 0 / y 37644. Do not recreate them or change unrelated frames.

This approved design supersedes the legacy task tabs and status-as-run presentation described above. It leaves run state as a separate system label and does not change scheduling order. All titles, task IDs, counts, dates, branches, priorities and tag assignments are illustrative fixtures, not real project data or shipping defaults.

| Frame | Node ID | State shown |
| --- | --- | --- |
| 01 · Tasks | `qVMJh` | Six workflow statuses in rows, priority icons, tag chips, separate run labels, readiness column and new tabs |
| 02 · Overview | `DtpHJ` | Workflow status, priority icons and tag chips on Overview rows; blocked-task notice |
| 03 · Header status | `gNExS` | Status picker, current selection and independent run label |
| 04 · Header priority | `Sd5qt` | Five priority choices; changing priority does not change runner order |
| 05 · Tag picker | `IOWwP` | Select existing tags, create while tagging, empty selection and resulting chips |
| 06 · Tag picker alternatives | `yrT73` | Case-insensitive duplicate, blank name, no project tags, chip overflow and save error |
| 07 · New task fields | `ixENv` | Status, priority, tags and Blocked off; focused organization-field specimen |
| 08 · Edit task fields | `ZLhiM` | Blocked on, free-text reason, unfinished/finished task links, remove/add link and unavailable Run |
| 09 · Blocked alternatives | `tUwII` | Task-link picker, self/cycle and required-reason validation, reason-only flag and finished dependency |
| 10 · Cancel task confirmation | `N1vwaM` | Active-run confirmation naming task/run, stop effect and retained branch |
| 11 · Cancel task alternatives | `Fhzcc` | Stopping, stop failure, canceled result and cancellation without an active run |
| 12 · Bulk workflow and priority | `Ayo8q` | ORC-30 selection bar, Set status/Set priority menus, mixed values, success and error |
| 13 · Bulk cancellation | `hFH4D` | Three selected tasks, one active run, confirmation and branch preservation |
| 14 · Bulk tags | `HI91q` | Add/Remove tags, create while adding, mixed membership counts, empty removal and results |
| 15 · Filters | `l7kBSq` | Multiple status/priority/tag selections, Match all selected tags and a matching task |
| 16 · Sort and list alternatives | `rVJoA` | Workflow/priority sort options and order, priority result, no matches and load error |
| 17 · Project tags | `y3ohgK` | Tag list, task counts, Rename/Merge/Delete, entry actions, empty list and load error |
| 18 · Create and rename tags | `YBfjV` | Name and chip preview, rename impact, duplicate/blank validation, save failure and result |
| 19 · Merge tags | `ZWSYj` | Target choice, confirmation, assignment deduplication, unavailable target, failure and result |
| 20 · Delete tag confirmation | `nOOi1` | Confirmation removes the tag from its tasks and project; tasks and other tags remain |
| 21 · Tag management alternatives | `VvhOQ` | Delete/merge pending, delete failure/result, unused tag wording and stale target |
| 22 · Task activity | `D1DC6T` | Automatic run/review/merge transitions and manual changes, with who/when/from/to |
| 23 · Separate run labels | `k3YiJ` | Running, Needs you, Failed, Stopped at limit and Paused; Blocked on every workflow status |
| 24 · Blocked task page | `Ncjvt` | Reason, unfinished-task link, unavailable Run and readiness after the dependency finishes |

### Workflow, priority and readiness

Workflow status is fixed: **Backlog, Todo, In progress, In review, Done, Canceled**. A person can choose any status. Run start automatically sets In progress; Ready for review sets In review; merge sets Done. Status does not itself start or pause a run. Running, Needs you, Failed, Stopped at limit and Paused remain separate system labels alongside the workflow status (for example, *In progress · Needs you*). The new Tasks tabs are All, Needs you, Backlog, Todo, In progress, In review and Done. Canceled is available through All and the Status filter.

*Where the displayed status comes from (ORC-51).* The tabs and the overview's In review count read the workflow status; the run label, the progress text, Needs you and the status bar's Queued count read the task's phase, derived from its latest run (*Ready for review* is a succeeded run not yet merged). Neither comes from the stored task status (`tasks.status`), which only says whether the task is a draft or was submitted, and stays *queued* after its run ends (`docs/architecture/storage.md`, "State machines").

*Work settled without Orchester (ORC-11, 2026-10-14).* A succeeded run that Orchester did not merge is checked against git each time the tasks are read (`orchester_core::integration::Branches`, `tasks::delivery`), the same way for the task list's rows, the Overview and the task page. It reads as *Merged into <base>* only when its branch has commits past the revision the run started from and the base branch holds all of them, as after a merge made with git or a pull request pulled back into the checkout. A run whose branch has no commit of its own reads *Nothing to merge*: neither Ready to merge nor Merged, no Merge action, not in Needs you. A task moved to Done or Canceled is no longer Ready to merge either. Known limits: git cannot see a squash-merged or rebased pull request (the base holds other commits), a base branch not pulled since the merge, or a run branch deleted or force-pushed; such work stays Ready to merge until the task is moved to Done. Without git, it stays Ready to merge too, logged once per checkout and base branch in a session.

*What the Tasks page's Needs you tab counts (2026-10-15).* The tab lists and counts the tasks something of waits for: a tab's count is what it lists, so a task that waits on several things (a plan and a question, say) counts once on the tab, while the pill and every other *Needs you* count each thing. What waits and belongs to no task (the project's memory records waiting for approval, a tool request whose run cannot be read) is counted by every other *Needs you* (the title bar pill and its panel, the status bar, a tab's mark, the Overview, All projects) and is named on this tab in an information notice under the search, above the rows or the tab's empty state (where `Epiq5` and `oDyZB` put a notice): the item's own words (*1 memory record waits for approval*) or how many wait (*2 things wait for you outside the tasks*), *This list shows tasks only. The Needs you panel lists everything that waits for you.*, and *Open Needs you*, which opens the pill's panel. Not drawn; built with the Notice component (`R3Pyx`) and the hand icon of the pill.

Priority choices are **Urgent, High, Medium, Low, No priority**. The neutral icon scale is circle-alert for Urgent, signal-high/medium/low and minus for No priority. Priority only changes task-list sorting/filtering. Workflow sort follows the fixed status order above; priority sort is urgent-first; ties use most recently updated first. The runner queue keeps its configured order.

Choosing Canceled with an active run opens the in-app confirmation before changing the status. It names the run, says it will stop, and says the work remains on its branch. Keep task or Escape leaves both unchanged. Confirm uses the existing cancel path; while stopping, retain the current workflow status and show a pending action. A stop failure keeps the status and offers Retry. A successful stop sets Canceled without deleting the branch or files. With no active run, set Canceled directly. Bulk cancellation confirms the total task count and active-run count before making changes; partial failure behavior must identify any successful and failed tasks rather than claim all tasks changed.

Blocked is independent of workflow status. The form has a flag, a free-text reason and optional blocking-task links, with search, add and remove. The reason is required when the manual flag is set. Self/cyclic links are unavailable, consistent with the existing dependency rules. The Dependency / readiness column and task page identify unfinished blocking tasks and explain why Run is unavailable. A finished link no longer blocks Run; clearing a manual flag does not silently remove task links, and changing workflow status does not bypass an unfinished prerequisite. The resolved specimen assumes no remaining manual block. Existing dependency completion gates still apply: a link requiring merge is not released just because a task was manually marked Done.

**Approved readiness policy B:** a reason-only manual Blocked flag is informational. Only links to unfinished blocking tasks disable Run, showing their reason. Resolving those links restores readiness without clearing the manual flag/reason.

Approved initial New task values are Backlog, No priority, no tags and Blocked off. Frames 07/08 focus on the organization fields; preserve the existing project, brief, criteria, context, pipeline, limits, Save as draft/Create & run and started-task editing behavior. They do not authorize removing unrelated controls or changing run snapshots. Agents neither receive nor change workflow status, priority or tags; these are not prompt variables or agent-editable tools.

### Tags, bulk actions and filtering

Tags belong to a project and have case-insensitively unique, nonblank names. All tags use one neutral theme style; there is no arbitrary color picker or per-tag color value. A task can have any number of tags. The header/form show selected chips and an Add/Edit tags entry; the picker searches existing names, selects/deselects tags and creates a new project tag while assigning it. A case-insensitive match offers the existing tag instead of creating a duplicate. The proposed row overflow is the chips that fit followed by +N; reveal the remaining tags from that affordance. Header/form chips wrap instead of overflowing.

Manage tags opens the current project's list from Tasks or the tag picker. Rename changes the name on every assigned task. Merge explicitly chooses another tag and confirms the replacement: union task assignments, retain one target assignment per task and remove the source tag. A tag cannot merge into itself; with no other tag, Merge is unavailable. Delete confirms removal from the project and all assigned tasks, retaining tasks and their other tags; recreating a deleted name does not restore assignments. The unused-tag confirmation still applies. Pending/error/stale-target states retain entered data and do not claim a successful mutation.

ORC-30's selection bar gains Set status, Set priority, Add tag and Remove tag alongside existing selection actions. A mixed status/priority has no checked menu option. Bulk Add tag preserves other assignments and deduplicates; bulk Remove tag choices are the union of tags on selected tasks, with membership counts, and remove only where present. Remove tag is unavailable if no selected task has tags. Keep selection and entered values on error; report actual outcomes.

Status and priority filters allow multiple values: OR within a facet, AND across facets and the selected workflow tab. The tag filter offers Match any selected tag or Match all selected tags; frame 15 shows Match all. Clear filters removes the active facets. No matches differs from an empty project or a load error. Loading/error must not falsely present a successful empty list. Task priority sorting never changes runner queue order.

### Components, implementation handoff and verification

The frames copy `J9467` (Tasks), `Twtqc` (Overview), `jgqPe` (task page) and `l2YA4` (form). They reuse `zo9Py`, `clVjO`, `zemhy`, `DeizF`, `u1XBP`, `ZGwlv`, `SOW6S`, `L20XvS` and `mBkeJ` through scoped instances/overrides. Shared masters and unrelated frames were preserved. Chips use existing raised/border/text roles, controls use the existing select/button styles, and confirmations use the established backdrop and panel styling. Keep the existing theme mapping and audit resolutions; do not introduce near-duplicate tokens. Name structural dimensions once beside their owning component with the design node ID; the expanded workflow/run cell is 212 wide in frame 01, with 184 for readiness.

Proposed locale groups: `tasks.workflow.*`, `tasks.priority.*`, `tasks.blocked.*`, `tasks.tags.*`, `tasks.filters.*`, `tasks.sort.*`, `tasks.selection.*`, `tasks.cancel.*` and `tasks.history.*`; reuse `common.*` and existing `runs.status.*` for shared controls and system labels. Every visible message needs a complete translation key and exact placeholder parity in shipped locales. Task titles, tag names and reasons are user data. Every visual value must map through existing theme roles or a named theme addition sourced from a Pencil node; no UI literals.

Transitions, confirmation-dependent cancellation, dependency readiness and history belong in core/store, independent of desktop components. History records the actor, time and old/new values; automatic transitions are attributed to Orchester, not an agent. Preserve history through renames/merges/deletions without relying on a deleted tag's live name. Implementation must cover transitions, cancel confirmation/failure, blocked readiness, priority queue independence, sorting/filter combinations, bulk outcomes, tag uniqueness and deletion in filtered core/store/runner and headless form_keyboard tests. Follow with `tools/check.ps1` and independent app/Pencil verification of the approved nodes at matching sizes; inaccessible states must be reported.

All 24 frames received `get_screenshot` and `snapshot_layout` inspection in the original design pass. Corrections covered column alignment, empty chip sizing, dependency-menu width, priority icons, selected bulk checkboxes and fixture identities; changed frames were rechecked, with no remaining reported layout problems. On resuming this session, the same 24 node IDs were confirmed present without recreating frames or repeating completed visual checks. These are static design checks, not independent verification of an implemented app. No Rust code, stable installation, processes or design-file disk contents were changed. No runtime tests, app design verification or macOS cross-check were run in this design step.

Repository gate (2026-09-30): `powershell -File tools/check.ps1` **passed** (formatting, workspace Clippy with warnings denied, file size and UI literals), using the authorized worktree-local `CARGO_TARGET_DIR=.tmp/orc39-check/target`. The initial attempt could not write to the inherited target outside the worktree; the local-target retry resolved that access failure. `git diff --check` passed. Only this handoff document changed in the worktree; ORC-39 is not moved to Done in the implementation backlog because the approval and implementation steps remain.

**Decisions (user, 2026-10-03).**
- *Organization history* (`D1DC6T` content) sits at the end of the task's Summary, since ORC-61's Conversation timeline replaced the Activity tab that held it.
- *A Canceled task is skipped by its group's scheduling.* The group never starts a Canceled member that has not started (no run, or only a Pending first run), so its status never flips to In progress, and goes on with its other tasks as if that member had ended; a group whose remaining members are all Canceled finishes. Its row reads *Canceled · not started by its group*; a Start request for it is refused with the same reason.
- *Existing tasks get a status from their runs* when the organization migration (39) runs: a run that has not ended (pending, running, paused, interrupted) → In progress; latest run merged → Done; latest run ready for review (succeeded, not merged) → In review; a queued task without one of those (no run yet, or a latest run that failed or was cancelled) → Todo; any other task (a draft, including one whose latest run failed or was cancelled) → Backlog. No history row is written for it.

**Review follow-ups (2026-10-03).** Saving Edit task writes only the organization fields the person changed from what the form loaded, over what is stored now (a status Orchester set meanwhile, or tags and priority a bulk change set, stay). A confirmed Canceled stops only the run the confirmation named; a run that started after it opened is not stopped, and the task is not canceled (*A run started after the confirmation opened…*). Only a change *to* Canceled waits for an active run; a Canceled task's other fields stay editable. A bulk change counts only the tasks it changed and names the rest (*1 selected task was already Todo.*).

Implementation handoff: [ORC-39 implementation](../evidence/orc39-implementation.md). Approval and readiness policy B are recorded above; implementation checks pass, and independent Review/app-Pencil Verify remain.

## Project orchestrator conversation

Orchestrator is a permanent project navigation entry between Overview and Tasks. It opens a project conversation without requiring a task. New conversation starts a separate discussion; History finds prior discussions within the selected project. The empty composition state is `dOIrp`; the ten-task proposal state is `I7TCI`. These are proposed product interactions, not an implemented chat service.

The project chat handles ideation, decomposition, queue coordination and cross-task progress questions. A task's Conversation stays scoped to that task/run, with Orchestrator or a participating agent as recipient. Its panel now explicitly says Task conversation and To: Orchestrator · this task. Opening a task or project chat preserves the other conversation and its draft. A project message is not silently broadcast into every agent's task context.

The header identifies the project Orchestrator profile and model; Agent profile links to the existing profile editor. Project instructions apply to the project coordinator, and Manage context shows attached files, selected tasks and applicable source versions. Chat text alone does not publish permanent instructions. Repository access is not proof the entire repository was read; responses must identify sources actually consulted. Tool credentials and raw secrets remain excluded. Provider/model names in this fixture have the same unverified illustrative status as the other model screens.

The shown user asks to plan from an attached scope.md. The Orchestrator proposes ten briefs with acceptance criteria, a merge dependency, parallelism of two tasks and a 14M-token aggregate cap. The proposal's local task numbers are not persisted task IDs. Review & create tasks opens the existing bulk brief editor (`OqmXc`) populated with that proposal. Selecting a proposed row opens its brief in that editor. Create 10 tasks then saves drafts and returns confirmed IDs/links to this conversation; it does not queue work. Run selected opens the existing parallel or sequential scheduling review with the proposal settings carried over. Edits preserve the conversation/proposal link. This is one connected flow, not a second task or scheduler implementation.

Natural-language queue commands use the same project actions, permission checks, scheduling limits and plan/tool approvals as direct UI controls. Resolve the intended task/group scope; ambiguous requests need clarification. An explicit authorized action need not acquire an unrelated extra chat approval. Changes and their outcomes remain inspectable with exact affected task links. Requests to pause scheduling hold new starts; pausing a working task is a separate action at its checkpoint. Progress answers read actual run state and link to tasks; an unavailable status source is reported as unavailable. Fresh authorization, missing credentials or a changed configuration cannot be bypassed through chat.

The composer is the existing `C3nu0z` promoted to Conversation / Composer and reused by linked references; `SGjO0` is the new Orchestrator / Proposed task row. Existing message, button, status, property and navigation components are reused. The composer is disabled while empty. Suggestions populate an editable prompt rather than dispatching an implicit operation. Enter adds a newline; Ctrl/Cmd+Enter sends. Escape closes History/context overlays and restores focus. History and context overlays are specified entry points, not additional drawn screens.

Proposed runtime states: preserve drafts on failed sends and show Retry; deduplicate message and action retries; distinguish Sending, Thinking, Delivered and action-confirmed outcomes. A response failure must not erase the user's sent message or existing proposal. Stop response only stops response generation, not running tasks. Provider access failure links to Connections while retaining the conversation. Loading must not impersonate a new empty conversation. Revalidate proposal/configuration versions before creation or launch; never apply stale settings silently. Conversation retention, context compaction and history search need an engineering contract. Proposed copy namespaces extend `orchestrator.*`, `conversation.history.*`, `conversation.context.*` and `orchestrator.proposal.*`.

At narrower widths the task proposal becomes an on-demand panel above the composer, keeping chat usable; collapse project navigation first. Keyboard focus, panel transitions, persistence, response streaming and actual routing require runtime acceptance tests. [Project chat overview](previews/organization/I0Ij1T.png).

Implemented, first version (OR1, 2026-10-14, the new app; `orchester_core::orchestrator`, `orchester_workspace::orchestrator`, `screens/orchestrator`). The sidebar's *Orchestrator* opens the conversation with the latest message, or a new one (`HvzWI`: the four suggestions fill the prompt and send nothing; the line under the composer says each reply gets the Orchestrator's rules and the conversation). Opening one from History shows its title and *Opening from History* while it loads (`kDOn9`). Each message a person sends is one turn of the CLI the Orchestrator runs on, which the project's runner starts in the project folder with a system prompt of its own and only the chat's domain tools (`docs/stable-build.md`, "Embedded MCP server"):
- Claude Code: only `Read`, `Glob` and `Grep` (no shell, no edit tool), with no allow rule and with the CLI's own `permissions.blockReadsOutsideWorkingDirectories`, so reads stay in the project folder; the runner's private files, the account's CLI folder and, for the Default account, `~/.claude.json` are denied by rule as well.
- Codex is not offered (settings list it with why). Its `read-only` sandbox keeps writes out but still gives the agent a sandboxed shell that can read `CODEX_HOME`, other repositories and the runner's private files, and the user's execpolicy `allow` rules run matching commands outside the sandbox. The runner has no Codex turn until those reads can be denied and shown to be on Windows and macOS; the one written for it (App Server, the chat's tools as its only MCP server, the thread resumed) is in the branch's history.

The next turn resumes the session while the Orchestrator stays on the same connection and account. When either changes, a new session starts and the conversation says so. One answer takes at most 30 agentic turns and 10 minutes. The chat's tokens count toward the project's daily budget and today's usage: a used budget refuses a turn as it refuses runs. It takes no agent slot.

The conversation keeps, per project in its database:
- messages and replies;
- the actions the agent took through the tools: a card each, done or refused with the tool's reason, with *Open* for the task it names;
- how each turn ended.

The agent's `run_task` starts nothing. It proposes the start, and the person takes it or not: *Start <task> on <pipeline>?* with *Start* (person-only; synthetic input is refused visibly) and *Dismiss*.

While it answers, the screen shows the working line and *Stop response*, which ends the CLI only. How a turn ends shows as follows:
- stopped, or cut short: an event line (*Response stopped by you · 14:33 · …*) and *Ask again*;
- failed: a danger notice with *Retry*, and *Open Connections* (not signed in, the account) or *Open settings* (the connection is gone);
- never taken by the runner: the message is not kept, and its draft stays with *Retry*, which sends it once.

History lists the project's conversations by the day they last changed, with a search by title; Esc closes it.

Orchestrator settings (`/orchestrator/settings`) choose a Claude Code connection, the model (with *Other model id…*) and the reasoning effort. They are versioned as agent profiles are (`qAq0z`): the first version is published from the form; after that a change is saved as a draft (*Save draft*) and published as the next version (*Publish vN*), which the chat answers on from its next reply. The Actions, Context and Conversations tabs are not built (locked, with why).

New starts can be held from Tasks (*Hold new starts* / *Release*, and a *Release* for each held group), from Orchester's MCP server and from the chat (`hold_queue`, `release_queue`). A hold covers the project or one reviewed group. Running runs go on; only first runs wait. No frame draws this in Tasks, so it uses the header's controls.

Not built: the bulk proposal editor, *Manage context* and attachments (shown unavailable), role instructions and project memory, history compaction, and the 960 and light passes.

## Task conversation

*Superseded 2026-10-01 by [Conversation (merged with Activity)](#conversation-merged-with-activity); the rules below still apply where that section does not change them.*

The task has a visible Conversation tab, an Open conversation entry on the review summary, and a conversation panel beside running/paused progress. The recipient defaults to Orchestrator; the picker can target a participating agent directly. Orchestrator routes messages, while direct messages retain their selected recipient. Messages belong to the task and record run, step, recipient and author.

Delivery states distinguish Queued, Delivered and Acknowledged. Acknowledgement does not prove the requested change was applied. Work-changing messages are delivered at the next checkpoint of their recipient; Pause is the visible control for an urgent interruption. While paused, messages to the suspended agent remain queued and are delivered before that agent's next action on resume. A direct message to a different agent waits for that agent's next turn, which may be later in the pipeline. Scope changes return to plan review rather than silently changing approved work.

Proposed interaction contract: failed or pending sends retain the draft; retries deduplicate the message; Enter adds a newline and Ctrl/Cmd+Enter sends. Escape closes the recipient picker and restores focus. Keyboard, delivery and persistence behavior remains to be implemented and tested.

Implemented (M8, 2026-09-27): **the Conversation tab** is 02.12 for every state of the task: the active work (`FjbKY`: notice, progress rows, *Working context* with branch, last checkpoint, changed files ↗ Changes, budget and permissions) beside the task conversation panel (`kjsmd`). The panel's status label says where the conversation stands (*Live*, *Waiting for you*, *Paused*, *Stopped*, *Interrupted*, *Ready for review*, *Ended*, *Not started*); the recipient select offers *To: Orchestrator · this task* and each agent of the run's pipeline; the participants line groups them as working, asking and available. Messages (`J5MIT`) are the person's — *Queued · waiting for the next checkpoint* (or for the run to start or continue; *Not delivered · the run has ended*), *Delivered to Builder*, *Acknowledged by Builder* — and the agents': the reply that ended the turn they read messages in (*Reply · Implement*) and questions (*Question · waiting for your answer*, on the raised fill with *Reply →*, which puts the focus in the composer; *Question · answered* once the step works again). The raw transcript stays in Activity. The composer (`C3nu0z`) grows to six lines; Enter adds a line, Ctrl/Cmd+Enter or *Send* sends; the draft stays until the message is stored, once per draft; *Attach* is unavailable (attachments are not built). The line under it says what sending does now. The policy line under the active work says only what is true (*An agent reads your messages when its current turn ends*); scope changes are not routed back to plan review. Runtime rules are in `docs/architecture/storage.md` ("Task conversation", "Questions", "Follow-ups"). On Ready for review (02.13) *Ask agent* opens the conversation with the focus in the composer and *Open conversation* opens it; a message to a run ready for review follows it up and it is ready for review again when the step is done. What the run did (readiness, stages, what changed, verification) scrolls there in a viewport with the [scroll gutter](#components-and-tokens), so the actions stay at the bottom with any number of steps. A run whose work was merged takes no messages: its panel reads *Merged*. When a run succeeds the status bar's latest event reads *Run #128 ready for review* (state A) and opens the task. A question counts in *Needs you*: the Overview card is `Mt22P` (*CLARIFICATION*, the question, *Builder is waiting · ORC-12*, *Answer question →*, which opens the conversation), the Summary's notice reads *Builder asks you a question*, and the status bar's latest event reads *Builder asks · ORC-12* and opens the conversation.

**Drawn as built (2026-09-27, `GvUXd` (removed 2026-10-04; now `A5H6ln` and `LpVFY`)).** *02.S · Working task / Conversation as built* (`JOG1i` (removed 2026-10-04; now C·01 `kpnmP`)) is 02.12 with the app's copy: the notice *Implement is running* (loader) · *The agent works in the run’s own worktree and branch; your checkout is not touched.*; one progress row per step (*Research · Explorer* … *Review · Reviewer*) with *Complete · attempt 1 · 84k tokens · 3 min*, *Working · …* or *Waiting*; the policy line *Questions and clarifications stay in this task. An agent reads your messages when its current turn ends.* In the panel the participants are agent roles joined with commas (*Builder working · Explorer, Planner, Reviewer available*); the agent's reply is its own entry (*Builder · 12:09*, the reply, *Reply · Implement*) after the message it acknowledged (*Acknowledged by Builder*); the composer is empty, so *Send* is disabled (0.4) and *Attach* is unavailable (0.4, trailing lock). *02.S2 · Conversation of a merged run* (`m3gLZ` (removed 2026-10-04; now C·14 `Y6gMx` (A))): the ready heading (status label *Merged*, *Run #128 · Feature delivery · Finished at 14:32*), no run controls, the notice *Merged into main* (git-merge) · *main moved forward to fa7e675 at 16:35. Its worktrees and merged branches were removed.*, every step *Complete*, the permissions row without *change ↗* and no policy line; the panel's status label reads *Merged* (git-merge), the recipient select is disabled (0.4), everyone is *available*, the composer's field reads *No agent of this run reads messages any more.* (`$muted`), *Send* is disabled and the line under it reads *This run's work was merged. Messages no longer reach its agents.*

**Conversation states drawn 2026-09-29 for U6 (`GvUXd` (removed 2026-10-04; now `A5H6ln` and `LpVFY`) row 3, based on 02.S `JOG1i` (removed 2026-10-04; now C·01 `kpnmP`)).** The policy line under the active work shows only while an agent invocation runs, so none of these three has it. *02.S3 · Interrupted* (`sYuiX` (removed 2026-10-04; now C·14 `Y6gMx` (B))): topbar *Continue run* (play, primary) and *Cancel run*; *Run 128 · Attempt 1 · interrupted at Implement*; the notice *The run was interrupted at Implement* (circle-pause) with the built detail; the Implement row *Interrupted* (circle-pause); the panel's label *Interrupted* (circle-pause), the participants line *Builder interrupted · messages wait in this task*, a direct message *You to Builder · 12:11* · *Queued · delivered when the run continues*, and under the composer *Messages stay queued until the run continues.* *02.S4 · Not started* (`Ok14G` (removed 2026-10-04; now C·06 `FBwIV`, C·14 `Y6gMx` (H))): a draft task, no run controls, *Draft · not queued for a run*, the notice *This task is a draft* (pencil) · *A run starts from Create & run once the task has every detail.*, no progress or working context; the panel's label *Not started* (clock), *No agent takes part in this task yet*, *You · 12:11* · *Queued · waiting for the run to start*, and *Messages reach the first step when the run starts.* A queued task whose run has not started uses the same panel with the notice *The run has not started*. *02.S5 · Recipient menu open* (`cj2a2` (removed 2026-10-04; now C·14 `Y6gMx` (C))): 02.S with the recipient select open: the trigger keeps the focus stroke and shows chevron-up; the menu (the `u0ByM` style, 400 wide, 4 below the trigger) lists *To: Orchestrator · this task* (checked) and one option per agent of the run's pipeline (*To: Explorer*, *To: Planner*, *To: Builder* hovered, *To: Reviewer*); see [Select menu](#select-menu). Paused and Stopped follow the same rules: *Builder paused · messages stay in this task* with *Queued · delivered before the next action on resume* for Builder and *You can write while paused. Resume keeps this run and its saved versions.*; *Builder stopped at the limit · messages wait in this task* with *You can write while the run is stopped. Continuing keeps this run and its saved versions.* A direct message to a later agent, such as Reviewer while Builder is active or suspended, instead reads *Queued · waiting for this agent’s next turn*. The saved Pencil frames do not depict this later-recipient variant.

## Task and project memory

Behaviour only (decided 2026-09-30, Memory 1/3); the screens come with Memory 3/3. Agents keep what they learn as **memory records**, a first-class, versioned record kept apart from the conversation, the run's events and its artifacts. Everything lives in the project database (SQLite built into the app): no server, no vector database and no embedding model.

**A record** belongs to one task (task memory) or to the whole project (project memory). It says what kind of knowledge it is (fact, decision, observation, artifact, handoff, to do, warning or summary), has an optional key (a short name such as `db.engine`; a key names at most one active record of its scope, and a second is refused), its text, tags and an importance (low, normal, high). It names its author: the agent (its profile, and the run, step and execution it worked in, with the transcript entries it came from, and the agent's confidence: low, medium or high, which people do not change) or a person. Its status is *proposed* (waiting for a person's approval), *active*, *superseded* (a newer record replaced it), *deprecated* (no longer true; a person may restore it), *rejected* (a proposal a person turned down; it is never restored) or *withdrawn* (a proposal its agent replaced with a newer proposal before anyone decided; it was never a version of anything). Only active records reach agents. A record whose stored row can no longer be read back is left out of listings, searches and prompts; the run's activity says how many were left out; the records are checked once each time a run's work starts or resumes. A check that fails is reported as well, and the agent starts all the same.

**How records change.** A person creates, edits, pins, deprecates, restores (a deprecated record only), approves or rejects records. A person's edit changes the record in place: the author stays and the record is marked as edited by a person. Superseding is the agents' path: an agent's newer record names the one it replaces, and once the newer one is active (at once, or when a person approves it) the older one is superseded and its pin moves to the newer one. Only a person deletes a record, and deleting it deletes its history (the earlier versions it really superseded) too, never the active record a proposal would replace, and never another record's history through a withdrawn proposal; invocations that were given it keep a reference that shows as deleted. Agents are never offered deletion. Every change names the revision of the record it was made from; a change made from an out-of-date copy is refused, so a stale edit or pin never overwrites a newer decision, and the runner reads and changes a task's records in one transaction. In Memory 1/3 the only records agents write are handoffs.

**Handoffs.** A step's structured result may carry, besides `outcome` and `summary`, the lists `completed`, `decisions`, `open_questions` and `next_agent_should_know`. They are read leniently: missing or null lists are empty, a single string is one item, other items and unknown fields are ignored, so a malformed handoff never costs the step its outcome. The runner keeps a step's handoff as task memory, written by the step's agent, once the step's fate is known: a step that passed (a gated step once its plan is approved; a proposed plan keeps nothing) leaves one handoff record with the summary and the lists and one decision record per decision; a step that failed leaves a handoff that says the attempt failed, without decisions. One handoff per step stays current: a step's new handoff supersedes its earlier one, its new decisions deprecate the decisions of its earlier executions, and its earlier proposals no person decided on are withdrawn. The latest passing attempt defines the step's decisions, even when it made none: once its handoff or one of its decisions is active (at once, or when a person approves it), the decisions of the step's earlier executions are deprecated in the same change, never those of a newer execution (an older proposal approved late leaves them alone; the runner also withdraws a step's older proposals when a newer execution keeps its handoff). A failed attempt's handoff is tagged `failed-attempt` and replaces no decisions. The handoff is kept before the step's end (or the plan's approval) is announced.

**Memory setting (Project settings › Memory).** For task memory and for project memory separately, how agents' writes are kept: *Save automatically* (active at once) or *Save after approval* (stored as proposed and never given to an agent until a person approves it). Defaults: task memory automatically, project memory after approval. Records already proposed stay proposed when a scope switches to saving automatically. The same setting limits what one agent invocation gets: records per invocation from 1 to 50 (default 8) and tokens per invocation from 500 to 20,000 (default 3,000); other values are refused. The setting is read whenever an agent starts, so a change applies to the next agent.

**What an agent gets.** Every invocation, new or resumed, gets a *Task memory* part in its prompt (a new session before its task, a resumed one before how to finish), chosen from the active records of its task and of the project:

1. the task's latest handoff;
2. pinned records;
3. the task's active decisions and warnings;
4. the records that match its step, found by full-text search over their text, key and tags with a query made from the task's title and brief first, then the step's name, then at most 16 terms of the step's instructions; common English and Ukrainian words are left out, and words of seven letters or more lose their last two. Two SQLite FTS5 indexes are searched: one of words (BM25 ranking), searched by word stems as prefixes, and one of trigrams, searched by fragments as substrings, which finds other forms of a word (Ukrainian `міграції` and `міграцію`), file paths and identifiers inside longer ones (`StepResult` in `parseStepResult`).

Within each group records are ranked by relevance (0.40), importance (0.20), recency (0.15, halving every 14 days), a role match (0.15: written by the same agent profile, or tagged with the agent's role) and confidence (0.10). When there are more matches than the other groups leave slots for, a match below a fifth of the best match's relevance is not given. A record with the key or the text of one already chosen is left out, and records are taken in order within the record and token limits (tokens estimated as one per four bytes of text). The latest handoff is always given: when it alone exceeds the token limit it is cut to fit, with a marker saying how much is shown. What each invocation was given — which records, why, their estimated tokens, whether a record was cut — and which ranked records the limits left out are recorded with the invocation.

In the prompt every record's text is quoted line by line (`> `), so nothing a record says can pass for a heading or a section of the prompt. A record's key is quoted on its label line, and keys hold no line or paragraph separators. The latest handoff leaves out the decisions given as records of their own, and its summary (everything before its lists) when the prompt already shows it among the results of earlier steps. What the next agent of a step would receive is computed by the same code, so the task's *Next agent receives* panel shows exactly what the next invocation gets while nothing changes.

**Secrets.** Before a record is stored, its text, key and tags are checked against the values of the project's secrets in the keychain, regardless of case; a value shorter than eight characters counts only as a whole word. A record holding a value is refused, and the refusal names the secret, never the value. Encoded (for example base64) or split values are not detected. A handoff that is not kept is reported in the run's activity (*The step's handoff was not saved to task memory*, with the secret it holds, the keychain that could not be read, or the database error when the project database could not store it) and the step goes on: keeping a handoff never blocks a step's pass or fail.

## Token budgets

Decided 2026-09-25 (user): budgets are measured in tokens, not money. Every former money amount in the design is now a token amount: new task (`l2YA4`), bulk limits (`OqmXc`), group and per-task caps (`W1Dq6`, `IyJ5a`), orchestrator proposal (`I7TCI`), run snapshot rows (`RD4pc`, `x3lVC`, `jgqPe`, `vpgrJ`, `Oqmog`, `J3JZaT`), step budget (`Q21HOA`, `zfjsK`, `yf6XH`, `UjHDL`), setup run limits (`x1YpfN`, `Q7htyU`), PR fix request (`Rrmhf`), the Overview metric (`Twtqc` and the lifecycle backgrounds) and the status bar item (`pmOpt`, states `keJ9Z`). Fixture values: 1.4M per task, 14M per group, 400k per step, 20M per project per day. The fixture task "Add task cost breakdown" is now "Add task token breakdown".

**What counts.** Token counts come from the usage each CLI reports (per turn or per invocation), never from estimates. Counted = input + output (reasoning output is part of output). Cached input is shown separately (*Cached input · 1.2M · not counted*) and does not count toward any budget. Where a CLI reports cached tokens as part of input (OpenAI-style usage), counted input = input − cached; where it reports them separately (Claude's cache reads), they are simply excluded; cache writes count as input. Field names are unverified. Money fields (`total_cost_usd` and similar) are ignored. Because usage arrives after a turn, a run can end slightly above its budget; the UI shows the real figure, never a clamped one.

**Auto mode's reviewer** (decided 2026-10-07). The reviewer that judges a run's requests in auto mode spends tokens on the person's subscription or API key like an agent, so each review that asked a model is recorded with the request it judged (run, step execution, invocation, request; provider, model, start and end, verdict, tokens): Claude Code's from the `usage` of the result line of its `--output-format stream-json` output, Codex's from the `usage` of `codex exec --json`'s `turn.completed`, a native connection's from the response's usage. Its counted tokens count like an agent's turn toward the run's budget (and with it a task group's cap), the budget of the step whose agent asked and the project's daily budget, when the review ends. A review that ran but reported no usage Orchester could read (it was stopped, timed out, or printed none) is recorded as unknown, never as 0, and counts nowhere; one that never started a model (no reviewer, a hard rule held the request, its CLI could not be started, a native request the API refused with an HTTP status) is not recorded. A native review's spend is unknown once any attempt may have been billed for more than it reported: a stream that broke after it began, a request lost on the way (a transport error does not tell a connection that never opened from a request sent and lost) or an answer in an unexpected form; a complete response the client asked again counts whole. The runner keeps the last 256 KiB of a reviewer CLI's output: Claude Code's result line carries the whole review's usage and verdict, so it is read as kept; a Codex output that lost its start (`thread.started`) may have lost a failure or a turn's usage, so the review counts as not run and its spend as unknown. The agent's end stops a review still under way and reaches the run only after that review recorded what it cost (an ended reviewer gets 10 s to report its exit, then counts as stopped, its spend unknown), so its tokens count once, toward the invocation that asked, and a review never holds up the step or the run. The run's token figures include the reviewer: *Run total* and *Budget* on the task page, `get_run`'s `tokens` on Orchester's MCP server, which adds `tokens.reviewer` (reviews, how many of them unknown, input, cached input, output). The invocation panel of the Conversation's *Details* (02.I `n0cEOG`) adds the line *Auto mode reviewer* · *1.1k tokens* after *Counted* for the reviews of that invocation's requests (*1.1k tokens · 1 of 3 not reported* when some are unknown), shown only once a reviewer was asked; no frame draws it. Step headers and the rail keep the agent's own figures. Plan limits a reviewer's Claude Code reports (`rate_limit_event`) go to the plan limits history like an agent's; `codex exec` reports none.

**Display rule** (specimen `T1n5BA`).

- Below 1,000: the exact number (`980`).
- From 1,000: `k`; from 1,000,000: `M`; from 1,000,000,000: `B`.
- One decimal while the scaled value is below 100, none from 100 up; a trailing `.0` is dropped: `12.5k`, `18k`, `250k`, `412k`, `1.4M`, `20M`.
- Round half up. A value that rounds to 1,000 of a unit moves to the next unit (`999,960` → `1M`).
- Add "tokens" wherever the label does not already say so (`412k / 1.4M tokens`; the Overview metric is labelled *Tokens today* and shows `3.8M`).
- Spend reads *used / budget*. Tooltips show the exact count, grouped for the locale (`412,345 tokens`).

Implementation sketch: `fn compact(n: u64) -> String` — pick the unit by the thresholds 1e3 / 1e6 / 1e9; if `n / unit < 100` round to tenths, else to a whole number, using integer arithmetic (half up) to avoid float drift; if the rounded value reaches 1000, move to the next unit; strip `.0`. Unit letters are not translated; the word "tokens" and the "used / budget" pattern are i18n keys with placeholders.

**Input rule** (user correction 2026-09-25): budget fields take **absolute whole numbers only**, never `1.4M` or `250k`. Field `sN3du` (*Field / Token budget*): the number, then a muted `tokens` unit inside the field. Digits are grouped with the locale's thousands separator while typing (en: `1,400,000`; user decision 2026-09-25); the caret stays after the same digit. In pasted text the locale's group separator, spaces and underscores are removed; a decimal separator, a unit letter or anything else makes the value invalid. A read-back under or beside the field shows the compact value (*= 1.4M tokens*; in the raise-limit panel also the difference, *600k more than now*). Validation appears under the field with an alert icon, and the field gets a 1.5px `$text` border — grayscale, like every other error:

| Input | Message |
| --- | --- |
| empty | a run, default or step budget: no limit, read back *No limit*; the daily project budget and a group's cap: *Enter a token budget.* |
| `1.4M`, `1.5`, letters | *Use digits only, for example 1,400,000.* (example grouped for the locale) |
| `0` | *Enter a budget above 0 tokens.* |
| more than a budget can hold (above 9,223,372,036,854,775,807) | *This number is too large for a token budget.* |

There is no minimum beyond more than 0 (user decision 2026-09-25) and no maximum: run, default and step budgets and time limits are optional, and empty means no limit, a normal state (user decision 2026-10-09). A run without a budget or time limit never stops at one; *Raise the limit* only appears for a run stopped at a limit it has.

**Project settings › Limits (02.M `c7e729`).** The sidebar already has *Project settings*; it opens a page with *General* (02.N `QV1af`), *Limits* and *Environment* (02.O `tXahd`) tabs. Budgets are project resources, so their defaults are project settings, not application settings. Limits holds: *Default run budget* (empty by default: no limit; prefilled in New task and bulk creation when set; no maximum), *Daily project budget* (20,000,000; status bar and Overview) and *Default time limit* (*No limit* by default). There is no *Maximum run budget* (removed by the user decision 2026-10-09: limits are optional and have no maximum). Settings save on change (*All changes saved*). Changes apply to new runs; runs already started keep their budget and time limit. The New task read-back is *= 1.4M tokens · input + output*, or *No limit* for an empty field.

**Agents working at once (ORC-80, decided 2026-10-07; implemented, not drawn yet).** Limits has a third section after *Time*: *Agents* (15/500) with the setting row *Agents working at once* — a whole-number field (`UnitInput`, 240 × 36, unit *agents*), laid out as the budget rows (name, description, field and read-back). Description: *Each agent at work in any run of this project takes one slot; command steps use slots of their own. Runs beyond it wait in the queue with the reason. A change applies at once: lowering it stops no agent, and new work waits until fewer agents work.* Read-back *1 to 16 · default 4*; outside 1–16 or not a whole number the field takes the error stroke and the field error *Enter a whole number from 1 to 16.*, and nothing is saved. It saves when the field is left or Enter is pressed, like the budgets, with the project's other limits. Unlike them it applies at once: the project's runner takes the new count without a restart (more slots start queued work at once; fewer stop no agent at work), and the notice's detail ends *Agents working at once applies at once.* The status bar's *Agents* `n/N`, the sidebar runner card (*n of N agent slots in use*), Overview's *Running now* (*n of N agents working*) and the scheduling review's capacity note (*Agents working at once: N (Project settings › Limits)*, with non-breaking spaces so *Project settings › Limits* never wraps apart) show the configured count as the runner reports it. Orchester's MCP server reads it in `list_projects` and sets it, with the other limits, through `update_project_limits`.

*Default time limit* is a select (`bPPG9`, `Field / Select` 240 × 36), not a free input; the read-back under it is *active agent time*. Open states in `u5Vn2` (drawn 2026-09-27 from the app): the menu (`Select menu` rules) offers *No limit* first, the default (user decision 2026-10-09), then eight fixed choices — **15, 30, 45, 60, 90, 120, 180, 240 minutes**; the selected one has the check. A stored limit that is none of the choices (set by an earlier version or in the project file, e.g. 50) is added in ascending order and selected, so the select never shows a value it can't offer; options beyond the menu's height scroll with the overlay thumb (`gGmy3`). Choosing saves at once (*All changes saved*); choosing another value drops the extra one from the list. Singular: *1 minute*. The run's own limits (New task, raise panel) stay whole-minute inputs, because a run limit can be any value above what was used.

The run-limits picker in bulk creation (`OqmXc`, *Run limits / task*) is a select that shows the compact value and opens the same inputs.

**Where budgets live.** Task/run budget (new task, bulk, queue), group cap (queue), step budget (pipeline step), project daily budget (status bar, Overview). Exhausting the group cap stops further scheduling with a reason; exhausting a run or step budget stops the run at its limit (below).

## Stopped at limit

Decided 2026-09-25 (user): when a run reaches its time or token budget, its agents are stopped, a checkpoint is committed and the run pauses as **Stopped at limit**. It continues only after the user raises the limit.

**02.L (`D6Nwxz`)**, built on 02.A Paused. The page description says what happened (*Stopped at the token limit at 12:31*). The top notice names the limit and the checkpoint (*Stopped at the token limit · checkpoint 7c1e9f2*) and states that nothing runs until the limit is raised (*Run 128 used 1.4M of its 1.4M tokens. Builder was stopped after its turn and its work was committed as a checkpoint.*). Below it, *Raise the limit to continue*: each limit with what was used (*Used 1.4M of 1.4M · Implement stopped mid-step*, *Used 24 of 30 min of active agent time*) and its input with the read-back (*= 2M tokens · 600k more than now*; *6 min left · pauses don't count*). Both are whole-number inputs with a unit (`tokens`, `minutes`; the time field `dpfJo` was a select until 2026-09-27). The token input is focused because the token limit was hit; after a time stop the time field is. The single primary action states the result (*Continue with 2M tokens*; after a time stop *Continue with 45 minutes*). The explanation says what continuing does: a new invocation of the stopped agent from the checkpoint, repeating the unfinished part of the step; the new limit applies to this run only. *Cancel run* stays in the top bar. Working context shows *Budget 1.4M / 1.4M tokens · limit reached* and *Last checkpoint 7c1e9f2 · 12:31 · saved at limit*. The conversation panel shows *Stopped*; messages queue until the run continues. The status variant *Stopped at limit* (octagon-pause) is in `P3HONM` for lists and the queue; such a run is not counted as running.

**02.L2 · Step budget (`A7nVbK`)**, the Summary tab as the app draws it. Page description *Run 128 · Attempt 1 · Stopped at the step budget of Implement at 12:31*; top bar *Cancel run* only. Notice (octagon-pause) *Stopped at the step budget of Implement · checkpoint 7c1e9f2* · *Implement used 412k of its 400k-token step budget in Run 128. Builder was stopped after its turn and its work was committed as a checkpoint. Nothing runs until you raise the step budget.* (the real figure, above the budget, never clamped). The panel is **Raise the step budget to continue** with one row only — *Step budget* · *Used 412k of 400k · Implement stopped mid-step* — and one focused token input (`600,000`, suggested = budget + half, rounded up to 100k; no maximum, and an empty input lifts the step's budget for the run, read back *No limit*) with the read-back *= 600k tokens · 200k more than now*; no time row. Explanation: *Continue starts a new Builder invocation in the same session from checkpoint 7c1e9f2 and repeats the unfinished part of Implement. The new step budget applies to Run 128 only; the pipeline keeps its own.* Primary *Continue with 600k tokens*. Errors: *Enter more than the 412k tokens Implement already used.* Below the panel the run's progress rows (`l3hyb`), then *Latest activity* (*12:31 Implement · Builder stopped*); on the right *This run* (Pipeline, Branch, Base, Last checkpoint, Model, Budget, Cached input, Time limit, Active agent time, Permissions — the permissions value wraps right-aligned with *change ↗* under it).

**02.L3 · Daily project budget (`F2aBX`)**. The run's own limits were not reached, so there is **no raise panel**: the run continues with **Resume run** (primary, top bar, beside *Cancel run*), like a pause. Page description *… Stopped at the daily project budget at 16:48*; notice *Stopped at the daily project budget · checkpoint 7c1e9f2* · *This project's runs used today's token budget. Builder was stopped after its turn and its work was committed as a checkpoint. Resume the run tomorrow, or raise the daily budget in Project settings and resume it now.* Resume while the day's budget is still used fails with the existing capacity message (*Daily token budget used — new runs wait until tomorrow or raise it in Project settings.*) as a field error under the page heading (02.L3 states `uOLP2`, drawn 2026-09-28: circle-alert 12 and Inter 11 in `$text`, padding 0 / 32 / 16, between the heading and the tabs); the run stays stopped. The status bar shows *Tokens today 20M / 20M*.

**Step row status.** While a run is stopped at any limit, the stopped step's row in the progress list reads **Stopped at limit** (octagon-pause, `Stopped at limit · attempt 1 · 412k tokens · 14 min`), never *Paused* — *Paused* is only a pause the user asked for. The task list shows the same label (see [Task rows](#task-library-bulk-creation-and-scheduling)).

**Summary column scrolls.** With the raise panel the Summary column is taller than a 960px window (02.L2): it scrolls in its viewport with the [scroll gutter](#components-and-tokens) (`ZVdRa`, thumb `PigH4`); *Latest activity* sits at the bottom when there is room and below the fold when there is not.

**Wording, unified 2026-09-27.** Token counts arrive when a turn ends, so every token-based stop (run budget, step budget, daily budget) lets the turn that used it finish: the sentence is always *{agent} was stopped after its turn and its work was committed as a checkpoint.* (without a checkpoint: *… was stopped after its turn; there was nothing new to commit, or the commit failed and Activity says why.*). A time stop ends the agent at once: *{agent} was stopped and its work was committed as a checkpoint.* The read-back *{step} stopped mid-step* stays for all of them: the step itself is not done.

Runtime contract: on reaching a token limit the runner lets the turn that used it end (the agent gets a short grace to exit; starting another turn ends it at once), stops new tool calls, ends the agent CLI process tree, commits the worktree state as a checkpoint on the run branch, then records the state — only then may the UI claim *Stopped at limit*. A time limit ends the agent at once, then the same. If the commit fails, the state is *Stopped at limit · checkpoint failed* with the uncommitted changes listed (not drawn). A step-budget stop behaves the same and names the step; other steps running beside it finish their invocations as for a pause. A raised limit can never be below what was already used. The time limit counts **active agent time only** (user decision 2026-09-25): time paused, stopped, or waiting for the user (plan approval, questions, permission requests) does not count. This is stated wherever the limit is set or shown: New task (`l2YA4`, read-back under Time limit), run limits in `W1Dq6`/`IyJ5a`/`OqmXc`/`RD4pc`/setup (*30 min active agent time*), the raise-limit panel (*Used 24 of 30 min of active agent time*) and Project settings › Limits.

Proposed copy namespace: `runs.limit.*`.

## Activity tab

*Superseded 2026-10-01 by [Conversation (merged with Activity)](#conversation-merged-with-activity): the transcript and the invocation panel move into the Conversation tab.* Since 2026-10-04 the frames of this section (02.I `SfXU1`, 02.I2 `iBzOj`, 02.I3 `BQgqg`, 02.I4 `tWO8E`, and the Activity frames of other topics: `BbGhA`, `IFmpE`, `HW9mB`, `IcGvI`, `a2PR7K`) draw the Conversation tab's *Show: Tools* view: no Activity tab, *Show* (Tools) and search in place of the step filter, the run-state chip and a pressed *Details*, step headers as dividers, and the invocation panel as the Details pane (head *Details* with *Steps*). The transcript rows are unchanged. The states board 02.I states (`hig8O`) was removed; its states are C·13 (D–F) and C·06.

The task page's Activity tab (`jQt8l`) is the agent transcript: a chronological, readable stream per step execution. **02.I (`SfXU1`)** shows working Run 128.

- **Toolbar:** step filter (*All steps · 3 executions*, a select listing each step execution with agent, invocation and status), *Show raw lines* switch (off by default), live state at the right (*Following live*).
- **Step execution header** (`mDnc3`): disclosure, step number, step · agent, provider / model · invocation and time span, counted tokens for that execution and a status label. Finished executions are collapsed (chevron-right); the running one is open.
- **Entries** (`N4SlQq`): time, glyph, one-line header with right-aligned mono meta, optional body text, optional output block. Kinds:
  - agent message — body text;
  - reasoning summary — collapsed by default; only the summary the CLI emits is shown;
  - command — the command, `exit N · duration`, output collapsible (collapsed on success, expanded on failure; long output shows the last lines first with *Show all*);
  - file change — path and `A/M/D · +added / −removed`; the row opens the file in the Changes tab (↗);
  - denied or failed tool call / stderr — alert glyph, what was blocked and why (*denied · not an allowed command*);
  - usage — *Usage reported* with `in · out · cached (not counted)` per invocation or turn;
  - unparsed raw lines — muted (60 % opacity), collapsed and counted (*2 unparsed lines*); shown line by line when *Show raw lines* is on. Nothing the CLI prints is dropped silently.
  - MCP server the agent works without — unplug glyph, *Working without MCP server docs* and why (04.G `a2PR7K`, see [MCP servers](#mcp-servers)).
- **Invocation panel** (right, 320px): step and attempt, agent and model, start time, input / output / cached / counted tokens, run budget and the permissions row (opens 02.J). Footnote: counts come from the CLI's reported usage; cached input is not counted.
- **Follow:** while the run is live the list follows new entries (*Following live*; footer *Builder is working · last event 2 s ago*, a real timestamp, never fake progress). Scrolling up stops following; a centred *Jump to latest · 6 new* button appears above the bottom edge and resumes following.

Compact states (`hig8O` (removed 2026-10-04; now C·13 `j0qLQz` (D–F) and C·06 `FBwIV`)): *No activity yet* (queued run; says why), *Loading activity for Run 128* (spinner; no skeleton that pretends to be entries), *Couldn't load activity* (the run is unaffected; *Open runner log*, *Try again*) and scrolled-up with *Jump to latest*. Secret values never appear in the transcript: tool output passes through the same redaction as logs before it is stored or shown.

Reused: `DeizF` status labels, `ZGwlv` select, `tfGRt` rows, `u1XBP` buttons, `K8ItM` switch. `mBkeJ` (Activity / Event) and `J5MIT` (Conversation / Message) were considered: `mBkeJ` has 74px rows with a subtitle and no meta or output slot, too loose for a dense transcript; agent messages in Activity are run output, not a conversation. Hence the new entry and step-header components. Proposed copy namespace: `runs.activity.*`.

Implemented (M9, 2026-09-27): **Changes tab and Ready for review.** The Changes tab has no frame of its own; it shows the latest run's diff from its base revision to its latest checkpoint with the pull request screen's file list (`h8PZJa`, `j9NDM`) and diff (`rz27k`, `vEZ42`), under a toolbar in the style of `SrS47` that names the two revisions, the checkpoint's step and time, and *Checkpointed*. What the run's worktree holds beyond its checkpoint (untracked files included) follows under *NOT CHECKPOINTED* and reads *Not checkpointed*; it is read again while an agent works. The diff is read with git and cached per pair of revisions; changes larger than the pull request limits (300 files, 20,000 lines) are measured, not shown. Before the first checkpoint the tab says *No checkpoint yet*. An Activity file change is one entry per file (*M · open in Changes* ↗; the CLIs report no line counts) and opens that file in the Changes tab, which keeps the file in its path. 02.13 (`vpgrJ`) shows on the Summary tab when a run succeeded; *Review PR* (`o5Txa1`) and *Ask agent* (`QzsSV`) are unavailable with the reason under them (no step opens a pull request yet; the task conversation comes later), as is *Open conversation*. Not shown until their data exists: the topbar *Review PR #42*, evidence rows (the run's steps verify the checkpoint's revision instead), review artifacts and the instruction snapshots row. The base is the run's base revision, not a branch; *What changed* lists what the steps reported.

Implemented (2026-10-02): **the Changes count.** The count on the Changes tab's label (`kUKxD`) and the Conversation's *N files · view changes* are the files the tab lists: the distinct files changed from the run's base to its latest checkpoint plus the changes in its worktree that are not checkpointed, a file changed in both counted once. Both are read for the task page whichever tab is open, so the label follows a new checkpoint, the run ending, the tab opening and, while an agent works, its worktree (read every few seconds while the task page is open in the window's active tab). The label shows no count while either part is being read, when either could not be read, when the changes not checkpointed are too large to list file by file, and before the first checkpoint while the worktree is clean; checkpointed changes too large to show count the files measured. A read again keeps the last count until it ends, but a read again after a failure, and the first read of a new run (*Run again*), show no count until they end.

Implemented (2026-09-27), **drawn 2026-09-27**: merging a run and clearing its worktrees ([storage.md › Integration](../architecture/storage.md#integration)), and Project settings › Environment. The frames draw the Ready for review Summary as the app builds it (these differ from the older 02.13 `vpgrJ`): breadcrumb *Kontur / Tasks / ORC-128* as one line; the heading's status label with *Run #128 · Feature delivery · Finished at 14:32*; tabs without counts; the readiness card *Ready for your review* · *Every step of the run passed. Its changes wait for your review.*; the steps as stages; *What changed*; *Verification* with *commit 8f3a2c1* and one progress row per step (*Complete · attempt 1 · 84k tokens · 3 min*); no review artifacts; *Run details* with *Base revision* and no instructions row. What the run did scrolls in a viewport with the [scroll gutter](#components-and-tokens); the actions and their hint lines (Inter 10 `$muted`, right-aligned, 8 below the row) stay at the bottom. *Review PR* is primary and unavailable (0.4, trailing lock) with *Review PR needs a pull request, which no pipeline step opens yet.* under the actions.

- **Merge into <branch>** — 02.P `GIiCb`: *Merge into main* (git-merge, secondary) between *Ask agent* and *Review PR*. The compact board 02.P states (`OP7dt`) draws the actions row available; *Merging…* (loader, disabled 0.4) while git runs, at the width of *Merge into main* (140), so *Ask agent* and *Review PR* do not move, and with the topbar *Remove worktree* unavailable (0.4) in every tab that shows the task (full screen 02.P4 `d3oSWA`, drawn 2026-09-29 for U2); unavailable (0.4, trailing lock) as *Merge into your branch* with a second hint *Merge is unavailable: this run didn’t record the branch your checkout had when it started (it started before Orchester kept it, or on a detached HEAD). Merge its branch yourself with git.*; and merged. 02.P2 `lDcdB`: a refusal or conflict is an emphasized notice (`q5LzI`: circle-alert and heading 12/500 in `$text`) between the viewport and the actions, *Not merged into main* · *The run’s changes conflict with your branch in src/auth/github.ts, src/auth/session.ts. The merge was undone, so your checkout is as it was. Merge the run’s branch yourself to resolve the conflicts.* (up to five files, then *…*); *Merge into main* stays for another try, and the viewport now overflows, so its gutter shows the thumb. 02.P3 `bMJUA`: the heading's status label reads *Merged* (git-merge); the readiness card reads *Merged into main* (git-merge), *main moved forward to fa7e675 at 16:35.* and *Its worktrees and merged branches were removed.*; no *Remove worktree* in the topbar; Run details' *Working branch* keeps the branch name and says it was deleted, *orc/github-auth · deleted* (while the app is still reading what the run left it shows the name alone, and *orc/github-auth · could not check* when git cannot be read); the actions are the scope and *Review PR* only — *Ask agent* is not offered once the conversation is closed (a proposal; the app still shows it). `OP7dt` also draws the card's other lines: *Merge commit 9c41e02 joined the run’s work into main at 16:35.* with *Its worktrees were removed. Kept, not merged: orc/github-auth-review.*, and *main already had the run’s work at fa7e675; recorded at 16:35.* with *One of its worktrees is left. Remove worktree in the top bar says what it holds.* Task rows: `mGbYs` ([Task rows](#task-library-bulk-creation-and-scheduling)).
- **Remove worktree** (folder-x, secondary, 36 high) is the topbar's only button for a run that ended and still has worktrees (02.P; *Remove worktrees* when there are several). It opens a confirmation in the frame of `jYL1V` (600 wide, over the `#000000A6` backdrop). 02.Q `LMrbK`: *Remove the worktrees of run #128?* · *Removes them from .orchester/worktrees with everything in them, build output included. Committed work stays on the branches: a branch whose commits are merged is deleted, the others are kept.*; a check row (`l3hyb`) per worktree — *Run #128* · file-diff *2 files not committed*, with the files in mono 11 `$muted` under it, indented to the title (*src/auth/github.ts, src/auth/callback.test.ts*), and *Review · ran beside other steps* · check *No uncommitted changes*; a row per branch — pin *orc/github-auth* *kept · not merged*, git-merge *orc/github-auth-review* *deleted · merged*; the switch (`K8ItM`, off) *Discard the uncommitted changes listed above*; *Cancel* (no icon) and primary *Remove worktrees* (folder-x), disabled (0.4) until the switch is on. 02.Q2 `g1HS8`: one worktree with *No uncommitted changes*, one branch kept, no switch, *Remove worktree* enabled. While removing, the primary reads *Removing…* with a loader instead of folder-x, at 0.4, and *Cancel* is unavailable (0.4) until git ends; behind the dialog the topbar *Remove worktree* and *Merge into main* are unavailable too (full screen 02.Q3 `g8ZALb`, drawn 2026-09-29 for U2; corrected in `jnagB`). After a failure *Cancel* is offered again. A failure is the error line at the left of the actions, wrapping beside its circle-alert (Inter 11/1.5 `$text`, the 12px icon centred in a 16.5 slot so it stays with the first line), e.g. *Git didn’t remove C:\Users\ivan\code\kontur\.orchester\worktrees\01J8Z6QK: Permission denied*; the primary is offered again. Both in 02.Q states `XIEn8` (drawn 2026-09-28, and built so).
- **Continue a failed run (02.S6 `vUPqr` (removed 2026-10-04; now C·05 `FeiRG`)).** The Conversation tab keeps a *Failed* status pill with circle-x,
  an enabled recipient selector limited to previously executed agents, and the existing composer. Its action is
  *Send & continue*, disabled while empty and *Sending…* during delivery. The hint reads *Your message continues
  this run from its saved work. The agent resumes its session when available; dependent steps run again.*
  Messages stay on this run and its branch; earlier failures remain in Activity and attempt history. A queued
  message reads *Queued · delivered when the run continues*. A removed worktree uses the existing closed composer
  and missing-worktree explanation, with the *Failed* pill retained. A failed pipeline with no executed agent
  disables sending and says *This run has no agent step to continue. Its saved work remains on its branch.*
  A successful recovery returns to the existing *Ready for review* state; a question uses *Waiting for you*.
- **After Remove worktree (drawn 2026-09-29: 02.Q4 `PacyY`, 02.Q5 `Y8hyj`).** A run ready for review whose own worktree was removed (or whose folder was deleted) can still be merged, but its agent cannot be asked again: it would continue its session in that worktree, and recreating it would bring back what the person just cleared. *Ask agent* stays between the scope and *Merge*, unavailable (0.4, trailing lock), with the hint *Ask agent is unavailable: the run’s worktree was removed, so its agent cannot continue.* under the actions, in the style of the Merge hint. The Conversation tab keeps the status label *Ready for review*; as in 02.S2 the recipient select is disabled, the composer reads *No agent of this run reads messages any more.*, *Send* is disabled and the policy line is left out; the line under the composer reads *The run’s worktree was removed, so its agent cannot continue. Messages no longer reach its agents.*, and a message still queued reads *Not delivered · the run’s worktree was removed*. The runner refuses a follow-up for such a run with that reason and leaves the run as it was. As built (U5): the readiness card keeps *Ready for your review* with the folder-x glyph and the one-line detail *Every step passed. The worktree was removed; committed changes remain ready.*; the topbar has no *Remove worktree*; *Merge into main* stays available and *Ask agent* is unavailable with its hint under the Review PR hint (02.Q4). On the Conversation tab the notice is *The run’s worktree was removed* (folder-x) · *Its committed changes remain ready for review. No agent can continue this run.*; the Permissions row has no *change ↗*; the participants line reads *Explorer, Planner, Builder, Reviewer cannot continue*; a message still queued reads *Not delivered · the run’s worktree was removed* (02.Q5).
- **Project settings › Environment** — 02.O `tXahd`, the third tab after General and Limits, laid out like Limits (content 760 wide, padding 24 / 32, gap 16): page description *Variables Kontur adds for its agents and command tools.*; *Environment variables* (15/500) and its explanation; the field label *Variables · NAME=value per line* (11 `$muted`); a mono 12 text area (`$sidebar`, `$border`, radius 7, padding 12, 4 to 12 lines, no placeholder); the hint *Stored and shown in plain text, so never put secrets here.*; the notice (history) *Changes apply to the next agent or command that starts* · *An agent or command already running keeps the environment it started with.*; the topbar says *All changes saved*. An invalid line strokes the area 1 `$subtle` (`BorderStrong`; 1.5 `$text` is the focus stroke) with the field error under it (circle-alert 12 and Inter 11, both `$text`, 4 apart), then the hint: the same state as the MCP server's variables in 04.E3 `Hpycl`.

### ORC-63 · Remove worktree for every ended run

Drawn 2026-10-04 at the user's request; built 2026-10-04 (branch `claude/orc63`, see `docs/plan.md`), design
verification outstanding. **Problem:** *Remove worktree* exists only in the task page's top bar
and only for the task's latest run, so the worktrees of older runs (and of interrupted runs) stay under
`.orchester/worktrees` forever (the Orchester project had 13). The app has no list of a task's runs except *Other runs
of this task* on the unreadable-run page (02.R `SUuwV`), and neither the Conversation (ORC-61, one timeline per run)
nor the top bar has a run switcher. So the run history is new:

- **Runs of this task** (approved by the user (2026-10-04)): a Summary section after the run's steps, on every task with a run. Heading
  15/500 with the count, the line *Each run keeps its worktree in .orchester/worktrees until you remove it. Committed
  work stays on the run's branch either way.* (11 `$muted`), then one row per run, newest first, hairlines between:
  *Run #N* (12/500; the shown run tagged *this run*), its status label (`DeizF`), what happened (11 `$text`), the
  worktree line (folder 12 + 11 `$muted`: *Worktree · no uncommitted changes · <branch>*, *Worktree · 2 files not
  committed · <branch>* with file-diff and `$text`, *Worktree removed Oct 3, 16:40 · <branch> kept* with folder-x, *No
  worktree*), when (mono 10 `$subtle`), and at the right *Remove worktree* (`u1XBP`, folder-x, 32 high; *Remove
  worktrees* when the run has several). At 960 a row stacks: run, status and the button on one line, what happened,
  the worktree line and when under it. As built (not drawn): the pages that lay out their own Summary end with it too,
  the plan review (02.11) after the plan in its scrolling viewport and the retry-limit page (02.B) after the evidence,
  which scrolls beside the decision panel. A worktree folder git forgot is read to its first 200 entries; past them
  its dialog row reads *Folder left behind with more than N files; all of them are deleted, including those not
  listed* (Run again's note says the same), and an entry that cannot be read is an error line under its row
  (*Couldn’t read <path>: <why>*), not a failure of the section; its dialog row counts only what was read (*Folder
  left behind with 1 file and an entry that couldn’t be read; all of it is deleted*). Counts use one/other wording.
- **Which runs offer it** (today's rules, extended to every run): a run with a worktree and no live work — failed,
  cancelled, ready for review (merged or not), merged with a worktree left, interrupted. No button while a run is
  running, waiting for you, pausing or paused; the worktree line says *Worktree in use · Remove worktree comes back when
  the run ends*. A run that never started says *No worktree*; an unreadable run says *Remove worktree is unavailable
  until run #N can be read* (lock). The latest run keeps its top-bar button as today.
- **The dialog is 02.Q / 02.Q2's**, for the row's run: *Remove the worktree of run #N?*, the same explanation, a check
  row per worktree (uncommitted files listed, indented), a row per branch (*kept · not merged* / *deleted · merged*), the
  switch *Discard the uncommitted changes listed above* gating the primary when files would be lost, *Removing…*,
  the error line. When another run of the task is at work it adds *Run #134 keeps working; its worktree is not
  touched.* (info 12, 11 `$muted`).
- **Interrupted runs** keep *Continue run* and gain *Remove worktree* (top bar and row). Their dialog leads with an
  emphasized notice (`q5LzI`, circle-pause): *Run #141 can't be continued afterwards* · *Continue run picks the run up
  from checkpoint 1b8e5c2 in this worktree. Without it the run stays interrupted for good; Run again starts a new run
  instead.*
- **After removal.** The row reads *Worktree removed <time> · <branch> kept* and has no action. An interrupted run whose
  worktree was removed shows the notice *Run #141 can't continue* (folder-x) · *Its worktree was removed at 11:50, so its
  agent has nothing to continue from. Its commits stay on <branch>. Run again starts run #142 in a new worktree, from
  main as it is now.*; *Continue run* stays in the top bar unavailable (0.4, trailing lock) with the lock line *Continue
  run is unavailable: run #141's worktree was removed.*; its step reads *Interrupted · can't continue*; Run details'
  *Worktree* reads *Removed · 11:50*. Failed and cancelled runs follow 02.Q4 / 02.Q5 (agent cannot continue).
- **Row states** (63.06): *Removing…* (loader, 0.4) on the row's button, and the other rows' buttons wait; a failure is
  the error line under the row's worktree line with the button offered again; removed.

| Frame | Node | Position |
| --- | --- | --- |
| 63.01 · Runs of this task / Remove worktree on ended runs (ORC-131: #134 running, #133 removed, #132 cancelled clean, #131 failed with 2 files) | `O0AMKY` | 48 / 100 |
| 63.02 · Runs of this task at 960 | `H9Z7D4` | 3088 / 100 |
| 63.03 · Remove worktree of an older run / Uncommitted files (#131, switch off) | `e4QBB` | 1568 / 100 |
| 63.04 · Remove worktree of an interrupted run / Continue run ends (ORC-132, #141) | `rkAU3` | 48 / 1148 |
| 63.05 · Interrupted run after Remove worktree / Can't continue | `LmTnc` | 1568 / 1148 |
| 63.06 · Run row states (compact): removing, failed, removed, live work, other ended runs | `VBR27` | 3088 / 1148 |

Canvas: group `j0VU3B` ("States · ORC-63 · Remove worktree for every ended run", 4896 × 2300) at y 10024 in 02.6
`v3U9c6`, which grew from 9988 to 12384; the sections below it in column D moved down 2396: 08 `McWTM` to y 25032 and
06 `V8h6d` to y 28608. Reused: `zo9Py`, `u1XBP`, `DeizF`, `tfGRt`, `q5LzI`, `K8ItM`, `SOW6S`; no new components or
variables. Fixture tasks, runs, branches, paths and times are illustrative. Proposed copy namespace:
`runs.history.*` (section, rows, worktree lines) and `runs.removeWorktree.*` for the new dialog lines.

**Decisions — approved by the user (2026-10-04), for the user:**

1. The run history is a *Runs of this task* section on Summary, not a run switcher or a menu (rows have no menus);
   the unreadable page's *Other runs of this task* becomes this section.
2. An interrupted run whose worktree was removed stays *Interrupted* in its record but no longer blocks a new run:
   *Run again* is offered (today an interrupted run counts as active, so it would block Run again for good).
3. *Stopped at limit* counts as live work (it can continue after the limit is raised), so it has no Remove worktree.
4. One removal at a time per task: while one row removes, the other rows' buttons wait (0.4).

## Conversation (merged with Activity)

Drawn 2026-10-01, built 2026-10-02 (ORC-61, see *Implemented* below). Group `fTfl8` (removed 2026-10-04; now `A5H6ln`) ("States · Merged conversation", last in 02 / Tasks & runs at x 0 / y 51875, height 4749; the section is now 56624 high). The user asked for one view where the person follows what the agents do and can chat with the working agent. Fixture: Kontur, ORC-128 *Add GitHub authentication*, Run 128 on Feature delivery (Research · Explorer on Claude / Opus 5.5, Plan · Planner and Implement · Builder on Codex / Sol, Run checks, Review · Reviewer); ORC-129 *Add repository switcher*, Run 129 queued. Every message, command, path, time, hash and count is illustrative. Proposed copy namespace: `runs.conversation.*` (timeline, composer, delivery), reusing `runs.activity.*` for the entries and the invocation panel.

**What replaces what.** The task page has one **Conversation** tab where it had *Conversation* and *Activity*; the tabs are *Summary · Conversation · Changes · Memory · Artifacts*. Its main column is one chronological timeline per run: the old Activity transcript (step executions, agent messages, reasoning summaries, commands, file changes, problems, usage) and the old conversation panel (your messages, questions, answers, plan decisions, follow-ups, Ask agent) in the order they happened. The step progress rows and *Working context* that 02.12 showed beside the conversation panel stay on Summary; the timeline's step headers carry each step's status and tokens. The invocation panel becomes the toggleable **Details** pane. *Ask agent*, *Open conversation*, *Answer question →* and *Reply →* open this tab with the focus in the composer.

Superseded (kept on the canvas, not redrawn):

- Activity: 02.I `SfXU1`, `BbGhA`, `IFmpE`, `HW9mB`, 04.H10 `IcGvI`, 04.G `a2PR7K` and 02.R2 `lmlkQ` (removed 2026-10-04; now C·14 `Y6gMx` (F)) as page layouts. Their entries (`N4SlQq`), step headers (`mDnc3`) and invocation-panel rows (`rd1Vo`, `F3XhQ`, `upPCd`) stay valid: entries appear inside tool groups and as problem rows, the panel rows in Details. From 02.I states `hig8O` (removed 2026-10-04; now C·13 `j0qLQz` (D–F) and C·06 `FBwIV`), *Loading* and *Couldn’t load* carry over to this tab unchanged; *No activity yet* is replaced by the empty state below and *scrolled up* by the follow rules below.
- Conversation: 02.12 `jgqPe`, 02.S `JOG1i` (removed 2026-10-04; now C·01 `kpnmP`), 02.S2 `m3gLZ` (removed 2026-10-04; now C·14 `Y6gMx` (A)), 02.S3 `sYuiX` (removed 2026-10-04; now C·14 `Y6gMx` (B)), 02.S4 `Ok14G` (removed 2026-10-04; now C·06 `FBwIV`, C·14 `Y6gMx` (H)), 02.S5 `cj2a2` (removed 2026-10-04; now C·14 `Y6gMx` (C)), 02.S6 `vUPqr` (removed 2026-10-04; now C·05 `FeiRG`) and 02.Q5 `Y8hyj`, and the side panel `kjsmd` (*Conversation / Task panel*) with its message row `J5MIT`. Their states and wording carry over to the composer and to the delivery lines (interrupted, paused, stopped, merged, worktree removed, not started, the recipient menu).

**Timeline.** The column is at most 840 wide. With Details closed it is centred in the tab; with Details open the 320 pane takes the right and the column fills the rest, so text does not reflow when the pane toggles. Items, top to bottom in time:

- **Step header** (`mDnc3`): number, step · agent, provider / model · invocation and times, tokens (*315k tokens · 43k before first edit*), status label. A finished step folds to its header (chevron-right); the running step and a step that waits for you are open, and so is the step the run ended on or stopped at — the last step that ran of a run ready for review, failed, waiting for a decision, paused or stopped (decided 2026-10-03, as `AzKYQ` (removed 2026-10-04; now C·04 `uDqzB`) draws 05 Review open). A step that never ran after a failure shows its header at 0.6 with *Not started* (circle-dashed). Opening a folded step shows its messages, groups, cards and result card.
- **Agent message** (*Conversation / Timeline message* `V17jfc`): bot glyph in a 24 `$raised` square, the agent (12/500), step and time (11 `$subtle`), the text at 13 `$text`, line height 1.6. Agent messages are never folded into a group.
- **Your message**: the same component on `$raised` with `$border`, radius 8, padding 10 / 12, user glyph on `$hover`, *You* and *to Builder · 12:05*, and its delivery line (icon 11 and text 10, below).
- **Tool group** (*Conversation / Tool group* `uoK7y`): a 32-high row indented 36 to the message text, hairline stroke, radius 7: wrench 12, the summary (12 `$muted`), *N tool calls · first–last time* (mono 10) and chevron-right. Expanded, the header turns `$text` with chevron-down and the group’s entries follow as Activity rows (`N4SlQq`, with the time column) inside the same stroke, at most 7 before *Show N more · 4 reads, 1 search*.
- **Cards** for decisions, on `$raised` with `$border`, radius 8, padding 14: header (glyph on `$hover`, title 12/500, step and time, status label at the right), body indented 36, actions under it. Question (*Builder asks you* · *Waiting for your answer*); plan (*Plan v1 · <task>* · *Waiting for approval*, the first plan lines and *Show the whole plan · 24 lines*, primary *Approve plan & continue*); tool approval (the ORC-41 request: server, action, target, *Deny* and *Approve once*). After the decision a card loses the raised fill and reads *Answered* / *Approved* / *Denied*.
- **Problem rows**: `N4SlQq` without the time column (time in the meta), glyph in `$text`: denied commands, *permission_denied*, refusals, runner problems, *Working without MCP server*, a handoff not saved. They always show on their own and are never folded into a group.
- **Result card** per step: border only, radius 8: glyph, *Review passed* / *Run checks failed* / *Research done* (12/500), what the agent or tool reported, duration and tokens, a one-line summary, links (*Changes · 4 files ↗*, *Summary ↗*, *Handoff in Memory ↗*). The run’s end is a centred hairline marker (*Run 128 is ready for your review · 14:32*).
- **Working line** at the end while an agent works: loader 12 and *Builder is working · last event 2 s ago* (a real time, never fake progress).

**Grouping rules.** A tool group is the longest run of consecutive tool events of one step execution between two items that are not tool events (a message, a card, a problem row, a result card or a step header). Tool events: commands, file reads and searches, file changes, MCP tool calls, memory searches and saves, reasoning summaries, usage reports and unparsed raw lines. The summary counts by kind in a fixed order — *Read N files · searched N times · searched memory · ran <command> · edited N files · N MCP calls* — naming a command when there are one or two (*ran pnpm test src/auth*) and counting them otherwise (*ran 5 commands*); kinds with no events are left out. A failed command stays in its group, is named with its exit code (*· exit 1*) and turns the group’s glyph into circle-alert in `$text`; opening the group shows its output expanded, as in Activity. A group that is still growing (the agent is calling tools now) ends with the action in progress (*· running pnpm test*, loader) — not drawn.

**Filters and search.** The toolbar (52 high) holds the step filter (`ZGwlv`, *All steps · 3 executions*), *Show* as a segmented control in the Usage period style (`$sidebar`, `$border`, segments 26 high, radius 5, selected `$hover`): **All** (default), **Messages** (messages, cards, problems, step headers and result cards; groups hidden and counted, *31 tool calls in 4 groups hidden · Show all*) and **Tools** (every tool event as a flat Activity transcript, messages as one-line entries; *Show raw lines* moves here — `q4fCXK` (removed 2026-10-04; now the Tools view in 02.I `SfXU1`)), *Search this run…* (180), then the follow state and *Details*. Search matches message text, commands, paths and output; items without a match hide, and a match inside a group opens that group (`a5hF4` (removed 2026-10-04; now C·08 `Hn1xv`); nothing found: `gLVQQ` (removed 2026-10-04; now C·09 `JPBHA`)). Picking one step execution shows only its items, a read-back line *Showing 01 Research only · 02 Plan and 03 Implement are hidden.* with *Show all steps*, and Details follows the picked step (as decided for `F3XhQ`). The composer is not filtered: it always addresses the run’s current state.

**Details pane.** *Details* (panel-right, 32 high) toggles the 320 `$sidebar` pane with the invocation panel: step and attempt, agent and model, started or ran, *Code index*, *Memory given*, tokens, run budget, tokens before first edit, permissions and the footnote. Pressed, the button is `$hover` with a `$subtle` stroke and panel-right-close. **Closed by default**, and remembered per person for every task: the tab’s job is to follow and to talk, the timeline needs the width for command output, and the figures in the pane are looked up now and then (each step header already shows its tokens). It opens on demand, and choosing a step’s tokens in its header opens it on that step.

**Following live (ORC-36).** While the end of the timeline is in view, it follows new items (*Following live*, radio). Scrolling up stops following (*Not following*, circle-pause) and shows *Jump to latest · 3 new* (arrow-down, 30 high) centred 16 above the composer; the count is of new timeline items, not raw events. Jumping, or sending a message, scrolls to the end and follows again. Opening the tab starts at the end, or at a card that waits for you.

**Composer.** Always visible at the bottom of the main column (dock: hairline above, padding 12 / 32 / 14), in the 840 column, with the recipient select and *Attach* (unavailable, 0.4) in its action row and one line under it that says what sending does now. Enter adds a line; Ctrl/Cmd+Enter presses the state’s primary action, except while an agent works, when it sends after this turn (below); drafts and retries follow the existing contract (kept until stored, sent once).

| Run state | Composer |
| --- | --- |
| An agent works | *To: Builder · working*; **Send now** (zap, primary) and **Send after this turn** (clock-3, secondary); Ctrl/Cmd+Enter sends after this turn, the safer default, and *Send now* is pressed only with its own button. Hint: *Send now reaches Builder at its next tool call, while it works. Send after this turn waits until Builder finishes its turn.* |
| A tool step runs, or the recipient is not the working agent | One **Send** that queues (*Queued · waiting for Reviewer’s next turn*) with the reason in the hint. |
| A question waits | *Answering Builder’s question from 12:09* above the field, the field focused, recipient fixed to the asker (0.4), **Send answer**; the agent continues at once. |
| A plan waits | *Changes to plan v1 · sent to Planner*, **Request changes** (undo-2), unavailable while empty; *Approve plan & continue* is on the card only. |
| Ready for review | *To: Builder · resumes its session* (any agent that ran), **Send follow-up**: the agent resumes its session in the worktree and the run is ready for review again when it finishes (Ask agent). |
| Failed or stopped | *To: Builder · continues the run* (agents that already ran), **Continue with instructions** (play, ORC-43): continues the run from its saved work; the agent resumes its session with the message, dependent steps run again. *Run again* (ORC-16) stays in the topbar, secondary on this tab. |
| Paused or interrupted (not drawn) | **Send** queues; *Queued · delivered before the next action on resume* / *when the run continues* (02.S3 wording). Superseded by ORC-79 (*The chat always answers*, below): **Resume and send** / **Continue run and send**. |
| Queued, not started | *To: Explorer · first step*, **Send when the run starts**; messages wait in the task. |
| Merged, worktree removed, no agent step | Visible but disabled: *No agent of this run reads messages any more.*, recipient and Send at 0.4, a lock and the reason (*The run’s worktree was removed, so its agent cannot continue.*, *This run’s work was merged.*, *This run has no agent step to continue.*). |

**Delivery.** *Send now* gives the message to the agent while its turn runs, after the tool call that is running: Codex through App Server, which steers the running turn (`turn/steer`); Claude Code through a stream-json user message on stdin (`--input-format stream-json`). That it lands after the running tool call is to verify in implementation, for both, on Windows and macOS; a connection that cannot take a message mid-turn offers only *Send after this turn*, with *Send now* unavailable (0.4, lock) and the reason. *Send after this turn* queues the message as the agent’s next input when its turn ends. A message to an agent that is not working waits for that agent’s next turn, as before. Delivery lines (icon 11 `$subtle`, text 10 `$muted`):

- *Sending now · Builder gets it at its next tool call* (loader);
- *Delivered mid-turn to Builder · 12:05, at its next tool call* (zap);
- *Queued · Builder reads it when its turn ends* (clock-3);
- *Delivered at end of turn to Builder · 12:09* (check); for *Send now* when the turn ended before another tool call: *… · its turn ended before another tool call*;
- *Could not deliver to Builder: <why>. Your message is kept here.* (circle-alert and text in `$text`) with *Send after this turn* and *Edit in composer*;
- *Not delivered · the run’s worktree was removed* (circle-x), and the other 02.S-family endings.

Scope changes still do not return to plan review by themselves.

**Frames** (x 48 / 1568 / 3088; rows at y 100 / 1148 / 2196 / 3244, labels 32 above):

| Frame | Node |
| --- | --- |
| Agent working: messages, collapsed tool groups, a mid-turn delivery and a queued message, composer in steer mode, Details closed. Canonical home of `V17jfc` and `uoK7y` | `RrScB` (removed 2026-10-04; now C·01 `kpnmP`) |
| A tool group expanded; scrolled up, *Not following* and *Jump to latest · 3 new*; empty composer, Send buttons 0.4 | `CxrSA` (removed 2026-10-04; now C·01 `kpnmP` and C·13 `j0qLQz`) |
| A question waits inline; the composer answers it | `ZxYGh` (removed 2026-10-04; now C·02 `BRx5g`) |
| Plan v1 waits inline: *Approve plan & continue* on the card, *Request changes* from the composer; no *Pause run* while the run only waits | `haIEI` (removed 2026-10-04; now C·03 `K4AOD`) |
| Ready for review: steps folded, the Review step’s message and result card, the run’s end marker, *Send follow-up* | `AzKYQ` (removed 2026-10-04; now C·04 `uDqzB`) |
| Failed at Run checks: the failing command with output, the result card, Review not started, *Continue with instructions* | `hep0l` (removed 2026-10-04; now C·05 `FeiRG`) |
| Details open (the invocation panel of the running step) | `gNa91` (removed 2026-10-04; now C·11 `oryKr`) |
| Step filter on 01 Research, Details following it | `LhLL8` (removed 2026-10-04; now C·10 `bdCsu`) |
| Run not started (queued): the empty timeline says why and lists the steps | `zxIMy` (removed 2026-10-04; now C·06 `FBwIV`) |
| Delivery states, the composer without Send now and unavailable, the inline approval card, an answered question, problem rows and the Messages view (compact, 1440 × 1445) | `OlLGD` (removed 2026-10-04; now C·14 `Y6gMx`) |
| ORC-61 · *Show: Tools*: step 03 as a flat Activity transcript with the time column (`N4SlQq`): messages as one-line entries (message-square *Builder: …* · *message*; user *You to Builder: …* · *delivered mid-turn* / *queued · after this turn*), reasoning, commands, file changes, the failed `pnpm test` open with its output; *Show raw lines* (`K8ItM`, off) in the toolbar after search; *Following live* (x 1568, y 3244) | `q4fCXK` (removed 2026-10-04; now the Tools view in 02.I `SfXU1`) |
| ORC-61 · Search *cancel*: the field shows the query and × (clear); read-back *4 matches for “cancel” in 03 Implement · items without a match are hidden.* with *Clear search*; steps and items without a match are hidden; the matching tool group opens with only its matching entry and *Show 4 without a match · 2 reads, 2 edits* (x 3088, y 3244) | `a5hF4` (removed 2026-10-04; now C·08 `Hn1xv`) |
| ORC-61 · Search finds nothing (compact, 1440 × 363, x 1568, y 4292): whole run (search-x, *Nothing in Run 128 matches “refresh token”*, what was searched, *Clear search*); step filter on 01 Research with matches elsewhere (*Nothing in 01 Research matches “cancel”*, *4 matches in 03 Implement; 02 Plan has none. …*, *Show all steps*) | `gLVQQ` (removed 2026-10-04; now C·09 `JPBHA`) |

Reused: `zo9Py`, `TSJpn`, `SOW6S`, `u1XBP`, `DeizF`, `ZGwlv`, `mDnc3`, `N4SlQq`, `tfGRt`, `C3nu0z`, the invocation panel of `IFmpE` and the segmented period control of the usage dashboard. New components: `V17jfc` *Conversation / Timeline message* and `uoK7y` *Conversation / Tool group*, canonical in `RrScB` (removed 2026-10-04; now C·01 `kpnmP`). No new variables.

**Decisions (2026-10-01).** The user approved the frames and answered their open questions. Where a frame differs, these decisions win; the frames were not redrawn.

- **No *Acknowledged by Builder*.** The agent’s reply shows in the timeline, so a delivered message keeps its delivery line and gets no acknowledgement line.
- **Ctrl/Cmd+Enter sends after this turn** while an agent works, the safer default; *Send now* stays a separate button. The steer hint in `RrScB` (removed 2026-10-04; now C·01 `kpnmP`), `CxrSA` (removed 2026-10-04; now C·01 `kpnmP` and C·13 `j0qLQz`), `gNa91` (removed 2026-10-04; now C·11 `oryKr`) and `LhLL8` (removed 2026-10-04; now C·10 `bdCsu`) now ends *Ctrl+Enter sends after this turn.* (corrected 2026-10-02, ORC-61).
- **Send now lands after the running tool call.** To verify in implementation for Codex App Server and Claude Code stream-json input.
- **Tool approvals.** On the Conversation tab a request is the inline card. When the person is on another page, the approval dialog (ORC-41) still opens; *Needs you* and the status bar lead to the inline card.
- **Failed state.** *Continue with instructions* is the primary action; *Run again* is secondary in the topbar on this tab.
- **Progress.** The step header is enough while chatting; the progress rows and *Working context* stay on Summary.
- **Specified, not drawn.** The paused and interrupted composer states and the still-growing tool group are specified here only; that is enough.
- **Search** (2026-10-02, after ORC-61's Design step drew Tools view and search, `q4fCXK` (removed 2026-10-04; now the Tools view in 02.I `SfXU1`), `a5hF4` (removed 2026-10-04; now C·08 `Hn1xv`), `gLVQQ` (removed 2026-10-04; now C·09 `JPBHA`)): following live continues while a search is active, and matches are counted, not highlighted inside the text.
- **Markdown in the chat** (ORC-78, 2026-10-07; the frames draw agent text plain, so formatting follows plan review's, 02.11 `x3lVC`). Agent messages, the person's messages and notes, question cards, plan cards (their first lines, or the whole plan) and the result card of the step the run ended on render their text as Markdown, as does each step's report in a run's summary (*What changed* on Ready for review, `Wegul`: 12 / 1.6 in `$muted`, its bullet centered on the report's first line). They use `theme::markdown`, the styling of a plan under review: headings step up from the body, bold in `$text`, inline code and code blocks in Geist Mono with the `code-*` colors, quotes and tables in the chrome's borders. The body keeps the frame's text style: 13 / 1.6 in a message and a question (`V17jfc`), 12 / 1.6 in a plan card and a result card (`FP6BD`, the result in `$muted`). The line breaks a message was written with stay line breaks, as a chat reads. A top-level code block's lines are never wrapped: a long line scrolls sideways inside its block, with the transcript command's thumb; a table's columns share the message's width. Links: there is no confirmation for opening a link anywhere in the app, so links follow plan review's rule — only an `https` address opens, in the browser; any other is not opened and is logged. Search finds a message by its text as written and as shown (`**bold** text` is found by *bold text*); matches stay counted, not highlighted. Lines that show a message on one line (*Tools*, a failed step's divider) show its rendered text without markup. Selecting text copies what is shown, not the Markdown it was written in; the chat has no Copy action for a message (the user, 2026-10-07: copying the Markdown syntax is not needed).

**Decided and approved: C with the rail on the right (user, 2026-10-04).** Of the options below the user chose C, with
the step rail moved to the right of the tab (chat on the left), and then approved the full set together with every
sub-decision marked *proposed* when it was drawn: the rail collapses to the 56 strip, *Details* replaces the rail
(*Steps* brings it back), and at 960 the rail is a strip. The set is group `A5H6ln` ("02.5 · Conversation · a chat with
the step rail on the right (approved, user 2026-10-04)", y 220 in 02.5 `yXEwR`, 4896 × 5560) and is the only
Conversation design in the file. Not built yet; until it is, the timeline described above is what the app does. Fixture as above: Kontur, ORC-128 Run 128 with seven steps (01 Research … 07 Docs & cleanup),
ORC-129 Run 129 queued; every message, path, time and count is illustrative.

**Layout.** The tab body is two columns: the chat (toolbar, chat, composer) fills the left, and the **step rail**
(248, `$sidebar`, hairline at its left) is on the right. The toolbar keeps *Show* (All / Messages / Tools), *Search
this run…*, the run's state chip (*Following live*, *Waiting for you*, *Ready for review*, *Failed*, *Queued*) and
*Details*. The step filter select is gone: the rail is the step filter.

- **Rail.** Head *Steps* with the count and *panel-right-close* (collapses the rail to the 56 strip). *All
  steps* first (executions and tokens), then one row per step execution: status icon (check, loader, hand / message-circle
  when it waits for you, circle-x, circle-dashed for not started at 0.7, undo-2 for a step that returned the work), the
  number (mono 10), the step's name (12), and one meta line (agent · tokens · time, or what it waits for). The selected
  row is `$hover`. Picking a step shows its items only, with the read-back *Showing 03 Implement only · 01, 02 and
  04–07 are hidden · Show all steps* under the toolbar; Details follows the picked step, as before. During a search each
  row shows its match count at the right and *no match* in its meta.
- **Chat.** Messages are rows: a 28 round avatar (bot, user, message-circle-question for a question, list-checks for a
  plan, terminal for a tool step), the name (12/500) and meta (step on the first message after a divider, time), the
  text at 13 / 1.6. A step starts with a thin **divider** (*07 Docs & cleanup · Builder · Codex / Sol · started 16:31 ·
  38k tokens*, status icon, `$text` while live) and ends with one (*06 Fix notes done · 16:30 · Changes · 3 files ↗*);
  steps that did not start read as one muted divider (*05 Review, 06 Fix notes and 07 Docs & cleanup did
  not start*). **Returns and restarts are dividers of the same kind** (*Review returned the work to 03 Implement ·
  attempt 2 · 15:12*, undo-2, `$text`). The run's end is a divider (*Run 128 is ready for your review · 16:52*).
- **Tool activity is chips** under the agent's message, one chip per tool group with ORC-61's summary and *N calls*
  (chevron-right); the group that is still growing reads *running pnpm docs:check* with a loader on `$raised`. **A chip
  opens its group in place** (chevron-down) as a bordered list of Activity rows with their times, the 7-row cap and
  *Show N more*; *Tools* still shows the flat transcript. A failed command in a group shows circle-alert; a tool step's
  failing command is the tool step's own message with the command, exit code and output open (C·05).
- **Cards are chat messages:** the question (C·02) and the plan (C·03, *Approve plan & continue* on the card, *Request
  changes* from the composer) sit in the asking agent's message; the tool approval card likewise (not drawn again). The
  result of a step is a bordered card (C·04).
- **Composer**: the one-box composer approved on 2026-10-04 (see *Composer* below), an instance of `q2XF4` in every
  frame; closed runs use `Hhug2`.
- **Details and the rail share the right side** (approved): *Details* replaces the rail with the 320 invocation pane;
  its head has *Steps*, which brings the rail back (and pressing *Details* again does too). They never show together, so
  the chat keeps its width.
- **960** (approved): the rail collapses to a 56 strip of the step numbers with their status icons (*All* first, the
  live step outlined); a number filters like a rail row, and *panel-right-open* opens the full rail over the chat. The
  composer's buttons shorten to *After this turn* and *Send now*.

| Frame | Node | Position |
| --- | --- | --- |
| C·01 · Agent working, rail right | `kpnmP` | 48 / 100 |
| C·02 · Waiting for your answer (question as a message, composer answers) | `BRx5g` | 1568 / 100 |
| C·03 · Plan waits for approval (steps after it not started) | `K4AOD` | 3088 / 100 |
| C·04 · Ready for review (result card, end divider, Send follow-up) | `uDqzB` | 48 / 1148 |
| C·05 · Run checks failed (failing command with output, Continue with instructions) | `FeiRG` | 1568 / 1148 |
| C·06 · Queued, not started (empty chat, controls 0.4, Send when the run starts) | `FBwIV` | 3088 / 1148 |
| C·07 · Review returned the work to Implement (return divider, attempt 2 in the rail) | `o0KDAq` | 48 / 2196 |
| C·08 · Search “callback” (match counts in the rail, a chip open in place with its matches) | `Hn1xv` | 1568 / 2196 |
| C·09 · Search finds nothing | `JPBHA` | 3088 / 2196 |
| C·10 · Step picked in the rail: 03 Implement only, a chip open in place | `bdCsu` | 48 / 3244 |
| C·11 · Details open in place of the rail | `oryKr` | 1568 / 3244 |
| C·12 · 960: the rail collapsed to a strip | `I35q7D` | 3088 / 3244 |
| C·13 · Marks, following and loading: automatic and manual returns, a restart, Not following with Jump to latest, loading, couldn't load (compact) | `j0qLQz` | 48 / 4292 |
| C·14 · Composer and tab states: closed, queued, recipient menu, Messages view, tool request card, unreadable run, delivery lines, not started (compact) | `Y6gMx` | 1568 / 4292 |
| C·15 · ORC-83 · Question with options, nothing picked (in group `msllw`, see *Questions with options*) | `lS9BF` | 48 / 100 in `msllw` |
| C·16 · ORC-83 · 960: an option picked with the keyboard | `juFIX` | 1568 / 100 in `msllw` |
| C·17 · ORC-83 · Question card states (compact) | `UpHyY` | 2608 / 100 in `msllw` |

Reused: `zo9Py`, `SOW6S`, `u1XBP`, `DeizF`, `ZGwlv`, `tfGRt`; the segmented *Show* of ORC-61. No new components or
variables; the chat row, chip, divider and rail row are frames in these screens (to become components when built).

**Pencil cleanup (2026-10-04).** The user asked that the file hold no old Conversation design. Removed, with what now
draws the state:

| Removed | Was | Now |
| --- | --- | --- |
| `fTfl8` and its frames `RrScB`, `CxrSA`, `ZxYGh`, `haIEI`, `AzKYQ`, `hep0l`, `gNa91`, `LhLL8`, `zxIMy`, `OlLGD`, `q4fCXK`, `a5hF4`, `gLVQQ` | ORC-61 timeline (stacked step headers, *All steps · N executions* select) | C·01–C·14; the Tools view in 02.I `SfXU1` |
| `yqkBy` (`TpdEd`, `HhKU2`, `z4emB`, `wKPfw`, `b113W`, `f3BZux`, `V7B8Mw`) | options A–C before the decision | `A5H6ln` |
| `GvUXd` (`JOG1i`, `m3gLZ`, `lmlkQ`, `cP8oh`, `sYuiX`, `Ok14G`, `cj2a2`, `vUPqr`) | Conversation as built, U6 composer states, unreadable run | C·01, C·05, C·06, C·14 |
| 02.B7 `zyusO` | returns and restarts as rows between step headers | C·07, C·13 (A–C) |
| 02.I states `hig8O` | Activity empty, loading, failed, not following | C·13 (D–F), C·06 |
| 49 *Activity* tabs in task-page tab rows | a tab that no longer exists | tabs read Summary · Conversation · Changes · Artifacts (Memory where drawn) |

Kept and moved: `mGbYs`, `SUuwV`, `LhIBQ`, `xgLjn` (unreadable run in the task list and on Summary) into the new group
`LpVFY` "States · Unreadable run" in 02.4 `U0KbN` (y 16004; `U0KbN` is 17224 high); components `V17jfc` and
`uoK7y` into Foundations `rIGE7` (no instances left). Updated in place to the new design: the Activity frames listed
under *Activity tab*; the approval frames `A9j8eD` and `INTTC` (no step filter, dividers, messages with avatars, the
tool group and the approval card inside Builder's messages); and the pre-ORC-61 Conversation tab bodies (step list,
*Working context* and the `kjsmd` side panel) of 02.A `Oqmog`, 02.J `q4K1G`, 02.K `H03XN`, 02.L `D6Nwxz`, 02.12
`jgqPe` and 02.Q5 `Y8hyj`, now the chat with the step rail, each keeping its subject (checkpoint pause, permission
request card, raise-limit card, closed composer). Section 02.5 `yXEwR` (y 19424, 8756 high) holds only `A5H6ln` and
the composer proposal `YJfPo`; 09 Usage `yqO5T` is at y 28580. (ORC-83, 2026-10-07: `yXEwR` grew to 11556 for the
group `msllw`, questions with options, and `yqO5T` moved to y 31380.)

**Composer (approved by the user 2026-10-04, as drawn in `YJfPo`: send now with a per-message *Send after this turn* menu; the permission chip uses the theme's existing colours — grey, `$danger` when the run allows more than its profile).** After the user's reference (one rounded box: input on top, a row inside
it with attach and a permission chip at the left, a selector and a round send button at the right), one composer is
drawn in group `YJfPo` (02.5, y 5876, 4896 × 2800): K·01 working with the send menu open `BRMpj`, K·02 answering
`K1oNwn`, K·03 follow-up after review `cHDF1`, K·04 not delivered `TdT7X`, K·05 960 `vOF1a`, K·06 every state
(compact) `l7cEtA`.

It is the component **`q2XF4` *Conversation / Composer*** in Foundations `rIGE7` (x 2280, y 1900, 860 wide), with
**`Hhug2` *Conversation / Composer · closed*** beside it (x 3200). States are overrides on the instance: *Context line*
`rBfvv` (enabled, text `EQ2oT`, icon `t8RQb`), *Input* `I2c7e3` (text; `$subtle` placeholder or `$text` draft), *Box*
`Z4Ytc` stroke `$muted` while focused, *Permissions text* `D3ZmA1` (hidden at 960), *Agent* `JG7DN` and *Agent state*
`HcdTV`, *Recipient chevron* `Nq982` (lock when the recipient is fixed), *Send options* `NEHne` (hidden unless the
agent is mid-turn), *Send icon* `jd9ia` (arrow-up or clock-3), *Send* `DPAlg` opacity 0.4 while empty, *Quiet line*
`ONl6b` (hidden unless it changes the outcome); the closed one overrides *Reason* `wnnO7`. Every frame that shows the
Conversation composer uses these instances: C·01–C·12 and C·14 (B, H) in `A5H6ln`; 02.A `Oqmog`, 02.J `q4K1G`, 02.K
`H03XN`, 02.L `D6Nwxz`, 02.12 `jgqPe`, 02.Q5 `Y8hyj` (closed); `A9j8eD` and `INTTC` (waiting for your decision); the
Tools-view frames `SfXU1`, `iBzOj`, `BQgqg`, `tWO8E`, `BbGhA`, `IFmpE`, `IcGvI` (working) and `HW9mB` (closed, merged);
and the two composers on the pull request page in 08 (`McWTM`, *Send fix request* and *Ask Builder*), which used
`C3nu0z`. The K frames in `YJfPo` are the drawn specification of the same values. The ORC-61 components `kjsmd` (with
`C3nu0z`), `V17jfc`, `uoK7y` and `mDnc3` stay in Foundations without instances because the built app and its theme
tests still name them; they go when the new design is built.

- **Box.** Radius 18, `$raised`, `$border` (`$muted` while focused); the input (13) on top, then one row: *+* (Attach,
  0.4, tooltip *Attaching files is not available yet*), the run's permission chip, space, the recipient as a quiet
  selector (*Builder* 12/500, *working* `$muted`, chevron; a lock instead when the recipient is fixed), and a 32 round
  send button (`$accent`, arrow-up `$inverse-text`). No microphone: Orchester has no voice input.
- **Send now or after this turn: (a) a chevron beside Send** (recommended). Send and Enter send now; the chevron opens
  *Send now · Enter* and *Send after this turn · Ctrl+Enter*, for that message only (never remembered). The chevron
  shows only while the agent is mid-turn. The toggle (b) was not drawn: a persistent mode is easy to forget.
- **The send icon says what happens:** arrow-up sends now; clock-3 queues (agent not running, run paused or stopped,
  waiting for your decision, not started).
- **Context lines sit above the box** (*Answering Builder's question from 16:42*, *Changes to plan v1 · Planner writes
  plan v2 from your note*, *Continue 03 Implement with your instructions · attempt 3*) with × to leave that mode.
- **Help text** moves to the send tooltip; one quiet line (10, `$subtle`) under the box while it changes the
  outcome (queued, paused, a follow-up resumes the session, continue starts a new attempt), and while an agent works,
  when it gets the message (*Builder gets your message at its next tool call.*). The line stays from one step to the
  next, so the chat above the box does not move as steps change (ORC-71; the Pencil frames do not show it while an
  agent works yet).
- **Permission chip** shows this run's permissions (*Read-only*, *Edit files · asks for the rest*) in `$muted`, and in
  `$danger` once this run allows more than its profile (*Network on · approves similar (2)*); it opens the run
  permissions popover (02.J). No new colour: the reference's orange would need a warning role the theme does not have.
- **Closed** keeps no controls, only the lock and the reason. Delivery lines under your message are unchanged.

PNGs: `.tmp/composer-design/K-*.png` (not committed).
The user's defaults applied: return and restart marks are dividers; a chip opens its group in place.

*Proposed* sub-decisions: Details replaces the rail (C·11); the rail can be collapsed by hand to the 56 strip at any
width, and is collapsed by default below 1200 (C·12); a step that did not start shows in the rail at 0.7 and in the chat
as one shared divider; the step that waits for you uses its waiting icon in the rail (hand for a plan, message-circle
for a question).

**Options (2026-10-04); C chosen, A and B not.** The user asked whether the tab could look more like a chat with selectors at
the top. Group `yqkBy` (removed 2026-10-04; now `A5H6ln`) ("States · Conversation as a chat · options", y 9461 in 02.5 `yXEwR`, which grew to 12841; 09
Usage `yqO5T` moved down to y 31409). The fixture is a running Run 128 with seven steps, 07 *Docs & cleanup* (Builder)
running a tool, and a waiting-for-you variant of each with the question as a chat message. Every option keeps one
timeline per run, *Show* (All / Messages / Tools), search, *Following live*, *Details*, the composer's *Send now* /
*Send after this turn* and *Send answer*. Tool activity becomes chips in the agent's message (one chip per tool group,
the running one with a loader); step headers become thin dividers, and finished steps fold into one line (*01–05 ·
five steps done · Show them*).

| Option | Frames | Selectors |
| --- | --- | --- |
| A · step strip | A1 `TpdEd` (removed 2026-10-04; now `A5H6ln`), A2 `HhKU2` (removed 2026-10-04; now `A5H6ln`), A3 at 960 `z4emB` (removed 2026-10-04; now `A5H6ln`) | A row of step chips under the tabs (*All steps*, 01 … 07 with status icons), then Show, search, follow state and Details; Slack-style rows |
| B · step and agent selects | B1 `wKPfw` (removed 2026-10-04; now `A5H6ln`), B2 `b113W` (removed 2026-10-04; now `A5H6ln`) | *Step* and *Agent* selects beside Show in one toolbar; chat bubbles, yours on the right; steps as centred pills |
| C · step rail | C1 `f3BZux` (removed 2026-10-04; now `A5H6ln`), C2 `V7B8Mw` (removed 2026-10-04; now `A5H6ln`) | A 232 rail at the tab's left listing every step with status, agent and tokens; Slack-style rows (a rail, not strictly top) |

Settled with the choice (user, 2026-10-04): option C with the rail on the right; marks for returns and restarts are dividers like the
step ones; a chip opens its group in place.

**Implemented (ORC-61, 2026-10-02).** The task page's tabs are *Summary · Conversation · Changes · Artifacts*: Activity is gone (an old `tab=activity` link opens the conversation) and *Memory* is not added, as its screens come with Memory 3/3. The timeline (`crates/workspace/src/tasks/timeline/`) is built as above, with these choices where the frames leave room: a step's items are its own (a person's message belongs to the step it reached or did not; one that waits for a step follows the steps, before the run's end), so steps that run beside each other are not interleaved; a failed step starts open as well, to show why (`hep0l` (removed 2026-10-04; now C·05 `FeiRG`)); what the agent CLI writes on standard error before its first event is a group of unparsed lines of its own; a step result the agent writes is its result card, not a message, and its *Reply* message is not repeated beside the agent's own; read and search kinds come from the tool's name (Claude Code's `Read`, `Grep`, `Glob`) or the program inside the CLI's shell (`pwsh -Command "…"`, `bash -lc '…'`: `cat`, `head`, `rg`, `grep`, `find`, `git grep`, …). Cards: a question with *Reply*; each plan version (the first six lines, *Show the whole plan · N lines*, *Approve plan & continue* on the waiting one; an earlier version reads *Earlier version*); each tool request of the run, pending with *Deny* and *Approve once*, decided as *Approved*, *Denied* or *Interrupted*. On that task's Conversation tab the approval dialog does not open, and *Needs you* opens the conversation for a request. A result card links *Changes ↗* and *Summary ↗* (no *Handoff in Memory ↗* until the Memory tab exists). Details (`gNa91` (removed 2026-10-04; now C·11 `oryKr`)) is remembered in the application database (`ui_state` `conversation.details`); a step header's tokens open it on that step. Following live counts new timeline rows for *Jump to latest · N new*; opening the tab starts at the first card that waits for the person, else at the end. Search matches without highlighting the words, and following goes on during a search (the two open questions of the design step). Send now and its delivery states follow `docs/architecture/storage.md` ("Send now"), checked live with Codex 0.159.2 and Claude Code 2.1.286 (`docs/evidence/orc-61-send-now-2026-10-02.md`); *Send now* is unavailable, with its reason, only for a provider that takes no message mid-turn (a native agent, which does not run in this version). Claude Code cannot refuse a message: for it, *Could not deliver* means its input did not take the message within five seconds. A task without a run lists the messages that wait for its first step under the toolbar-less tab, and its composer reads *To: Orchestrator · this task* (no run's steps are known yet). Within a step execution, the person's messages and the cards come between the tool events in the order they happened, so one sent between two tool calls splits their group. A return or restart (02.B7 `zyusO` (removed 2026-10-04; now C·07 `o0KDAq` and C·13 `j0qLQz`)) is a row of its own before the header of the step its attempt starts from, also while that step is folded and in *Messages* and *Tools*. While the run waits for the person's decision after its retries (02.B3–02.B6), the toolbar's state reads *Waits for your decision* (hand) and the composer queues with *Waiting for your decision* · *Open the decision →* above its field, which opens Summary. Details holds the invocation's permission row (ORC-44): *Running with* with *change ↗* for the open invocation of a live run, *Ran with* otherwise. Narrower than the toolbar's line (1104, more with Details open, in the Tools view or while the run waits for a decision), *Show* and the search go to a second toolbar line; a step header keeps its step's name and lets its engine and times give way. The toolbar's state is the run's where no agent works: *Waiting for you*, *Queued*, *Ready for review*, *Failed* as drawn, and for the undrawn states the task status's words (*Paused*, *Stopped at limit*, *Interrupted*, *Merged*, *Cancelled*). A tool step's failing command is a row of its own with its output open, as `hep0l` (removed 2026-10-04; now C·05 `FeiRG`) draws it; an agent's failing command stays in its group. Before the run starts, the toolbar's controls are unavailable (0.4) beside *Queued*, and the empty state reads *Nothing in this conversation yet* with the steps on one line; the reason it waits is generic (*starts when an agent slot is free*), since the desktop does not know which limit holds it. Narrower than its line (980 less the Details pane), the composer puts its buttons on a line of their own and the recipient gives way.

**The chat always answers (ORC-79, 2026-10-07).** A message never sits in the queue without an agent that will read it: in every state of the run the composer says, before sending, who reads it and when, and its primary action lets the run go on with it. Not drawn: every state below is the approved composer `q2XF4` with its existing overrides (context line, placeholder, recipient state, send icon, send menu, quiet line), so the K frames of `YJfPo` are its reference. Decided where the design was silent:

- **Waiting for your decision** (retries used up, or the step broke again): the composer continues the run as for a failed run (*Continue with instructions*, play). The context line reads *Continue 02 Review with your instructions · attempt 3* with *Open the decision →* at its right, so Retry, Return with note and Stop stay a click away. The recipient is the agent of the step the run stopped at (*Builder · continues the run*); choosing another agent that ran continues from that agent's step, and its dependent steps run again. The agent resumes the session of the execution that failed with the message as its instructions, so a question is answered in that step, which may then act. The runner records it as the person letting the run go on (`RetryAllowed`) and a recovery attempt. A run that waits on a step with no agent step before it keeps the old line (*Waiting for your decision · Open the decision →*) and queues.
- **Stopped at its own limit** (tokens, time, a step budget): the context line (octagon-pause) says which limit and what was used (*Stopped at the token limit · used 9.7k of 5k tokens*, *Stopped at the time limit · used 31 of 30 min of active agent time*, *Stopped at the step budget of Implement · used …*). The recipient is the agent that reads first once the run goes on (*Builder · stopped at limit*). **Raise the limit and continue** (clock-3: the message queues) stores the message and opens the Summary on *Raise the limit to continue* with its input focused: the composer does not choose a new limit, which stays the person's choice in that one panel. Stopped by the project's daily budget or its task group's budget, **Send** queues and the quiet line names that budget and when the run goes on.
- **Paused or interrupted**: **Resume and send** / **Continue run and send** (arrow-up, Enter) resumes or continues the run, and its agent reads the message before its next action. The chevron beside Send offers the same two choices as the send menu does while an agent works: *Resume and send · Enter* and *Send without resuming · Ctrl+Enter* (*Send without continuing*), for that message only. The recipient is the agent that reads first. An interrupted run whose worktree was removed can never continue, so it is treated as ended (next item).
- **Ended** (cancelled, failed with no agent step to continue or with its worktree removed): Orchester has no project chat yet, so no Orchestrator reads it. The context line (lock) says *No agent of this run reads messages any more*, the placeholder *Write to the first agent of the next run…*, and **Run again with this message** (arrow-up) starts the task's next run at once, keeping what the ended run left (the topbar's *Run again* still confirms and offers to remove its worktree). The message is stored for no run, so the next run's first agent reads it; when Run again is refused, the reason shows under the box and the message stays queued as *Queued · waiting for the task's next run*. A merged run, or one ready for review whose worktree was removed, stays closed: a task runs again only after a failed or cancelled run.
- **Who reads a queued message**: delivery lines name the agent when the run is suspended (*Queued · Builder reads it before its next action on resume*, *Queued · Builder reads it when the run continues*).
- **Automation**: Orchester's MCP server has `send_message {project, run, text, timing}` for the task's latest run, with the same rules: the recipient the composer defaults to, `timing` (`now` or `after_turn`) only while an agent works that takes a message mid-turn, the state's action after storing (and *Request changes* while a plan waits); a run stopped at its limit keeps the message and answers `awaits_limit`; a closed run refuses with `no_agent_reads`.

**Questions with options (ORC-83, proposed 2026-10-07, waiting for approval).** When an agent asks with suggested answers (Claude Code's `ask_question` tool through the runner's relay, Codex `requestUserInput` with options, the native agent's `ask_question`), the question card in the asking agent's message lists them and the person picks one, several, or types their own answer, as in Claude Code's and Codex's own UIs. A plain question without options stays as C·02 (`BRx5g`): the composer answers it. Group `msllw` ("ORC-83 · Agent questions with options", in 02.5 `yXEwR` at x 0 / y 8776, 4896 × 2700, below the composer proposal `YJfPo`; `yXEwR` grew to 11556 and 09 Usage `yqO5T` moved to y 31380). Fixture: Run 128 of ORC-128, 07 *Docs & cleanup*, Builder asks at 16:42.

| Frame | Node | Position in `msllw` |
| --- | --- | --- |
| C·15 · ORC-83 · Question with options, single choice, nothing picked; Answer at 40 % (1440) | `lS9BF` | 48 / 100 |
| C·16 · ORC-83 · 960: option B picked with the keyboard, focus ring on it; Answer ready (960) | `juFIX` | 1568 / 100 |
| C·17 · ORC-83 · Question card states (compact, 1440 × 2510): A single, nothing picked · B picked with the mouse · C several allowed, two checked · D your own answer typed · E decided by you, Builder continues · F decided through the MCP server, two checked plus own text · G two questions in one request (Codex) · H a secret question refused · I a question arrives while you type (no focus steal) | `UpHyY` | 2608 / 100 |

- **Card.** The question card of C·02 (`$raised`, `$border`, radius 8, padding 14), gap 12. Head: message-circle-question 14, *Builder asks you* (12/500), the question's optional header as a chip (*Docs layout*: 11 `$muted`, `$border` stroke, radius 5, 20 high), status label *Waiting for your answer* (`DeizF`, message-circle). Then the question (13 / 1.6) with *Pick one* or *Pick one or more* under it (11 `$muted`).
- **Options** are rows (radius 7, padding 8 / 10, gap 2 between rows): the control (a 16 radio for single choice, a 14 checkbox for several, the existing radio and checkbox of the design), the letter in an 18 key (radius 4, `$border` stroke, mono 10 `$muted`), then the label (13 / 1.5 `$text`) and the optional description under it (12 / 1.5 `$muted`). Letters run A, B, C… in the agent's order. A picked row is `$hover` with its radio filled (or checkbox `$accent` with a tick) and its key stroked `$muted` with a `$text` letter.
- **Your own answer** is always the last row, whatever the agent allows: its letter, then a field (32 high, `$sidebar`, `$border`, radius 7, 13; placeholder *Type your own answer…*). Typing in it picks that row (for single choice it replaces the other pick; with several allowed it is added to the checked ones). An answer is the picked labels plus the optional text.
- **Footer.** keyboard 12 and a hint (10 `$subtle`: *A–D or ↑ ↓ to move · Space picks · Enter answers*; *Space checks* with several; *Enter answers · Esc back to the options* in the field), then **Answer** (primary `$accent`, corner-down-left, 30 high) at the right, at 40 % until something is picked or typed. With two questions in one request (G) the card title says *Builder asks you 2 questions*, each question has its number (*1 of 2*, mono 10) and its own header chip, options and own answer, separated by `$hairline`; one **Answer** sends all once each has an answer, and the footer says which one is missing (*Question 2 has no answer yet*, info).
- **Keyboard and focus.** The card never takes focus by itself: a question that arrives while the person types leaves the composer's focus and draft alone (I). Tab or a click reaches the card; a letter picks its row (the letter of the own answer focuses its field), ↑ ↓ move, Space picks or checks, Enter answers. Keyboard focus is the inset focus ring (`$focus-ring` 2, C·16); a row picked with the mouse shows no ring (B).
- **Decided** (E, F). The card stays in the chat as the record: *Builder asked you*, status *Answered* (check), the question, then only what was answered, one line each (check 14, the key, the label, or the typed text in quotes), and the line *Answered by you · 16:44 · Builder continued* (corner-down-right, 10 `$muted`) with *Show all options* (11/500) at the right, which opens the full list read-only. Answered through Orchester's MCP server (`answer_question`), it reads *Answered through Orchester's MCP server · 16:49 · Builder continued*. No separate *You* message is added. The agent continues in the same turn (its next message follows, E).
- **Composer while a question with options waits.** Not in answering mode: placeholder *Message Builder…*, recipient *Builder · asking you*, send icon clock-3 (queues), quiet line *Builder is waiting for your answer above; a message here reaches it after you answer.* Waiting questions count in *Needs you* and in the rail (message-circle) as today.
- **Refused secret question** (H, Codex `isSecret`). A problem row (circle-alert avatar) *Builder asked for a secret: GitHub client secret* · *refused · 16:55*, the reason (12 / 1.6 `$muted`: *Orchester never passes secret values through questions, so the question was declined and Builder was told why. Store the value in Secrets and grant it to this run; Builder can then use it without seeing it in the chat.*) and *Open Secrets ↗*.
- **Reused**: `DeizF`, `u1XBP`, `q2XF4`, `zo9Py`, `SOW6S`, the C·02 card and message rows, and the radio and checkbox frames already in the design. No new components or variables. Proposed copy namespace `conversation.question.*`.

**Implemented (ORC-83, 2026-10-07).** A question is `orchester_core::question` (text, optional header, two to 26 options with different labels and optional descriptions, several or one; the own answer always allowed), kept per request in the `questions` table with its run, step execution and invocation, and answered once through the runner (`AnswerQuestion`, protocol 19) by the card and by Orchester's MCP server alike. Decided where the design was silent:

- **Claude Code** is not offered its own `AskUserQuestion` (`--tools` lists only what the step's permissions need, so its headless behaviour was not tried); the runner's relay serves Orchester's `ask_question` beside Orchester's tools, with an allow rule, and the prompt's *Orchester tools* paragraph names it. Every Claude Code step has that relay: it comes with the permission prompt, which every Claude Code step has, since its commands always ask (*Any command in the worktree* is refused for Claude Code before the step starts). The call waits in the agent's turn and the answer is its result. A plain question still ends the turn with the `question` outcome.
- **Codex**: a `requestUserInput` about an MCP call the stream follows stays that call's approval; any other is a question (one request may hold several). Codex has no multi-select, so its questions take one answer. The own text is one more answer string. A question for a secret (`isSecret`), or one without two different options, is answered at once with why and shows as a problem row (H; the option-less one reads *Asked a question without options*); the secret row offers *Open Secrets ↗*.
- **Native agent**: `ask_question` takes options with descriptions, a header and `multi_select`; it ends the step, which waits. The answer reaches the agent in a person's message the runner writes and the chat does not show (the card shows the answer); while such a question waits, other messages do not resume the step and reach the agent with the answer.
- An answer lists the picks in the options' order, then the own text, whether the card or the MCP server gave it. While the agent waits in its turn its time stops counting, the task counts in *Needs you*, the task page's notice says it asks, and its rail row reads *waiting for your answer* (message-circle). An agent that ends before the answer reaches it leaves the card *Ended without an answer* with every option.
- **Keys**: rows are Tab stops (the own answer's field too); the inset ring shows only after a key, so a pick with the pointer shows none. ↑ ↓ cross into the next question's rows; ↑ or Esc in the field goes back to the last option.
- **MCP**: `get_run` lists `awaiting.questions` with their options; `answer_question` answers a request of one question (the run's only waiting one when none is named) and refuses `unknown_choice`, `repeated_choice`, `several_choices` (also an option plus own text for one answer), `empty_answer`, `question_not_found`, `no_waiting_question` (none named, none waits) and `question_ambiguous` (none named, several wait); a request of several questions is refused (`several_questions`) and answered on its card.
- Copy is in `runs.conversation.question.*`, beside the other cards' keys, not the proposed `conversation.question.*`.
- While an agent waits in its turn for the answer, the page reads as asking, not working: the run line says it asks you, the toolbar's state is *Waiting for you* (message-circle), and there is no working line. The card is the record of the agent's `ask_question` call, so the call has no tool row; a call that failed asked nothing and keeps its row.

## Code index

Drawn 2026-09-30, group `HBQJY` ("States · Code index", last in 02 / Tasks & runs at y 34268, height 3216; the section is now 37484 high). Fixture: Kontur at main `4d2b8e0`, 1,284 tracked files. Orchester indexes a project's repository per git revision (tracked files with language and line counts, symbols, per-file outlines, Markdown headings and a generated package map) and gives Codex and Claude Code a built-in skill that searches the index with ripgrep instead of reading whole files. The skill is internal and never shown. Proposed copy namespaces: `settings.codeIndex.*`, and `runs.activity.*` for the run details.

**Project settings › Code index** is a fourth tab after General, Limits and Environment, laid out like them (content 760 wide, padding 24 / 32, gap 8; setting rows with the 300-wide value column and `$hairline` under each row; the topbar says *All changes saved*; the setting saves on change). Heading *Code index* (15/500) and one explanation of what the index gives agents. The row *Use code index* (`K8ItM`) with *Agents that start after a change follow it. An agent already working keeps what it started with.* Under it the `$raised` panel *Indexes*, the list of the indexes Orchester keeps, then *Search tool* › *ripgrep*.

**Off by default (decided 2026-10-07).** *Use code index* is off until a person turns it on: measured with Claude Code and Orchester's file tools, agents hardly used the index and runs with it cost more (`docs/evidence/code-index-measurement-2026-10-07.md`, ORC-69). A project that never set it opens on the **Off** frame; a project a person turned on before keeps it on.

**Copy without unmeasured claims (2026-10-07).** The measurement does not show that agents without the index take longer or use more tokens, so the copy no longer says so: the explanation ends at *…searches the index with ripgrep.*, Off reads *Agents read and search the repository directly* and *They read and search the repository directly. No index is built while this is off; turn it on to try the index.*, and the ripgrep-not-found notice says agents search with git grep or their own tools. The Pencil frames still show the earlier text (the explanation in `i220W`, `LC2V0`, `RkMap`, `h6yMk`; the read-back and notice in `RkMap`; the notice in `h6yMk`); the copy here and in `settings.codeIndex.*` is the reference, and the frames were not edited for copy only.

**How indexes are kept (decided 2026-09-30).** An index belongs to one git revision (commit hash). It is built only when a run starts from a revision that has none; nothing is built in the background, for example when the default branch moves, and nothing is built while the setting is off. It is stored outside the worktrees and shared read-only by every run that starts from that revision. An agent only ever sees the index of its own run's base revision; what it changes in its worktree is not in the index, and it reads changed files directly. A rebuild of the same revision gives the same content and replaces the index atomically, so agents that use it are not disturbed. An index is removed together with the worktrees that used it.

**The index list.** Title *Indexes* (12/500) with the total at the right (*3 indexes · 3.2 MB in total*, 11 `$muted`; *None kept* when empty), one explanation line (*Built per revision when a run starts, shared read-only by the runs from that revision, and removed together with the worktrees that used it.*), then one row per revision, divided by `$hairline`. Row line: the short hash (mono 12), what uses it (11 `$muted`: the branch and/or the tasks and runs whose worktrees use it, *main · ORC-38 · Run 2*), the state (`DeizF`, 22 high, 10) and the row's action (`u1XBP`, 28 high). Detail line (11 `$muted`) by state:

- *Ready* (check) — *3.2 MB · 1,284 files · 18,406 symbols · built today 14:02 in 9 s*; action *Rebuild* (refresh-cw).
- *Building* (loader) — the current step, never a percentage: *Indexing 1,291 files… · started 14:21, 6 s ago* (also *Listing tracked files…*, *Writing the package map…*); *Rebuild* disabled (0.4).
- *Failed* (circle-x) — the reason beside circle-alert 12 (Inter 11/1.5 `$text`, as the worktree error in `XIEn8`) and what it means for the run: *Orchester couldn’t read src/generated/schema.rs: access is denied. Run 3 continues without the index; its agents search the files themselves.*; action *Retry* (rotate-ccw).
- *Outdated* (circle-dashed) — an index of an earlier format that runs still read: *Earlier index format · rebuilt when a run starts from it*; action *Rebuild* (implemented 2026-09-30, no frame of its own).

Rows are ordered Ready (newest first), Building, Outdated, Failed. Several runs of one task read *KON-1 · Runs 4, 5, 7*; a run's branch is named only while it still points at the index's revision. The total leaves out the size while no index is ready (*1 index*). A Rebuild that finds the index in use (an agent's search holds one of its files open, Windows) keeps the index and says *The index was not rebuilt: a running agent is reading it. Try again when it finishes.* under the list.

Frames:

- **On, index list** — `i220W` (x 48, row 1): *On · agents search the index*, read-back *3 indexes kept · 1 building, 1 failed*; the list with `4d2b8e0` (*main · ORC-38 · Run 2*, Ready), `9e17c4a` (*orc/github-auth · ORC-41 · Run 1*, Building) and `b31f0d7` (*ORC-36 · Run 3*, Failed). ripgrep: check *Found · ripgrep 14.1.1* with where it came from (*Bundled with Codex CLI · agents get a copy in .orchester/tools*; also *On PATH*, *Bundled with Claude Code*, *Installed with winget*; the drawn read-back *copied to .orchester/tools* was changed 2026-09-30 because the page only looks for ripgrep: the runner copies it when an agent starts). ripgrep is looked up on PATH, then where winget installs it, then in the Codex CLI and Claude Code installs, and copied into the project's `.orchester/tools/` so Codex's sandbox can run it.
- **On, no index yet** — `LC2V0` (x 1568, row 1): read-back *No index yet*; the list reads *None kept* and, under the explanation, *No index yet — built when the next run starts* (database 14, 12 `$muted`, centred).
- **Off** — `RkMap` (x 3088, row 1): switch off, *Off · agents get no index*, read-back *Agents read and search the repository directly*; the index list and Search tool are hidden; the notice (circle-off) *Agents get no index and no index skill* · *They read and search the repository directly. No index is built while this is off; turn it on to try the index.*
- **On, ripgrep not found** — `h6yMk` (x 48, row 2, the page scrolled to Search tool; the list above shows a Ready and a Failed row): *Not found* (circle-alert) · *Not on PATH, not bundled with Codex CLI or Claude Code*. Under it the notice (search-x, `$raised`, radius 8) *Agents fall back to slower search* · *Without ripgrep the index skill can’t run, so agents search with git grep or their own tools instead.*, the label *Install ripgrep runs* and the exact command in a mono 12 box (`$sidebar`, `$border`, radius 7) — Windows `winget install --id BurntSushi.ripgrep.MSVC --exact`, macOS `brew install ripgrep` — the hint *Nothing runs until you click. Afterwards Orchester looks for rg again and copies it into .orchester/tools.*, then *Install ripgrep* (download) and *Check again* (refresh-cw).
- **Install ripgrep states** — compact board `NvR7K` (x 1568, row 2): *Installing* (the row reads *Installing…* with a loader and *winget started at 14:25 · 18 s*; the notice *Installing ripgrep with winget* · *Agents that start before it finishes still use slower search. You can leave this page; the install continues.*, *Running* with the command, and *Last output:* with the installer's last line; *Installing…* (loader) and *Check again* disabled at 0.4, and *Cancel* (circle-x), which stops the installer with what it started (added 2026-09-30); an install stops after 15 minutes; on Windows closing Orchester stops it, on macOS the detail says it goes on after Orchester closes); *Install failed* (emphasized notice: circle-alert and the 12/500 heading in `$text`, *ripgrep was not installed*, the exit code — winget's in hexadecimal, *0x8A150011* — and the installer's message, *Nothing was installed*, what to do; *Install ripgrep runs* with the command and the hint; *Try again* and *Check again*); the same notice when the install was cancelled (*You cancelled the install; winget was stopped. …*) or ran out of time (*winget did not finish within 15 minutes and was stopped. …*); *macOS · Not found* (the brew command); *macOS · Homebrew is not installed* (no command; *Install ripgrep is unavailable: Homebrew (brew) was not found. Install ripgrep yourself, … then choose Check again.*; *Install ripgrep* at 0.4 with a lock; a missing winget on Windows reads the same with *winget*); *After install* (*Found · ripgrep 14.1.1* · *Installed with Homebrew · copied to .orchester/tools*). *Install ripgrep* stays a secondary button (decided 2026-09-30).

**Run details (Activity).** `BbGhA` (x 48, row 3) is 02.I with two small additions. The invocation panel gains *Code index* after *Started* (`4d2b8e0 · main` in mono; otherwise *Off · project setting*, *Unavailable · build failed* or *Unavailable · still building* — the setting was turned on while the run executed and the index was still being built when the agent started). Without ripgrep the agent still gets the index (decided 2026-09-30), so the drawn *Unavailable · ripgrep not found* became a two-line row: `4d2b8e0 · main` in mono, then *ripgrep not found · searches with git grep* (Codex) or *… · searches with its own tools* (Claude Code), wrapping; the transcript's problem row names only that agent's CLI and whether it got the index. The panel gains, after *Run budget*, *Tokens before first edit* (12/500) with *Input tokens* 38k, *Cached input* *412k · not counted*, *Output tokens* 5.2k and *First edit* *12:05 · src/auth/github.ts*; the footnote adds *Before first edit adds up the turns until the agent first changed a file.* Each step header's token meta says it too: *315k tokens · 43k before first edit* (counted = input + output, as everywhere), *18.2k tokens · no edit* for a step that changed no file. The compact board `rd1Vo` (x 1568, row 3) draws the panel with the index off, unavailable for each reason, and *No file changed yet* (*21k so far*, …) while the running step has not edited anything. The invocation panel follows the step picked in the step filter (decided 2026-09-30): a finished step shows its own rows and tokens before first edit, and the title names the step instead of *Current invocation*. Compact board `F3XhQ` (x 3088, row 3): *Research · Explorer* picked, *Ran* *12:01–12:02 · 1 min*, *First edit* *No file changed in this step*.

## Usage dashboard

Drawn 2026-09-30, group `b9V39h` ("Screens · Usage dashboard", last in 02 / Tasks & runs at y 37580, height 4287; the section is now 41867 high). Fixture: Kontur, Sep 24 – Sep 30 by day, 64 runs; all figures are illustrative. No money anywhere: the dashboard shows tokens only. Proposed copy namespace: `usage.*`.

**Where** (decided 2026-09-30). *Usage* (chart-column) is a project sidebar item after *Pull requests*, route `/usage`; the sidebar instances in the new frames show it, and the `zo9Py` component gains it when this is built. All projects gets two tabs under its heading, *Projects* (the list, 01.06) and *Usage* (the same dashboard for every project, with a breakdown by project).

**Layout** (project, `C2pTYF`, full page 1440 × 1905). Project bar, then a pinned toolbar: title *Usage*, description, the period control (*Today* · *7 days* · *30 days* · *Custom…*, 36 high, selected option `$hover`) and *Group by* (`ZGwlv`, *By hour* / *By day*); under it the filter chips *CLI*, *Model*, *Agent profile*, *Pipeline*, *Tag* (30 high, `$sidebar`, name `$muted`, value 11/500) and the range read-back (*Sep 24 – Sep 30 · compared with Sep 17 – Sep 23*). The rest scrolls with the [scroll gutter](#components-and-tokens):

1. **Summary** — five figures in the Overview's metric style (mono 23): *Spent tokens*, *Runs*, *Agent time* (active time only), *Cache share*, *Failed runs*; each with the change against the previous period of the same length (trending-up/-down icon and words, *12% more than Sep 17 – 23*; points for shares) and a detail line (*input − cached + output*, *58 finished · 6 failed*, *70M of 98M input tokens cached*, *6 of 64 runs · 3.5M tokens*). A change is never colored good or bad.
2. **Tokens over time** — stacked bars by CLI (Codex below, Claude above, 2px gap, 4px rounded top), one bar per hour or day, y axis in compact tokens, *Today* labelled. Switch *Spent* / *Cached* / *Output*. Legend with each CLI's total.
3. **Plan limits** — % used per window as a line: solid for weekly windows, dashed for 5-hour windows, hue by CLI; a current point and a legend with the current value and reset time (*Codex · weekly 41% resets Oct 4, 09:00*); a dashed warning threshold labelled with its value at its end (default 80%; see below); a reset marker (rotate-ccw and a `$border` rule) where a weekly window reset. Grouped by day, 5-hour windows are not drawn (they would be a comb of spikes) and their legend entries are dimmed but keep the current value; grouped by hour they are drawn (board `e5PbG`). Scope (decided 2026-09-30): the project view shows the same account-wide chart, labelled *shared by all projects · account-wide* — limits belong to the CLI account, so it uses snapshots from runs in every project.
4. **Where tokens go** — three columns of horizontal bars, stacked by CLI and scaled to the column's largest row, with value and share: *By agent profile · step*, *By model* (mono), *By pipeline*. Selecting a row adds it as a filter.
5. **Most expensive** — *Tasks* / *Runs* switch, sorted by spent tokens in the period. *Tasks*: Task (key in mono, title), Pipeline, Runs, Spent tokens, Cached input, Outcome (`DeizF`), chevron; a row opens the task; footer *5 of 38 tasks with runs in the period* · *Show all 38 →*. *Runs* (`sOQmy`): Run (*ORC-128 · Run 3* in mono, task title), Pipeline, Started, Agent time, Spent tokens, Cached input, Outcome, chevron; a row opens the run; footer *5 of 64 runs in the period · a row opens the run* · *Show all 64 →*.
6. **Efficiency** — *Cache share* per day (one line in `$muted`, direct label on the last point); *Tokens before first edit*, median per invocation that changed a file, with and without code index (single-series bars in `$subtle` with invocation counts; invocations without an edit are left out and counted); *Failed runs* (count and share, tokens spent on them and their share of spent as a bar, the largest failed run and *Show failed tasks →*, which opens Tasks › Failed).

**Metrics.** Token counts are what the CLIs reported (see [Token budgets](#token-budgets)). *Spent* = input − cached input + output (= counted). *Cached* = cached input. *Output* includes reasoning. *Cache share* = cached input ÷ all input. *Runs* are runs with agent activity in the period; a run spanning the boundary counts in both periods, its tokens by turn time. *Agent time* is active agent time. *Failed runs* are runs whose latest attempt failed, as in the Overview filter. Values use the display rule (`31.6M`); tooltips and tables show exact grouped counts.

**Plan limits history.** Orchester stores a snapshot whenever a CLI reports its limits during a run and keeps it for 90 days (decided 2026-09-30; the footnote and the no-history note say so): Codex `rate_limits` (5-hour and weekly windows, used %, reset time), Claude `rate_limit_event` (e.g. `seven_day` utilization and `resetsAt`; the field names are verified against the real CLI during implementation). Snapshots exist only while Orchester runs agents, so the chart is honest about gaps: consecutive snapshots of one window are joined with a solid line; across a stretch without snapshots (no run reported, so use outside Orchester is unknown) the join is dotted and 1px; before a CLI's first snapshot the plot is hatched (`#FFFFFF0F` lines at 45°) and the footnote says *no Claude data before Fri 26, 14:00*. Nothing is extended to the edges and no value is invented after a reset.

**Interaction.** Hovering a bar shows a hover band and a tooltip (`$raised`, `$border`, radius 8, popover shadow): the date, each CLI's exact count with its swatch, *Spent*, *Runs*. Hovering a limit point shows the window, report time, % used, reset time and the run that reported it; hovering a reset marker names the window and time. Every chart has a *Chart* / *Table* switch; the table shows the same data with exact numbers (and makes the charts readable to automation and screen readers). Legends always carry labels; identity never relies on color alone (line style for windows, names on every bar row, direct labels on single lines). Filters are multi-select menus, with search where lists are long (*Model*), a count in the chip (*2 of 4*) and × to clear; *Tag* lists tags with run counts and matches any selected tag (*No tags yet* until tasks have tags). *Custom…* opens From / To date fields and *Show Sep 1 – Sep 30*; *By hour* is offered for ranges up to 7 days. Plan limits follow the CLI filter only.

**Warning threshold** (decided 2026-09-30: a setting, not fixed). There is no application settings screen, and the limits are account-wide, so the threshold lives in the Plan limits card's menu (… button, 28 × 28, `$sidebar`, `$border`, radius 6, at the right of the card header) and is saved app-wide, not per project. The popover (`$raised`, radius 11, popover shadow): *Warning threshold*, a whole-number field (64 wide, mono) with *% of a window used*, the hint *Draws the dashed line in Plan limits. Limits belong to your accounts, so it is saved for Orchester and every project shows the same line. 1–100.* and, after a change, *Saved for all projects* (check). Saves on change; the line and its label move at once (board `e5PbG`: saved at 70%, the hourly chart's line reads *70%*). Default 80%.

**All projects** (`p4eVV6`, full page 1440 × 2031). The All projects topbar, heading and actions, the tabs with *Usage* selected, then the controls with a *Project* filter first and the read-back *… · 4 projects*. Same sections at 1344 wide; *Where tokens go* adds *By project* first (a project without runs is named under it: *Atlas had no runs in the period.*), and the table adds a *Project* column.

**Chart colors** (new variables, validated for color-vision deficiency on `$surface` and `$raised`, all pairs: worst CVD ΔE 9.4, normal-vision ΔE 20.9, each ≥ 3:1): `chart-series-1` `#3987E5` (Codex), `chart-series-2` `#D95926` (Claude), `chart-series-3` `#199E70` (reserved for a third CLI). Color follows the CLI, never its rank. Grid `$hairline`, baseline and reset rules `$border`, axis text `$subtle` 10 (figures and clock times in mono, days in Inter, as the frames draw them; a label at the plot's right end ends there, `24:00` in `e5PbG`), threshold `$muted` dashed; single-series marks use `$muted` / `$subtle`. Text never takes a series color.

**States.**

| State | Frame |
| --- | --- |
| Project · loaded, 7 days by day (full page) | `C2pTYF` (x 48, row 1, y 100) |
| All projects · Usage tab, loaded (full page) | `p4eVV6` (x 1568, row 1) |
| Controls and chart details (compact): custom range, group by, Model and Tag filters, no tags yet; Today by hour with 5-hour windows, a point tooltip and the threshold saved at 70%; reset marker tooltip; plan limits as a table; the Plan limits menu with the warning threshold, saved | `e5PbG` (x 3088, row 1) |
| Most expensive with *Runs* selected (page scrolled to the table) | `sOQmy` (x 3088, y 1107, under `e5PbG`) |
| Tooltip on the Mon 29 bar | `BLY4R` (x 48, row 2, y 2219) |
| Tokens over time as a table | `CDcij` (x 1568, row 2) |
| Loading: dashes in the summary, a loader and *Reading token usage from Kontur’s runs…* per chart, no skeletons | `qQOKw` (x 3088, row 2) |
| No runs in the period (*Today*): zeros where known, dashes for shares, *No runs in Kontur today* with the last run's time and *Show last 7 days*, and the last known plan limits | `AY6FM` (x 48, row 3, y 3267) |
| Filters match nothing: active chips (`$hover`, `$subtle` stroke, ×), *Clear filters*, *No usage matches these filters* naming them, and plan limits for the CLI filter only | `YxAed` (x 1568, row 3) |
| Plan limits without history: *No plan limits recorded yet* in the chart's place | `T6i75` (x 3088, row 3) |
| *Where tokens go* as a table at 960 (page scrolled to the card; ORC-84) | `kIsZS` (x 48, group `vrZxB`) |
| *Where tokens go* as a table at 1440 (page scrolled to the card; ORC-84) | `v9L3oZ` (x 1128, group `vrZxB`) |
| The same breakdown table at in-between widths: All projects at 960, project at 1100 and 1280 (compact; ORC-84) | `Ncyg7` (x 2688, group `vrZxB`) |

Decided 2026-09-30 (user): plan limits stay in the project view as the shared account-wide chart; the threshold is an app-wide setting in the card menu; snapshots are kept 90 days; *Usage* follows *Pull requests*; All projects has the *Projects* | *Usage* tabs; the table's *Runs* mode has its own frame.

Built 2026-10-06 (ORC-72). Each project's runner stores the reports of its runs in that project's database; the dashboard reads every listed project's for the account-wide chart. Two consecutive reports of a window more than an hour apart are joined dotted (a gap); a report whose reset passed before the next is not joined to it. A *profile · step* row of *Where tokens go* adds its profile to the *Agent profile* filter. *Custom…* takes dates as `2026-09-01`. Narrower than 1280 (lurq's `Xl` breakpoint) the cards that sit side by side stack. The compact Plan limits card of the no-runs and filters-match-nothing states keeps the … menu, leaves out windows that reset since their last report (unless nothing else is known), and *Tokens before first edit* counts spent tokens (input − cached + output) up to the first edit.

Decided 2026-10-07 (auto mode's reviewer, see [Token budgets](#token-budgets); no frame draws it): a review counts like a turn of its run in the hour it ended, on the reviewer's CLI and model. Summary, Tokens over time (in the reviewer's CLI series), Most expensive and Efficiency's cache share and failed runs include it. *Where tokens go* gives it rows of its own in *By agent profile · step*, one per model: the model first in mono at its whole width (`claude-haiku-4-5`, Geist Mono 11 as in *By model*), then *auto mode reviewer* in Inter 11 `$muted`, 4 apart, in the room that is left, ending in an ellipsis when it does not fit; the model is cut only when the room is narrower than the model itself. The bar is in the reviewer's CLI color, and the rows are ranked with the profile rows. The table's *Name* cell reads the same in its body style, so the models of two reviewer rows line up. Every row's name takes the room its figures leave, on one line, and gives way with an ellipsis; the name keeps 4 from the figures, which keep 8 apart, so at 960 (columns about 195 wide) a reviewer row still shows its whole model and the words' first glyph with the ellipsis. At 960 the table's narrower *Name* cell may cut a model's end, never before the part that tells two models apart. A reviewer row has the inset of the rows that press (3 above and below) without their hover. While reviewer rows are shown, a line under the card's title, which wraps in its column and never runs under the Chart / Table switch, reads *Auto mode reviewer rows can't be selected as a filter.*; the note beside the title stays *Spent tokens · select a row to filter by it*. *By model* and *By pipeline* count it under its model and its run's pipeline. The reviewer is no agent profile: its rows only read (selecting adds no filter), the *CLI* and *Model* filters match the reviewer's CLI and model and list them, and while an *Agent profile* filter is chosen its tokens are left out. A review whose usage is unknown counts nowhere on the dashboard.

**Where tokens go as a table** (drawn 2026-10-08, ORC-84; group `vrZxB` "States · ORC-84 · Where tokens go as a table" in *09 / Usage* `yqO5T`, under `b9V39h` at y 4607; the section is now 5900 high). Before this, a breakdown's table split its column 1.4 / 1.0 / 0.6, so at a 960 window (columns about 197 wide) every exact figure was cut. Now, at every width:

- *Spent tokens* and *Share* are each as wide as their heading or their widest figure in that table, measured once per table with the theme's fonts and given to the heading row and every row alike, as *Most expensive* measures its narrow columns (`measure.rs`). In the fixture both headings are the widest (68 and 31: *Spent tokens* is wider than `15,204,330`, *Share* than `48%`). Figures are never cut. Each breakdown's table measures on its own. Until the first measurement the table keeps today's shares for one layout.
- *Name* takes what is left, on one line, and ends in an ellipsis. Columns are 8 apart below lurq's `Xl` breakpoint (1280) and 16 from it, as in *Most expensive*. At 960 the Name cell is about 82 wide (`Builder · Im…`, `Feature del…`). At 1100 it is 129 and at 1280 it is 173, so profile names are whole. At 1440 it is 226. On All projects at 960, with four columns, it is 73.
- Names in *Body* (13, `$text`). Models are in mono (Geist Mono 11, `$text`) in *By model* too, as in the Chart view (the table drew them in Inter until now). An auto mode reviewer row reads as in the Chart view: its model in mono at its whole width, then *auto mode reviewer* in *Body* `$muted`, 4 apart, in the room left, ending in an ellipsis (`claude-haiku-4-5 auto mode revie…` at 1440). When the room is narrower than the model, the model's end is cut: `claude-haik…` / `claude-sonn…` at 960, and `claude-ha…` / `claude-so…` at 73.
- Unchanged: headings 11 `$subtle` in a 28-high row, rows 30 high, hairlines under each, every row in the table (not only six), no row presses, and the card's note and the reviewer line (*Auto mode reviewer rows can't be selected as a filter.*) under the title. The frames draw the columns 24 apart, as the app lays them out (`C2pTYF` draws 32). The 960 frame draws the toolbar as the app wraps it when narrow (the controls under the title, the read-back under the chips). That wrapping is for context only and is not part of this change.
- Built 2026-10-08 (ORC-84, `screens/usage/breakdown_table.rs`): each breakdown's table is its own component with its own measurer. Its id names the breakdown (`usage-breakdown-table-profiles`, `-models`, `-pipelines`, `-projects`), with rows `{table}-row-{n}`, cells `{table}-row-{n}-cell-{column}` and names `{table}-row-{n}-name`.

## Memory screens

Drawn 2026-09-30 (ORC-27, Memory 3/3), group `K4KyPj` ("States · Memory", last in 02 / Tasks & runs at y 41963, height 4408; the section is now 46371 high). Screens and interaction only; what memory is and how it is kept is described with the behaviour. Fixture: Kontur, ORC-128 *Add GitHub authentication*, Run 128 in 03 Implement. Proposed copy namespace: `memory.*` (screens, dialogs), `settings.memory.*` and `runs.activity.*` for the invocation panel.

- **Record row** — reusable component `c5ws29` *Memory / Record row*, canonical in board `oghss`: type glyph (lucide 14 `$muted`: handoff arrow-right-left, decision signpost, warning triangle-alert, fact info), optional key (mono 11 `$muted`), text (12/1.6), meta line (11 `$muted`: *profile · run · step · time*, or *You · time · edited time*; project records add the task) with tag chips (10, `$raised`, `$border`, radius 7), then at the right the pin marker (pin 13 `$text`), a status label (`DeizF`, 22 high: *Proposed* clock, *Superseded* replace, *Deprecated* archive; active records have none) and, on hover or keyboard focus, 28-high icon buttons Edit, Pin / Unpin, Deprecate, Delete. *Supersedes 1 earlier version* (chevron, 11/500) opens the history under the text: each earlier version muted, indented behind a 2px `$border` rule, with who superseded it and when. A deprecated record's text is `$muted`; its actions are Restore and Delete.
- **Task › Memory tab** (after Activity; count of active records). `K11Gn` loaded: toolbar with *Search text, keys and tags…* (280, radius 7), *Type*, *Status* (default *Active, proposed*), *Author* filters (`u1XBP` with chevron-down, 32 high) and *New record*; the list is grouped by type in a fixed order (waiting for approval, handoffs, decisions, warnings, facts, todos, summaries, observations, artifacts; empty groups hidden) under 10/500 uppercase headings with a mono count. A proposed record sits first on `$raised` with *Reject*, *Edit* and primary *Approve* (28 high). A handoff shows *Completed*, *Decisions*, *Open questions*, *Next agent should know* as label / text lines; the latest carries the label *Latest · given to every agent*. At the right the 320 `$sidebar` panel *Next agent receives* (`tfGRt` rows): latest handoff, active decisions and warnings, pinned, best matches, *Records 8 of 8*, *Tokens ≈ 2,140 of 3,000*, both saving settings and *Change in Project settings › Memory ↗*; footnote on lexical ranking and that proposed records are not given. `M6wPGL` empty (brain 24, *No memory for this task yet*, what agents save, which step is working, *New record*; search and filters hidden; the panel says the prompt has no Task memory part). `nn89c` search with no results (*No decisions match “oauth refresh”*, lexical explanation, *2 matches in other types*, *Show all types*, *Clear search*).
- **Project › Memory** — a sidebar entry after Instructions (brain) whose count is the number of records waiting for approval, hidden at 0 (all frames in the group use this sidebar). `Qxdzi` loaded: title *Memory*, *Waiting for approval* (15/500, count, note that agents receive a record from their next invocation after approval) with the queue for both scopes in a `$sidebar` box (project records read *Project · …*, task records *Task ORC-131 · …*; a proposal that replaces a record shows *Supersedes 1 active record*), then *Records* with search and filters and the same grouped list; right panel *Project memory* (counts by status, both saving settings, the limits, settings link, a note that agents can’t delete). `JCbWN` nothing waiting: an inbox line *Nothing waiting for approval* with why records wait here; no sidebar count.
- **Record editor** — a dialog in the style of *Add secret*. `pZxXq` *Review proposed record* (640): subtitle with scope and author, *Type* select and *Key · optional* (mono), *Text* area (`$sidebar`, radius 7, 96 high), *Tags* chips with *Add a tag…*, *Importance* select, the inline notice (replace) when approving supersedes an active record, *Source* (events in Activity ↗) and *Agent’s confidence*; actions *Reject* left, *Your edits are saved with the approval.*, *Cancel*, primary *Approve*. In `oghss`: *Edit record* (same fields plus *Pinned · always give it to agents* switch, *Cancel* / *Save record*, note that the author is kept and *edited by you* added) and *New record*, which adds *Save to* (*This task · ORC-128* or *Project · Kontur*; a person’s record is active at once) and the error when the key is already used in that scope.
- **Delete record** — `S2wV6S`, the dialog of `C0QDl9` (600) over the Memory tab: *Delete this record?*, what deleting means (stops being given from the next invocation, earlier invocations show *Deleted record*, can’t be undone), the record in a `$sidebar` preview, *To stop giving it to agents but keep it and its history, deprecate it instead.*; *Deprecate instead* left, *Cancel*, primary *Delete record* (trash-2). Only people delete; agents have no delete action.
- **Project settings › Memory** — fifth tab after Code index, full page (1104 high), laid out like Limits: *Saving* with *Task memory* and *Project memory*, each two option cards side by side (*Save automatically* / *Save after approval*; *· default* on task automatic and project after approval); *Given to each agent* with *Records per invocation* (8, 1–50) and *Tokens per invocation* (3,000, 500–20,000) in `sN3du` fields with read-backs; notice (history) *Changes apply to the next agent invocation*. `B64KNU` saved; `X1X42` out of range: field stroke `$subtle`, circle-alert 12 + 11 `$text` error that names the saved value (*Enter a whole number from 1 to 50. Still 8.*), topbar *2 values not saved*.
- **Activity** — `IFmpE` is 02.I with the invocation panel row *Memory given* · *8 records · 2.1k tokens ›* after *Code index*, a footnote sentence, and transcript entries with the brain glyph (*Searched memory* “session refresh” · 3 records; *Saved to task memory* with its text, ↗ to the Memory tab). Board `upPCd`: the row expanded into the records as given, grouped (latest handoff, decisions, warnings, best matches; glyph, text, estimated tokens in mono), *Open in Memory ↗*; records changed later (*superseded since*, *edited since*, *Deleted record · by you at 12:40* muted with circle-off); *Token limit reached: 4 lower-ranked matches were left out.*; *None* for an invocation before any record existed.

## Delivery

Drawn 2026-10-01, group `i2o8ko` ("States · Delivery & merge queue", last in 02 / Tasks & runs at x 0 / y 46467, height 5312; the section is now 51779 high). Approved by the user 2026-10-01, not built. Fixture: Kontur, base `main` (illustrative: the base is the project's base branch setting); ORC-128 *Add GitHub authentication* (run #128), ORC-131 *Add switcher keyboard shortcuts* (run #134), ORC-135, ORC-137, ORC-139 and the hashes are illustrative.

**Why.** A run used to end in *Ready for review* and a person pressed *Merge into main* (02.P). The run started from the base as it was then, main moved meanwhile, so merges conflicted; Check, Review and Verify had run against the old base, not against what lands; and several finished runs were not ordered.

**Behaviour (decided with the user).**

- **Deliver step.** A pipeline ends in a *Deliver* step, set in the pipeline editor. Modes: *Approve to merge* (default; the run waits in Needs you), *Auto-merge* (merges when every step passed), *Pull request* (visibly unavailable: *Coming later*, never offered as working). Merge method: *Merge commit* (default) or *Squash*. *Resolve conflicts with* chooses the agent profile of the Integrate step. *Re-run Check on the merged result* is on by default and locked on for Auto-merge.
- **Merge queue per project.** Runs that passed every step queue and deliver one at a time, first finished first. For each: merge the current base into the run's branch; if clean, run the Check tool again on the merged result; if it passes, merge into the base (merge commit or squash), remove the run's worktree and its code index, delete the merged branch and mark the task done.
- **Conflicts.** An *Integrate* step runs with the integrator profile: it resolves the conflicts, re-runs Check and briefly reviews its own resolution, then delivery goes on. If it fails, the run leaves the queue as *Failed* and the task goes to Needs you with the conflicting files; nothing reaches the base.
- **Never touch a dirty checkout.** If the base is checked out in the user's repository with uncommitted changes, delivery waits (*Blocked*) and says why; it continues by itself once the checkout is clean, or on *Retry*.
- **The base moved during the re-check.** The check no longer matches what would land, so the run syncs again and Check runs again; a run is never merged on a check of an older base. The outdated check stays in Activity as *Check superseded*.
- **Waiting for approval does not hold the queue.** Later runs go ahead; when the person merges, the waiting run syncs and checks again first if the base moved since its check.

**States of a queue row** (`DeizF` label, then the run, then actions): *Queued* (clock-3; position; *Move up*, *Move to top*, *Remove* = leave the queue and return to Ready for review; no dragging), *Syncing with main* (loader), *Re-checking* (loader), *Integrating conflicts* (loader, agent working) — all three *Open task*; *Waiting for approval* (hand; *Open task*, primary *Review & merge*), *Blocked* (circle-pause; the reason, the uncommitted files in mono; *Open task*, *Retry*), *Failed* (circle-x; the reason; *Open task*), *Merged* (git-merge; hash, time, method, *worktree and code index removed*; *Open task*). The active run shows its phases: *Synced with main › Check on the merged result › Your approval › Merge into main*.

**Where the queue lives.** A sidebar entry *Merge queue* (git-merge) right after Tasks, with the number of runs in the queue as its count (hidden at 0). Not a tab on Tasks: Tasks already has a *Queue* tab for starting runs, and the merge queue is one ordered, project-wide line whose blocked state stops every run in it, not a filter of tasks. Not Pull requests: that screen is GitHub review, and pull request delivery is not available. Every frame in the group uses this sidebar.

**Frames** (x 48 / 1568 / 3088; rows at y 100 / 1148 / 2196 / 3244 / 4292, labels 32 above):

| Frame | Node |
| --- | --- |
| Pipeline / Deliver step, Approve to merge (inspector, *Delivers: approve to merge* chip in the toolbar) | `j8OB3` |
| Pipeline / Deliver step, Auto-merge with squash, re-check locked (0.45, lock and reason) | `jMrlP` |
| Overview / Needs you: *READY TO MERGE* card (*Review & merge*) and *CONFLICTS NEED YOU* | `Trfj3` |
| Ready to merge / dialog (720) over the task: run, branch → base, changes (files, +/−), checks incl. the check on the merged result, commits; *Reject*, *Open changes*, primary *Merge into main* | `L1SoP` |
| Integrate failed / dialog: conflicting files with *Open file*, what Integrate did, the integrator's words; *Reject*, primary *Resolve myself* | `ZTHyA` |
| Merge queue / delivering now, waiting (with order), today | `u2l38w` |
| Merge queue / blocked by uncommitted changes in the checkout | `hz1Nz` |
| Merge queue rows: every state with its actions, and the re-sync when main moved (compact) | `bPm7k` |
| Merge queue / empty, naming a pipeline without a Deliver step | `crYQv` |
| Task / Delivering: status *Delivering*, stage *Deliver*, verification rows *Deliver · sync with main*, *Deliver · Check on the merged result* | `rS8mx` |
| Task / Integrating: stages *Integrate* then *Deliver*, Integrator in *Agents on this run* | `TfSI6` |
| Task / *Merged into main · 9c41e02 · worktree removed* | `wuHGi` |
| Activity / Deliver and Integrate executions: joined the queue, sync with conflicts, resolved files, Check, the integrator's review, check superseded, re-sync, merge, clean-up | `HW9mB` |

Reused: sidebar `zo9Py` with nav items `TSJpn`, `u1XBP`, `DeizF`, `ajikA`, `ZGwlv`, `K8ItM`, `tfGRt`, `l3hyb`, `i9rVFu`, `mDnc3`, `N4SlQq`, `mBkeJ`, `zemhy`, `SOW6S`, the dialog layout of `C2CPbs`, the Memory page layout of `Qxdzi`. No new components or variables. Proposed copy namespace: `delivery.*` (`delivery.step.*`, `delivery.queue.*`, `delivery.readyToMerge.*`, `delivery.integrate.*`).

**Decisions (2026-10-01).**

- **Base branch.** Delivery targets the project's base branch setting; `main` in the frames is fixture data.
- **Approval does not hold the queue.** See above. The frames draw it.
- **Reordering.** *Move up* and *Move to top* on queued rows. There is no dragging. The frames draw *Move up* only; *Move to top* sits beside it.
- **Re-sync limit.** A run re-syncs at most 3 times in a row because the base moved during its check. After that it leaves the queue and goes to Needs you with the reason *main keeps moving* (the base branch's name).
- **Resolve myself.** It shows the run's worktree path with *Open folder* and *Open in editor*. When the person presses *I’ve resolved it*, Orchester:
  - checks that no conflict markers are left;
  - checks that the branch merges cleanly with the current base;
  - runs Check again;
  - puts the run back in the queue.

  This view is not drawn yet.
- **Squash message.** The squash commit is titled `ORC-N: <task title>`; its body holds a short summary of the work and the run number.
- **Pipelines without a Deliver step.** They keep the manual *Merge into <base>* (02.P) on the task page; their runs never join the queue.
- **Ready to merge checks.** The dialog lists exactly the steps the run's pipeline has, plus the re-check on the merged result, so a pipeline with *Verify* shows it. `L1SoP` draws only *Run checks* and *Review* of the fixture pipeline; the built dialog also lists its other steps.

## Application lifecycle and runner connection

These states express the lifecycle in [storage.md › Lifecycle](../architecture/storage.md#lifecycle): closing the window closes the application, the runner and agents cannot outlive it, and nothing restarts automatically after the application itself ended; runs interrupted by a runner drop while the application is open continue automatically after reconnecting. They sit in section 02, group `Z7Fyr8` ("States · Application lifecycle"), over the project Overview (`Twtqc`) background. [Group overview](previews/organization/Z7Fyr8.png).

**Close with active work (`Xz30C`).** The close request is vetoed and a modal opens. Title: *Close Orchester while agents are working?* Explanation: closing stops the runner and every agent; there is no background mode. It lists the tasks with an agent at work (task key, title, current step · agent role, Running status, provider · model), reusing `clVjO` Tasks / Queue row without its selection and open cells. Tasks that only wait for input are named in a note and are unaffected. A short block explains both exits:

- *Wait for agents to save* (primary, right): no new steps or tool calls start; each agent continues to its next checkpoint, runs are marked paused, then Orchester closes. "This can take a few minutes."
- *Quit now* (secondary, isolated on the left with a power icon): stops runner and agents immediately; work after each run's last checkpoint is not recorded as progress; uncommitted changes stay in the worktree and are shown on next start. It is grayscale; the distinction is wording, position and the explanation, not color.
- *Cancel*: keeps the app open, nothing changes.

**Waiting (`a0tTV`).** Same modal, title *Saving before closing*. Summary "1 of 2 runs saved"; each row's status moves from *Saving at next checkpoint…* (loader) to *Saved* (check) with "Paused at 9d07b3f". *Quit now* stays available; the primary becomes a non-interactive "Closes after 1 more run" indicator; Cancel is not offered once the runner has stopped scheduling (proposed; pending product confirmation, since cancelling here would leave the runs paused without closing).

**Runner connection.** The runner can drop; the app reconnects automatically. Connection problems appear as an inline notice (`q5LzI`) at the top of the Overview body and in the sidebar runner card; interrupted runs show an *Interrupted* status (circle-pause icon) with their last checkpoint in the task list, "Running now" drops to 0 with "2 runs interrupted", and the activity feed records the runner events. Runs interrupted by a runner drop continue automatically from their last checkpoint once the runner is back (user decision, 2026-09-24).

- *Reconnecting* (`MAn7V`): "Runner stopped unexpectedly — reconnecting (attempt 2 of 5)". "The 2 active runs were interrupted and keep their last checkpoint. They continue from it automatically once the runner is back; new work waits until then." Trailing: "Next attempt in 8 s", *Open runner log*.
- *Reconnected* (`YHS1v`): "Runner reconnected". "The 2 interrupted runs continued from their last checkpoint." Auto-dismisses ("Closes in 5 s", with a manual close). The task rows in this frame still show the interrupted state and need updating to running.
- *Could not restart* (`EFY7I`): "Could not restart the runner". "Tried 5 times since 14:02. Interrupted runs keep their last checkpoint. Nothing runs until the runner starts." Actions *Open runner log* (opens `.orchester/logs/runner-<date>.log`) and *Restart runner* (primary). Persistent until resolved.

Sidebar runner card (`O5NoB1`), always icon + label: *Starting local runner* / "Agents can start when it's ready" (loader); *Local runner online* / "2 of 4 agent slots in use" (existing dot); *Reconnecting to runner* / "Attempt 2 of 5 · runs interrupted" (refresh); *Runner offline* / "Could not restart · see runner log" (unplug). The card states are overrides of the existing sidebar card inside `zo9Py` instances; no new component was added. The [status bar](#status-bar) proposal recommends removing this card and the 02.F/02.G notices in favour of the bar, keeping only the 02.H notice (pending product-owner decision).

Implemented (2026-09-27): the card is shown as drawn, above Project settings, with the project
selector's switch mark (`Y9sVM2`). The slot count is the runner's own report (agents at work of
its agent slots, as the status bar's *Agents*); right after the runner comes online and before it
reports, the line reads *Agent slots not reported yet*. The reconnecting line names interrupted
runs only when there are some (*Attempt 2 of 5*), and a runner that never started reads *Could
not start · see runner log*. Stopping and stopped read *Stopping local runner* (loader)
and *Local runner stopped* (unplug) without a second line (drawn 2026-09-28 in `O5NoB1`: `hGfPh`,
`ToZBQ`). Not built: selecting the offline card
does not open the notice actions; *Restart runner* and *Open runner log* stay in the status bar
and the Overview notice. The selector opens the project switcher (01.06 B, see [All projects](#all-projects)).

**Session ended abnormally (`o8DCd`).** On the next start after *Quit now* or a crash, a modal: "The last session ended before agents could save". "Orchester stopped unexpectedly on Sep 23 at 14:12 and three runs were interrupted. Nothing has been restarted. Continue all of them from their last checkpoints, or none; work after a checkpoint is not counted as done." A table lists each interrupted run: task (key, title, project · run), *Interrupted during* (step, agent role · provider · model), *Reason*, *Last checkpoint* (commit, time saved), *Worktree* ("3 files not committed" with View changes, "No uncommitted changes", or "Could not read the worktree") — no per-run action. The runs may come from several open projects. A run stopped during an external tool call says "outcome unknown", and a notice asks the user to check the target before continuing, because continuing repeats the step from its checkpoint. Footer: *Open runner log*, *Don't continue* (runs stay interrupted and can each be continued later from their task conversation), *Continue all 3 from checkpoints* (primary). Uncommitted changes are never discarded silently. After *Wait for agents to save*, no dialog appears; runs are simply paused. User decision (2026-09-24): the modal is all or nothing — *Continue all N from checkpoints* or *Don't continue*; rows have no per-run action. Runs not continued stay interrupted and can each be continued later from their task conversation; that task-level action is not designed yet.

**Lifecycle dialog width.** The three lifecycle dialogs (02.C `um9B1`, 02.D `KS2RU`, 02.E `BovYZ`) share one width: 880 at most, the width of the close dialogs (reduced for 02.E from 1120 on user feedback, 2026-09-26). In a window narrower than 880 plus two 32px margins, the dialog shrinks so it keeps at least 32 from the left and right window edges; it never touches them (at the minimum window width, 960, it is still 880 wide). The recovery table (`KzyXA`) fits the 824 content width with fixed columns *Interrupted during* 140, *Reason* 150 (wraps), *Last checkpoint* 88 and *Worktree* 160, 16 apart; *Task* takes the rest (222 at full width) and its title and project · run line stay on one line each, ending in an ellipsis when they do not fit. A worktree state longer than its column wraps beside its icon, and the icon stays with the **first line** (drawn 2026-09-27 in the PW-14 row, `Py65X`: circle-alert *Could not read the* / *worktree* in `$muted`): the state row is top-aligned, the label is Inter 11 with line height 1.5 (16.5px lines), and the 12px icon sits centred in a 16px-high slot, so it lines up with the first line whether the label takes one line or two. All three worktree states use this row (`BwjS0` *3 files not committed* in `$text` with file-diff, `Bv9Wc` *No uncommitted changes* in `$muted` with check).

Proposed copy namespaces: `lifecycle.close.*`, `lifecycle.recovery.*`, `runner.connection.*`, `runner.card.*`. Retry count (5), backoff and the auto-dismiss delay are illustrative values. All seven states were checked with full-size export review and depth-14 layout checks. The only remaining findings are disabled "open task" cells in the queue-row instances in the two close dialogs, which the checker reports at stale positions; they are not rendered.

## Pull request review and agent feedback

The user requested viewing pull requests and leaving comments for an agent to fix code or answer a question. Pull requests is now a project navigation entry after Tasks. It lists PRs from the selected project's repository with linked task, feedback state, current head and agent verification. Ready for review (`vpgrJ`) now opens Review PR in Orchester; Ask agent opens general feedback for the linked PR. Open in GitHub remains a clearly separate external destination.

The new inbox (`wGtxT`) shows three illustrative open PRs: #42 needs review, #43 has an agent fixing feedback, and #44 is not linked to an agent task. An unlinked PR remains readable; link or create an authorized project task before asking an agent to execute work. PR open/draft/merged/closed state is distinct from agent readiness and local verification. Refresh errors preserve existing data with a stale marker and last successful sync time (08.E `FYQMv`); a rate limit shows the reset time and pauses automatic sync until then (08.F `P1tSu`); loading and empty results must not fabricate PRs. Without a usable GitHub connection the screen shows *Pull requests are unavailable* with *Open Connections* (08.D `dmib5`). The list is always the project's one linked repository, shown as a read-only label (`rQjxm`) instead of a repository selector. Sync rules are in [GitHub](#github). Repository lookup, GitHub synchronization and access have not been implemented or verified by these screens.

The diff view (`Rrmhf`) includes a changed-file list, selected revision range, unified code diff, Viewed state and a contextual feedback panel. Code additions/removals use plus/minus signs, line numbers and muted green/red surfaces; a comment selection uses blue, so meaning is not dependent on color alone. The selected fixture shows new lines 43–45 of src/auth/github.ts at 8f3a2c1. The full PR totals six files and +248 / −32; the shown handler is one excerpt, not the entire diff. The test counts are illustrative agent/local evidence, not a claim about GitHub Actions or required branch checks.

Code styling follows the color roles in the official [GitHub theme](https://github.com/primer/github-vscode-theme/blob/main/src/theme.js), adapted to the existing application. Semantic tokens: code-bg #0D1117, code-text #E6EDF3, code-keyword #FF7B72, code-string #A5D6FF, code-function #D2A8FF, code-variable #FFA657, code-constant #79C0FF and code-comment #8B949E. Diff additions use #14261C with #7EE787 markers; removals use #301C20 with #FFA198 markers; selected ranges use #1B3652 with a #58A6FF edge. The application navigation, forms, buttons and feedback panels remain grayscale. These are design tokens inspired by GitHub Dark, not a claim of pixel-identical GitHub rendering.

The shared diff-line component now contains colored text spans with the same 12px Geist Mono typography and original whitespace; line content is retained in metadata for inspection. All 39 displayed code lines across the three review screens were checked by concatenating rendered token contents against the original strings. A linked component specimen in Foundations (`z8v8m7`) covers comment, deletion, addition and selected-addition states. This is editable design content, not a production tokenizer. The desktop highlights with syntect's bundled Sublime grammars (pure-Rust regex engine, no themes): a grammar's scopes are reduced to the design's token roles as GitHub Dark maps them — comments `code-comment`; strings `code-string`; constants, numbers, `variable.language`, `support.*` and markup tags `code-constant`; function names `code-function`; keywords, storage and operators `code-keyword`; declared names (`entity.name.*`) and other variables `code-variable`; parameters, plain variables and punctuation `code-text`. Each hunk starts from the grammar's initial state (a diff shows fragments of a file); removed lines continue the old side, added lines the new side, context lines both. A line cut short leaves its side wherever the cut fell (e.g. inside a string), so after it that side continues from the other side's state, and after a cut context line both start again. TypeScript is read with the JavaScript grammar (the bundled set has none); files without a grammar stay `code-text`. Tabs show as spaces to 4-column stops, control characters as U+FFFD, and lines longer than 1,000 characters are cut with a visible *Line shortened* note. Line numbers use `code-comment`.

Agent feedback has explicit Ask a question and Request a fix modes. Both record project/repository, PR identity, selected base/head, file, diff side, line/range, source excerpt, author and target agent. General PR feedback omits the line anchor but retains the revision context. The target selector defaults to the linked task's Builder profile and displays its model and task; runtime must resolve the actual profile and permissions, not infer an agent from a GitHub author name. Questions request an explanation without changing code. A fix request is an explicit instruction to continue the linked task on its existing PR branch, preserving the applicable run configuration and approval policies.

Proposed visibility choice: the shown comments are Orchester-only and this is labeled beside every thread and before sending. They are not GitHub review comments, do not post to GitHub, and do not constitute a GitHub Request changes review. GitHub discussions, formal approval and merge remain available through Open in GitHub. Native GitHub discussion import/posting is outside this increment and would need explicit source/visibility and delivery states. No actual GitHub comment, review, task execution or push occurred during this design work.

Sending preserves a local draft until persistence is confirmed and deduplicates retries. Distinguish Saved/Queued, Delivered, Working, Answered, Ready to verify and Resolved; a successful send alone must not claim execution. Questions can fail or need clarification without becoming code work. A queued request (`uD6Ho`) links to the existing task queue. A fix respects task concurrency, agent capacity, branch/worktree serialization and the remaining task budget; multiple requests must not create concurrent writers on one PR branch. The sample shows 432k tokens remaining of the existing 1.4M-token budget. If budget, time, access or agent availability prevents execution, show the actual blocking reason and retain the request. Previously finished tasks can return from review to implementation; never create a duplicate task or silently reset limits.

On completion (`i4GUz`), the thread links the agent's answer, pushed commit b7c92e4, changed files and checks for that exact revision. Since your review compares 8f3a2c1 with b7c92e4 and shows two updated files (+4 in the handler, +12 in its test), while the PR still contains six changed files in total. The original 43–45 anchor maps to current 47–49; the added cancellation guard is highlighted separately. The agent reports what it did, but the thread remains Ready to verify. Mark resolved is the reviewer's action and closes only the Orchester thread; it does not approve or merge the PR. Reply or request another fix keeps the conversation and source references together. Resolved threads remain inspectable and can be reopened.

The question state (`qJKSC`) is an alternative first-thread scenario on the original head, not a second event after the fix. It shows a question about lines 48–49, the Builder's explanation and a source link. No code changed and no implementation run was requested by that question. Follow-up questions remain questions; Request a fix explicitly changes the requested action. Answered does not silently mean Resolved.

New commits never discard comments or move them to unrelated code. Track anchors against immutable revisions and explicitly identify a mapped/current or outdated location. If the PR changes while a fix request is drafted (`hfoDa`), preserve the draft, expose the new head, and review/reanchor before sending work against it. A question about historical code can retain its original revision. Force-push, deleted files, renames, binary/large/generated files, repository permission loss and merged/closed PRs need clear handling; do not invent a mapping or write to an obsolete branch. Revalidate the head before applying work and serialize external updates with runner changes. Viewed markers and evidence apply to known content/revisions; modified files and new code require renewed review. Preserve the original review baseline when incoming updates arrive.

Keyboard requirements: file search, file selection, focusable diff lines, keyboard range selection and a visible Add feedback action; Escape closes the contextual composer while retaining the draft and restores the selected-line focus. Ctrl/Cmd+Enter sends, Enter inserts a newline, and empty input disables sending. Pending/failed sends expose progress or Retry without duplicating work. At narrower widths the file list collapses and feedback opens as a panel, leaving the diff readable with deliberate horizontal scrolling for long code. Screen-reader labels must include old/new line, change type and thread state.

Implemented read-only (G4, 2026-09-25): a list row opens 08.02 without the agent feedback parts (feedback panel `q0Dz8e`, line selection `i1dEXj`, agent verification `Cttad`, the linked task button `Y0cC2`); the Agent feedback, Agent checks and Overview tabs, the revision range select and *Viewed* show as unavailable. The pull request is read with `gh pr view` when the screen opens and on Refresh (added to the topbar beside Open in GitHub); the diff is GitHub's compare of the pull request's base and head revisions, fetched only when no diff of that head is stored. A failed refresh keeps the stored diff under a notice and the context line reads *Last synced … · GitHub unreachable* (as 08.E); rate limit, account, not found and missing revisions have their own notice. A diff GitHub does not return for a pull request it did read keeps the pull request synced (*Synced just now*) and explains the diff failure in the notice or panel. Pull requests changing more than 300 files or 20,000 lines are not fetched (*Too large to show here*, numbers written as the locale groups them; the Files changed tab still counts the files GitHub reports); files longer than 5,000 diff lines show their first 5,000 and say how many are not shown. The three new reusable components are QHBNZ (PR list row), j9NDM (changed file), and vEZ42 (diff line). Messages, composer, navigation, tabs, status, properties, notices and recovery panels reuse existing linked components. Proposed copy namespaces: `pullRequests.*`, `pullRequests.diff.*`, `pullRequests.feedback.*`, `pullRequests.revisions.*` and `pullRequests.recovery.*`. Source code, review text and repository paths are user/repository data. [Review-flow overview](previews/organization/McWTM.png).

## Agent profiles and connections

An agent profile defines a role, responsibilities, role instructions and a project connection/model. Connections establish project access to repositories, agent providers and MCP servers. Tools expose the actual callable actions. A provider is selected per profile; the project does not impose one provider on every agent.

The profile editor has *Profile*, *Permissions* and *Used in pipelines* tabs; see [Agent permissions](#agent-permissions).

**Header states (drawn 2026-09-29 for U8, `iWIfR`).** The topbar shows the save status (11 `$muted`), *Save draft* (save, secondary, 36 high) only while there are unsaved edits, and primary *Publish vN*. Published, no draft: *Published v2*, *Publish v3* unavailable (0.4) in the same 107 × 36 slot (04.03A `dWnw7`; Permissions tab 04.04A `t2wCI`, whose footer notice reads *Applies to new runs after publishing*). Saved draft: *Draft saved · Published v2*, *Publish v3* available, no Save draft (04.03 `WPmtS`, 04.04 `T1rjv`, unchanged). Unsaved changes: *Unsaved changes · Published v2*, *Save draft* available and *Publish v3* unavailable until the edit is saved (04.03B `Jw98x`). A draft based on an older version reads *Draft based on v1 · Published v2*; a profile never published reads *Not published*.

The mixed-provider fixture assigns Explorer and Reviewer to Claude · Kontur / Opus, and Orchestrator, Planner and Builder to Codex · Kontur / Sol. The Builder inspector contains separate Connection and Model selectors; provider-supported settings such as reasoning/context appear conditionally. Pipeline steps select a published project profile and inherit its model by default. A step can explicitly override the model, and the inspector shows the effective connection and model.

Connections (`a4Lqb`, section 04) lists GitHub · github.com, Codex · Kontur, Claude · Kontur and Project knowledge MCP; its tabs are All connections, Agent providers, GitHub (formerly Repositories) and MCP servers. The selected connection shows provider, execution location, authentication source, status, last check, available models and read-only Used by links. CLI connections (agent CLIs and `gh`) use the account the CLI is signed in to on this computer; Orchester never signs in, stores or sees tokens, and the project records which account it expects (`J48psR`). The GitHub connection is described in [GitHub](#github). Add connection (`h6CmJ`) configures an agent provider and validates access before saving. Expired MCP access (`RZUKo`) exposes Reconnect and preserves task work. Revocation blocks future calls immediately; ordinary profile publishing does not rotate or revoke credentials.

**Agent connection inspector (`JuVaA` in 04.01, redrawn 2026-09-27 from the app; replaces `ez6IT`).** 344 wide, `$sidebar`, padding 24 / 12 (right) / 24 / 24, gap 20. A details viewport (`xAfju`) scrolls with the [scroll gutter](#components-and-tokens) (thumb `nf9DG`); the actions are pinned below it (`K9gvh`, right padding 12 so buttons end 24 from the edge): *Test connection*, *Edit connection* and the status detail in Inter 10 `$muted` (*The CLI answers and is signed in. Orchester runs it with the account it is signed in to.*; for other states the matching `connections.detail.*` sentence). Details, in order: name, status label, then the rows **Provider** (*Anthropic*), **Program file** (Geist Mono 10, the path shortened in the middle to 36 characters: `C:\…\AppData\Roaming\npm\claude.cmd`), **Runs with** (*Node.js* for an npm launcher of a JavaScript file; for an npm launcher of a program file, such as Claude Code's since 2.1, that program's path in Geist Mono 10, shortened like the program file: `C:\…\claude-code\bin\claude.exe`; *cmd.exe* for another command script), **Script** (Geist Mono 10, the launcher's JavaScript file `C:\…\claude-code\cli.js`; only for an npm launcher), **Version** (*2.0.14*; *Not checked yet* before the first check, *Unknown* when the check could not read it), **Execution** (*This computer*), **Authentication** (*CLI login*), **Last checked**; then *Available models* (heading, model chips, the source note in Inter 10 `$muted`, 12 apart), *Refresh models*, and *Used by* (heading and profile links, 12 apart). *Runs with* and *Script* appear only when what starts is not the program file itself; an `.exe` shows neither. In the fixture the details are taller than the window, so the viewport is cut at *Refresh models* and the thumb shows. The sentence about disconnecting was removed: the app has no remove action.

Sol, Luna and Opus are illustrative labels supplied during the design discussion. Provider adapters, public model identifiers, authentication options and supported settings were not verified by this design task. The implementation must obtain actual model catalogs and capabilities from each adapter and validate profile compatibility; the mockups do not establish working Codex or Claude integration.

## Agent permissions

Decided 2026-09-25 (user): permissions are set on the agent profile and can be changed during a run. They are part of the profile version, like connection and model: *Publish vN* makes them available to new runs, and a run keeps the permissions it launched with unless you change them for that run.

**Profile editor tabs.** The profile editor (`WPmtS`) now has three tabs under the page heading: *Profile* (identity, connection & model, role instructions), *Permissions* and *Used in pipelines* (count). The former "Used in pipelines" block moved to its tab. A one-line summary of the permissions stays visible at the right of the tab strip on every tab (*Edits files in the task worktree · 3 allowed commands · Network off*).

**Permissions tab (04.04 `T1rjv`).** Three settings, then an enforcement panel:

| Setting | Options (fixture: Builder) | Notes |
| --- | --- | --- |
| Repository access | *Read-only* · **Edit files in the task worktree** | Option cards (`lcwsy`). Edits are only ever inside the run's own worktree. |
| Commands | *Ask before running commands* · **Allowed commands** · *Any command in the worktree* | The allowlist is edited as chips (`bpmzN`) plus *Add command*. Entries match by prefix: `pnpm test` also allows `pnpm test src/auth`. Known-safe read-only commands (`ls`, `cat`, `git log`, …) also run once the list has an entry, as Codex runs its own (user decision 2026-10-01, `docs/plan.md` C4); with an empty list every command asks you first (summary *No listed commands, all ask*; mechanism *no listed commands · all ask*). Any other command asks you first in "Needs you" (Claude Code; user decision 2026-10-01, "List + ask"). *Any command in the worktree* runs commands in Codex's sandbox; commands that need more than the sandbox ask you first (only Codex connections add that sentence to the option, since Claude Code cannot offer it). *Ask before running commands* asks about every command (Claude Code) or every command a rule asks about (Codex). |
| Network | Switch (`K8ItM`), **Off** | Off: package installs and web requests fail. |

The *Enforced by <CLI>* panel lists each setting with the mechanism the chosen connection uses and whether it is enforced. It comes from a capability check of the connection's CLI (*checked 14:02*), not from a static table. There is no bypass, "skip permissions" or "full auto" option anywhere, and no setting to add one; the panel says so. Wording in the UI is provider-neutral except in this panel, which names the CLI. For Codex it ends with *Your own Codex rules still apply* (triangle-alert) `allow rules run unsandboxed, unasked` (the user's execpolicy rules are the user's choice, user decision 2026-10-01) and *Repository .codex settings ignored* `worktree untrusted · none load`. The run permissions popover says both under Commands on a Codex step (triangle-alert, hint text). A run whose branch has a `.codex/` folder notes *This branch has .codex/ settings; Orchester ignores them* in its activity. Its Commands row says what happens to other commands: Claude Code *allowed tools · 3 rules · others ask* with the detail *Claude Code also runs known-safe read-only commands, such as ls, cat or git log, without listing them. Other commands ask you first.*; Codex *sandbox · escalations ask you* (or *Codex App Server · one command approval*) with the detail *Beyond the sandbox, commands ask you first.* A read-only Claude Code step with a list reads `edit tools off · list runs · others ask` on Repository access (`edit tools off · commands ask` without one).

**Options a connection can't enforce are disabled (04.B `W0Owo`, compact; user decision 2026-09-25).** An option the chosen connection's CLI cannot enforce is not offered: it is disabled (45% opacity) with a lock icon and the reason under it (*Unavailable on this connection: Claude Code can't confine commands to the worktree.*). To use it, assign a connection that can enforce it. Fixture: Explorer on Claude · Kontur — *Any command in the worktree* is disabled; the network switch is locked **On** because Claude Code blocks its web tools but not network use by commands (*Off is unavailable on this connection … so network stays on. The run summary says so.*). Network-off follows the same rule because disabling it leaves a usable profile (network simply stays on, and says so). The exception: if the rule would leave a setting with no enforceable option (e.g. neither *Allowed commands* nor *Any command* is enforceable), the profile cannot run and publishing is blocked with that reason and a link to change the connection — Orchester never falls back to an unenforced option. When the connection changes, a selected option that becomes unavailable is reset to the most restrictive available one and the change is shown before publishing.

**Pipeline steps narrow, never widen.** The step inspector (`Q21HOA`, and the same inspector in `zfjsK`, `yf6XH`, `UjHDL`) shows *Repository access*, *Commands* and *Network* under the step limits. A step can only choose values at or below its profile's (read-only instead of edit, a subset of the allowlist, network off); wider values are not offered. A narrowed row says so (*Commands · narrowed* `git, pnpm test`) and a note names what the profile additionally allows (*Reviewer v2 also allows pnpm lint*). With no profile selected (`UjHDL`) a single *Permissions —* row and a note say that permissions come from the profile. Effective step permissions = profile ∩ step. The former single "Repository access" value is now the first of these rows.

**On the task page.** The run panels show the current agent's effective permissions as a row with a link: *This run* in 02.10 (`RD4pc`, *Explorer · Read-only · view ↗*) and *Working context* in 02.12 (`jgqPe`) and 02.A (`Oqmog`) (*Builder · Edit files · 3 commands · Network off · change ↗*). The Activity tab's invocation panel repeats it.

**What the row shows: the launch, not the next settings (ORC-44, drawn 2026-10-02).** The row shows what the step's current or last invocation actually launched with. The label carries the qualifier and the value stays bare: *Running with* (an invocation is open), *Last ran with* (the step's last invocation ended), *Next run uses* (nothing launched yet). The value is *Builder · Edit files · 3 commands · Network off · MCP asks*; the MCP part is *MCP asks* or *MCP auto*. An invocation that exists but has not recorded its launch reads *Running with · starting…*; an older invocation from before launches were recorded reads *Running with · not recorded*. A value wraps only at the "·" separators, never inside a part; when the row cannot hold it on one line, the value stacks (right-aligned) above *change ↗* / *view ↗*, which is never clipped. Drawn in place: 02.10 `H1wCPK` (stacked in the 288 panel), 02.12 `YOcfT`, 02.A `d6xleF` (*Last ran with*, paused), 02.S `Lr8Dg` and the 02.J anchor `rnGN7` (one line at 720). All states, the inline row at window 1440 (row 720) and at 960 (row 240, stacked), and the Activity invocation panel (*Running with* for the open invocation, *Ran with* for an ended one, *Ran with · no permissions · it did not start* for one that never started) are on the compact board `iPHGE` in 04.2 `DMBmo`. *Ran with* and the never-started wording are proposals that differ from the app's current *This invocation did not start · it ran with no permissions*.

**Decisions (2026-10-02, ORC-44).** The user approved the frames and the designer's choices: during Apply now the change is named in the notice title (*Applying pnpm build added at the next checkpoint…*), not appended to the row; Activity labels a finished invocation *Ran with* and one that never started *Ran with · no permissions · it did not start*; in ask-before-commands mode the command part reads *Commands ask*; the ended-run popover dims its controls to 0.5; the 960 inline row is specified by its specimen on board `iPHGE`.

**Change for this run (02.J `q4K1G`).** The link opens a popover anchored above the row (same surface as the status bar popover: `$raised`, `$border`, 12px radius, shadow 0 8 24 #00000066; 460px wide). It shows the agent, step, model and the profile version the values came from, then the three settings in compact form, with changed values outlined (fixture: `pnpm build`, *added for this run*). Timing, stated in the popover: **changes apply from the next agent invocation; the invocation running now keeps the permissions it started with.** This is the honest rule for CLIs that read permissions at launch. An invocation is one launch of the agent CLI (a step execution, a retry, a continue after pause or limit). Two ways to apply, both for Run 128 only: *Apply from next invocation* (primary) and *Apply now* (user decision 2026-09-25). *Apply now* lets the agent finish the command it is running, stops it at the next checkpoint, commits the worktree as a checkpoint and restarts the current step from it as a new invocation with the new permissions. The popover has one key/value line *Running with · Builder · Edit files · 3 commands · Network off · MCP asks* between its header and the takes-effect notice, and no extra heading (2026-10-02: `UPDBO` in `NNRH8`, `zCqp2` in `W29Od2`'s `ecCTw`, which wraps it at a "·"); the notice was shortened so the popover stays about as tall as before, and it was moved up by the difference so it still ends 6 px above its row. An ended run (04.2 `YgLEg`) shows *Last ran with*, the controls read-only (0.5, no remove or add), the notice *This run has ended, so its permissions can’t be changed here. Continuing the conversation starts a new invocation of Builder with these settings.* and only *Close*. *Apply from next invocation* shows the notice *Saved for the next invocation · pnpm build added* (bookmark-check, same `q5LzI` notice) while the row keeps *Running with …* (`A3h7B4`, first in `LJmTH`). Pending state (`LJmTH`, middle; changed 2026-10-02): the row keeps *Running with …* and the notice title carries the pending change, *Applying pnpm build added at the next checkpoint…* (chosen over appending *· applying … at next checkpoint* to the row, so the row keeps one meaning); the loader notice says what happens to in-flight work — the running command completes, nothing in the worktree is lost, the agent's unsaved reasoning since its last turn is not carried over unless the CLI can resume its session — and *Keep for next invocation instead* cancels the restart. Once the agent has stopped at its checkpoint the notice reads *Restarting with the new permissions…* with no button, and says Implement restarts as soon as the run continues; if the restart never started it reads *New permissions not active: Builder did not restart* (triangle-alert) and points to Activity; both rows read *Last ran with …* (04.2 `D8qjLz`, 2026-10-02). Applied state (right): *New permissions active since 12:09*, the new checkpoint (*saved to apply permissions*) and invocation 2; the first invocation's transcript stays in Activity. *Also save to the Builder profile* adds the same change to the profile draft (v3); new runs get it only after publishing. Escape or *Cancel* closes without changes and returns focus to the row. 

**Blocked by permission (02.K `H03XN`) — later milestone, not v1.** When an agent asks for something outside its permissions (fixture: `pnpm add @octokit/auth-oauth-app`, which also needs network), the task page shows a *Needs you* request card at the top of the run: the exact command, the worktree, what else it needs (network), the agent's stated reason, and three decisions — *Deny* (the agent is told and continues without it), *Allow once* (this exact command, one time, with network for that command) and *Allow for this run* (adds `pnpm add` and network access to this run's permissions from now on). The agent waits on that command. **Timeout: 10 minutes, then denied** (user decision 2026-09-25); the card says *denied automatically in 9:42 (at 12:18) if you don't answer*, and on expiry the card is replaced by a *Denied · no answer* entry in Activity. The request counts in *Needs you* in the status bar and Overview, and takes the status bar activity slot with its countdown (state I, `OwnMs`). It needs a CLI hook that asks Orchester instead of denying: Claude Code's permission-prompt tool can route the decision to an MCP tool the runner serves; `codex exec` has no such hook (Codex's app-server protocol has approval requests). **Implemented 2026-10-01 in a smaller form** (`docs/interactive-tool-approvals.md`): a command outside the allowed list (Claude Code) or one that needs more than the sandbox (Codex, every step through App Server) reaches the existing *Needs you* approval with *Approve once* and *Deny*; there is no *Allow for this run*, and no timeout (the agent waits, its active time stopped). Deny fails the step. A file change outside the writable folders is still denied without asking.

**How settings map to CLI flags (implementation notes — every item is unverified until checked against the installed CLI versions, on Windows and macOS).**

| Setting | Codex CLI (`codex app-server`) | Claude Code (`claude -p`) |
| --- | --- | --- |
| Working directory | process cwd = worktree, `thread/start` `cwd` and the turn's local environment | process cwd = worktree |
| Read-only | `thread/start` `sandbox: read-only` | deny edit tools: `--disallowedTools Edit Write NotebookEdit` |
| Edit files in the worktree | `sandbox: workspace-write` | `--permission-mode acceptEdits`; with allowed commands `default` and `--allowedTools "Edit(//<worktree>/**)"` |
| Allowed commands | unavailable: no way to deny every command outside a list | `--allowedTools "Bash(git:*)" "Bash(pnpm test:*)"` in `--permission-mode default` with `--permission-prompt-tool`; commands Claude Code classifies as read-only also run, any other command asks you first |
| Any command | sandbox, approval policy `{"granular":{"sandbox_approval":true,…}}`: commands that need more than the sandbox ask you first | **not confined to the worktree** (Claude Code's own sandbox is not used) → option disabled |
| Network off | `workspace-write` network access off (Windows: not enforceable, network stays on) | `--disallowedTools WebFetch WebSearch`; commands keep network → *Off* disabled, network stays on |
| Network on | `-c sandbox_workspace_write.network_access=true` | web tools allowed |
| Ask before running commands | approval policy `rules` (every command an execpolicy rule asks about) besides `sandbox_approval` | `--settings {"permissions":{"ask":["Bash"]}}` with `--permission-prompt-tool <mcp tool>` |
| Never used | `--dangerously-bypass-approvals-and-sandbox`, `--full-auto`, `codex exec` | `--dangerously-skip-permissions`, `--permission-mode bypassPermissions`, `dontAsk` with commands |

The fixture shows all three Builder settings enforced by Codex. *Allowed commands* is a disabled option for Codex profiles (then only *Any command in the worktree* remains, which the sandbox enforces, and *Ask before running commands*). Sandbox behaviour differs by platform (Codex's sandbox on Windows is the least certain); the capability check must run per machine, and whatever is missing is disabled with its reason, never silently treated as enforced.

**Extra writable folders (implemented 2026-09-28, not drawn yet).** Folders outside the task worktree that an agent which edits files may also write to, such as a build or cache folder outside the worktree: without it, Codex's `workspace-write` sandbox refuses those writes. A project variable that names a folder gives no write permission of its own; the person adds the folder here like any other. They belong to the profile's permissions, not to the project, because permissions are set per profile and published with it (above): a read-only Explorer needs no build folder, the enforcement panel names what each profile's CLI grants, and a step that only reads gets none (steps cannot add folders). They apply only with *Edit files in the task worktree*; with *Read-only* they are kept but unused.

- A folder is an absolute path or a path relative to the repository root, checked when it is added and again when each agent starts (`orchester_core::permission::writable`): it must exist, or lie under the repository's `.orchester/`, where the runner creates it before the CLI starts. Refused: the repository root, a folder that contains it, anything else in the checkout, `.orchester/` itself and its `worktrees` and `logs`, the home folder, a folder that contains it, and a file system root; links are followed first. A folder that can no longer be given (moved, deleted) stops the agent before its CLI starts, and its activity says why.
- Agents › Permissions: *Extra writable folders* after *Repository access*, built from the allowed commands (a field and *Add folder*, Enter adds, then the entries `bpmzN`): each folder is an entry on its own line, as wide as the column, that removes itself (a long path ends in an ellipsis before its ×); the refusal is an alert line under the field that wraps, and the hint says what the folders mean (or, with *Read-only*, that they are unused). The summary reads *Edits files in the task worktree and 1 more folder*; the enforcement panel adds *Extra writable folders · enforced* with the mechanism.
- Codex: `-c sandbox_workspace_write.writable_roots=["<folder>",…]` (TOML basic strings), on new and resumed sessions alike (`exec resume` has no `--add-dir`). Checked 2026-09-28 with Codex CLI 0.139.0 on Windows 11 and an empty `CODEX_HOME`, without a model turn: `codex exec` and `codex exec resume` list the folders among the sandbox's writable roots, `codex debug prompt-input` shows the model is told them, and `codex sandbox` with `windows.sandbox=unelevated` lets a command write in a listed folder and denies one beside it. A folder that does not exist is accepted silently, hence the runner creates it. The elevated sandbox needs a one-time setup per `CODEX_HOME` (an administrator prompt), so it was not run from an empty one; its log in the user's own Codex home shows it grants the sandbox write access to each writable root. The panel reads *sandbox · writable roots*.
- Claude Code: one `--add-dir <folder>` each, so its edit tools may write there as in the worktree; its commands need nothing, since Claude Code does not confine commands to any folder (see *Any command* above). Checked with Claude Code 2.1.89 and an empty `CLAUDE_CONFIG_DIR`, without a model turn: the debug log adds each existing folder as a permission directory and leaves a missing one out; the folder's `.claude/settings.json` is looked up but, with `--setting-sources ""`, its rules were not applied. The panel reads *--add-dir for edit tools · commands not confined*.

Proposed copy namespaces: `agents.permissions.*`, `agents.permissions.enforcement.*`, `pipelines.step.permissions.*`, `runs.permissions.*`, `runs.permissionRequest.*`.

## Command categories and approval policy

Drawn 2026-10-01, not built. Group `lozqh` ("States · Command categories and approval policy") in the section `DMBmo` ("States · Agent runtime permissions (ORC-44)", top level at x 46784), after its ORC-44 frames, at x 0 / y 1864, 4896 × 4924. The section grew from 3040 × 2300 to 4896 × 6884 so the group fits the x 48 / 1568 / 3088 columns. The user decided the behaviour; these frames show it. Fixture: Kontur, ORC-142 *Split the sync worker module*, Run 142 on Feature delivery, 03 Implement · Builder on Claude / Opus 5.5 (Builder v3), and Reviewer v2 for the read-only variant. Every command, path, time and count is illustrative. Proposed copy namespaces: `agents.permissions.policy.*`, `pipelines.step.permissions.policy.*`, `runs.permissions.policy.*`, `runs.permissionRequest.*` (the dialog) and `runs.activity.decision.*` (marks and the decision summary).

**What it does.** When an agent's CLI asks before running a command (Claude Code's permission prompt, Codex's `sandbox_approval`), Orchester classifies the command and answers by its category, according to the profile's policy. Only what the policy sends to a person reaches *Needs you*. Every answer still covers one request: Orchester never gives the CLI a session grant, an `acceptForSession`, an `updatedPermissions` rule or an execpolicy amendment. Commands the CLI runs without asking (a listed command, a known-safe read-only one, a command inside Codex's sandbox) never reach the policy.

### Categories and classification

| Category | What lands here | Examples |
| --- | --- | --- |
| Read-only | Reads files, history and state; changes nothing | `cat`, `rg`, `wc`, `git status`, `git diff`, `git log` |
| Worktree | Writes, moves or deletes only inside the run's worktree | `sed -i`, `git mv`, `rm`, `git add`, `git commit`, `mkdir`, `cargo build`, `cargo test` |
| Run temp | Temp-folder files this run created earlier | `rm /tmp/worker_orig.rs` after this run wrote it |
| Outside write | Writes or deletes outside the worktree, including a temp file the run did not create | `cp src/sync/worker.rs /tmp/worker_orig.rs` |
| Network | Reaches the network | `curl`, `git fetch`, `git clone`, `cargo add`, `pnpm install` |
| History / destructive | Rewrites or discards history, or deletes outside the temp folder | `git push`, `git reset --hard`, `git rebase`, `git branch -D`, `rm -rf` outside the temp folder |
| Unknown | Anything not parsed to the end | scripts, `-Command` content Orchester can't read, unexpanded variables, aliases and functions |

Rules (board `U97QE`):

- **Parse into simple commands.** Bash and PowerShell, joined by `&&`, `||`, `;`, `|` or a new line, including subshells and `$( )`. A redirection is a write to its target (`> /tmp/out.txt` is Outside write).
- **Classify each part** by program, subcommand, flags and the paths it writes, resolved against the worktree with links followed. `git push` is History / destructive although it also uses the network.
- **The strictest part decides.** Each part gets its category's policy value; the command gets the strictest: Deny over Ask me over Allow. On a tie the stricter category names it, in this order: History / destructive, Unknown, Outside write, Network, Worktree, Run temp, Read-only.
- **Run temp is earned.** A file in the temp folder is Run temp once a parsed command of this run created it and that command ran successfully (for example an approved Outside write that exited 0). Another run's temp files, and files whose creating command failed or was not parsed, stay Outside write (user decision 2026-10-01).
- **Nothing is guessed.** What Orchester cannot read to the end is Unknown, and the dialog says where reading stopped.
- **Compound lines are read to the end** (decided 2026-10-14, after 106 of the person's 134 decisions in 16 runs were compound Claude Code lines read as Unknown). A pipeline, a `;`, `&&` or `||` chain and a new line are as strict as their strictest part, so `cargo clippy … 2>&1 | grep -E "…" | head -30` is Worktree. `head`, `tail`, `grep`, `sort`, `uniq`, `wc` and `tee` without a file are Read-only filters. `2>&1` only joins outputs. A redirection to `/dev/null`, `NUL` or PowerShell's `$null` writes nothing; one to any other file is a write there. Quoted arguments are one word, operators inside them included. `echo` prints any argument, `---` too. The status an earlier command left is a number whatever ran: bash's `$?` and `${PIPESTATUS[N]}` and PowerShell's `$?` and `$LASTEXITCODE` are read only as arguments of `echo`, `Write-Output` or `Write-Host` (`echo "exit=${PIPESTATUS[0]}"`). Anywhere else (`rm $?`, `x=$?`, `> out$?`, `bash -c "echo $?"`) they are Unknown, and so is `${PIPESTATUS[@]}`. A variable the line sets to a literal is read where it is used. Its value may hold a backslash (`L="$USERPROFILE/.cargo/…"` on Windows), and then it is read only where it is used quoted. Still Unknown: a command substitution (`$( )`, backticks), a subshell or group, any other variable, an alias or function, a loop that is not the whole line, a `**` glob, and a variable set before a program.
- **Dependency sources read like the worktree, by plain readers only** (decided 2026-10-14; narrowed the same day after a security review). The crates Cargo unpacks (`<cargo home>/registry/src`), its git checkouts (`<cargo home>/git/checkouts`) and the toolchains' library sources (`<rustup home>/toolchains/*/lib/rustlib/src`) hold published code nobody here wrote. A link in it may lead anywhere, and a `.cargo/config.toml` or `.git/config` in it may name a program. The Cargo home is `~/.cargo` and `CARGO_HOME`; the rustup home is `~/.rustup` and `RUSTUP_HOME`.
  - Only plain readers read these folders as they read the worktree: `cat`, `head`, `tail`, `wc`, `nl`, `cut`, `sort`, `uniq`, `cmp`, the checksum programs, `ls`, `rg`, `grep`, `sed`, `awk`, `file` and `stat`, in their known-safe forms. They read the files and folders the line names, or the folder they run in, and walk a folder without following its links. A known-safe read of that kind runs on its own, and auto-approval approves it.
  - Any other program there reads outside the worktree.
  - A `cd` there leaves the line working outside the worktree.
  - A write or delete there is outside the worktree.
  - A `.git` folder in them is a credential location. So is the rest of a Cargo or rustup home (`credentials.toml`, `config.toml`, the registry's index and cache, `settings.toml`), wherever the home lies.
  - Orchester's `read_external_file` and `search_external_file` run there without asking, as in the run's sources. They check every file they read with its links followed, and refuse `.git`.
  - npm's cache is not counted: it is not plainly published sources.
- **A folder's settings run only in the agent's own folders** (decided 2026-10-14 after the same review). Some programs load settings or code from the folder they run in and the folders above it: git (`core.fsmonitor`, `diff.external`), `gh`, Cargo and rustup (`build.rustc-wrapper`, `rust-toolchain.toml`), the package managers, `go`, `dotnet`, the build tools of the Worktree list, and Python and Node (their modules). Outside the worktree and its writable folders such a program is Unknown, whatever it does there, even `git status` or `cargo metadata`. The worktree's own settings are the agent's own.
- **Readers that follow links are Unknown** (same decision). `diff -r` (unless `--no-dereference`), `tree -l`, `du -L`, `grep -R` and `find -L` follow the links in a folder they walk out of it, in the worktree too, so they are Unknown everywhere. The classifier resolves only the paths the line names, not links found inside a folder.

### Policy, defaults and narrowing

The profile's Permissions tab gets **When the agent asks** after *Commands* (04.04 layout; frames `p8g7C`, `jYJJa`, `xMjXP`, shown scrolled to the section with the scroll thumb): a heading with the state (*Defaults for a profile that edits files* / *3 changed from the defaults*) and *Reset to defaults* (0.4 while nothing changed), one sentence on what it does, the relation to Allowed commands, then one row per category (*Approvals / Policy row*: icon, name, examples, and the *Approvals / Policy choice* control Allow · Ask me · Deny in the segmented style of the Conversation toolbar). A changed row is outlined (1.5 `$text`) and its examples end *· changed from Ask me*; the save status and *Save draft* follow the U8 header states. The tab's summary adds *When it asks: 3 allow, 4 ask*. The policy is part of the profile version: *Publish vN* makes it available to new runs.

| Category | Writing profile | Read-only profile |
| --- | --- | --- |
| Read-only | Allow | Allow |
| Worktree | Allow | Ask me; Allow unavailable (0.4) |
| Run temp | Allow | Allow |
| Outside write | Ask me | Ask me |
| Network | Ask me | Ask me |
| History / destructive | Ask me, or Deny; Allow unavailable | the same |
| Unknown | Ask me, or Deny; Allow unavailable | the same |

The read-only frame (`xMjXP`, Reviewer) says why Worktree asks: Reviewer doesn't edit files, and an approved command runs outside any sandbox, where it can change files.

**Narrowing.** Effective policy = profile ∩ step ∩ run, per category. A pipeline step (`gPZzc`, the Review step inspector scrolled to its permissions) shows *When the agent asks* under the step constraints with a compact choice per category; values wider than the profile's are unavailable (0.4), a narrowed row is outlined and bold, the heading counts *2 narrowed*, and a note names what the profile allows (*Reviewer v2 asks you about Outside write and Network; this step denies them*). The run popover (`BuFfP`, 02.J layout) shows the effective policy in two columns, narrowing only (*History* denied for Run 142, *1 narrowed for this run*). A policy change applies **from the next request**, because Orchester answers each one; the other permissions keep their next-invocation / *Apply now* rule, so with only a policy change the primary button is *Apply from next request* and *Apply now* is 0.4. *Also save to the Builder profile* adds the narrowing to the profile draft. The popover's MCP rows are unchanged and left out of this frame for height.

**Allowed commands and Deny.** Allowed commands stay an exact prefix list on top of the categories: a command whose every part matches a listed prefix runs without asking, whatever its category, **except a category set to Deny: Deny wins over a listed command** (user decision 2026-10-01). Orchester enforces it at the CLI: for Claude Code it adds deny rules for the denied category's forms (for History / destructive, `Bash(git push:*)`, `Bash(git reset --hard:*)`, …), which Claude Code checks before its allow rules; for Codex it declines such a request in its own approval handling. Every other command is classified. When a listed prefix overlaps a denied category, the profile warns (`jYJJa`: *git in Allowed commands also covers git push and git reset --hard*); with that category on Deny, the warning says those forms stay denied. *As drawn, `jYJJa`'s warning and the classification board `U97QE` (rule *Allowed commands come first*, enforcement row *Allowed commands run first, unseen by the policy*) say the list wins, Deny included; the frames were not redrawn, and this text wins.*

**Enforcement panel** (`U97QE`, and the panel of `p8g7C`): rows *When the agent asks · Orchester decides by category* (`permission prompt → policy · one answer per request`; Codex `sandbox_approval → policy · accept, never acceptForSession`), *Allowed commands run first, except denied categories* (`allowed tools · 3 rules · deny rules for denied categories`; Codex: `declined in Orchester's approval handling`), *Every part of a command is classified* (`bash and PowerShell parser · strictest part decides`) and *No session grants* (`updatedPermissions never sent`). Codex adds *Commands inside the sandbox don't ask* (`Read-only and Worktree run in the sandbox · the policy sees escalations`) and keeps *Your own Codex rules still apply*. The steps-narrow line reads *A pipeline step or a run can make the policy stricter, never wider.*

### The approval dialog

The `GbNSA` dialog at 720 wide (`vyShL`, over the task Summary; on the Conversation tab the same request is the inline card, below). Header: *Approve command?* and what the policy did. Body: the category chip (*Approvals / Category chip*) with the policy value (*Ask me · Builder v3*) and *waiting since 12:11*; the reason in one line (*Writes outside the worktree:* `/tmp/worker_orig.rs`); the whole command in a mono block, never shortened; Agent, Working directory and Agent's note; what *similar* means here; *Each answer covers one request; Orchester never gives Builder's CLI a session grant.* Footer: *Always allow for this profile…* (text action, left), *Deny*, *Approve similar in this run* and primary *Approve once*. Deny keeps the command from running; the agent is told why and continues, the same for a person's Deny and a policy Deny (user decision 2026-10-01). Today both CLIs' approvals fail the step on Deny (`docs/interactive-tool-approvals.md`); the implementation must change that so the step continues. The footer drawn here is superseded by [Approval choices and waiting requests](#approval-choices-and-waiting-requests) (2026-10-02): Deny and Approve once in the footer, the wider answers as tiles above it, and Decide later.

- **Compound command** (`k4cz1C`, first row): the dialog lists the parts (number, the part as written, category chip, what the policy says: *Allow · policy*, *Allowed command*, *Ask me · asks you*). The part that forced the ask is outlined (1.5 `$text`, on `$raised`), and the chip and reason name it (*asks you · forced by part 2 of 3*, *Part 2 writes outside the worktree*). Approve once runs the whole command once; Approve similar applies to the forcing part; with several asking parts, Approve similar is unavailable (*Several parts ask: approve once or deny.*).
- **What "similar" means** (`k4cz1C`, second row): the same program and subcommand (`cargo add`; the program alone when it has no subcommands, `cp`) classified in the same category (Network), until this run ends. Flags, arguments and packages may differ. A command with another subcommand, a part in another category, or another part that asks still asks. The scope panel opens under the request while *Approve similar in this run* has hover or focus, and the dialog grows upward to make room, so that the answers do not move (2026-10-06): the rule, then *Runs without asking* and *Still asks you* examples with why. Each later request is still answered one at a time and marked *approved by similar rule*. Rules live with the run, are listed with *Revoke*, and never reach the CLI.
- **Always allow for this profile** (`ZJt4d`, first row) opens a second step in the dialog: the prefix to add, starting at program + subcommand (`git fetch`), with the exact command as the narrowest choice and a prefix that would also cover History / destructive forms locked with the reason (`git`); then the version note *Publishes Builder v4: v3 plus git fetch in Allowed commands*: runs that start later use v4, Run 142 keeps v3 and also allows the prefix until it ends (a change for this run), an unsaved draft is kept and gets the prefix, and the pipelines that use Builder are named. *Back* and primary *Add to v4 and approve*.
- **Unknown** (`ZJt4d`, second row): *What Orchester read*, part by part, and where it stopped (`& $env:SYNC_SCRIPT -Rebuild` calls the program named by a variable that is unset until it runs). Approve similar and Always allow are unavailable (0.4, lock): *Orchester can't tell which later commands would be alike.*
- **Deny by policy** asks no one: Orchester denies the request with the reason as the CLI's message, for example *Denied by Orchester policy (History / destructive: git push --force). Don't retry it. Continue without it, or say in your final message why it is needed.* For Unknown: *Orchester can't check what ./scripts/regen.sh runs. Run its steps as separate commands instead.* The step continues, as after a person's Deny.

### Decision marks and the run's decisions

Every command an agent asked about carries a mark (*Approvals / Decision mark*: 18 high, icon 11 and text 10 `$muted`, hairline outline, radius 4) in the Conversation timeline (inside tool groups), the Tools view and Activity rows, beside the exit code and duration:

| Mark | Icon | When |
| --- | --- | --- |
| *approved by policy · Worktree* | shield-check | the effective policy is Allow for the category |
| *approved by you · Outside write* | user-check | Approve once; after Approve similar it adds *· similar rule added* |
| *approved by similar rule · Network* | copy-check | a rule from Approve similar answered it; the row names the rule |
| *denied by policy · History* | shield-x | Deny without asking; the only filled mark (`$raised`, `$border`, `$text`), on a problem row that shows what the agent was told and never folds into a group |
| *asked you · Unknown* | hand | waiting; once answered it becomes *approved by you* or *denied by you* |
| *not asked · Allowed command* / *· known-safe read-only* | list-checks | the CLI ran it without asking; shown so nothing is silent |

On the Conversation tab a waiting request is the inline approval card (as decided for the merged Conversation) with the category chip, reason, command and the same answers (`A9j8eD`), and the working line reads *Builder waits for your answer · since 12:19, its active time stopped*.

**Per-run summary.** The Details pane opens on **Command decisions** (`A9j8eD`): totals by answer (by policy, by you, by similar rule, denied by policy, waiting) and a table by category with the same four answers as icon columns. Each row is a link that shows those commands in the timeline (*Tools*, filtered: `INTTC`, with *Showing Network commands only · 4 of 43 … · Show all*); the line under the table counts what ran without asking. Below it, *Approved as similar in this run* lists the rules with *Revoke*. Under the run's end marker one line counts the decisions with *Details ↗* (`E2cL0U`); the task Summary repeats it under the run's permissions.

**Similar rules and Revoke.** Listed in the run popover (`BuFfP`) and the Details pane: the rule (`cargo add …`, category chip), since when and how many commands it approved, and *Revoke*. Revoke applies from the next request: those commands ask again (*asked you · Network*, and the row says the rule was revoked at 12:22); commands approved before stay approved. *Undo* restores the rule until the run ends; rules always end with the run (`E2cL0U`).

### Frames

Columns x 48 / 1568 / 3088 in `lozqh`; rows at y 100, 1148, 2355 and 3868 (the compact boards are taller than 960); labels 32 above.

| Frame | Node | Position |
| --- | --- | --- |
| Profile permissions · When the agent asks · defaults, writing profile (Builder), enforcement panel with the policy rows | `p8g7C` | 48 / 100 |
| Profile permissions · edited, unsaved (Network Allow, History and Unknown Deny) and the listed-prefix warning | `jYJJa` | 1568 / 100 |
| Profile permissions · read-only profile (Reviewer): Worktree asks, Allow unavailable | `xMjXP` | 3088 / 100 |
| Classification rules, categories table, shared parts (canonical home of the new components) and the Claude Code and Codex enforcement panels (compact, 1440 × 1119) | `U97QE` | 48 / 1148 |
| Pipeline step inspector · the Review step narrows the policy | `gPZzc` | 1568 / 1148 |
| Run permissions popover · the policy for this run, narrowed, and the similar rules with Revoke | `BuFfP` | 3088 / 1148 |
| Approval dialog · one command, its category and reason, four answers | `vyShL` | 48 / 2355 |
| Approval dialog · compound command; Approve similar scope explained (compact, 1440 × 1425) | `k4cz1C` | 1568 / 2355 |
| Approval dialog · Always allow with the profile-version note; Unknown command (compact, 1440 × 1225) | `ZJt4d` | 3088 / 2355 |
| Conversation · decision marks, denied by policy, inline Unknown request, Details on Command decisions | `A9j8eD` | 48 / 3868 |
| Conversation · Tools view filtered to Network from the summary | `INTTC` | 1568 / 3868 |
| Decision marks legend, denial rows, a revoked similar rule, the run's end summary (compact, 1440 × 797) | `E2cL0U` | 3088 / 3868 |

Reused: `zo9Py`, `TSJpn`, `SOW6S`, `u1XBP`, `DeizF`, `ZGwlv`, `lcwsy`, `bpmzN`, `tfGRt`, `K8ItM`, `q5LzI`, `jNTVN`, `ajikA`, `mDnc3`, `N4SlQq`, `V17jfc` and `C3nu0z`, and copies of `HnBBr`, `wujuh`, `W29Od2`, `t0q3ws` and `gNa91` (removed 2026-10-04; now C·11 `oryKr`). New components, canonical in `U97QE`: `M7pU0A` *Approvals / Decision mark*, `EfQuh` *Approvals / Category chip*, `J8OVYQ` *Approvals / Policy choice* and `KU2fY` *Approvals / Policy row*. No new variables.

**Decisions (2026-10-01).** The user approved the frames and answered their open questions. Where a frame differs, these decisions and the text above win; the frames were not redrawn.

- **Allow is unavailable** for Unknown and History / destructive on every profile, and for Worktree on a read-only profile, as drawn.
- **Deny wins over a listed command.** Orchester adds CLI deny rules for the denied category's forms (Claude Code checks deny rules before allow rules) and, for Codex, declines the request in its own approval handling. The profile keeps its warning when a listed prefix overlaps a denied category. `jYJJa` and `U97QE` still say the list wins.
- **Deny lets the agent continue,** for a person's Deny and a policy Deny alike: the agent is told why. Both CLIs' approvals currently fail the step, so the implementation must change that.
- **Always allow** publishes a new profile version with only that prefix added and keeps any unsaved draft, as drawn.
- **Run temp** is a file in the temp folder that a parsed command of this run created, where that command ran successfully.
- **The two extra marks stay:** *not asked · Allowed command* / *· known-safe read-only*, and *denied by you*.

### Approval choices and waiting requests

Drawn and approved 2026-10-02; built 2026-10-02 on `claude/approval-choices` (`docs/interactive-tool-approvals.md`, "Answers and waiting requests"). Six frames in `lozqh`, in two new rows under the four above: row 5 at y 4956 and row 6 at y 6540, columns x 48 / 1568 / 3088, labels 32 above. The group grew from 4924 to 8193 high and the section `DMBmo` from 6884 to 10153; no frame moved. The dialog is the same for a command and for an MCP tool call, so it replaces the footers of the command dialogs above (`vyShL`, `k4cz1C`, `ZJt4d`), of ORC-41's *Approve tool action?* (`t0q3ws`, in `sJeSn`) and of 04.H9 `wvh5j` / `coKjY`; those frames were not redrawn. Fixture: Kontur; ORC-146 *Design the sync status panel*, Run 146 on Design review v2, 02 Design · Designer on Claude / Opus 5.5 (Designer v2) calling pencil `batch_design`; ORC-142, Run 142, Builder v3 with `cargo add serde --features derive` and a 15-line pwsh script. Every name, path, time and count is illustrative. Proposed copy namespaces: `runs.permissionRequest.*` (dialog, wider answers, Decide later, queue), `runs.activity.decision.*` (marks), `runs.permissions.similar.*` (the rules in Details) and `statusBar.approvals.*`.

**Why.** In a long Design step every pencil `batch_design` call stopped the whole window for a person, one call at a time, and the only way to stop asking (the server's *Tools that run without asking*) reached the agent only at its next invocation.

**The four choices** (user decision 2026-10-02):

| Choice | Command | MCP tool |
| --- | --- | --- |
| **Approve once** | this request only | this call only |
| **Approve similar in this run** | the same program and subcommand in the same category until the run ends, as in `k4cz1C` | this tool of this server, any arguments, until the run ends. It takes effect at once, for the current invocation too: Orchester answers each later call itself, one at a time. |
| **Always allow…** | a second step: the exact prefix added to the profile's Allowed commands in a new profile version (*Adds cargo add to Builder › Permissions › Allowed commands, published as Builder v4*), the version note of `ZJt4d`, and *A denied Network still wins*: where the profile, a step or the run sets the category to Deny, the listed prefix stays denied | a second step: the server and the tool (*Tool: pencil · batch_design*), where it is added (*Connections › MCP servers › pencil › Tools that run without asking*), every profile that gets the server, by name (*Designer, Builder · every profile that gets pencil; Reviewer doesn’t*), and when it starts. A connection setting, saved at once, not a profile version. |
| **Deny** | the command does not run; the agent is told and continues | the call does not run; the agent is told and continues |

Deny shows what the agent is told, word for word, as a line in the request (message-square-x 12 `$subtle`, text 11 `$muted`): *Deny tells Designer: “The person reviewing this run denied pencil batch_design. Continue without it.”*; for a command the existing *Denied by the person reviewing this run. Continue without it.* The step goes on. Today both CLIs' approvals fail the step on a person's Deny, for MCP calls too (`docs/interactive-tool-approvals.md`); the implementation must change that.

**Layout** (user decision 2026-10-02). The footer keeps two answers: *Deny* and the primary *Approve once*, which has the focus when the dialog opens (drawn with the focus ring). *Decide later* sits at the footer's left (clock-3 14, label 12/500 `$muted`, the key *Esc* in mono 10 `$subtle`). The two wider answers move out of the footer into a quiet row between the request and the footer, *Or answer more widely* (11 `$subtle`, padding 14 / 28 / 16 / 28, hairline above): two tiles side by side (1 `$hairline`, radius `$radius-control`, padding 10 / 12; icon 14 `$muted`, title 12/500 `$text`, what it covers 11/1.5 `$muted`): *Approve similar in this run* (copy-check; *pencil batch_design, every call until Run 146 ends. Starts now, for this invocation too.* / *cargo add commands in Network, until Run 142 ends. …*) and *Always allow…* (bookmark-plus; *Add batch_design to pencil’s tools that run without asking, for every profile that uses pencil.* / *Add cargo add to Builder’s Allowed commands, in a new profile version, for runs that start later.*). Hover or focus outlines a tile 1.5 `$text` on `$hover`, and on *Approve similar* opens the scope panel of `k4cz1C` under the request (for MCP: *pencil · batch_design · any arguments · until Run 146 ends · starts with this invocation*, *Runs without asking* / *Still asks you*: another tool of pencil, another server, another run). The panel is the request's last part, under what Deny tells and right above the tile it explains. While it is open the dialog keeps its bottom edge and grows upward, so the wider answers and the footer stay where they were; grown from its centre, the dialog moved the tile away from the pointer, which closed the panel again (seen 2026-10-06). A dialog that reaches the window's margin grows no more: the request scrolls to its end so the panel is in view, and back when the panel closes. Why this layout and not the others: a split button would hide the answer the user needs most behind a menu; a scope selector with one confirm button would change what Enter does, so a scope left on *Always* would approve more than meant. Here Enter and Esc always mean the same thing, the footer looks as calm as before, and each wider answer says what it covers before it is chosen. When a wider answer is unavailable (an Unknown command, several parts asking), both tiles stay in place at 0.4 with a lock icon and the reason in the tile, so the dialog does not change shape between requests.

**Keyboard.** Enter is Approve once; Esc is Decide later, never Deny. Tab follows the visual order (‹ ›, Copy, Approve similar, Always allow…, Decide later, Deny, Approve once), wraps, and stays in the dialog. In the Always allow step Enter is the primary (*Add to v4 and approve*, *Add to pencil and approve*) and Esc is *Back* (*proposed*).

**Waiting requests and Decide later.** Today the dialog blocks the whole window (`dismiss_on_escape(false)`) and *Needs you* shows 0 while it is open. Instead:

- *Decide later* (or Esc) closes the dialog and answers nothing. The request stays waiting; the agent keeps waiting with its active time stopped, as now. It stays in *Needs you* (Overview), on the task page (the Summary's top card *Designer waits for your answer · pencil batch_design* with *Review request*, and the run's step row *Waiting for you · pencil batch_design · since 14:06*) and in the status bar's activity slot (*3 approvals waiting · Designer asks to use pencil batch_design · ORC-146* with *Review*).
- **One at a time, oldest first** (user decision 2026-10-02, rather than a list in the dialog): the header shows *1 of 3 waiting* with ‹ › (26 square secondary buttons, the unavailable one 0.4). Each request shows its own agent, run and task. Answering one shows the next; the dialog closes when none is left.
- **Reopening.** *Review* in the status bar, *Review request* on the task page, a row in *Needs you* or the inline card on the Conversation tab; each opens the dialog on that request.
- **The counter counts waiting requests** whether the dialog is open or not: *Needs you 3* while it is open.
- After Decide later a new request does not open the dialog by itself; the status bar shows it and the count goes up. The dialog opens by itself only when nothing was deferred. A click on the backdrop does nothing (*proposed*), so nothing is deferred by accident.
- The inline card on the Conversation tab is not a dialog: it keeps its one-row footer (*Always allow…*, *Deny*, *Approve similar in this run*, *Approve once*) and simply stays until answered (*proposed*).
- **A request whose agent is gone** (built 2026-10-03, not drawn; bug fix). A request waits only while its agent can still receive the answer. When the agent's invocation ends, its run stops or fails, its runner is restarted, or the request was recorded without a live agent, the runner ends it without an answer itself (`tool_approvals.state = 'interrupted'`, the existing state; no migration): at the invocation's or the run's end, when the next runner attaches, and at the latest when someone answers it (the runner then refuses the answer, *no longer waits*). It no longer counts in *Needs you*, the status bar or the step row. An answer the runner refuses that way drops the request from the dialog at once: the next waiting request shows, with *The previous request ended without an answer: its agent is no longer waiting.* in the footer's message slot (where an answer's error goes) until the person acts in the dialog, or the dialog closes; the status bar's activity slot records *Request ended*, which opens its task. When the bar is narrow, the whole bar is one line that gives way in a fixed order (lurq `shrink_order`/`shrink_limit`, `status_bar::item::give_way`), so it never shows more at a narrower width: the room between the groups first; what the oldest waiting request asks trims its end down to 100px, then drops out; which run auto-approves and the times drop out whole; then the right group: the version, *Tokens today*, the counts' labels, the plan limits' resets and reasons; then the activity drops its time and trims its words; the words after the approval count drop out (*2 approvals waiting* reads *2*); last the runner's words (its icon keeps the state). The hand, the count, *Review*, *Auto-approving*, *Stop*, the counts, the plan limits and MCP never give way, and nothing is cut to a stub. This replaces the narrow-window breakpoints of R (`VTnmr`) below: the right group now gives way only when the line is full, in the same order. A second answer to a request answered already is refused as such (`ToolApprovalAnswered`): the request leaves the dialog without the notice, and the card reads *Answered*. The inline card reads *Ended without an answer* (circle-pause, the decided card's look) and loses its answers; a request that ended before an answer reads so too (it read *Interrupted*). Esc (*Decide later*) closes the dialog in every state; the backdrop still does nothing, as decided on 2026-10-02.

**Long values** (matches the transcript command box `uA3Sh` of `N4SlQq`, as in 02.I2 `iBzOj` and 02.I4 `tWO8E`). Action, Target, Command and Arguments stay a property row while the value fits on one line in the right column. Otherwise the value moves under its label as a full-width mono box: fill `$canvas`, 1 `$hairline`, radius `$radius-control`, padding 6 / 6 / 6 / 0; viewport padding 4 / 0 / 4 / 12; Geist Mono 11 / 1.6 `$text`; the value text uses the box's full width. *Copy* is not in the box (decided 2026-10-02): it sits on the label row (*Target*, *Arguments*, *Command* …), aligned right, as a ghost text button — Button / Secondary with no fill and no border, 22 high, padding 0 / 6, copy icon 13 and label 11 in `$muted`; on hover it fills `$hover` and icon and label turn `$text` (specimen beside the dialog in `bjrgu`). A value that stays beside its label has no *Copy*. No wrapping: the lines exactly as recorded, never shortened or joined. A line wider than the box scrolls sideways; while it does, the viewport's bottom padding is 10 and the 4-high `$border` thumb (radius 2) sits in that band under the last line. The box is capped at 12 lines (232 with its padding); more lines scroll vertically, with a 4-wide thumb at the box's right edge, over the text. Arguments are pretty-printed JSON with a two-space indent; a string stays one JSON string with its escapes, so a long `input` scrolls sideways. When the window is short, the request scrolls between the fixed header, wider answers and footer. *The design system has no value for the 12-line cap or for the 10 bottom padding; both are proposed.*

**Approved as similar in the run's Details.** The Details pane (`A9j8eD`) and the run popover (`BuFfP`) list MCP rules with the command rules under *Approved as similar in this run*: *pencil · batch_design* with an *MCP tool* chip (plug), *since 14:07 · approved 14 calls without asking you* and *Revoke*, next to a command rule (*cp … (outside the worktree)*, Outside write). Revoke applies from the next call, in the current invocation too; calls approved before keep their mark; Undo restores the rule until the run ends. An Always allow is not a run rule, so Details shows it below without Revoke: *batch_design added to pencil’s Tools that run without asking at 14:11, by you* with *Connections ↗* (for commands, the profile version).

**Marks** (*Approvals / Decision mark*, unchanged component):

| Mark | Icon | When |
| --- | --- | --- |
| *approved by similar rule · pencil batch_design* | copy-check | a rule from Approve similar answered the call; for a command the mark keeps the category (*· Network*) |
| *always allowed · pencil batch_design* | bookmark-check | Always allow answered it: the request itself, and later ones Orchester answers before the CLI knows the rule (MCP: the rest of the invocation; commands: the rest of the run, *always allowed · cargo add*) |
| *denied by you · the agent continued* | x | a person's Deny; outline, on a row that shows what the agent was told and never folds into a group |
| *asked you · pencil* | hand | waiting, also after Decide later |
| *approved by you · pencil* | user-check | Approve once |
| *not asked · exempt tool of pencil* | list-checks | a later invocation, run without asking by the CLI (built) |

| Frame | Node | Position |
| --- | --- | --- |
| MCP tool dialog over the task page: four choices, Deny's message, Decide later, *1 of 3 waiting*, focus on Approve once; the status bar shows the request and *Needs you 3* while the dialog is open (1440 × 988) | `kpuQj` | 48 / 4956 |
| Command dialog (`cargo add serde`, 2 of 3) with the same layout; the command Always allow step: the exact prefix, where it is added (Builder v4) and *A denied Network still wins* (compact) | `NV6xJ` | 1568 / 4956 |
| MCP Approve similar focused with its scope panel (starts now); the MCP Always allow step: server, tool, where, every profile it affects, starts now (compact) | `XKja4` | 3088 / 4956 |
| Long values: Target and Arguments as full-width mono boxes scrolling sideways; a 15-line command capped at 12 lines; an Unknown command with both wider answers unavailable (compact) | `bjrgu` | 48 / 6540 |
| After Decide later: the task page with *Review request*, the step waiting, the status bar with *3 approvals waiting · Review* and *Needs you 3* (1440 × 988) | `G0rzJy` | 1568 / 6540 |
| Three waiting requests in Needs you, the status bar while open and after Decide later; the new marks; Designer's pencil calls with them; MCP similar rules in Details with Revoke; Always allow without Revoke (compact) | `ZkOPf` | 3088 / 6540 |

Reused: `zo9Py`, `u1XBP`, `tfGRt`, `EfQuh`, `M7pU0A`, `N4SlQq`, `X9gPDr`, `pmOpt`, `DeizF`, `l3hyb`, `mBkeJ`, `SOW6S`, and copies of `vyShL`, `ZJt4d`, `k4cz1C`'s scope panel and `E2cL0U`. No new components or variables.

**Decisions (2026-10-02).** The user's:

- **The four choices**, for commands and MCP tools alike: Approve once, Approve similar in this run, Always allow, Deny.
- **Deny lets the agent continue**, now for MCP calls as for commands: the agent is told and the step goes on.
- **MCP Approve similar applies immediately**, for the current invocation too.

Also the user's (approved 2026-10-02, "я згоден з усіма дизайнами"): the layout (two answers in the footer, the wider answers as tiles in a row above it, Decide later at the left); one request at a time with *1 of 3*; no automatic reopening after Decide later and no deferral by a backdrop click; MCP Always allow also starting at once for the current invocation; Esc as Back in the Always allow step; the 12-line cap; the inline card keeping its one-row footer; the *always allowed* mark on later requests Orchester answers in the same invocation or run.

**Auto-approval (user decision 2026-10-02, after the frames; not drawn).** An auto-approve mode at two scopes, on top of the four answers:

- **Per run**: *Approve everything in this run*, a fifth answer apart from the two tiles (its own row, *Or stop asking for this run*), and a switch in the run's "This run" panel. From then on Orchester answers the run's requests itself until the run ends or a person turns it off.
- **Per profile**: *Approve automatically (don't ask me)* under the profile's Permissions (*When the agent asks*), part of the profile version like every other setting; every run that starts with that version auto-approves until it stops it.
- **MCP tool calls stay auto-approved** (user decision 2026-10-03): under auto-approval every MCP tool call is approved, since a person chose the servers and their tools for the profile; only commands are classified (`run::auto_verdict`).
- **Never approved automatically**, as without it: a category set to Deny (no category policy is built yet, so none can be); dangerous requests, which still go to *Needs you*: a write, move or delete outside the worktree, its writable folders and the run's temporary files (Run temp is not told apart yet, so every temporary file counts as outside); a read of a credential location (the home folder's dot files and folders such as `~/.claude.json`, `~/.codex`, `~/.ssh`, everything under `AppData` and `Library` but the temporary folder (CLI tokens, the Windows credential folders, the macOS keychains, browser and editor profiles), credential store programs, the vault's secrets); installing software or packages (package-manager install and add commands, `npx`-style runners, global installs). History / destructive and Unknown commands also ask, as their category's Allow is unavailable and what cannot be classified safely asks; in a run with auto mode (decided 2026-10-06), an Unknown line goes to auto mode's reviewer instead, unless the step's network is off or it only reads, while any part that reads a credential, rewrites history, deletes or writes outside the worktree, or pushes anything but the run's branch still goes to the person (`run::review::review_hold`). Everything else is approved: commands that read or stay inside the worktree, and MCP tool calls; a network command asks (decision of 2026-10-03, below). The classification lives in core next to the command categories (`permission::category`, `run::auto_verdict`).
- **Network asks; the classifier fails closed** (user decisions 2026-10-03, after the security review of the classifier: "Deny + dangerous must ask"). Auto-approval no longer approves a command that reaches the network (`curl`, `git fetch`, `gh pr view`, `ssh`): it asks like the dangerous ones; *Approve similar* and *Always allow* stay available for it as a person's explicit choice. Auto-approval, *Approve similar* and *Always allow* cover only what the classifier positively understands: an unknown program or subcommand, an option not listed for its program (one that names a file, starts a program or reads a configuration: `git fetch --upload-pack`, `ssh -o ProxyCommand`, `curl -K`, `rg --pre`, `tree -o`), a variable set before a command (`PATH=`, `LD_PRELOAD=`, `GIT_…=`, `BASH_ENV=`), `$'…'` quoting, `git config` that writes, `git -c`, `git remote add` / `set-url` and every `gh` command other than the reads, pull request and issue creation and comments, merges and deletes it lists are Unknown and ask. *Approve similar* never covers a part that rewrites history, deletes outside the worktree, reads a credential or is Unknown, nor one that reaches outside the worktree in another folder than the approved command did; its rule names `gh` by command and action (`gh pr view`), and `ssh`, `scp` and `rsync`, whose arguments run or copy on another machine, make no rule. *Always allow* offers a prefix only where every command it allows stays like the approved one: builds, tests and installs, which run the worktree's or the packages' code whatever their arguments, and the git subcommands that stay in the repository; for anything else its tile says the prefix would also allow riskier forms. A command is classified in the working folder its request reports (Codex), else in the worktree.
- **Visibility**: while a run auto-approves, *Auto-approving* under the task heading and in the status bar's left group (*Auto-approving · Run 142 · ORC-142*, or how many runs), each with a one-click *Stop*; each answer is marked *auto-approved · <category>* (or the MCP tool); the run's "This run" panel counts the answers by who gave them (by you, by similar rule, always allowed, auto-approved, denied by you, waiting).

States built without frames, for a designer and a verifier: the dialog's *Approve everything in this run* tile and its row heading; the run panel's *Approvals* section (the switch with its note, the six counts, *Approved as similar in this run* with Revoke or *revoked at …*, *Always allowed during this run* without Revoke); the *Auto-approving* line under the task heading with Stop; the status bar's *Auto-approving* item with Stop; the profile's *When the agent asks* switch with its note; the Summary's *Review request* card (drawn in `G0rzJy`, built as an emphasized inline notice).

**Subject line and reasons (built 2026-10-03).** The request and the Always allow step open with the line of `Cv2MG`: the category chip (*Credential read* with a key for a command that reads a credential location, wherever a pending request's category shows, and in its decision mark), or the MCP server's chip; what made it ask; and *waiting since*. No category policy is built, so *what made it ask* is the asking profile's real setting at the version the run pinned (*Asks before commands*, *Not an allowed command*, *Asks to leave the sandbox*, *Asks before MCP actions* · *Builder v3*), never the drawn *Ask me*. The classifier says why a command asks (`CommandReason`), which labels the command box (*Command · installs a package from the network: crates.io · serde*) and is the reason line of the command's Always allow step. The explanation under the title says what the policy asks about, by category (`xNcLS`, `V8j3QO`); a read-only step keeps the warning that an approved command can change files. *A denied Network still wins* (`NV6xJ`) is not built: no category can be set to Deny yet (deviation accepted by the user 2026-10-03; it comes with category Deny). While the run already auto-approves, *Approve everything in this run* is shown on and unavailable, saying why this request still waits: auto-approval leaves it to a person, or it came before auto-approval was on (auto-approval answers a request when it arrives, not the ones already waiting). A step waiting on a request says so in the run's progress (*Waiting for you · pencil batch_design · since 14:06*), and the *Review request* card counts the requests waiting in other runs.

## MCP servers

Implemented 2026-09-28. Claude Code agents run with the user's own CLI configuration left out (`--strict-mcp-config --setting-sources ""`), so the only MCP servers they get are the project's own. Codex agents run as App Server (since 2026-10-01), which loads the user's configuration; Orchester turns the user's own servers off for the thread by name, with plugins, apps and `notify`, and never loads the repository's own `.codex` folder (`docs/interactive-tool-approvals.md`). The user's own execpolicy rules still apply, as the user's choice. An MCP server is a project connection (the *MCP servers* tab of 04.01, the *Project knowledge* row `enoFW` and its details `pQ0DW` in 04.A `RZUKo`); Orchester configures it, the agent's CLI starts or reaches it with each agent and ends it with the agent. For CLI agents Orchester never talks to a server itself; for native agents (decided 2026-10-01, `docs/architecture/native-agent.md`, phase P4) the runner is the MCP client and starts or reaches the server itself, with the same configuration, secrets and approvals.

**Model (`orchester_core::connection::mcp`).** A server has a name, a transport, variables and an enabled flag. The name is the key both CLIs know it by (`mcp_servers.<name>` in Codex, `mcp__<name>__<tool>` in Claude Code): small letters, digits, `-` and `_`, starting with a letter, at most 48 characters, unique among the project's servers. The transport is a program the CLI starts over stdio (a program found on the `PATH` or in the worktree, or an absolute path; its arguments as they are; optionally the folder it starts in, inside the worktree or absolute) or a streamable HTTP URL (`http://` or `https://`, no user name or password in it) with an optional bearer token from a project secret. A program's variables are plain text or the value of a project secret, named by the secret's id; an HTTP server has none. An agent profile chooses its servers (none by default; profile schema 3); a disabled server is given to no agent. Import does not map a profile's servers yet: a copied profile keeps naming the source project's servers, which its agents then work without (see below).

**Invocation.** Checked 2026-09-28 against Codex CLI 0.139.0 and Claude Code 2.1.89 on Windows 11 without a model turn (neither CLI was signed in for the check: an isolated `CODEX_HOME` / `CLAUDE_CONFIG_DIR`), so what happens when an agent calls a server's tool is not verified.

| | Codex (`codex app-server`) | Claude Code (`claude -p`) |
| --- | --- | --- |
| Servers | `-c mcp_servers.<name>.command="<program>"`, `.args=[…]`, `.cwd="<folder>"`, `.env.<VAR>="<text>"`; `-c mcp_servers.<name>.url="<url>"` (accepted with `--ignore-user-config`; `codex mcp list --json` shows them) | `--mcp-config <file>` with `{"mcpServers":{"<name>":{"type":"stdio","command","args","env"}\|{"type":"http","url","headers"}}}` next to `--strict-mcp-config` |
| Secret values | set on the Codex process under the variable's name and handed over with `mcp_servers.<name>.env_vars=[…]` (a server starts with a few system variables, its `env` and its `env_vars` only); a bearer token the same way through `bearer_token_env_var="ORCHESTER_MCP_TOKEN_<NAME>"`; `-c shell_environment_policy.exclude=[…]` keeps them out of the agent's commands (not observed with a model turn) | written into the file only; the file is created for the invocation in the user's temporary folder (`<temp>/orchester-mcp/<project id>/<invocation id>.json`: `0600` in a `0700` folder on macOS, the profile's own access list on Windows) and removed when the CLI has ended; a runner that ended abnormally leaves files that the next runner of the project removes |
| Folder | `cwd` honoured | ignored: every server starts in the worktree, so a server set to start elsewhere is left out |
| Tool calls | a call that asks for approval reaches you with *Ask before MCP actions*, and is declined otherwise (approval policy `mcp_elicitations`); a tool the server lists to run without asking gets `mcp_servers.<name>.tools.<tool>.approval_mode="approve"` as an override of the Codex process while the step asks | allowed only for the chosen servers with `--allowedTools mcp__<name>` (print mode denies what no rule allows); with *Ask before MCP actions* no `mcp__<name>` rule, only `mcp__<name>__<tool>` for each tool the server lists to run without asking |
| A server that did not start | `mcpServerStatus/list` names it with its `runtimeStatus`; the agent works without it and its activity says so, unless the step asks before MCP actions, which then does not start | `system/init` lists each server with `status` `connected` or `failed` |

Secret values never appear in arguments, logs, prompts, `Debug` output, events or the MCP tree: they are read from the keychain when the agent starts (once per secret), held as redacted secret values, and passed only through the process environment (Codex) or the private file (Claude Code). A Windows `.cmd` launcher of npm is started through Node.js, or, when it starts a program file, as that program; another command script through `cmd.exe /d /c` when its arguments pass through `cmd.exe` unchanged.

**Servers the agent works without.** Before the CLI starts, the runner leaves out a server it cannot give as configured, and the invocation's activity says why (*Working without MCP server docs*: the profile names a server the project no longer has, the server is turned off, its program was not found, Claude Code cannot start it in its folder, a secret is gone, archived or not in the keychain, or — Codex — two servers take one variable from different secrets or a secret variable is one the CLI needs). A server the CLI reports as not started is listed the same way. The agent works on without it; the step does not fail.

**Enforcement.** Both CLIs run only the chosen servers. Claude Code allows only their tools; with Codex App Server a tool call that asks for approval reaches a person only with *Ask before MCP actions* (otherwise it is declined). Neither CLI's permissions reach inside a server: a server is a program (or service) of its own that works outside the worktree's limits and reaches the network whatever the Network setting says. The profile's enforcement panel says so (below).

**Tools that run without asking** (built 2026-10-02, not drawn; `docs/interactive-tool-approvals.md`). A server can list tools, one name per line in its dialog, that run without asking when a profile asks before MCP actions; every other tool of the server still asks, and a profile that does not ask is unchanged. Claude Code gets an exact `mcp__<name>__<tool>` allow rule per tool, Codex the tool's `approval_mode = "approve"` as an override of its process. A tool whose Claude Code name another chosen server could also have (`__` or a trailing `_` in a server name) gets no rule and asks, and the activity says why. Such a call is marked *not asked · exempt tool of <server>* in the activity. Every Codex MCP call has an activity row of its own (*approved by you · <server>*, *asked you · <server>*, or *declined, not asked* when a step that does not ask gets an approval request, which is declined without a person). The Connections inspector lists the tools after the variables (or *None*), the profile's enforcement panel adds *pencil: 6 tools run without asking* with the CLI's rule while the profile asks, and the pipeline step inspector repeats that line under *Ask when a tool needs approval*. It fits the [command categories](#command-categories-and-approval-policy) as an MCP counterpart of their *Read-only* category: the list is per server, chosen by a person, and each call is still one call, never a session grant. When MCP actions get a policy of their own, the list can become that policy's *Allow* for those tools without changing what is stored. *Always allow…* in the approval dialog adds a tool to this list, and *Approve similar in this run* answers a tool's later calls at once, in the current invocation too ([Approval choices and waiting requests](#approval-choices-and-waiting-requests), drawn 2026-10-02).

**Screens (implemented; drawn 2026-09-28 in `m3wObx`, "States · MCP servers" in section 04).** Server names, programs, hosts, secrets and profiles are fixture data.

- **Connections › MCP servers as built (04.D `QdLRd`)**, the *MCP servers* tab of 04.01 selected. Rows (`sFtAz`, plug, type *MCP*): `zAFzk` *project-knowledge* · *Remote MCP · mcp.example.dev* · *2 agents* · *On* (circle-check); `zh2zU` *docs* · *Program · npx* · *2 agents* · *On*, selected (`$raised`); `YHRmd` *issue-tracker* · *Program · uvx* · *0 agents* · *Turned off* (circle-minus). The provider notice is not on this tab; the ownership line stays at the bottom. The details (`OXa0o`, laid out like `JuVaA`: padding 24 / 12 / 24 / 24, a details viewport with the scroll gutter, thumb `i2XiRp`, and pinned actions) read: *docs* (18/500), *On*, then **Type** *Program on this computer*, **Program** `npx`, **Arguments** `-y @kontur/docs-mcp --root docs` (Geist Mono 10), **Starts in** *The worktree*, **Started by** *The agent’s CLI, with each agent*; *Variables* (13/500) with `DOCS_INDEX` `.orchester/docs-index` and `DOCS_API_KEY` *From DOCS_API_KEY (Staging)* (a secret by its name and environment only); *Discovered tools* with *Orchester does not connect to MCP servers itself, so it cannot list their tools yet. The agent’s CLI lists them when an agent starts.* (10 `$muted`) and *Refresh tools* unavailable (0.4); *Used by* with *Builder ↗* and *Reviewer ↗*. Pinned: *Test connection* unavailable (0.4), *Edit connection*, *Turn off* (power), and the detail *The agent’s CLI starts or reaches this server when an agent that gets it starts, and ends it with the agent. A server that does not start is listed in the run’s activity.* A remote server shows **Endpoint** (the host) and **Authentication** (*Bearer token from KNOWLEDGE_TOKEN (Staging)* or *None*) instead of program, arguments, folder and variables. *Last checked* and the expired-access state of 04.A `RZUKo` (a proposal for OAuth, not built; its row `enoFW` and details `pQ0DW` are superseded by 04.D) are not shown: nothing checks a server.
- **Add MCP server / Program (04.E `yRb2k`)**, the connection dialog frame (`IAN4P`: 800 × 832, header 82, actions 76, padding 28, fields 22 apart, label and control 8 apart) over 04.D. Its fields scroll in a viewport with the scroll gutter (form padding 28 / 16 / 28 / 28 beside the 12px gutter, thumb `GaG21`). In order: **Name** (`docs`, Inter 12) with the hint *Agents know the server and its tools by this name.* (10/1.5 `$muted`) | **Connection type** select *MCP server*; **Type** select *Program on this computer* | **Started by** fixed *The agent’s CLI, with each agent*; **Program** (`npx`, Geist Mono 12) | **Starts in** (placeholder *The worktree* in mono `$muted`) with *A folder in the worktree, or an absolute path. Claude Code starts every server in the worktree.*; **Arguments**, a mono area (`$sidebar`, `$border`, radius 7, padding 12, 12/1.6, 3 to 8 lines) with one argument per line and *One argument per line, passed as written.*; **Variables**, the same area (`DOCS_INDEX=.orchester/docs-index`, `DOCS_API_KEY=${secret:DOCS_API_KEY@Staging}`) with the hint *One NAME=value per line. ${secret:NAME@environment} takes the value of a project secret.* and, at its right, *Add a variable from a secret…* (key-round, secondary, 30 high; opens the project's secrets as *NAME (environment)* in the select menu and appends a `NAME=${secret:NAME@environment}` line; without secrets it is unavailable and reads *This project has no secrets*); the notice (shield-check) *Secret values stay out of arguments and logs* · *They are read from the system keychain when an agent starts and handed to its CLI only through its environment (Codex) or a file only it reads, removed when the agent ends (Claude Code).*; *Choose which agent profiles get this server on their Permissions tab.* (12/1.6 `$muted`). Actions *Cancel* and *Save connection*.
- **Add MCP server / Remote MCP (04.E2 `Qvhc6`)**: *project-knowledge*; **Type** *Remote MCP* | **Reached by** *The agent’s CLI, over HTTP*; **Endpoint** (`https://mcp.example.dev/mcp`, mono); **Authentication** select (*None*, or *Bearer token from NAME (environment)* for each project secret; fixture *Bearer token from KNOWLEDGE_TOKEN (Staging)*); the secrets notice and the next step. It fits, so the gutter shows no thumb.
- **Edit MCP server / Values it cannot save (04.E3 `Hpycl`)**: the connection type is fixed (no chevron). A field whose value cannot be used strokes 1 `$subtle` and has the field error under it (circle-alert 12 and Inter 11, `$text`, 4 apart) before its hint: name *Docs* → *Use small letters, digits, - and _, starting with a letter*; starts in `../shared/docs` → *Use a folder inside the worktree, or an absolute path*; a variable naming `DOCS_KEY@Staging` → *This project has no secret DOCS_KEY@Staging*. A text field shows its error only once something is entered; an empty required field says why only in the action bar. The action bar says the first reason in field order at its left (circle-alert 12 and Inter 11, `$muted`) and *Save connection* is unavailable (0.4). The other errors read *Enter a name*, *A server name is at most 48 characters long*, *Another connection of this project already has this name.*, *Enter the server’s URL*, *The URL starts with http:// or https:// and names a host*, *Keep user names and passwords out of the URL; use a bearer token from a secret*, *Remove the spaces and line breaks*, *This is too long*, the program's errors of the tool form, and for variables *Write each variable as NAME=value: …*, *… is not a variable name: …*, *… is set more than once.* and *A variable cannot be used as written*.
- **Agent profile › Permissions (04.F `VI4c6`, compact)**, after Network on 04.04 `T1rjv`, beside the enforcement panel (`kXyN7`, 300 wide). *MCP servers* (13/500), then one row per project server (10 apart): the switch (`K8ItM`; on: `$accent` track, `$inverse-text` knob at the right), the name (12 `$text`) and where it runs (11 `$muted`), 12 apart; under a server an agent would work without, the reason indented 22 with an 11px lock (10/1.5 `$subtle`): *Turned off in Connections: agents work without it until it is turned on.* or *Claude Code can’t start it in tools/docs-mcp: agents on this connection work without it.*; then the hint *Agents of this profile get the servers that are on. Their tool calls are not limited by the settings above.* (10/1.5 `$subtle`). Without servers: *This project has no MCP servers yet.* and *Add an MCP server* (plus, 36 high), which opens this tab of Connections. The panel adds, after the three settings: Codex with a chosen server *Only the chosen MCP servers run* `Codex App Server uses your normal account. Your own MCP servers, plugins and notify program are turned off.`, *MCP approval requests are denied* (triangle-alert) `not asked · the call fails` (or, asking before MCP actions, *Ask before MCP actions that require approval* `One requested action only; no session or permanent grant`), *Servers work outside these limits* (unplug) `own processes · network on`; Claude Code (Explorer, one usable) *Only the chosen MCP server loads* `--strict-mcp-config --mcp-config`, *Only their tools are allowed* `--allowedTools mcp__* · unverified` and the same last row; chosen servers none of which can load (Reviewer) *No MCP servers load* `none can start here · user’s own ignored` only; none chosen (Planner, and Codex's Builder) *No MCP servers* `none chosen · user’s own ignored` only, for either CLI.
- **Run permissions popover (02.J `NNRH8`, group `mKDBS`)**: after Network, *MCP servers* (11 `$muted`), the servers the agent gets from its profile version as a comma list (`project-knowledge, docs`, Geist Mono 11 `$text`, or *None*), and *From the profile version; they are not changed for a run.* (10 `$muted`), 8 apart. The popover grew 72 and moved up by as much, so it stays anchored above the permissions row.
- **Activity (04.G `a2PR7K`, compact, for 02.I)**: at the start of an invocation, one transcript entry (`N4SlQq`) per server the agent works without, glyph unplug, no meta: title *Working without MCP server issue-tracker* (or *Working without an MCP server* when the CLI did not name it), the reason as the body, and the CLI's own words, when it gave any, in the output block (*MCP startup failed: program not found*). The board lists every reason as the body reads it.

**Aligned with the drawings (2026-09-28):** *Add a variable from a secret…* is the 30px secondary button at the right of the Variables hint (unavailable, reading *This project has no secrets*, when the project has none). Its menu is 260px wide (`u0ByM`), wider than the button, and shows each secret as *NAME (environment)* on one line. A remote server's fixed field reads **Reached by**; the connection dialogs' fields scroll beside the scroll gutter with form padding 28 / 16 / 28 / 28 (all three connection dialogs share the frame); activity entries for a server the agent works without use the unplug glyph. Stored secret references use canonical lowercase environment slugs (for example `@staging`); `@Staging` in the drawing is illustrative.

- **Remote server selected (04.D2 `a4lm6D`, drawn 2026-09-29 for M5)**: 04.D with *project-knowledge* selected. **Type** *Remote MCP*, **Reached by** *The agent’s CLI, over HTTP*, **Endpoint** *mcp.example.dev*, **Authentication** *Bearer token from KNOWLEDGE_TOKEN (Staging)*; no variables. Property text sits at the top of each 20px row. A value too long for one line wraps under itself, right-aligned (the app ends it with an ellipsis today, which hides which secret it is). The details overflow, so the gutter shows its thumb and *Used by* scrolls into view.

Not built: discovering a server's tools, OAuth sign-in to a remote server (Codex `codex mcp login`), servers per pipeline step (steps inherit the profile's), changing a run's servers, and MCP tools in the Tools library (06.01). The Secrets screen's *Used by* lists MCP servers (M3, see [Secrets and access](#secrets-and-access)).

## Native agent connections

Drawn 2026-10-01; 04.H1–H6 and H8–H11 built in P1e (ORC-60), with Test connection and Fetch models unavailable and DeepSeek, Custom and OpenAI API locked; H9–H11 render from recorded runs, which native agents make from P1d on. Not built: H7; the transcript rows of H10 (file tools, *shell · pwsh*, the approval and denial entries) moved to P1d with the event data they need (user decision of 2026-10-02), and H10's *Effort* row waits for the invocation to record it (P1d); the panel's endpoint is the connection's as it is now (*Endpoint now*). Group `PNDQm` ("States · Native agent connections", last in 04 / Agents & connections at y 9452, height 4468). Behaviour, presets, permissions and the decisions are in [native-agent.md](../architecture/native-agent.md) (§3.3, §5, §7); this section covers the screens only. Fixture: Kontur; *Claude API · Kontur* (Official), *DeepSeek · Kontur* (DeepSeek preset), *Lab gateway · Kontur* (Custom), the model ids, limits, balances, hosts and secret names are illustrative. Proposed copy namespaces: `connections.native.*`, `connections.models.*`, `connections.dataNotice.*`, `agents.permissions.enforcement.*`, `runs.activity.*`.

- **Add connection** (the 800 × 832 connection dialog of 04.E, its form scrolling beside the gutter). *Kind* is four option cards (`lcwsy`): *Codex CLI*, *Claude Code CLI*, *Anthropic API (native)*, *OpenAI API (native)*. The native form: *Preset* radios with the host (*Official*, *DeepSeek*, *Custom*; on Anthropic API the DeepSeek preset is unavailable with its lock reason, because the preset uses OpenAI Responses and DeepSeek's Anthropic route is reachable only through Custom); *Base URL* shown in full, read-only until *Advanced* is opened, then an editable mono field with the https/localhost rule as its hint and the wire format beside it; *API key · project secret*: a select of Kontur's secrets plus *New secret…*, which turns the field into a name and a masked value saved to Secrets with the connection, never shown again; *Test again* with the result line (DeepSeek adds the balance); *Models* as a table (model, source *From the endpoint · time* or *Added by hand*, context, max output, efforts) with *Fetch models* and *Add a model by hand*; *Default model* and *Default effort* (the model's own efforts); *Provider fallback* as a switch, off, *Retry declined requests on another model* (Official Anthropic only; Custom says it is not offered). DeepSeek adds the data-location notice (`q5LzI`, globe) under the preset. Errors follow 04.E3: stroke `$subtle`, the field error, the first reason at the left of the actions and *Save connection* at 0.4; besides the URL rule they read *Kontur already has a secret NAME*, *Keep user names and passwords out of the URL*, *Remove the query or fragment*, *Choose or create the API key's secret* and *Add at least one model* (action bar only). Frames: 04.H1 `Byo8K` (Official, top of the form), 04.H1B `h2OUr` (the same, scrolled to the end), 04.H2 `rkeoO` (OpenAI API, DeepSeek preset, Advanced open, key created in place), 04.H3 `KGciR` (Custom, values it cannot save; the test and *Fetch models* unavailable until the URL is valid).
- **Native connection inspector** (`JuVaA` variant). 04.H4 `lJG42`: *DeepSeek · Kontur* selected in the list (type *Native*, cpu glyph). Rows **Provider** *OpenAI API (native)*, **Preset**, **Endpoint** (mono, in full), the data lines (*Prompts, repository content and command output are sent to api.deepseek.com.*, *Data is stored in the PRC, per DeepSeek’s privacy policy.* and who acknowledged it when), **Authentication** *API key · DEEPSEEK_API_KEY*, **Execution** *This computer (Orchester runs the agent)*, **Model traffic** *Orchester’s own HTTPS*, **Balance**, **Last checked**, then *Models* (id, default label, source, limits and efforts) and *Used by*. Statuses in the compact board 04.H5 `dtxII` (1161 high, so row 3 starts at y 2400): *Connected* (Official, with three fetched models), *Key missing* (archived secret; *Choose a secret*, *Open Secrets*), *Key rejected* (401; *Replace in Secrets*), *Billing problem* (402, *the balance is empty*, balance 0.00), *Endpoint unreachable* (Custom, timed out; a model *Added by hand*).
- **Models added by hand** — 04.H6 `zHowy`, a 720 dialog *Models · Lab gateway · Kontur* over Connections: one row per model (id, default label, source and limits, *Edit*, *Remove*), the row being edited opens in place (*Model id*, *Context window · tokens · optional*, *Max output · tokens · optional*, effort chips *low medium high xhigh max*, *Remove model*, *Cancel*, *Save model*), and removing a model a profile uses asks inline (*Planner uses lab-mini-8b. Removing it leaves Planner without a model…*, *Keep* / *Remove*); *Add a model*; *Save models*.
- **Data-location acknowledgement** — 04.H7 `M2ctRR`, a 600 dialog over the task when *Run again* would start the first run on a flagged preset in the project: *Send Kontur’s work to DeepSeek?*, three consequences (what is sent and to which host; stored in the PRC and can’t be recalled; asked once for Kontur, recorded with name and time, other projects ask for themselves), the connection, endpoint and profiles, *Cancel* (the run doesn't start, nothing is sent) and primary *Acknowledge*.
- **Enforced by Orchester** — 04.H8 `yh0Xe`, the profile's Permissions tab for Builder on DeepSeek · Kontur. *Ask before running commands* is available; *Allowed commands* reads *Listed and known-safe read-only commands run. Every other command asks you first.*; *Any command in the worktree* at 0.5 with the lock reason; Network locked on (0.45) with the reason. The panel lists the §5.4 mechanisms (*file tools · worktree only, .git protected*; *listed and known-safe commands run unconfined · others ask you*; locked *Any command · not enforceable*; locked *Network stays on · off is not enforceable*; *MCP servers are left out*), the note that a listed command is chosen, not confined, and the no-bypass and steps-narrow lines. The summary reads *Network stays on*.
- **Approval of a shell command** — 04.H9 `wvh5j`, the `GbNSA` dialog (640) for `run_command` with `shell: true`: the shell row (*pwsh · -NoProfile -NonInteractive -Command*; *Windows PowerShell 5.1 · powershell.exe …* when pwsh is not found, *zsh · /bin/zsh -c* on macOS), the full string in a mono block, never shortened, the working directory and time limit, the notice *Runs outside any sandbox*, and that Deny lets the agent continue.
- **Activity** — 04.H10 `IcGvI`. Native tool calls reuse the transcript entry `N4SlQq`; file tools get their own glyph and the tool name first in the meta (*read_file · lines 1–140 · 4.8 KB*, *search · src/** · 9 matches in 4 files*, *edit_file · M · +38 −6 · open in Changes*); `run_command` rows are the CLI command rows unchanged, with *shell · pwsh* in the meta when it used a shell. An approval is an entry (*Approved by you · …*), and a denial is *Denied by a person · the agent continues* (circle-x) with the shell string as its body and, in the output block, the result the agent was given; the agent's next message and command follow (decision 2). The invocation panel names the endpoint, effort, *Network · Stays on · off not enforceable* and the tool calls.
  - *A CLI's calls of Orchester's file tools (decided 2026-10-07).* Claude Code's `mcp__Orchester__edit_file` and Codex's `edit_file` of the server `Orchester` (and `write_file`, `read_file`, `list_files`, `search`) read as the native rows above, from what the call's input asked: the target as the title (a search's pattern in quotes), the tool first in the meta (*edit_file · M · +3 −1*; *edit_file · M · replaces every occurrence* for an edit with `replace_all`, whose count the input does not say; *write_file · 12 lines written*, *1 line written*, *empty file written*, which has nothing to open; *search · src* when a search names a path or glob; *read_file*: the CLI's answer does not say lines or matches). Unlike the native edit row, which opens Changes, such an edit row opens (chevron) to the change of that one call: the output block's frame (`bjcu3`) holding each edit's lines on the diff backgrounds with their mark (`vEZ42` colors: `code-bg`, `diff-add-*`, `diff-remove-*`; Geist Mono 12, a line 24 high, a longer one wrapping within the row), the lines the edit's `old` and `new` share at their start and end as context (compared without their line breaks), a hairline between edits, a caption on an edit that replaces every occurrence (*Replaces every occurrence; the lines show one*) or changes CRLF and LF in the lines it keeps (*Line endings change in the lines it keeps*) or the final line break (*Adds the final line break*, *Removes the final line break*), each such line counted as removed and added, as git counts it, a write's content as added lines, no line numbers (the lines are the call's text, not the file's); its first 200 lines, with *1 more line of the change is not shown* / *N more lines of the change are not shown* above, a line over 1,000 characters cut with *Line shortened*; registered secrets are redacted in the whole texts before the lines are cut. Up to those 200 lines of what an agent wrote are stored with the transcript, as written apart from registered secrets (`docs/architecture/storage.md`). The rows start closed, like commands. A failed, denied or refused call stays the command row it was (its name and path, the error in its output) and shows no change, and one still running when its turn ended shows *edit_file · running* and nothing to open; the change the call reports after its end (first-edit tracking) never shows as a row of its own. No frame draws the open change; it reuses the output block and diff line parts as written here.
- **Run summary note** — 04.H11 `ZvWcK`: under the readiness card, the notice *What Orchester could not enforce for Builder* with *Network stayed on* and *Listed commands were not confined* (who ran unconfined, file tools stayed in the worktree) and *Builder’s permissions ↗*, the native counterpart of the CLI line *Network stays on*.

Reused: `zo9Py`, `TSJpn`, `u1XBP`, `ZGwlv`, `lcwsy`, `DeizF`, `tfGRt`, `q5LzI`, `K8ItM`, `bpmzN`, `sFtAz`, `jNTVN`, `SOW6S`, `mDnc3`, `N4SlQq`, `l3hyb`, `i9rVFu`, `mBkeJ`, the connection dialog of 04.E `yRb2k`, the approval dialog `GbNSA` over the task page of `t0q3ws`, the permissions page of `bYDCI`, the Activity page of `HW9mB` and the task Summary of `rS8mx`. No new components or variables.

**Decisions (2026-10-01).** The user approved the frames and answered their open questions. Where a frame differs, these decisions win; the frames were not redrawn.

- **Key secret with its environment.** A secret used as a connection key shows its environment when it has one, as elsewhere: *API key · DOCS_API_KEY (Staging)*, in the key select, the inspector's *Authentication* row and the status board. The frames show the name only.
- **Preset-filled limits.** Context, max output and efforts that the preset fills read *From the preset*, not *From the endpoint*; *From the endpoint* is only for values the endpoint reported. The DeepSeek rows in 04.H2 and 04.H4 should read *From the preset*.
- **Removing a model a profile uses is blocked.** The dialog says which profiles use it and that they must switch to another model first; it has no *Remove* for that model. 04.H6 draws a warning with *Keep* / *Remove* instead. Confirmed on 2026-10-02: the block covers every model the list has that a published profile version, a draft, a profile version an unfinished run pinned, or a step of a published pipeline or its draft uses; a model the list never had blocks nothing. Changing the id of such a model is refused under its *Model id* field. An engine is named after whoever receives the prompts: the provider on its official API, the preset otherwise (*DeepSeek*), a custom endpoint's host.
- **Acknowledge starts the run.** The acknowledgement is asked once per project and preset: a second DeepSeek connection in Kontur does not ask again, another project does. 04.H7's third consequence should say so.
- **Repository agent files.** Native agents load repository agent files (AGENTS.md, skills) by the same project setting as the CLIs (Project settings › General); the enforcement panel shows that row as for the CLIs. 04.H8 leaves it out.
- **Line counts.** *+38 −6* on native file edits stays, although CLI file rows have no counts.
- **The shell string is never shortened.** In the approval dialog the full string is always shown; a long one scrolls inside its mono block, with nothing hidden behind *Show all*.

## All projects

Drawn 2026-09-28 (`wa6B7`) and implemented. **All projects** (01.06 `Z7IZZ`, route `/projects`)
lists the projects the application database knows, most recently opened first. It sits outside
any project, like first launch: the welcome topbar, no sidebar, and a status bar without the
project items. The sidebar's *All projects* item and the switcher open it; the app starts on it
when the most recently used project cannot be opened (a first launch without projects still opens
01.01).

- **Rows** (`tBszO`): initial tile, name, folder (mono), status, *Active runs*, *Needs you*, *Last
  opened*, the × that removes it from the list and, when it opens, the chevron. Selecting the row
  opens the project's Overview as a new history entry. The list, folders, folder checks and runner
  states are known at once; the counts are read from each project's database without migrating
  it, and show a dash until read (01.06 E `t8TEOZ`). Active runs counts runs with an agent at
  work, only while the project's runner is online; *Needs you* counts runs waiting for a person.
- **Status**: *Runner not started* (no runner in this session), *Runner starting*, *Runner
  online*, *Reconnecting*, *Runner stopping*, *Runner stopped*, *Runner offline*; *Folder missing*
  (folder-x) with *The folder is not there any more…* under the path; *Can't be opened*
  (circle-alert) with the reason (e.g. data from a newer version). Neither of the last two opens.
- **Open project folder…** opens the project a folder already keeps (e.g. one removed from the
  list), and registers it again. A failure is the error line `tmE5H` (01.06 D `ucI7v`) naming the
  folder; it stays until the next attempt. **Add project** opens setup (01.02); its *Back* returns
  to where setup was opened.
- **Remove from list** (01.06 A `EtMXj`): the application database forgets the project; its folder
  and `.orchester` data stay on disk, as the dialog says. Enter confirms, Escape or *Cancel*
  closes. A removed project's history entries lead to All projects. While agents are at work in
  the project (01.06 F `glurU`, drawn 2026-09-28) *Remove from list* is unavailable (0.4) and the
  action bar says *Agents are at work in Kontur. Remove it once its runs are paused or finished.*,
  wrapping at its left beside circle-alert (Inter 11/1.5 `$text`, the icon in a 16.5 slot with
  the first line, as every wrapping field error); when the
  project's runner runs without agent work, removing it stops that runner at checkpoints.
- **Empty** (01.06 C `A4FJG`): *No projects in the list* under the same heading and actions.
- **Switcher** (01.06 B `n9vdhp`): the sidebar's project selector opens a menu in the select
  menu's style: RECENT (the five most recently opened projects whose folder is there, the shown one
  checked), *All projects*, *Add project*. Choosing another project opens its Overview as a new
  history entry (Back returns); one that cannot be opened any more leads to All projects. Rows are
  pointer-only like the rest of the sidebar; Escape or a click outside closes the menu. Pending the
  tabs work: the *Open in new tab* button (`NcJQ0`).
- **Runners**: switching keeps the previous project's runner running in the background, as
  `Runners::show` already did; All projects shows no project and stops nothing.

Deviations: *Last opened* reads as relative time (*2d ago*), as *Last published* in the pipelines
library, rather than the fixture's *Today* / *Sep 12*; the × has no tooltip (lurq has no tooltip
for a pressable yet).

Copy namespace `projects.*`.

## First project setup

First launch (`ZBoBm`) appears when the personal workspace has no projects or resumable setup. It has no selected project or project navigation, and no fabricated recent projects, activity or connected-runner status. Create your first project opens the first setup step. The name field initially starts empty; the following setup fixtures show the state after the user enters Kontur. The three-step preview explains repository access, mixed-provider agent assignment and the first task. The primary action creates a setup draft once, rather than starting agents or triggering provider authorization by itself. Existing incomplete setup should resume at its saved step; a configured returning workspace opens its projects rather than repeatedly showing Welcome.

Setup covers project folder → runner and agents → readiness. The first step asks for a name and a local folder (01.02 `jnpRq`: no folder yet, *Create project & continue* disabled with "Choose the project folder to continue."). With a folder chosen, the footer reads "Creates the project in the repository’s .orchester folder and opens it." (01.03 `muQCP`): the later setup steps are not built yet, so creating the project opens it rather than keeping setup in progress. After the folder is chosen, Orchester detects its GitHub repository and shows the result (01.03 `W6uXiL`, block `XcbNT`); GitHub is optional and never blocks *Create project & continue*. The detection states and the *Change GitHub account or access* panel are in [GitHub](#github). The second step configures the runner and each role's connection/model, including mixed providers. The final review checks the folder and GitHub repository, runner, required profiles, instructions and the starter pipeline. Finish & create first task publishes the reviewed starter configuration and opens New task; it does not start a run automatically.

An unusable required agent connection blocks completion with Reconnect or change-profile recovery. Save & leave retains incomplete setup so it can be resumed. Importing configuration from another project never copies an expected account; the destination project checks its own connections again.

## GitHub

Decided 2026-09-25 (user). Proposed copy namespace `github.*` (`github.detection.*`, `github.connection.*`, `github.account.*`, `github.pullRequests.*`); account logins, hosts, repository names, paths and commands are data, not translated.

Rules:

1. **Sign-in is the user's own `gh` CLI**, the same rule as agent CLIs. Orchester never signs in, never stores or sees a token and never opens a browser OAuth flow. A GitHub connection is a CLI connection: host (`github.com` or a GitHub Enterprise host), program (`gh` path and version), signed-in account, scopes, last check and *Check again*.
2. **The repository source is the local project folder.** The GitHub repository is detected from the folder's git remote (`origin` by default) and `gh repo view`. Nothing is cloned.
3. **GitHub is optional.** Local-only projects work. Without a usable GitHub connection, the Create pull request tool and the steps that use it are unavailable, and Pull requests shows *Pull requests are unavailable* (08.D). Project creation is never blocked.
4. **One linked repository per project**, remote `origin`. Push and pull request go to the same repository. Forks are a later decision.
5. **Push uses git's own credentials.** Orchester never changes git config. A push rejected for authentication stops the step with the hint *Run `gh auth setup-git` or sign in to git, then retry* (06.B `YDKJS`).
6. **gh accounts are machine-wide.** The project records the login it expects (set from the account detected when the project is created). If gh's active account differs, the state is *Signed in as X, expected Y* and pull request creation and sync are blocked. Orchester never switches gh accounts. The user can change what the project expects: *Use signed-in account* in Project settings → General (02.N `QV1af`), or *Use this account (X)* in every wrong-account state (setup `fhqgC`, connection `P89Dl`, Pull requests `E9ZThy`). Both only record the account gh is using now; they never change gh.
   - **The project folder must be a git repository** (runs work in git worktrees). A folder without git blocks setup: *This folder is not a git repository* with `git init` and *Create project & continue* disabled (01.C9 `w1Pvl`). A git repository without a GitHub remote is still allowed (local only).
   - **The only required gh scope is `repo`.** A sign-in without it is its own state, *Missing scope: repo*, with the command `gh auth refresh -h <host> -s repo` (Copy button) and *Check again*: setup 01.C10 `MTPqL`, connection 04.C5 `HwBI7`, Pull requests 08.H `tu1fW`. It is separate from *No access* (the account can't read the repository).
   - **Commands are shown as separate mono command rows** (Geist Mono 11 on `$canvas`, hairline border, radius 7, with a *Copy* button; source row `LBSz7` in 01.D), introduced by a sentence such as "Run this in a terminal, then check again." Sentences don't embed commands, except in the fixed-layout recovery panel component (`iN90G`: `w5ELZ6`, `YDKJS`), where the implementation renders the command as an inline mono span.
7. **Pull requests open as drafts by default.** The Create pull request tool has an *Open as draft* switch, on by default (06.A `mtPZd`).
8. **Change GitHub account or access** (`w5XOc` in 01.03, `wUapc` in 04.05) opens a small panel (01.D `hNPA1`), not a browser: it shows host, signed-in account, scopes and the expected account, explains `gh auth login` / `gh auth switch -h <host> -u <login>` and `gh auth refresh -h github.com -s repo` with Copy buttons, and ends with *Close* and *Check again*.
9. **Sync:** reads the 50 most recently updated open pull requests (`gh pr list`, without check details) and then counts the checks of their head commits by state in one `gh api graphql` request; asking `gh pr list` for 100 with `statusCheckRollup` made GitHub time out (HTTP 504) on large repositories. Sync runs when Pull requests opens, on *Refresh*, after Orchester creates a pull request, and every 60 s only while the Pull requests screen is visible. No background polling otherwise. A failed sync keeps the last list and shows *Last synced … · GitHub unreachable* (08.E); a GitHub server error (HTTP 5xx, a timed-out or cut-off answer) is shown the same way as *Last synced … · GitHub is having trouble* with "GitHub is having trouble (502). Try again in a moment…"; a rate limit shows the reset as relative time, *Rate limited · resets in 12 min*, with the exact local time in a tooltip (*Resets at 14:32 (local time)*, `Wn3fn`), disables Refresh and resumes after the reset (08.F).
10. **Pull request feedback stays in Orchester** and is never posted to GitHub (see [Pull request review](#pull-request-review-and-agent-feedback)).

### Project setup detection

01.03 `W6uXiL` shows the detection block `XcbNT` under the folder field `Y1vmV`: *GitHub repository* with status *Detected*, then Repository (`ivanzaida / kontur`), Remote (`origin · github.com/ivanzaida/kontur`), Default branch (`main`) and Account (`ivanzaida · gh on github.com`), the note "Kontur will expect ivanzaida. Orchester uses gh's own sign-in and never stores or sees a token.", *Check again* and *Change GitHub account or access* (`w5XOc`). The default branch is read from GitHub, not chosen here. 01.02 `jnpRq` shows the folder field before a folder is chosen and the notice `hfxMm` explaining that GitHub is detected from the folder and is optional.

Detection states (compact, same block, 640 wide):

| State | Node | Status | Copy (title · body) | Actions |
| --- | --- | --- | --- | --- |
| Detecting | `fL24i` | Checking… | GitHub repository · "Reading the folder's git remote and asking gh about the repository. You can already create the project." | — |
| No GitHub remote | `rwnig` | Local only | No GitHub remote · "You can create the project. Pull request steps and the Pull requests screen stay unavailable until the folder has a GitHub remote." | Check again |
| Remote is not on GitHub | `JjJiN` | Not GitHub | Remote is not on GitHub · "…If this is a GitHub Enterprise host, add a GitHub connection for it, then check again." | Check again, Add GitHub connection |
| gh not found | `Yz9Ig` | gh not found | GitHub CLI (gh) not found · "Orchester reaches GitHub only through gh. Install it or choose its program file, then check again. You can create the project now." | Check again, Choose program… |
| Not signed in | `MMSQ9` | Not signed in | gh is not signed in to github.com · "Run this in a terminal, then check again. Orchester never signs in for you." + command `gh auth login --hostname github.com` | Check again |
| Wrong account | `fhqgC` | Wrong account | Signed in as ivan-work, expected ivanzaida · "Switch gh back to ivanzaida in a terminal and check again, or make Kontur expect ivan-work. Orchester never switches gh accounts; pull requests can't be created until they match." + command `gh auth switch -h github.com -u ivanzaida` | Check again, Change GitHub account or access, Use this account (ivan-work) |
| Repository not accessible | `C1JtM` | No access | ivanzaida can't access this repository · "GitHub found no repository this account can read. Ask the owner for access, or switch accounts in gh, then check again." | Check again, Change GitHub account or access |
| Unrecognized gh output | `v85QyU` | Unrecognized | Couldn't read gh's answer · "gh 1.9.2 answered in a format Orchester doesn't recognise. Update gh, then check again." | Check again |
| Not a git repository | `w1Pvl` | Not a repository | This folder is not a git repository · "Runs work in git worktrees, so the project folder must be a git repository. Initialise it in a terminal, or choose another folder, then check again. Create project & continue stays disabled until then." + command `git init` | Check again, Create project & continue (disabled) |
| Missing scope | `MTPqL` | Missing scope | Missing scope: repo · "gh is signed in as ivanzaida without the repo scope that Orchester needs to read and create pull requests. Add it in a terminal, then check again. You can create the project now." + command `gh auth refresh -h github.com -s repo` | Check again |

*Create project & continue* stays enabled once a folder is chosen in every state except *Not a git repository*. Row 3 of the board (C9, C10) is at y 980; `GojEY` is 1400 high and the section `K7IL6` 3920.

### GitHub connection

The GitHub row (`iYGKy` in 04.01, same text in `LfeKD`, `an07d`, `x45t6`) reads *GitHub · github.com*, "gh · github.com · signed in as ivanzaida", type GitHub, 1 repo, status *Signed in*. 04.05 `CHa9s` selects it; the inspector `C0jjP` (redrawn 2026-09-27, replaces `l8BJVr`; padding 24 / 12 / 24 / 24, gap 16) has a details viewport `KAFgT` with the [scroll gutter](#components-and-tokens) and the actions pinned below it (`RlQQ2`). Details: Host, Program (`gh 2.62.0` — the version is part of this row, so there is no separate Version row), Path (Geist Mono 10, shortened to 34 characters; fixture `C:\Tools\gh\gh.cmd`), **Runs with** (*cmd.exe*, because the fixture path is a command script; an npm launcher shows *Node.js* and a mono **Script** row; `gh.exe` shows neither), Account, Scopes, Last checked, then *Used by Kontur*: Expected account, Repository, Remote, Used by tool (Create pull request); the state hint ("Orchester runs gh with its own sign-in on this computer. It never signs in, stores or sees tokens.") and, for states with one, the command row. Pinned: *Check again*, *Change account or access* (`wUapc`, opens 01.D), *Edit connection* (in other states also *Choose program…* and *Use this account (X)*), and an error line when an action failed. The details fit in the fixture, so the gutter is empty. The "Removing this connection…" sentence was removed: the app has no remove action.

Connection states (compact inspector heads in `DLRW5`): not signed in `Cr0P2` (command `gh auth login --hostname github.com`), gh not found `ve4G9` (Program *Not found on PATH*, *Choose program…*), wrong account `P89Dl` (Account ivan-work, Expected account ivanzaida, command `gh auth switch -h github.com -u ivanzaida`, *Check again*, *Change account or access*, *Use this account (ivan-work)*), unrecognized output `kqfaL`, missing scope 04.C5 `HwBI7` (Scopes *read:org*, command `gh auth refresh -h github.com -s repo`, *Check again*; x 3644). Signed in is 04.05.

### Pull requests states

08.D `dmib5` unavailable (no linked GitHub repository: *Kontur isn’t linked to a GitHub repository. Pull requests come from the repository detected in the project folder’s origin remote, read through the GitHub CLI (gh). Tasks and runs keep working locally.*, *Open Connections*, no tab row; panel 520 wide), 08.E `FYQMv` stale, 08.F `P1tSu` rate limited (relative reset, tooltip `Wn3fn` with local time), 08.G `E9ZThy` wrong account (sync status *Paused · gh signed in as ivan-work*, the last list kept, command `gh auth switch -h github.com -u ivanzaida`, *Check again*, *Use this account (ivan-work)*), 08.H `tu1fW` missing scope (command `gh auth refresh -h github.com -s repo`, *Check again*). While a search filters the list, the footer counts both: *4 of 8 open pull requests*. The footer follows the last row 20px below it; rows scroll only when they don't fit. 08.G and 08.H are at x 48 / 1072, y 1200 in `u77pi` (now 1680 high; section `McWTM` 3152).

### Project settings / General

02.N `QV1af` (in `BnxJ1`, x 3088 / y 2196, next to 02.M Limits) is the General tab, minimal: Project name (`Kontur`), Project folder (path in mono, "Git repository · moving the project is not available yet"), GitHub repository (read-only `ivanzaida / kontur`, "github.com · origin · default branch main", and a small secondary *Check again* that detects the folder's repository again, as in 01.03), Expected GitHub account (`ivanzaida`, read-back "gh is signed in as ivan-work · pull requests blocked", button *Use signed-in account (ivan-work)*, row `yBZWh`) and the notice `V9tFZ` "Use signed-in account changes only what Kontur expects — it records the account gh is using now. It doesn't sign in, switch or change gh." The button and the notice are shown only when gh's account differs from the expected one.

06.A `mtPZd` shows the push hint as a command row (`gh auth setup-git`).

Add GitHub connection (04.06 `D4mDB`) reuses the agent-CLI dialog `IAN4P`: Name, Connection type *GitHub*, Host (`github.com` default; *Other host…* takes a GitHub Enterprise host such as `github.example.com`), Program (gh discovered on PATH with its version, or a chosen file — the same disabled-option rule as the select menu, e.g. *Not found on PATH*), Program file with a folder picker, the notice *Uses gh's own sign-in*, the verification row (*Signed in* · account · scopes · *Check again*) and "After saving, Kontur expects ivanzaida…". There is no model section.

The former open questions (non-git folder, changing the expected account, required scopes, rate-limit time format) were decided by the user on 2026-09-25 and are rule 6 (with its sub-points) and rule 9 above.

## Instructions

Instructions are stored and versioned within a project in Orchester. Scope navigation covers project rules, agent roles, pipeline overrides and step overrides. A rule can target selected agent roles and an injection event. The mockup selects agent start/resume; the final event vocabulary remains to be specified. Importing an instruction from another project creates a new locally owned definition rather than a shared rule reference.

The preview displays proposed assembly order within the selected project: agent role → project rules → pipeline instructions → step instructions → task context. This order is an inspectable composition, not a promise that arbitrary contradictory prose can be resolved automatically. Inherited rules should retain source and version; disabling or replacing one must be explicit. Runtime permissions and budgets are separate enforced controls, not prompt text.

Publishing a draft creates a new version used by new runs. Existing runs retain their snapshot. Applying a new version to an active run needs a separate explicit action at a defined boundary; it is not designed as a silent update.

View full prompt opens the source-indexed read-only viewer. Variables are resolved for the selected task/agent/injection point. Copy full prompt copies composed instructions while excluding secret material. Unknown variables and missing required sources receive actionable validation. Closing with Escape restores focus to the invoking control and retains edits.

The Builder preview now accounts for seven individual sources: role, Implementation standards draft v3, Git conventions v1, Communication style v1, pipeline v3, step v2 and task context. Review checklist v2 is explicitly excluded because it targets Reviewer. The same execution context appears in the preview, full viewer and variable catalog: task ORC-128, historical Run 071 / Attempt 1, Builder / Implement, agent start before the first commit, with draft substitution identified. It is a historical preview context, separate from the current working Run 128. A task summary links to per-agent snapshots because applicable source counts differ by role.

**ORC-15 · Repository agent files, Orchester-owned skills and import (drawn 2026-09-29).** Fixture: Kontur's repository has AGENTS.md, CLAUDE.md (only `@AGENTS.md`), `.claude/skills/run-checks`, `.claude/skills/write-migration`, `.agents/skills/github-oauth` and `.agents/skills/old-deploy`.

- **Repository agent files** is a project setting in Project settings › General, a new section *Agents and the repository* after the GitHub rows; both frames show the page scrolled to it (in `BnxJ1`, beside 02.L3 states). The tab strip lists General, Limits and Environment. The row: *Repository agent files* · *Whether agents may read the repository’s own AGENTS.md, CLAUDE.md and skills folders besides Orchester’s instructions and skills. A change applies to agents that start after it.*; at the right the switch (`K8ItM`) and its state. *On* (`qO84y`): *On · agents may read them*, read-back *Found: AGENTS.md, CLAUDE.md, 4 skills folders*, and the `$raised` panel *What each CLI may load from the repository* — Codex `AGENTS.md · .agents/skills`, Claude Code `CLAUDE.md · .claude/skills` — with *Agents read these files as they are in the run’s worktree, so they are not versioned with the run. To keep them in Orchester instead, import them as drafts and turn this off.* and *Import from repository*. *Off* (`Sr5gP`): *Off · agents load none of them*, *Agents get only Orchester’s instructions and skills*, and the panel *Not loaded while this is off* — Codex *AGENTS.md and repository skills (.agents/skills)*, Claude Code *CLAUDE.md and .claude/skills* — with *Neither CLI loads these files, so everything agents know comes from Orchester’s published instructions and skills. The files stay in the worktree like any other file, and nothing in the repository changes.* The setting saves on change, like Limits.
- **Enforcement panel** (`bYDCI`, a copy of 04.04A with one row added; 04.04 and 04.04A are unchanged until approved): after *Network off* the row *Repository agent files off · enforced* with the mechanism (Codex: `project_doc_max_bytes=0 · skills off`), then a note (folder-git-2) *Repository agent files is set for all of Kontur in Project settings › General, not per profile.*; the tab-strip summary adds *Repository files off*. With the setting on, the row reads *Repository agent files on* with what the CLI loads. The mechanisms are unverified: Codex's `project_doc_max_bytes=0` should stop AGENTS.md; how Codex and Claude Code are kept from loading repository skills, and how Claude Code is kept from loading CLAUDE.md, must be checked against the installed CLIs on Windows and macOS. Where a CLI cannot enforce *off*, the rule of 04.B applies: a profile on that connection cannot run while the project has it off, and says why; Orchester never falls back to loading the files silently. *As built (2026-09-29, Windows, Codex 0.159.0 and Claude Code 2.1.89, checked without a model turn against the request each CLI would send):* Codex leaves `AGENTS.md` out with `-c project_doc_max_bytes=0` and each repository skill with `-c skills.config=[{path="…/SKILL.md",enabled=false}]`, on new and resumed sessions; Claude Code is always started with `--setting-sources ""`, which already keeps `CLAUDE.md` and `.claude/skills` out, so it loads neither in either state and the *On* panel and enforcement row say so instead of listing `CLAUDE.md · .claude/skills` (a user decision is needed if *on* should load them: it would take `--setting-sources project`, which also loads the repository's permission rules and hooks). Both CLIs enforce *off*, so no profile is refused for it. macOS is unchecked. Orchester's `list_skills` and `read_skill` (2026-10-08) offer the repository's skills only as the CLI itself loads them: Codex's `.agents/skills` and `.codex/skills` while the setting is on, none for Claude Code; the setting's meaning is unchanged. The new-project setup has no Ready step yet, so that entry point is not built.
- **Skills library** (section `HhPZg`, "States · ORC-15 · Skills and import from repository", row 1). A *Skills* item (sparkles) follows *Instructions* in the sidebar; the frames override the sidebar's navigation, and the shared `zo9Py` component is unchanged until approved. *Skills library* (`qO1Az`, built on 03.04): topbar *Import from repository* (folder-input, secondary) and *New skill* (primary); *Skills* · *Know-how agents load when a task needs it, versioned like instructions. Runs use a published version, never a draft.*; filter *All scopes* and *4 skills · 2 with unpublished drafts*; rows (`hn8qO`) with the name in mono and the description under it, *Files*, *Version* (*Published v2*, *v1 · Draft v2 saved*, *Draft v1 · not published*), *Scope* (*Project*, *Role · Reviewer*, *Step · Run checks*, *Pipeline · Feature delivery*) and *Last published*; the ownership note says the repository’s own skills folders are read only when Repository agent files is on; the right column *How skills reach agents* (scope decides who gets it, Publish vN, runs keep their version). *No skills yet* (`YRaLK`, built on 03.A): *No skills in Kontur yet* with what a skill is and, when the folder has agent files, the card *Found in this repository · 6 items* listing each file and what it becomes, *Import from repository* (primary) and *New skill*. Without agent files in the repository the card is not shown and *New skill* is primary. *Skill editor* (`ecJtg`, built on 05.01): the list of skills at the left (name in mono, *v3 · Draft v4 · Step*), tabs *All skills*, *Project*, *Agent roles*, *Pipelines & steps*; the editor with the status label *Step skill · Draft v4*, *Name* (mono), *Scope* (select: the project, a role, a pipeline or a step), *Description · what an agent reads to decide whether to load it*, *Body* (*Markdown · SKILL.md*) and the footer *Publishing applies to new runs. Active runs keep v3.*; topbar *Draft saved · Published v3* and *Publish v4*, as for instructions. The right panel *Reference files* lists each file (mono, kind and size, × to remove) with *Add file*, then *LOADED FOR* (Pipeline, Step), *Imported from* `.claude/skills/run-checks` and *Imported on* *Sep 20 · as draft v1*, and at the foot *Editing here never changes .claude/skills/run-checks in the repository, and later changes there do not reach this skill.*
- **Import from repository** (row 2). *Review* (`wulRC`, over *No skills yet*): a 1040 × 780 dialog *Import from repository* · *Found in C:\Users\ivan\code\kontur. Choose what becomes a draft in Orchester; open an item to see what it will contain.*; at the left the found items (`cuocA`: checkbox, path in mono, what it becomes, status label) in two groups, *INSTRUCTIONS · EACH BECOMES A DRAFT PROJECT INSTRUCTION* (AGENTS.md; CLAUDE.md unselectable at 0.45 with *Only points to AGENTS.md (@AGENTS.md) · nothing to import*) and *SKILLS · EACH BECOMES A DRAFT SKILL* (`.claude/skills/*` and `.agents/skills/*`, each with its file count); the open item has the `$hover` fill and its preview is at the right: *Becomes* *Draft skill run-checks · v1*, *Scope* *Project · change it in the editor*, *Description*, *Reference files*, and the file itself in mono. Under both, the notice (shield-check) *Creates drafts only: nothing is published and no repository file changes* · *You review and publish each draft yourself. Agents read the repository’s own AGENTS.md, CLAUDE.md and skills only while Project settings › Repository agent files is on.*; the footer *4 of 6 selected · 1 instruction, 3 skills*, *Cancel* and *Create 4 drafts*. *Result* (`Z8mGB0`, over the library, which now lists the three drafts): *4 drafts created* · *Nothing is published and no repository file changed. No agent gets a draft: review each one and publish it when it is ready.*, one row per draft (AGENTS.md *Draft project instruction v1*, each skill *Draft skill v1 · N files*), *NOT IMPORTED* with why, and *Open Instructions* / *Review skills*. **Entry points** (`DO46L`): the Skills library topbar and empty state; the Instructions topbar (*Import from repository* before the save status and *Publish*); and the new-project setup's Ready step, where a folder with agent files shows the card *This repository already has instructions for agents* with *Import from repository* (the setup's other actions are unchanged).

Proposed copy namespaces: `skills.*`, `skills.import.*` (one dialog for Instructions, Skills and setup), `settings.repositoryAgentFiles.*`, `agents.permissions.enforcement.repositoryFiles.*`.

## Variable catalog proposal

The Variables control is visible beside the prompt editor. The catalog searches names and descriptions, groups variables by source, and shows type, explanation, current value and availability. Selecting a variable does not modify the prompt; Insert writes its expression at the retained cursor and creates one undoable edit. Escape cancels insertion. Lists need a defined rendering format before implementation.

The count of 25 in the mockup refers to this proposed catalog, not a discovered runtime API:

| Group | Variable | Type | Availability |
| --- | --- | --- | --- |
| Task | `task.id` | string | After task creation |
| Task | `task.title` | string | Task selected |
| Task | `task.description` | string | Task selected |
| Task | `task.acceptance_criteria` | list of strings | Task selected; may be empty |
| Task | `task.attachments` | list of safe attachment references | Task selected; may be empty |
| Task | `task.created_at` | timestamp | After task creation |
| Project | `project.id` | string | Project selected |
| Project | `project.name` | string | Project selected |
| Project | `project.repository` | sanitized repository reference | Repository connected |
| Project | `project.default_branch` | string | Repository metadata loaded |
| Agent | `agent.id` | string | Agent assigned |
| Agent | `agent.name` | string | Agent assigned |
| Agent | `agent.role` | string | Agent assigned |
| Agent | `agent.model` | string | Model selected |
| Pipeline | `pipeline.id` | string | Pipeline selected |
| Pipeline | `pipeline.name` | string | Pipeline selected |
| Pipeline | `pipeline.version` | integer | Published version selected |
| Run | `run.id` | string | Run created |
| Run | `run.attempt` | integer | Attempt created |
| Run | `run.budget` | integer, tokens | Run limits configured |
| Run | `run.branch` | string | Working branch created |
| Run | `run.base_revision` | string | Run created |
| Run | `run.revision` | string | After the run's first checkpoint commit; at Agent resume the commit the agent's worktree is at |
| Step | `step.id` | string | Step selected |
| Step | `step.name` | string | Step selected |

The sample syntax is `{{task.title}}`. Variables are evaluated as data, not executable expressions. Secret values are excluded from this catalog. Missing values must not silently become empty text: explain the unavailable phase and offer an appropriate injection point or an explicitly configured fallback.

`run.base_revision` is the revision the run started from. `run.revision` is the run's latest checkpoint commit; a step's prompt gets the latest checkpoint at the time its invocation starts. Before the first checkpoint it has no value at Agent start: an agent whose prompt uses it does not start, and its step fails naming the variable. At Agent resume it is the commit the agent's worktree has checked out as the invocation starts, which is the real current state: its latest checkpoint, or a commit the agent made or merged itself after it (a clean worktree records no checkpoint), or the base revision while nothing has moved its branch (user decision 2026-10-14). Only when git cannot read that commit does it fall back to the latest checkpoint, and before the first one the step fails naming the variable, as at Agent start. This is why *Inject at Agent resume* resolves the unavailable-variable frame, and an instruction using `run.revision` publishes only when it is injected at Agent resume alone.

The unavailable-variable frame demonstrates `run.revision` before the first commit. Required policy blocks injection; the user can choose a later injection event or explicitly configure a fallback. The UI must not fabricate a revision or silently insert an empty string.

## Project tools and imports

Tools, secrets, instructions, agent profiles, pipelines and connections are all project-owned. The navigation entry opens an explicit project context; the breadcrumb and project selector show the current owner. A pipeline selects tools and profiles available in its project. Connections represent provider/server authentication within the project; Tools represent concrete callable actions. Their exact backend relationship remains a separate engineering contract.

The tool library offers creation of HTTP, command and database tools and discovery/import from an MCP connection. Setup owns action configuration, input/output mapping, credentials, allowed environment, access rules and version. The sample HTTP deployment tool selects `STAGING_DEPLOY_TOKEN` from Kontur and maps it to bearer authentication. The example deployment endpoint is fictional. Configuration validation must not execute a deployment. A future test action must clearly expose its actual side effects and apply the normal access policy.

A tool may only bind secrets owned by its project. Tool settings offer New project secret, returning the newly created reference to the binding selector. Secret settings have no tool picker or editable tool policy: Used by is a derived read-only list linking to the consuming tool. The secret creation screen collects name, type, value and environment in its current project.

Import resources is available in project navigation and from resource libraries. It covers pipelines, agent profiles, instructions, tools and secrets. The proposed default is an independent copy with a new local identity and source provenance. Dependencies must be enumerated: pipeline steps reference local agent/tool/instruction versions, and tools reference local connections and secrets. Tool credential dependencies are mapped to an existing compatible target-project secret, imported as a new secret, or supplied with a new value. Standalone secret import is also supported. Connections are checked again in the target project before use (for CLI connections: program, sign-in and the target project's expected account); importing configuration must not silently confer access. No credential value is displayed in the preview or placed on the clipboard.

The import fixture uses Select resources → Resolve dependencies for Preview release v3 from Personal website into Kontur. Seven mapping rows show the pipeline and six dependencies: reuse local Builder published v2 and Implementation standards v2; copy Build preview; rename the copied Deploy preview to Website preview because Kontur already contains a draft Deploy preview; import PREVIEW_HOST_API_KEY; and authorize Preview host configuration in Kontur. The library shows that existing draft among five tools, with two existing secret bindings.

The result is four new resource copies, two reused local resources and one separately authorized connection. The blocked and ready states differ in destination authorization; source access is never copied. The target policy is explicit: Build preview → Build, Website preview → Deploy, Preview environment, each deployment requires approval, all other callers denied. New configuration stays draft until reviewed and published; import does not start a run. Source/version and destination mappings remain inspectable.

Source edits do not propagate. Permission to read/import source resources and create target resources must be checked independently. Name collisions require an explicit rename or reuse selection, never a silent overwrite. Missing credentials leave the tool unready. Import failure preserves mappings and the original project and allows an idempotent retry without duplicate resources. Success opens the imported pipeline draft for review.

**Rename copy name field (06.C `FKzQ9`, accepted by the user; component `ixzlL`).** Choosing *Rename copy* in a mapping row shows a text field beside the select, in the same target cell: both 34 high, 8 apart, sharing the width. The select then reads just *Rename copy* (the name is in the field beside it). The field is the select's style (`$sidebar`, `$border`, radius 7, padding 0 12, Inter 12), placeholder *Name of the copy* in `$subtle`. The resolution column says whether the name works: *Name resolved* · *Deploy preview already exists*; empty — *Name needed* · *Enter a name for the copy*; taken — *Name taken* · *Kontur already has Deploy preview*. A row with a problem strokes the field 1.5 `$text`, the error stroke of every field, and the dialog footer names the row that blocks the import (*Name the copy of Deploy preview to continue.*, *Rename Deploy preview to continue.*) while *Import resources* stays disabled. `ixzlL` is a variant of `HfNoA` with the same cells (source 240, target fill, resolution 196); 06.05 `QqnDd` still shows the earlier row without the field.

## Secrets and access

The proposed design stores credentials separately from prompt rules and task context. A secret has an owner project, environment, name, type and value. The tool owns credential references, operation constraints and step access rules. Values are masked in management UI; replacing a value rotates it. Used by is a read-only reverse reference to local tools. No actual secrets exist in this design file or its previews.

Agents request capabilities through named tools. The tool receives the credential for the allowed operation/window; the model must not receive raw secret values through prompts, variables, copied instructions, logs or tool output. Implementation must keep credentials out of agent-readable process state and redact tool output; a mask in the UI is not a security boundary. Storage encryption, key management, runner isolation, provider adapters and rotation are open engineering decisions requiring separate review.

The sample database capability uses a database account with read-only privileges and a constrained query tool. The deployment tool binds a token and grants its `deploy.staging` capability to a specific step. Read-only guarantees require database/tool enforcement, not instructions asking the agent to avoid writes. Rotation and archival remain secret lifecycle actions; usage remains observable from the secret, while editing permissions happens in Tools.

The Access tab (`H97stw`) edits the allowed pipelines and steps, environment, approval policy, access window and denied behavior. Configured Used by links are distinct from the allowlist. The selected policy asks for each deployment; policy configuration may permit previously authorized scopes without repeated prompts. The shown deployment approval names project, environment, run, step, action, exact revision, tool and credential reference. It authorizes one deployment and expires when the step finishes. A different revision or operation requires a fresh policy decision. Denial leaves the step awaiting a changed plan; it does not imply a crash or erase work. All other callers are denied. Revoke access is a separate immediate action.

**Used by MCP servers (drawn 2026-09-29 for M3, `x91Ww`).** A secret is used by a tool that binds it and by an MCP server that takes a variable or its bearer token from it; a disabled server still counts. Agent profiles are not listed: a profile reaches a secret only through a server it chooses. The row says *1 tool*, *N tools*, *1 MCP server*, *N MCP servers*, or *N resources* when both kinds use it (link glyph; *Unused* with minus when none). The inspector's heading follows (*USED BY · 1 MCP SERVER*, *USED BY · 2 RESOURCES*, *USED BY · NONE*); tools come first, then servers as *MCP · name* (external-link, 40 high), which opens that server in Connections › MCP servers. A tool link is unavailable (0.4, trailing lock) until the Tools screen exists. The note under the links reads *Kontur / Connections* · *Secret references are edited on the MCP server.* for servers only, or *Kontur / Tools and Connections* · *Bindings are edited on each tool or MCP server.* for both. The title is the secret's kind (*Access token*, *API key*) and *Last rotated* reads *Never · added Sep 20, 2026* until a value is replaced. 07.B `wPXgW`: KNOWLEDGE_TOKEN, the bearer token of *project-knowledge*. 07.C `R2ImH`: DOCS_API_KEY, bound by the tool *Search docs* and taken by *docs* as a variable. 07.D `RxpL1`: *Archive DOCS_API_KEY?* (600 wide) with the warning *Tools and MCP servers use this secret (2 total). They are not ready until they select another secret.* in the handling-notice style (shield-check); for servers only it reads *1 MCP server uses this secret…* / *N MCP servers use this secret…*. The frames show *Recent access* as built (*Access to secrets is recorded once tools can use them. No tool can use a secret in this version yet.*).

Add-secret's successful fixture uses the unique name STAGING_READ_API_KEY. The duplicate state shows STAGING_DEPLOY_TOKEN already exists in Kontur / Staging, disables Save and links to the existing secret. Replace value remains the distinct rotation action.

Audit records contain who/what requested access, capability, scope, run/step, decision, time and outcome without credential values. Revocation affects future calls immediately; completed external side effects are not reversed. Network failure must not display a successful save, deployment or query. Retries need idempotency and explicit outcome handling.

## Shared server

Drawn 2026-10-03 for phases S2a–S3 of `docs/architecture/server.md` (its *Desktop data* › "Where it is configured",
*Claims*, *Live run status*, *Live updates*, *Migration*, *Two people share one repository*, *Accounts and tokens*,
*Security*). **Approved by the user 2026-10-03** (frames and every decision below). Canvas: a sixth column at x 26080
— the index `yuqEI` ("INDEX / F · Shared server", y 400) and the section `yYqBE` ("10 / Shared server", y 1800,
4896 × 15582) with five groups; no existing frame moved. Frames are 1440 × 960 (1440 × 988 where the status bar
matters, as `kpuQj`), 960 × 988 for the narrow task list; columns x 48 / 1568 / 3088 in each group, labels 32 above.

**Fixture** (illustrative, never shipped): server *Kontur team* at `https://orchester.kontur.lan` (API 1), *Platform
guild* at `https://orchester.guild.lan`, *Local test server* at `http://localhost:8740`, a plain-HTTP *Lab* at
`http://10.0.4.20:8740`. People: Alice Moreau (`alice`, admin, owner of ORC; she shared Kontur first, so 10.11–10.13
are her computer), Oleh Petrenko (`oleh`, owner), Bohdan Koval (`bohdan`, admin of the server, member of ORC; every
other frame is his computer), Maria Lind (`maria`), Dmytro Savchenko (`dmytro`, disabled). Repository
`github.com/ivanzaida/kontur`; ORC holds ORC-1 … ORC-40 from Alice; Bohdan's local key is KON with six tasks
KON-11 … KON-16, of which KON-12, -13, -14, -16 become ORC-41 … ORC-44 and KON-11, KON-15 stay *Not shared*.

**Components and variables.** No new components or variables. Two existing components gained one child each, hidden
by default like the plan limits item `XCXvw`, so no other screen changes: `zo9Py` (sidebar) has a *Settings* item
`s9zzPC` (settings icon) after *Import resources* in the application group, and `pmOpt` (status bar) has a *Server
connection* item `qqtph` after *Runner* (icon `Fsj7R`, label `W92Zmn`, mono detail `CbpHY`, action `s54X8F`). The
shared task rows follow `clVjO`'s metrics (76 high, padding 0 12, gap 16, hairline) with a Holder cell; they are
frame rows, not instances, because `clVjO` has no Holder slot. Proposed copy namespaces: `servers.*`, `sharing.*`,
`sharing.join.*`, `claims.*`, `tasks.conflict.*`, `statusBar.server.*`.

### Settings › Servers (S2a)

- **10.01** The entry point: *Settings* in the sidebar's application group opens Settings (title *Settings*, one tab
  *Servers*); the status bar's server item opens the same page at its server. Left, the server list (380): name,
  URL (mono), connection label (*Connected*, *Signed out*, *Offline*, *Not signed in*) and the account line. Right,
  the selected server: name, label, URL · API · HTTPS, *Sign out*; tabs *Account* and *Users* (admins only). Account:
  signed in as, role, signed in, shared projects there, *Change password*; *Your sessions* (device, signed in, last
  used; *Revoke* on every row but this computer's); the keychain line.
- **10.02 / 10.03** *Add server* checks `GET /server` before sign-in: the result card names the server, its API and
  HTTPS. 10.03: checking; plain HTTP to another host (strong warning, opt-in *Allow plain HTTP for host:port*, Continue
  unavailable until ticked; stored with the server); plain HTTP on loopback (allowed, says why); a redirect is an error
  naming its target; not reached / not an Orchester server / certificate not trusted (no skip switch); `needs_bootstrap`
  with the server-host command and Copy; already in the list; newer API.
- **10.04 / 10.05** *Sign in to Kontur team* (username, password; says where the password goes and that the token
  goes to the keychain). 10.05: wrong username or password (one message for both), 429 with the time, the forced
  *Choose your password* step (only *Sign out* and *Save password*), its errors (12–256 characters, mismatch),
  *Change your password* naming the other sessions it signs out, and *Sign out?* with what happens to the shared
  project, claims and runs.
- **10.06** Signed out (expired or revoked): the server says *Signed out*, a strong notice explains the causes, *Sign
  in* is primary, *While you are signed out* lists read-only list, claims going stale, runs working, nothing queued;
  the status bar reads *Signed out of Kontur team · Sign in*.
- **10.07 / 10.08** Users (admins): name and username, role, state, *Reset password* and *Disable* per row (*Enable*
  for a disabled user; your own row points to Account). 10.08: *Create user* (username, display name, Admin switch),
  the first password shown once with Copy and *Done*, *Reset …'s password?*, *Disable …?* (sessions, released claims),
  the last admin refused, username taken.

### Project settings › Sharing (S2b, S2c)

A fourth Project settings tab, *Sharing*, in the setting-row layout of General (792 column).

- **10.11** Local only (default): *Task list · Local only*, the repository identity with what Orchester found on the
  signed-in servers, *Share on* (select of signed-in servers, *Share on Kontur team…*), *What sharing sends*.
- **10.12** The Share confirmation: tasks uploaded with their numbers, where new numbers continue, tasks claimed for
  you (unfinished runs), the project key; switch *Write orchester.toml into the repository* (on; never committed).
- **10.13** Shared, seen by an owner: *Shared on Kontur team* with key, role and last sync; *Discovery file* state;
  *Members* (username field with @, role select, *Add member*; rows with role, added, *Make member* on an owner's row
  and *Make owner* on a member's, *Remove*, *Leave* on your own); *Make local…*.
- **10.14** Add member errors (no such user, already a member), a member's read-only view with *Leave* (no role or
  remove actions), *Make Bohdan Koval an owner of ORC?*, *Make yourself a member of ORC?* (asked because you lose member
  management), the last owner refused (`409 last_owner`) with *Make Oleh Petrenko an owner…* as the way out, *Remove …
  from ORC?* (claims released), *Make local* with its consequences (organization kept as last synced, ORC's tags become
  local tags), and *Make local* while the server can't be reached (claims go stale instead).
- **10.15** The repository is shared there and you are a member: a strong notice with *Review and join…*; Share is not
  offered.
- **10.16** Not a member (owners named, your username with Copy, *Check again* and its last result; nothing uploaded);
  can't be shared (origin is a local path, no origin); Share answered `409 remote_taken` → Join; discovery step 1
  (*Add the server named in orchester.toml?* with the host large and the source named); discovery after steps 1–2;
  a plain-HTTP address in the file uses 10.03 B.
- **10.17** `orchester.toml` found: a strong notice with the three steps (add the host, sign in, review and join), each
  confirmed when it comes, *Sign in and join…* / *Not now*.

### Join review and Not shared (S2c)

- **10.21** *Join ORC on Kontur team* (page under Sharing; *Cancel*, *Join and upload 4* in the topbar): the review bar
  (*4 of 6 ticked* and what happens), rows with local key, title, the hint *Similar on ORC: ORC-23 … · Alice Moreau*,
  workflow and run, *Becomes* ORC-4x or *Not shared*. Rows with a run that hasn't ended keep a muted tick and say why
  they can't be unticked and that they are uploaded claimed. The note says status, priority, tags and links go with each
  task: tags match ORC's by name (new names become ORC tags) and a link to a task that stays here is dropped.
- **10.22** After joining: the confirmation notice and *Keys before joining* (KON-12 → ORC-41 …, Not shared rows), with
  the branch note; it stays in Sharing.
- **10.23** Tasks with the *Not shared* filter on (button *Not shared · 2* on the query's second line): an explaining
  notice and the read-only rows.
- **10.24** A Not shared task: *Not shared* label, KON-15, read-only; its organization as plain labels (*Backlog*, *Low*,
  tag chips) with *Tags are kept by name until the task is shared.*; *Edit* unavailable, *Share task* primary; its runs
  stay readable; the Sharing panel says what it would become.
- **10.25** A joined task (ORC-41): *was KON-12* after the run metadata; under it ORC-39's organization row (status,
  priority, tag chips, *Tags*), now shared; the right panel's *On Kontur team* section
  (holder, since, heartbeat, shared status, created as KON-12, last edit); *Release* unavailable with the reason while
  the agent works.
- **10.26** *Share KON-15 on ORC?*; search finds a former key; *Kontur is local again* with KON-11 → ORC-45, KON-15 →
  ORC-46; a join that failed on the way (nothing uploaded, ticks kept).

### Claims and live run status (S2d, S3)

- **10.31** The shared list: *Shared on Kontur team as ORC* beside the description; the query's second line has
  *Status*, *Priority*, *Tags*, *Not shared* and *Manage tags*; columns ID, TASK / TAGS (tag chips and *was …* under the
  title), PRIORITY 96, WORKFLOW / RUN 212 (with *⊘ Blocked* when flagged), HOLDER 172, DEPENDENCY / READINESS 160; rows
  84 high. PIPELINE is not shown at 1440: Holder takes its width, so the combined list's breakpoints move up by Holder's
  188 (Pipeline returns from a 1536 window; the pipeline filter stays). Another member's run reads *Running · Review · Alice ·
  since 14:02* under the workflow label; Holder shows the person and *run claim · since 13:55*; stale is *Maria Lind ·
  stale* (clock-alert, `$text`) with *last heartbeat 13:21*; merged is *Merged by Oleh Petrenko*; yours is *You*; free
  is *Not taken*; a hovered free row shows a quiet *Take* in the Holder cell. Above the list, the lost-claim notice
  (*Oleh Petrenko took over ORC-43 at 14:20*, why, what happens to your run, *Open ORC-43*, *Dismiss*).
- **10.32** The same at 960 × 988: as in 02.03E, Priority and the tag chips drop at 960 (the filters stay); Pipeline
  and Readiness drop too, Holder stays; the status bar drops version, tokens and count labels as before and keeps the
  server item.
- **10.33** Another member's live run on the task page: *Running* with *Review · Alice Moreau · since 14:02 · run #3 on
  her computer*; the shared organization row (*Status, priority and tags are shared; any member can change them.*); a
  notice that conversation and changes stay on her computer; *Start run* unavailable with the lock
  line naming the holder and when it becomes possible; *Edit* stays (any member may edit); the panel's *On Kontur
  team* (holder, claim, heartbeat, run, phase, phase since, last edit; *Take* unavailable).
- **10.34** The panel in each claim state (not taken → *Take*; yours by hand → *Release*; yours after a succeeded run →
  kept, *Release*; merged; someone else's fresh → *Take* unavailable with the 15-minute rule; stale → *Take over…*;
  yours while offline → when it goes stale; taken over from you), the task page's taken-over notice, and the refusals
  (start refused naming the holder, Take lost a race, Take over refused because the claim was refreshed, Send &
  continue refused with the message kept).
- **10.35** *Take over ORC-37 from Maria Lind?* names the holder, her last heartbeat and her last run status, and what
  follows (manual claim for you, she is told on reconnect, her reports stop, it is recorded).

### Conflicts and connection (S2b, S2d)

- **10.41** Revision conflict in *Edit task*: the topbar says *Not saved · Alice Moreau saved a newer version*; a strong
  notice; fields both changed show *Your version · not saved* and *Alice Moreau's version · saved 14:12* side by side,
  one chosen; fields only she changed are marked *kept* (*added by Alice*); *Save with your version* / *Use Alice
  Moreau's version*. Your text is never dropped until you choose.
- **10.42** *Offline — read-only* in the description line and as a strong notice with the last sync, what waits and
  *Try now*; *New task* unavailable; holders *as of 14:31*; status bar *Kontur team offline · since 14:31 · next try in
  32 s*.
- **10.43** The status bar item: connected (hint), reconnecting (attempt, next try), offline, signed out (*Sign in*),
  not a member, two servers (worst state leads; the hint lists each), none when no shared project is open.
- **10.44** No longer a member: the list read-only, your claims released, *Make local…* and *Check again*; status bar
  *Not a member of ORC*.

| Frame | Node | Position (group) |
| --- | --- | --- |
| 10.01 · Settings › Servers / Account (1440 × 988) | `sXIz1` | 48 / 100 (`A37vc0`, y 220) |
| 10.02 · Add server / Address checked | `e9DDI` | 1568 / 100 |
| 10.03 · Add server states (compact, 1440 × 1337) | `BWyKt` | 3088 / 100 |
| 10.04 · Sign in / Username and password | `X0cARN` | 48 / 1548 |
| 10.06 · Settings › Servers / Signed out (1440 × 988) | `YMwe5` | 1568 / 1548 |
| 10.05 · Sign-in and password states (compact, 1440 × 1399) | `mcI7H` | 3088 / 1548 |
| 10.07 · Settings › Servers / Users | `BWEZ9` | 48 / 3036 |
| 10.08 · Admin user dialogs (compact, 1440 × 1331) | `kT0ui` | 1568 / 3036 |
| 10.11 · Sharing / Local only | `xnhV2` | 48 / 100 (`Ux22E`, y 4784, 3682 high) |
| 10.12 · Share on Kontur team / Confirm | `IDfLs` | 1568 / 100 |
| 10.13 · Sharing / Shared · members | `V4ZxQG` | 3088 / 100 |
| 10.14 · Members, roles and Make local states (compact, 1440 × 1354) | `NaUDV` | 48 / 1180 |
| 10.15 · Sharing / Already shared → Join | `y8uP2` | 1568 / 1180 |
| 10.16 · Join and discovery states (compact) | `AZwJE` | 3088 / 1180 |
| 10.17 · Sharing / Discovery: Sign in and join | `g5r5VU` | 48 / 2622 |
| 10.21 · Join ORC / Review local tasks | `UlHPB` | 48 / 100 (`idluC`, y 8562) |
| 10.22 · Sharing / Joined · key mapping | `m9omas` | 1568 / 100 |
| 10.23 · Tasks / Not shared filter (1440 × 988) | `l9jYZS` | 3088 / 100 |
| 10.24 · Not shared task / Share task | `FAKNF` | 48 / 1180 |
| 10.25 · Task page / was KON-12 · your claim | `RuBhQ` | 1568 / 1180 |
| 10.26 · Former keys and Not shared states (compact) | `m7jOn9` | 3088 / 1180 |
| 10.31 · Tasks / Shared list · claims and live status (1440 × 988) | `jakf4` | 48 / 100 (`sk7nC`, y 10898) |
| 10.32 · Tasks / Shared list at 960 (960 × 988) | `ByIkx` | 1568 / 100 |
| 10.33 · Task page / Another member's live run | `NikWo` | 3088 / 100 |
| 10.34 · Claim states and refusals (compact) | `omHGh` | 48 / 1180 |
| 10.35 · Take over / Confirm | `U4GzjB` | 1568 / 1180 |
| 10.41 · Edit task / Revision conflict | `RmDpQ` | 48 / 100 (`pRtpN`, y 13234) |
| 10.42 · Tasks / Offline — read-only (1440 × 988) | `Cc4WH` | 1568 / 100 |
| 10.43 · Status bar server item (compact, 1504 wide) | `hYres` | 3088 / 100 |
| 10.44 · Tasks / No longer a member (1440 × 988) | `Y6H8A` | 48 / 1180 |

**Decisions (user, 2026-10-03, after the first drawing).**

- **Member roles.** Owners and admins change a member's role (`PATCH …/members/{user}` `{ role }`, server decision 25):
  *Make owner* / *Make member* on the member rows (10.13), a confirmation when you demote yourself, and the last owner
  refused with a way out (10.14). A sole owner can hand over and then leave.
- **Task organization is shared.** Workflow status, priority, tags, blocking links and the manual Blocked reason
  are shared fields kept with the task's revision; tags are the server project's (create, rename, merge, delete by
  members under ORC-39's rules) and the organization history is shared (server decisions 23–24, *Task organization*).
  The shared list shows Priority and tag chips again (10.31, and the read-only lists 10.23, 10.42, 10.44), the task
  pages show ORC-39's organization row (10.25, 10.33, 10.35; read-only in 10.24), and *Manage tags* is back on the
  query line. ORC-39's own frames (section 02.3 `Yn5Wo`: pickers, Blocked fields, tag manager, history) apply
  unchanged to a shared project; the history names the person who made each change. Offline and for a former member
  these controls are unavailable like every other change.

**Decisions the architecture does not settle — approved by the user 2026-10-03** (8 and 12 were settled first):

1. *Settings* is a sidebar item in the application group (beside *All projects*), and the status bar's server item
   opens it too. The page keeps the project's sidebar; opened with no project (from All projects) it would use the
   welcome topbar — not drawn.
2. Settings › Servers is a list with a detail pane; the detail has *Account* and *Users* (admins only) tabs.
3. *Change password* on Account (the API has it; the document names only the forced change). *Revoke* acts at once
   without a confirmation; the current session has no Revoke (Sign out instead). A session's device reads *Orchester on
   <computer name>* with the platform.
4. Plain-HTTP opt-in is a checkbox inside the warning; Continue stays unavailable until it is ticked.
5. The initial password dialog has *Copy* (handing it to the person is its purpose) and is shown once; admins never
   see a person's own password.
6. Admin rows offer Reset password and Disable/Enable; *Sign out everywhere* and changing a user's admin flag (both in
   the API) are not drawn.
7. Sharing is a fourth Project settings tab; *Write orchester.toml* is a switch in the Share confirmation, on.
8. ~~Member role change~~ — decided (user, 2026-10-03), see above.
9. *Make local* keeps the membership (Join again later works); *Leave* on your own member row removes it.
10. The join review is a page under Sharing, not a dialog; the similar-title hint names the server task, title and
    creator.
11. *Not shared* is a toggle button on the task query's second line; while on, tab counts follow it.
12. ~~Priority and tags in shared projects~~ — decided (user, 2026-10-03): shared, see above.
13. The shared list adds HOLDER (172) in place of PIPELINE at 1440 (Pipeline returns from a 1536 window); at 960 Readiness
    drops too, after Priority and tag chips as in 02.03E.
14. Rows keep no menu (ORC-16): a free row shows a quiet *Take* in its Holder cell on hover or focus; Release and Take
    over are on the task page only (S2d's box says "on the task row and page").
15. A takeover gives a manual claim; the lost-claim notice stays until dismissed, on the list and on the task page.
16. *Reconnecting* until the backoff reaches 60 s, then *Offline*; both make the list read-only. The status bar shows
    the worst state when several servers are in use.
17. The conflict compares only fields both people changed, one choice per field; the other person's other changes are
    kept; saving sends yours on top of the new revision.
18. Organization fields are merged on a conflict without asking (core's `TaskOrganization::rebase`, server decision 26);
    only content fields reach the conflict view.

**Not drawn, or not expressible as a frame:** heartbeats and SSE themselves (no visible state; `reset` reloads the
list silently), two desktops seeing changes within a second, a heartbeat from a second computer keeping claims fresh,
a shared task whose pipeline is missing on this computer (today's preflight message applies), the Conversation and
Changes tabs of another member's run (empty here), PATCH of a project's name or key by owners, and Settings without
an open project.

## Copy, layout and implementation acceptance

Proposed i18n namespaces: `nav.*`, `workspace.*`, `tasks.*`, `runs.status.*`, `runs.controls.*`, `planReview.*`, `conversation.*`, `pipelines.*`, `agents.*`, `connections.*`, `setup.*`, `resourceVersions.*`, `instructions.*`, `variables.*`, `tools.*`, `projectImport.*`, `secrets.*`, `access.*`, `recovery.*`. Examples: `instructions.publishVersion`, `instructions.previewEffective`, `variables.insert`, `variables.unavailableAtPhase`, `tools.credentials`, `tools.validateConfiguration`, `projectImport.resolveDependency`, `projectImport.independentCopies`, `secrets.usedBy`, `secrets.add`, `secrets.replaceValue`, `access.allowDeployment`, `access.deny`, `agents.inheritModel`, `connections.refreshModels`, `conversation.queuedAtCheckpoint`, `planReview.requestChanges`, `setup.resume`. Prompts, task descriptions, secret names and repository content are user-authored data.

Screens are designed at 1440 × 960; the app window is verified at 1440 × 1020 (title bar, screen and status bar, see [Window chrome](#window-chrome)). At narrower widths collapse navigation first, then open inspectors on demand; stack creation-form columns. Dialogs need viewport-constrained scrolling and focus trapping. Search, pagination, keyboard navigation, Escape, clipboard, screen-reader labels, loading, validation and persistence still need runtime implementation and testing. Native/web runtime choice and mobile editing are not established by these screens.

Visual evidence: the original 13 screens and 20 additional screens were inspected through live Pencil nodes and screenshot/export review. The independent audit was repeated against the expanded 32-screen set; its remaining import/library fixture mismatch was corrected by adding the existing draft Deploy preview and updating the count. The subsequently requested first-launch screen received primary-agent layout and full-size visual review. Full-size export review caught and corrected a tool-list overlap and import-tab spacing that the structural check did not flag. Repeated controls use 30 linked component definitions. See the [audit and resolution record](../evidence/design-consistency-audit-2026-09-23.md).

The subsequent canvas-organization pass grouped the same 33 screens into eight numbered sections, aligned the layer tree and screen labels, and separated normal flows from alternate states. All 33 screens passed depth-9 layout checks after relocation; section exports were inspected for actual visibility and ordering. Reflow exposed excess tool-list height, which was corrected in the library and three import backgrounds. The product flows and screen IDs were preserved.

The task-management extension added five screens for the ten-task backlog, bulk creation, parallel scheduling, sequential scheduling and the active queue. Each received a full-size visual review and passed depth-9 layout checks; Foundations and the expanded section hierarchy also passed. That extension brought the design to 38 screens and 32 reusable components. It received primary-agent design verification, not another independent audit. [Task-management overview](previews/organization/UIc5y.png).

The subsequent project-orchestrator extension added the new-conversation and task-proposal screens, bringing the design at that point to 40 screens and 34 reusable components. Both new screens were inspected at 1440 × 960. The shared sidebar and task-chat labeling were checked visually on the working-task and Instructions screens; all 40 screens plus Foundations passed depth-9 layout checks. That root layout retained eight numbered sections. The new chat screens sit first in section 02; old screen IDs remain stable and the numbered labels were updated. This was primary-agent verification, not a new independent audit.

The PR-review extension adds four full screens and two compact recovery states in section 08. All four screens and both compact states received full-size visual inspection; the updated task-review entry point and section overview were also inspected. A clipped footer on the fix-verification panel was corrected by reducing its layout gap and rechecked. All 44 full screens, Foundations and the PR alternate-state group passed depth-9 layout checks; root layout passed. The shared sidebar now includes Pull requests. The current document has nine numbered sections and 37 reusable components. This was primary-agent design verification; execution, GitHub synchronization, comment persistence and diff anchoring remain unimplemented specifications.

The subsequent colored-code request updated the same three diff screens and shared diff-line component without changing the fixtures or screen IDs. All three screens and Foundations passed depth-11 layout checks after token sizing was corrected; final full-size exports were inspected. Code-only color exceptions are now part of the user-selected visual direction. [Syntax and diff states](previews/z8v8m7.png).

These are static design checks. Actual orchestration, provider authentication, chat delivery, persistence, responsive behavior, accessibility and credential isolation were not implemented or tested. No production code or real external service state was changed by this design work.
